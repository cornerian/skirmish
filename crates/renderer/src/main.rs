use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use clap::Parser;
use menus::Unlocks;
use renderer::{
    asset_menu::{AssetImportMenu, ImportAction},
    audio::AudioOutput,
    controls::{ControllerPorts, KeyboardInput},
    menu::{FixedMenuClock, MenuEvent, MenuSession},
    renderer::{WindowRenderer, render_headless, render_menu_headless},
    scene::Scene,
};
use sdl3::{
    event::{Event, WindowEvent},
    keyboard::Scancode,
};
use skirmish::{assets, controller::host::ControllerHub};

const FRAME_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Debug, Parser)]
#[command(
    name = "skirmish-renderer",
    version,
    about = "Skirmish SDL3 menus and wgpu scene preview"
)]
struct Cli {
    /// Load a native scene export instead of the built-in demonstration.
    #[arg(long, value_name = "PATH")]
    scene: Option<PathBuf>,
    /// Start in the interactive menu. F1 toggles menus and scene preview.
    #[arg(long)]
    menus: bool,
    /// Open the in-game asset import screen.
    #[arg(long)]
    import_assets: bool,
    /// Override the player asset storage directory (normally the OS app-data folder).
    #[arg(long, value_name = "DIRECTORY")]
    asset_dir: Option<PathBuf>,
    /// Make All-Star available in the menu preview.
    #[arg(long)]
    all_star: bool,
    /// Make Sound Test available in the menu preview.
    #[arg(long)]
    sound_test: bool,
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
    ports: ControllerPorts,
    keyboard: KeyboardInput,
    menu: MenuSession,
    menu_active: bool,
    asset_menu: AssetImportMenu,
    import_active: bool,
    clock: FixedMenuClock,
    reset_elapsed: bool,
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
    fn update_menu(&mut self) {
        self.renderer.set_menu(self.menu_active.then(|| {
            if self.import_active {
                self.asset_menu.view()
            } else {
                self.menu.view()
            }
        }));
        self.dirty = true;
    }

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
                    self.keyboard.clear();
                    self.ports.release();
                    self.menu.release_input();
                    self.asset_menu.release_input();
                }
                WindowEvent::FocusGained => {
                    self.focused = true;
                }
                // Occlusion can follow an Exposed event during a Wayland
                // resize, whose requested frame must still be presented.
                WindowEvent::Hidden | WindowEvent::Minimized => {
                    self.visible = false;
                    self.keyboard.clear();
                    self.ports.release();
                    self.menu.release_input();
                    self.asset_menu.release_input();
                    self.clock.reset();
                    self.reset_elapsed = true;
                }
                WindowEvent::Shown
                | WindowEvent::Restored
                | WindowEvent::Maximized
                | WindowEvent::Exposed => {
                    self.reset_elapsed |= !self.visible;
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
                if !repeat && code == Scancode::F1 && !self.import_active {
                    self.menu_active = !self.menu_active;
                    self.keyboard.clear();
                    self.ports.release();
                    self.menu.release_input();
                    self.clock.reset();
                    self.reset_elapsed = true;
                    self.update_menu();
                } else if !repeat && code == Scancode::Q {
                    self.quit = true;
                } else if self.menu_active {
                    self.keyboard.key(code, true, repeat);
                } else {
                    self.scene_key(code, repeat);
                }
            }
            Event::KeyUp {
                window_id,
                scancode: Some(code),
                ..
            } if window_id == self.renderer.window_id() => {
                self.keyboard.key(code, false, false);
            }
            Event::DropFile {
                window_id,
                filename,
                ..
            } if window_id == self.renderer.window_id() && !self.asset_menu.busy() => {
                self.menu_active = true;
                self.import_active = true;
                self.asset_menu.start(assets::Source::File(filename.into()));
                self.keyboard.clear();
                self.ports.release();
                self.asset_menu.release_input();
                self.update_menu();
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

    fn tick_menu(&mut self) {
        let mut held = [0; 4];
        if self.focused {
            if let Some(controllers) = &mut self.controllers {
                match controllers.poll() {
                    Ok(devices) => held = self.ports.sample(&devices),
                    Err(error) => {
                        eprintln!("warning: controller input unavailable: {error}");
                        self.controllers = None;
                        self.ports.release();
                    }
                }
            }
            held[0] |= self.keyboard.sample();
        }
        if self.import_active {
            let before = self.asset_menu.view();
            match self.asset_menu.tick(held) {
                Some(ImportAction::Search) => self.asset_menu.start_search(),
                Some(ImportAction::Browse) => {
                    let callback = self.asset_menu.dialog_callback();
                    if let Err(error) = self.renderer.choose_iso(callback) {
                        self.asset_menu.dialog_failed(error);
                    }
                }
                Some(ImportAction::Cancel) => self.asset_menu.cancel(),
                Some(ImportAction::Back) => {
                    self.import_active = false;
                    // Retain the current Confirm/Back edge in the originating
                    // menu, so closing this screen cannot immediately quit.
                    self.menu.synchronize_input(held);
                }
                None => {}
            }
            if !self.import_active || before != self.asset_menu.view() {
                self.update_menu();
            }
            return;
        }
        if let Some(event) = self.menu.tick(held) {
            if matches!(event, MenuEvent::QuitRequested) {
                self.quit = true;
            } else {
                if matches!(event, MenuEvent::ImportAssetsRequested) {
                    self.import_active = true;
                    // Consume the opening press without selecting Search.
                    self.asset_menu.release_input();
                    self.asset_menu.tick(held);
                }
                self.cue();
                self.update_menu();
            }
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
        let mut last_update = Instant::now();
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
            if self.asset_menu.poll() && self.import_active {
                self.update_menu();
            }
            let now = Instant::now();
            // Activation events can arrive after an inactive 250 ms wait.
            // That inactive time must not advance the newly resumed menu.
            let elapsed = if std::mem::take(&mut self.reset_elapsed) {
                Duration::ZERO
            } else {
                now.duration_since(last_update)
            };
            last_update = now;
            if self.menu_active && self.drawable() {
                for _ in 0..self.clock.advance(elapsed) {
                    self.tick_menu();
                    if self.quit {
                        break;
                    }
                }
            } else {
                self.clock.reset();
            }
            if self.quit {
                break;
            }
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
            if self.drawable() {
                if self.menu_active {
                    wait = wait.min(
                        self.clock
                            .until_next_tick()
                            .saturating_sub(last_update.elapsed()),
                    );
                }
                if self.dirty || self.frame_limit.is_some() {
                    wait = wait.min(self.next_frame.saturating_duration_since(Instant::now()));
                }
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
    let scene = match &cli.scene {
        Some(path) => {
            Scene::load(path).with_context(|| format!("loading scene from {}", path.display()))?
        }
        None => Scene::demo(),
    };
    for warning in &scene.warnings {
        eprintln!("warning: {warning}");
    }
    let mut menu = MenuSession::new(Unlocks {
        all_star: cli.all_star,
        sound_test: cli.sound_test,
    });
    menu.enable_asset_import();
    if let Some(output) = &cli.headless {
        if cli.import_assets {
            let import = AssetImportMenu::new(
                Ok(cli
                    .asset_dir
                    .clone()
                    .unwrap_or_else(|| PathBuf::from("assets"))),
                vec![],
            );
            render_menu_headless(&scene, &import.view(), cli.width, cli.height, output)?;
        } else if cli.menus {
            render_menu_headless(&scene, &menu.view(), cli.width, cli.height, output)?;
        } else {
            render_headless(&scene, cli.width, cli.height, output)?;
        }
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
    let mut renderer = pollster::block_on(WindowRenderer::new(window, &scene))?;
    println!("Graphics adapter: {}", renderer.adapter_name());
    println!(
        "SDL3 host: F1 toggles menus/scene. Menus: arrows/D-pad, Enter/A selects, Esc/B backs. Q quits."
    );
    println!("Scene: arrows orbit, +/- zoom, R resets, Space plays a cue.");
    let menu_active = cli.menus || cli.import_assets || cli.scene.is_none();
    let asset_destination = cli.asset_dir.map(Ok).unwrap_or_else(|| {
        sdl3::filesystem::get_pref_path("Skirmish", "Skirmish")
            .map(|path| path.join("assets").join(assets::BUNDLE_NAME))
            .map_err(|error| format!("Asset storage is unavailable: {error}"))
    });
    let mut search_roots = Vec::new();
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        let home = PathBuf::from(home);
        search_roots.extend([
            home.join("Downloads"),
            home.join("Games"),
            home.join("Desktop"),
        ]);
    }
    for path in ["/mnt/archive/datasets/melee", "/mnt/shared/Games"] {
        if std::path::Path::new(path).is_dir() {
            search_roots.push(path.into());
        }
    }
    let asset_menu = AssetImportMenu::new(asset_destination, search_roots);
    renderer.set_menu(menu_active.then(|| {
        if cli.import_assets {
            asset_menu.view()
        } else {
            menu.view()
        }
    }));
    let events = sdl.event_pump().context("creating shared SDL event pump")?;
    let controllers = match ControllerHub::with_sdl(&sdl) {
        Ok(hub) => Some(hub),
        Err(error) => {
            eprintln!("warning: controllers unavailable: {error}");
            None
        }
    };
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
        ports: ControllerPorts::default(),
        keyboard: KeyboardInput::default(),
        menu,
        menu_active,
        asset_menu,
        import_active: cli.import_assets,
        clock: FixedMenuClock::default(),
        reset_elapsed: true,
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
