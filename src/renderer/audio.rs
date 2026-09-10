//! Small procedural audio output, independent of rendering and simulation.

use std::f32::consts::TAU;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use anyhow::{Context, Result, bail, ensure};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use rtrb::{Consumer, Producer, RingBuffer};

const COMMAND_CAPACITY: usize = 32;
const MAX_VOICES: usize = 8;
const CUE_SECONDS: f32 = 0.18;
const CUE_GAIN: f32 = 0.035;

#[derive(Clone, Copy, Debug)]
enum Command {
    PlayCue,
}

/// A running default-device stream. Dropping this owner stops playback.
pub struct AudioOutput {
    // Drop the stream before the producer so queue cleanup stays off its callback.
    _stream: cpal::Stream,
    commands: Producer<Command>,
    errors: Arc<AtomicUsize>,
}

impl AudioOutput {
    /// Opens the default output with its preferred sample rate and channel count.
    /// Device absence and unsupported formats are returned to the caller.
    pub fn open_default() -> Result<Self> {
        let device = cpal::default_host()
            .default_output_device()
            .context("no default audio output device")?;
        let supported = device
            .default_output_config()
            .context("query default audio output configuration")?;
        let config = supported.config();
        ensure!(config.channels > 0, "audio output has zero channels");
        ensure!(config.sample_rate > 0, "audio output has zero sample rate");
        let (commands, consumer) = RingBuffer::new(COMMAND_CAPACITY);
        let errors = Arc::new(AtomicUsize::new(0));
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(&device, config, consumer, errors.clone()),
            SampleFormat::F64 => build_stream::<f64>(&device, config, consumer, errors.clone()),
            SampleFormat::I8 => build_stream::<i8>(&device, config, consumer, errors.clone()),
            SampleFormat::I16 => build_stream::<i16>(&device, config, consumer, errors.clone()),
            SampleFormat::I24 => {
                build_stream::<cpal::I24>(&device, config, consumer, errors.clone())
            }
            SampleFormat::I32 => build_stream::<i32>(&device, config, consumer, errors.clone()),
            SampleFormat::I64 => build_stream::<i64>(&device, config, consumer, errors.clone()),
            SampleFormat::U8 => build_stream::<u8>(&device, config, consumer, errors.clone()),
            SampleFormat::U16 => build_stream::<u16>(&device, config, consumer, errors.clone()),
            SampleFormat::U24 => {
                build_stream::<cpal::U24>(&device, config, consumer, errors.clone())
            }
            SampleFormat::U32 => build_stream::<u32>(&device, config, consumer, errors.clone()),
            SampleFormat::U64 => build_stream::<u64>(&device, config, consumer, errors.clone()),
            format => bail!("unsupported audio output sample format: {format}"),
        }?;
        stream.play().context("start audio output stream")?;
        Ok(Self {
            _stream: stream,
            commands,
            errors,
        })
    }

    /// Queues a quiet, 180 ms synthetic cue without waiting.
    /// Returns false if the queue is full. Cues arriving while all eight voices
    /// are active are discarded by the mixer.
    pub fn play_cue(&mut self) -> bool {
        self.commands.push(Command::PlayCue).is_ok()
    }

    /// Stream errors are counted by the callback; the caller can report them.
    pub fn error_count(&self) -> usize {
        self.errors.load(Ordering::Relaxed)
    }
}

fn build_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut commands: Consumer<Command>,
    errors: Arc<AtomicUsize>,
) -> Result<cpal::Stream> {
    let mut mixer = Mixer::new(config.sample_rate);
    let channels = usize::from(config.channels);
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                // A producer cannot extend this loop indefinitely while refilling.
                for _ in 0..COMMAND_CAPACITY {
                    let Ok(Command::PlayCue) = commands.pop() else {
                        break;
                    };
                    mixer.play_cue();
                }
                mixer.fill(output, channels);
            },
            move |_| {
                errors.fetch_add(1, Ordering::Relaxed);
            },
            None,
        )
        .context("build audio output stream")
}

#[derive(Clone, Copy, Default)]
struct Voice {
    frame: u32,
    active: bool,
}

/// Fixed-size, deterministic mixer, usable without a device in offline tests.
struct Mixer {
    sample_rate: f32,
    duration_frames: u32,
    voices: [Voice; MAX_VOICES],
}

impl Mixer {
    fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate as f32,
            duration_frames: (sample_rate as f32 * CUE_SECONDS).round() as u32,
            voices: [Voice::default(); MAX_VOICES],
        }
    }

    fn play_cue(&mut self) -> bool {
        let Some(voice) = self.voices.iter_mut().find(|voice| !voice.active) else {
            return false;
        };
        *voice = Voice {
            frame: 0,
            active: true,
        };
        true
    }

    fn next_sample(&mut self) -> f32 {
        let mut mixed = 0.0;
        for voice in &mut self.voices {
            if !voice.active {
                continue;
            }
            let time = voice.frame as f32 / self.sample_rate;
            let remaining = (self.duration_frames - voice.frame) as f32 / self.sample_rate;
            let envelope = (time / 0.004).min(1.0) * (remaining / 0.020).min(1.0);
            mixed += (TAU * 660.0 * time).sin() * envelope * CUE_GAIN;
            voice.frame += 1;
            voice.active = voice.frame < self.duration_frames;
        }
        mixed.clamp(-1.0, 1.0)
    }

    fn fill<T: Sample + FromSample<f32>>(&mut self, output: &mut [T], channels: usize) {
        output.fill(T::EQUILIBRIUM);
        if channels == 0 {
            return;
        }
        for frame in output.chunks_exact_mut(channels) {
            // Advance once per frame, then duplicate into each output channel.
            frame.fill(T::from_sample(self.next_sample()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_duration_and_channel_alignment_follow_device_format() {
        for sample_rate in [44_100, 48_000, 96_000] {
            for channels in [1, 2, 6] {
                let mut mixer = Mixer::new(sample_rate);
                assert!(mixer.play_cue());
                let cue_frames = (sample_rate as f64 * 0.18).round() as usize;
                let mut output = vec![f32::NAN; (cue_frames + 128) * channels];
                mixer.fill(&mut output, channels);
                assert_eq!(output[0], 0.0);
                assert!(
                    output[channels..cue_frames * channels]
                        .iter()
                        .any(|s| s.abs() > 0.01)
                );
                assert!(
                    output[(cue_frames - 16) * channels..cue_frames * channels]
                        .iter()
                        .any(|s| s.abs() > 0.00001)
                );
                assert!(output[cue_frames * channels..].iter().all(|s| *s == 0.0));
                for frame in output.chunks_exact(channels) {
                    assert!(frame.iter().all(|s| *s == frame[0] && s.is_finite()));
                }
            }
        }
    }

    #[test]
    fn bounded_polyphony_stays_quiet_and_reuses_finished_voices() {
        let mut mixer = Mixer::new(48_000);
        for _ in 0..MAX_VOICES {
            assert!(mixer.play_cue());
        }
        assert!(!mixer.play_cue());
        let mut output = vec![0.0_f32; 48_000];
        mixer.fill(&mut output, 1);
        assert!(output.iter().all(|s| s.is_finite() && s.abs() <= 0.281));
        assert!(output.iter().any(|s| s.abs() > 0.2));
        assert!(mixer.play_cue());
    }

    #[test]
    fn split_buffers_preserve_the_waveform() {
        let mut whole = Mixer::new(48_000);
        let mut split = Mixer::new(48_000);
        whole.play_cue();
        split.play_cue();
        let mut expected = [0.0_f32; 1024];
        let mut actual = [0.0_f32; 1024];
        whole.fill(&mut expected, 2);
        for block in actual.chunks_mut(128) {
            split.fill(block, 2);
        }
        assert_eq!(actual, expected);
    }

    #[test]
    fn unsigned_silence_and_partial_frames_are_initialized() {
        let mut mixer = Mixer::new(48_000);
        let mut output = [0_u16; 17];
        mixer.fill(&mut output, 2);
        assert_eq!(output, [32_768; 17]);
        mixer.play_cue();
        mixer.fill(&mut output, 2);
        assert_eq!(output[16], 32_768);
    }
}
