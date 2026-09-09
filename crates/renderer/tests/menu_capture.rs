use menus::input::pad;
use menus::{Menu, MenuState, Unlocks};
use renderer::{menu::MenuSession, renderer::render_menu_headless, scene::Scene};
use std::{
    fs::File,
    io::BufReader,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const WIDTH: u32 = 257;
const HEIGHT: u32 = 193;

fn read_capture(path: &Path) -> Vec<u8> {
    let decoder = png::Decoder::new(BufReader::new(File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!([info.width, info.height], [WIDTH, HEIGHT]);
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    pixels.truncate(info.buffer_size());
    assert_eq!(pixels.len(), (WIDTH * HEIGHT * 4) as usize);
    assert!(
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255)
    );
    let background = &pixels[..4];
    let foreground = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| *pixel != background)
        .count();
    assert!(foreground > 500, "only {foreground} non-background pixels");
    pixels
}

fn changed_pixels(before: &[u8], after: &[u8]) -> usize {
    before
        .as_chunks::<4>()
        .0
        .iter()
        .zip(after.as_chunks::<4>().0)
        .filter(|(before, after)| before != after)
        .count()
}

#[test]
#[ignore = "requires a Vulkan/Metal/DX12/GLES graphics adapter"]
fn native_menu_input_changes_the_gpu_capture_and_preserves_unpadded_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("menu.png");
    let scene = Scene::demo();
    let mut session =
        MenuSession::from_state(MenuState::at(Menu::Main, 0, Unlocks::default()).unwrap());

    assert_eq!(session.view().selected_label, "1-P Mode");
    render_menu_headless(&scene, &session.view(), WIDTH, HEIGHT, &path).unwrap();
    let first = read_capture(&path);

    session.tick([pad::DOWN as u32, 0, 0, 0]);
    assert_eq!(session.view().selected_label, "VS. Mode");
    render_menu_headless(&scene, &session.view(), WIDTH, HEIGHT, &path).unwrap();
    let selected = read_capture(&path);
    assert!(
        changed_pixels(&first, &selected) > 300,
        "moving the selection must visibly move its highlight"
    );

    session.tick([0; 4]);
    session.tick([pad::A as u32, 0, 0, 0]);
    assert_eq!(session.view().menu, Menu::Versus);
    render_menu_headless(&scene, &session.view(), WIDTH, HEIGHT, &path).unwrap();
    let submenu = read_capture(&path);
    assert!(
        changed_pixels(&first, &submenu) > 100,
        "entering a branch must render its own heading and entries"
    );
    assert!(changed_pixels(&selected, &submenu) > 300);
}

#[test]
#[ignore = "requires a Vulkan/Metal/DX12/GLES graphics adapter"]
fn headless_menu_capture_needs_neither_an_sdl_video_driver_nor_a_display() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("headless-menu.png");
    // Isolate environment changes to a child process; initializing SDL video
    // would fail because this deliberately nonexistent driver cannot be loaded.
    let mut child = Command::new(env!("CARGO_BIN_EXE_skirmish-renderer"))
        .env(
            "SDL_VIDEODRIVER",
            "skirmish-headless-must-not-initialize-sdl",
        )
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("DISPLAY")
        .args(["--menus", "--width", "257", "--height", "193", "--headless"])
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "headless menu capture timed out: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    read_capture(&path);
}
