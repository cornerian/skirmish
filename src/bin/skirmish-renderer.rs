use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use clap::Parser;
use sdl3::{
    event::{Event, WindowEvent},
    keyboard::Scancode,
    mouse::MouseButton,
};
use skirmish::{
    controller::host::ControllerHub,
    menu::{
        MenuEffect,
        melee::{main_definition, main_interaction_map},
    },
    renderer::{
        audio::AudioOutput,
        gpu::{WindowRenderer, render_headless},
        melee as melee_renderer,
        menu_host::MenuHost,
        scene::Scene,
        viewport::MELEE_AUTHORED_EXTENT,
    },
};

const FRAME_RATE: u8 = 60;
const FRAME_BASE_NANOS: u64 = 1_000_000_000 / FRAME_RATE as u64;
const FRAME_REMAINDER_NANOS: u8 = (1_000_000_000 % FRAME_RATE as u64) as u8;
const MAX_CATCH_UP_TICKS: u8 = 4;

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
    controllers: Option<ControllerHub>,
    menu: Option<MenuHost>,
    orbit: [f32; 3],
    focused: bool,
    visible: bool,
    quit: bool,
    dirty: bool,
    next_frame: Instant,
    frame_phase: u8,
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
                WindowEvent::FocusLost => {
                    self.focused = false;
                    if let Some(menu) = &mut self.menu {
                        menu.clear_input();
                    }
                }
                WindowEvent::FocusGained => self.focused = true,
                WindowEvent::MouseLeave => {
                    if let Some(menu) = &mut self.menu {
                        menu.pointer_leave();
                    }
                }
                // Occlusion can follow an Exposed event during a Wayland
                // resize, whose requested frame must still be presented.
                WindowEvent::Hidden | WindowEvent::Minimized => {
                    self.visible = false;
                    if let Some(menu) = &mut self.menu {
                        menu.clear_input();
                    }
                }
                WindowEvent::Shown
                | WindowEvent::Restored
                | WindowEvent::Maximized
                | WindowEvent::Exposed => {
                    let resumed = !self.visible;
                    self.visible = true;
                    self.dirty = true;
                    if resumed {
                        self.next_frame = Instant::now();
                        self.frame_phase = 0;
                    }
                }
                WindowEvent::Resized(_, _)
                | WindowEvent::PixelSizeChanged(_, _)
                | WindowEvent::DisplayChanged(_) => {
                    let (width, height) = self.renderer.pixel_size();
                    self.renderer.resize(width, height);
                    let transform = self.renderer.presentation_transform(MELEE_AUTHORED_EXTENT);
                    if let Some(menu) = &mut self.menu {
                        menu.pointer_reproject(transform);
                    }
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
                } else if let Some(menu) = &mut self.menu {
                    menu.key(code, true, repeat);
                } else {
                    self.scene_key(code, repeat);
                }
            }
            Event::KeyUp {
                window_id,
                scancode: Some(code),
                repeat,
                ..
            } if window_id == self.renderer.window_id() => {
                if let Some(menu) = &mut self.menu {
                    menu.key(code, false, repeat);
                }
            }
            Event::MouseMotion {
                window_id, x, y, ..
            } if window_id == self.renderer.window_id() && self.focused => {
                let transform = self.renderer.presentation_transform(MELEE_AUTHORED_EXTENT);
                if let Some(menu) = &mut self.menu {
                    menu.pointer_motion([x, y], transform);
                }
            }
            Event::MouseButtonDown {
                window_id,
                mouse_btn: MouseButton::Left,
                x,
                y,
                ..
            } if window_id == self.renderer.window_id() && self.focused => {
                let transform = self.renderer.presentation_transform(MELEE_AUTHORED_EXTENT);
                if let Some(menu) = &mut self.menu {
                    menu.pointer_primary_down([x, y], transform);
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

    fn advance_frame_deadline(&mut self) {
        let mut nanos = FRAME_BASE_NANOS;
        self.frame_phase += FRAME_REMAINDER_NANOS;
        if self.frame_phase >= FRAME_RATE {
            self.frame_phase -= FRAME_RATE;
            nanos += 1;
        }
        self.next_frame += Duration::from_nanos(nanos);
    }

    fn drop_frame_debt(&mut self, now: Instant) {
        self.next_frame = now;
        self.frame_phase = 0;
        self.advance_frame_deadline();
    }

    fn apply_menu_effects(&mut self, effects: Vec<MenuEffect>) {
        for effect in effects {
            match effect {
                MenuEffect::SelectionChanged {
                    selected, sound, ..
                } => {
                    println!(
                        "Melee menu selection: {}{}",
                        selected.as_str(),
                        sound.map_or_else(String::new, |sound| format!(" (sound {})", sound.0))
                    );
                }
                MenuEffect::ActionRequested {
                    trigger,
                    item,
                    action,
                } => {
                    println!(
                        "Melee menu action: {trigger:?}{} -> {}{}",
                        item.map_or_else(String::new, |item| format!(" on {}", item.as_str())),
                        action.destination.as_str(),
                        action
                            .sound
                            .map_or_else(String::new, |sound| format!(" (sound {})", sound.0))
                    );
                }
            }
            self.dirty = true;
        }
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
            let frame_due = self.drawable() && now >= self.next_frame;
            if frame_due {
                let controller_samples = if let Some(controllers) = &mut self.controllers {
                    controllers.poll().context("polling menu controllers")?
                } else {
                    Vec::new()
                };
                let mut due_ticks = 0;
                while now >= self.next_frame && due_ticks < MAX_CATCH_UP_TICKS {
                    self.advance_frame_deadline();
                    due_ticks += 1;
                }
                if now >= self.next_frame {
                    self.drop_frame_debt(now);
                }
                for tick in 0..due_ticks {
                    if let Some(menu) = &mut self.menu {
                        let effects = if tick + 1 == due_ticks {
                            menu.tick(&controller_samples, self.focused)
                        } else {
                            menu.tick_previous()
                        };
                        self.apply_menu_effects(effects);
                        // The presentation clock will consume every menu tick as
                        // runtime animation tracks are connected.
                        self.dirty = true;
                    }
                }
            }
            if frame_due && (self.dirty || self.frame_limit.is_some() || self.menu.is_some()) {
                let [yaw, pitch, zoom] = self.orbit;
                let presented = self
                    .renderer
                    .render(yaw, pitch, zoom)
                    .context("rendering an SDL frame")?;
                self.dirty = !presented;
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
            if self.drawable() && (self.dirty || self.frame_limit.is_some() || self.menu.is_some())
            {
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
    let melee_mode = cli.melee_menu_assets.is_some();
    let scene = match (&cli.scene, &cli.melee_menu_assets) {
        (Some(path), None) => {
            Scene::load(path).with_context(|| format!("loading scene from {}", path.display()))?
        }
        (None, Some(path)) => melee_renderer::load_main_menu_default_pose(path)
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
    if melee_mode {
        println!("SDL3 host: direct Melee UI runtime. Q quits.");
        println!(
            "Menu: arrows/stick move, A/Enter confirms, B/Escape backs out, mouse hovers and clicks."
        );
    } else {
        println!("SDL3 host: scene preview. Q quits.");
        println!("Scene: arrows orbit, +/- zoom, R resets, Space plays a cue.");
    }
    let events = sdl.event_pump().context("creating shared SDL event pump")?;
    let controllers = melee_mode
        .then(|| ControllerHub::with_sdl(&sdl).context("initializing menu controllers"))
        .transpose()?;
    let menu = melee_mode
        .then(|| MenuHost::new(main_definition(), main_interaction_map(), []))
        .transpose()
        .context("initializing Melee Main menu")?;
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
        controllers,
        menu,
        orbit: [0.0, 0.0, 1.0],
        focused: true,
        visible: true,
        quit: false,
        dirty: true,
        next_frame: Instant::now(),
        frame_phase: 0,
        presented_frames: 0,
        frame_limit: cli.frames,
    }
    .run(events)
}
