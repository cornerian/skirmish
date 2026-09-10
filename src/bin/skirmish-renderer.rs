use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use skirmish::{
    controller::host::ControllerHub,
    menu::melee::{main_definition, main_interaction_map, resolve_internal_destination},
    renderer::{
        audio::AudioOutput,
        gpu::{WindowRenderer, render_headless},
        melee as melee_renderer,
        menu_host::MenuHost,
        scene::Scene,
        window_host::{MenuWindow, WindowHost, WindowMode},
    },
};

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
    let mode = match menu {
        Some(menu) => WindowMode::Menu(Box::new(
            MenuWindow::new(menu, controllers, resolve_internal_destination)
                .context("initializing Melee Main presentation")?,
        )),
        None => WindowMode::Preview,
    };
    WindowHost::new(renderer, audio, mode, cli.frames).run(events)
}
