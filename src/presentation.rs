//! Renderer-independent presentation playback state.
//!
//! This module turns declarative animation cues into authored-frame samples.
//! It deliberately knows nothing about scene graphs, renderers, or Melee menu
//! identities, so the same clock can drive replacement and modded resources.

pub mod instance;

use thiserror::Error;

use crate::menu::{AnimationCue, AnimationId, FrameRange};

/// Unit-rate playback of one declarative animation cue.
///
/// A request latches the cue's start frame. The first [`Self::tick`] returns
/// that frame without advancing; later ticks advance by one authored frame.
/// This makes cue changes deterministic even when a request and presentation
/// update happen during the same host tick.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationPlayback {
    cue: AnimationCue,
    frame: f32,
    first_sample_pending: bool,
}

impl AnimationPlayback {
    /// Requests a cue and positions it at its authored start frame.
    pub fn new(cue: AnimationCue) -> Result<Self, PlaybackError> {
        validate_range(&cue)?;
        let frame = cue.frames.start;
        Ok(Self {
            cue,
            frame,
            first_sample_pending: true,
        })
    }

    /// Replaces the current cue and restarts playback at its start frame.
    ///
    /// Validation happens before changing any state, so a rejected cue leaves
    /// the existing playback untouched. A cue restarts even when it uses the
    /// same stable animation identity as the previous request.
    pub fn restart(&mut self, cue: AnimationCue) -> Result<(), PlaybackError> {
        *self = Self::new(cue)?;
        Ok(())
    }

    /// Stable resource identity of the requested animation.
    pub fn animation_id(&self) -> &AnimationId {
        &self.cue.id
    }

    /// Authored frame interval of the requested cue.
    pub fn frame_range(&self) -> FrameRange {
        self.cue.frames
    }

    /// Current authored frame without consuming a presentation tick.
    pub fn frame(&self) -> f32 {
        self.frame
    }

    /// Samples this presentation tick and returns the current authored frame.
    ///
    /// Looped ranges treat `end` as the wrap boundary, matching the pinned
    /// `mn_8022ED6C` comparison: advancing onto an integer end frame wraps it
    /// before it can be displayed. Non-looped ranges display and hold `end`.
    pub fn tick(&mut self) -> f32 {
        if self.first_sample_pending {
            self.first_sample_pending = false;
            return self.frame;
        }

        let advanced = self.frame + 1.0;
        self.frame = match self.cue.frames.loop_start {
            Some(loop_start) if advanced >= self.cue.frames.end => {
                loop_start + (advanced - self.cue.frames.end)
            }
            Some(_) => advanced,
            None => advanced.min(self.cue.frames.end),
        };
        self.frame
    }
}

/// A malformed declarative presentation request.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PlaybackError {
    #[error("animation {0:?} has a non-finite or unordered frame range")]
    InvalidFrameRange(AnimationId),
}

fn validate_range(cue: &AnimationCue) -> Result<(), PlaybackError> {
    let frames = cue.frames;
    let valid = frames.start.is_finite()
        && frames.end.is_finite()
        && frames.start <= frames.end
        && frames.loop_start.is_none_or(|loop_start| {
            loop_start.is_finite()
                && frames.start <= loop_start
                // Unit-rate playback can safely apply the source's single
                // wrap correction only when the loop spans at least one tick.
                && loop_start + 1.0 <= frames.end
        });
    if valid {
        Ok(())
    } else {
        Err(PlaybackError::InvalidFrameRange(cue.id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(id: &str, start: f32, end: f32, loop_start: Option<f32>) -> AnimationCue {
        AnimationCue {
            id: AnimationId::from(id),
            frames: FrameRange {
                start,
                end,
                loop_start,
            },
        }
    }

    #[test]
    fn request_samples_start_before_advancing_at_unit_rate() {
        let mut playback = AnimationPlayback::new(cue("idle", 10.0, 20.0, None)).unwrap();

        assert_eq!(playback.frame(), 10.0);
        assert_eq!(playback.tick(), 10.0);
        assert_eq!(playback.tick(), 11.0);
        assert_eq!(playback.tick(), 12.0);
    }

    #[test]
    fn loop_wraps_at_end_and_preserves_overshoot() {
        let mut playback = AnimationPlayback::new(cue("loop", 0.5, 2.25, Some(1.0))).unwrap();

        assert_eq!(playback.tick(), 0.5);
        assert_eq!(playback.tick(), 1.5);
        assert_eq!(playback.tick(), 1.25);
    }

    #[test]
    fn looped_integer_end_frame_is_not_displayed() {
        let mut playback = AnimationPlayback::new(cue("loop", 10.0, 15.0, Some(12.0))).unwrap();

        let samples = (0..7).map(|_| playback.tick()).collect::<Vec<_>>();

        assert_eq!(samples, [10.0, 11.0, 12.0, 13.0, 14.0, 12.0, 13.0]);
        assert!(!samples.contains(&15.0));
    }

    #[test]
    fn non_looped_clip_clamps_and_holds_end() {
        let mut playback = AnimationPlayback::new(cue("once", 2.0, 3.5, None)).unwrap();

        let samples = (0..5).map(|_| playback.tick()).collect::<Vec<_>>();

        assert_eq!(samples, [2.0, 3.0, 3.5, 3.5, 3.5]);
    }

    #[test]
    fn restart_resets_first_sample_even_for_the_same_animation_id() {
        let mut playback =
            AnimationPlayback::new(cue("shared-resource", 0.0, 49.0, Some(20.0))).unwrap();
        assert_eq!(playback.tick(), 0.0);
        assert_eq!(playback.tick(), 1.0);

        playback
            .restart(cue("shared-resource", 50.0, 99.0, Some(70.0)))
            .unwrap();

        assert_eq!(playback.animation_id().as_str(), "shared-resource");
        assert_eq!(playback.frame_range().start, 50.0);
        assert_eq!(playback.frame(), 50.0);
        assert_eq!(playback.tick(), 50.0);
        assert_eq!(playback.tick(), 51.0);
    }

    #[test]
    fn invalid_restart_is_transactional() {
        let mut playback = AnimationPlayback::new(cue("valid", 3.0, 8.0, None)).unwrap();
        playback.tick();

        let error = playback
            .restart(cue("invalid", f32::NAN, 8.0, None))
            .unwrap_err();

        assert_eq!(
            error,
            PlaybackError::InvalidFrameRange(AnimationId::from("invalid"))
        );
        assert_eq!(playback.animation_id().as_str(), "valid");
        assert_eq!(playback.frame(), 3.0);
        assert_eq!(playback.tick(), 4.0);
    }

    #[test]
    fn rejects_non_finite_and_unordered_ranges() {
        let invalid = [
            cue("nan-start", f32::NAN, 10.0, None),
            cue("infinite-end", 0.0, f32::INFINITY, None),
            cue("reversed", 10.0, 9.0, None),
            cue("nan-loop", 0.0, 10.0, Some(f32::NAN)),
            cue("early-loop", 0.0, 10.0, Some(-1.0)),
            cue("late-loop", 0.0, 10.0, Some(11.0)),
            cue("zero-span-loop", 0.0, 10.0, Some(10.0)),
            cue("sub-tick-loop", 0.0, 10.0, Some(9.5)),
        ];

        for cue in invalid {
            let expected_id = cue.id.clone();
            assert_eq!(
                AnimationPlayback::new(cue).unwrap_err(),
                PlaybackError::InvalidFrameRange(expected_id)
            );
        }
    }
}
