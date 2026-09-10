use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use clap::Parser;
use sdl3::{
    event::{Event, WindowEvent},
    keyboard::Scancode,
};
use skirmish::renderer::{
    audio::AudioOutput,
    gpu::{WindowRenderer, render_headless},
    melee,
    scene::Scene,
};

const FRAME_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Debug, Parser)]
#[command(
    name = "skirmish-renderer",
    version,
    about = "Skirmish SDL3/wgpu scene and direct Melee UI host"
)]
struct Cli {
    /// Load a native scene export instead of the built-in demonstration.
    #[arg(long, value_name = "PATH")]
    scene: Option<PathBuf>,
    /// Load the four MnMaAll roots selected by the original main-menu source.
    /// This development view executes the currently connected original source slice.
    #[arg(long, value_name = "SCENE.json", conflicts_with = "scene")]
    melee_menu_assets: Option<PathBuf>,
    /// Render one PNG without opening SDL or an audio device.
    #[arg(long, value_name = "OUTPUT.png", conflicts_with = "frames")]
    headless: Option<PathBuf>,
    #[arg(long, default_value_t = 1280, value_parser = clap::value_parser!(u32).range(1..=8192))]
    width: u32,
    #[arg(long, default_value_t = 720, value_parser = clap::value_parser!(u32).range(1..=8192))]
    height: u32,
    /// Exit after this many successfully presented frames.
    #[arg(long, value_name = "COUNT", value_parser = clap::value_parser!(u64).range(1..))]
    frames: Option<u64>,
    /// Disable audio device initialization.
    #[arg(long)]
    no_audio: bool,
}

struct App {
    renderer: WindowRenderer,
    audio: Option<AudioOutput>,
    orbit: [f32; 3],
    focused: bool,
    visible: bool,
    quit: bool,
    dirty: bool,
    next_frame: Instant,
    presented_frames: u64,
    frame_limit: Option<u64>,
}

impl App {
    fn event(&mut self, event: Event) {
        match event {
            Event::Quit { .. } | Event::AppTerminating { .. } => self.quit = true,
            Event::Window {
                window_id,
                win_event,
                ..
            } if window_id == self.renderer.window_id() => match win_event {
                WindowEvent::CloseRequested => self.quit = true,
                WindowEvent::FocusLost => self.focused = false,
                WindowEvent::FocusGained => self.focused = true,
                // Occlusion can follow an Exposed event during a Wayland
                // resize, whose requested frame must still be presented.
                WindowEvent::Hidden | WindowEvent::Minimized => {
                    self.visible = false;
                }
                WindowEvent::Shown
                | WindowEvent::Restored
                | WindowEvent::Maximized
                | WindowEvent::Exposed => {
                    self.visible = true;
                    self.dirty = true;
                }
                WindowEvent::Resized(_, _)
                | WindowEvent::PixelSizeChanged(_, _)
                | WindowEvent::DisplayChanged(_) => {
                    let (width, height) = self.renderer.pixel_size();
                    self.renderer.resize(width, height);
                    self.dirty = true;
                }
                _ => {}
            },
            Event::KeyDown {
                window_id,
                scancode: Some(code),
                repeat,
                ..
            } if window_id == self.renderer.window_id() && self.focused => {
                if !repeat && code == Scancode::Q {
                    self.quit = true;
                } else {
                    self.scene_key(code, repeat);
                }
            }
            _ => {}
        }
    }

    fn scene_key(&mut self, code: Scancode, repeat: bool) {
        match code {
            Scancode::Escape => self.quit = true,
            Scancode::Left => self.orbit[0] -= 0.1,
            Scancode::Right => self.orbit[0] += 0.1,
            Scancode::Up => self.orbit[1] = (self.orbit[1] + 0.1).min(1.2),
            Scancode::Down => self.orbit[1] = (self.orbit[1] - 0.1).max(-1.2),
            Scancode::Equals | Scancode::KpPlus => self.orbit[2] = (self.orbit[2] * 0.9).max(0.2),
            Scancode::Minus | Scancode::KpMinus => self.orbit[2] = (self.orbit[2] * 1.1).min(5.0),
            Scancode::R => self.orbit = [0.0, 0.0, 1.0],
            Scancode::Space if !repeat => {
                self.cue();
                return;
            }
            _ => return,
        }
        self.dirty = true;
    }

    fn cue(&mut self) {
        if let Some(audio) = &mut self.audio {
            let _ = audio.play_cue();
        }
    }

    fn check_audio(&mut self) {
        if let Some(audio) = &self.audio
            && audio.error_count() > 0
        {
            eprintln!("warning: audio stream failed; disabling audio");
            self.audio = None;
        }
    }

    fn drawable(&self) -> bool {
        let (width, height) = self.renderer.pixel_size();
        self.visible && width > 0 && height > 0
    }

    fn run(mut self, mut events: sdl3::EventPump) -> Result<()> {
        let mut pending = None;
        while !self.quit {
            // This is the sole event consumer. The event returned by waiting is
            // dispatched too, rather than being lost before the next poll.
            if let Some(event) = pending.take() {
                self.event(event);
            }
            for event in events.poll_iter() {
                self.event(event);
            }
            if self.quit {
                break;
            }
            let now = Instant::now();
            self.check_audio();
            if self.drawable()
                && now >= self.next_frame
                && (self.dirty || self.frame_limit.is_some())
            {
                let [yaw, pitch, zoom] = self.orbit;
                let presented = self
                    .renderer
                    .render(yaw, pitch, zoom)
                    .context("rendering an SDL frame")?;
                self.dirty = !presented;
                self.next_frame = Instant::now() + FRAME_INTERVAL;
                if presented {
                    self.presented_frames += 1;
                    if self
                        .frame_limit
                        .is_some_and(|limit| self.presented_frames >= limit)
                    {
                        println!("Presented {} frames.", self.presented_frames);
                        break;
                    }
                }
            }
            let mut wait = Duration::from_millis(250);
            if self.drawable() && (self.dirty || self.frame_limit.is_some()) {
                wait = wait.min(self.next_frame.saturating_duration_since(Instant::now()));
            }
            // SDL waits in integer milliseconds. Round up to avoid busy polling
            // for the fractional millisecond at the end of a 60 Hz tick.
            pending =
                events.wait_event_timeout_ms(wait.as_nanos().div_ceil(1_000_000).min(250) as u32);
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let scene = match (&cli.scene, &cli.melee_menu_assets) {
        (Some(path), None) => {
            Scene::load(path).with_context(|| format!("loading scene from {}", path.display()))?
        }
        (None, Some(path)) => melee::load_main_menu_default_pose(path)
            .with_context(|| format!("loading Melee menu assets from {}", path.display()))?,
        (None, None) => Scene::demo(),
        (Some(_), Some(_)) => unreachable!("clap rejects conflicting scene arguments"),
    };
    for warning in &scene.warnings {
        eprintln!("warning: {warning}");
    }
    if let Some(output) = &cli.headless {
        render_headless(&scene, cli.width, cli.height, output)?;
        println!(
            "Rendered {}x{} to {}",
            cli.width,
            cli.height,
            output.display()
        );
        return Ok(());
    }

    let sdl = sdl3::init().context("initializing SDL3")?;
    let video = sdl.video().context("initializing SDL video")?;
    let window = video
        .window("Skirmish", cli.width, cli.height)
        .resizable()
        .high_pixel_density()
        .position_centered()
        .metal_view()
        .build()
        .context("creating SDL window")?;
    let renderer = pollster::block_on(WindowRenderer::new(window, &scene))?;
    println!("Graphics adapter: {}", renderer.adapter_name());
    if cli.melee_menu_assets.is_some() {
        println!("SDL3 host: direct Melee UI development scene. Q quits.");
    } else {
        println!("SDL3 host: scene preview. Q quits.");
    }
    println!("Scene: arrows orbit, +/- zoom, R resets, Space plays a cue.");
    let events = sdl.event_pump().context("creating shared SDL event pump")?;
    let audio = if cli.no_audio {
        None
    } else {
        match AudioOutput::open_default() {
            Ok(audio) => Some(audio),
            Err(error) => {
                eprintln!("warning: audio unavailable: {error:#}");
                None
            }
        }
    };
    App {
        renderer,
        audio,
        orbit: [0.0, 0.0, 1.0],
        focused: true,
        visible: true,
        quit: false,
        dirty: true,
        next_frame: Instant::now(),
        presented_frames: 0,
        frame_limit: cli.frames,
    }
    .run(events)
}
