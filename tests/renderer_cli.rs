use std::{
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

fn cli_without_display() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_skirmish-renderer"));
    command
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("DISPLAY");
    command
}

fn assert_usage_error(output: &Output, option: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains(option), "{stderr}");
    assert!(!stderr.contains("initializing SDL"), "{stderr}");
    assert!(!stderr.contains("creating SDL window"), "{stderr}");
    assert!(!stderr.contains("audio unavailable"), "{stderr}");
}

#[test]
fn help_is_available_without_a_display() {
    let output = cli_without_display().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skirmish-renderer"), "{stdout}");
    assert!(stdout.contains("--headless"), "{stdout}");
    assert!(stdout.contains("--menus"), "{stdout}");
    assert!(stdout.contains("--melee-menu-assets"), "{stdout}");
    assert!(stdout.contains("--no-audio"), "{stdout}");
    assert!(output.stderr.is_empty());
}

#[test]
fn direct_melee_assets_reject_the_legacy_menu_overlay_before_loading_assets() {
    let output = cli_without_display()
        .args([
            "--melee-menu-assets",
            "does-not-need-to-exist.json",
            "--menus",
        ])
        .output()
        .unwrap();
    assert_usage_error(&output, "--menus");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--melee-menu-assets"), "{stderr}");
    assert!(!stderr.contains("loading Melee menu assets"), "{stderr}");
}

#[test]
fn dimensions_outside_the_supported_range_fail_before_startup() {
    for option in ["--width", "--height"] {
        for value in ["0", "8193"] {
            let output = cli_without_display()
                .args([option, value])
                .output()
                .unwrap();
            assert_usage_error(&output, option);
        }
    }
}

#[test]
fn a_frame_limit_must_be_positive() {
    let output = cli_without_display()
        .args(["--frames", "0"])
        .output()
        .unwrap();
    assert_usage_error(&output, "--frames");
}

#[test]
fn headless_output_rejects_a_window_frame_limit() {
    let directory = tempfile::tempdir().unwrap();
    let image = directory.path().join("preview.png");
    let output = cli_without_display()
        .arg("--headless")
        .arg(&image)
        .args(["--frames", "1"])
        .output()
        .unwrap();
    assert_usage_error(&output, "--headless");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--frames"), "{stderr}");
    assert!(!image.exists());
}

#[test]
fn malformed_scene_fails_before_window_and_audio_initialization() {
    let directory = tempfile::tempdir().unwrap();
    let scene = directory.path().join("invalid.json");
    std::fs::write(&scene, "{").unwrap();
    let output = cli_without_display()
        .arg("--scene")
        .arg(&scene)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("loading scene from"), "{stderr}");
    assert!(stderr.contains("invalid.json"), "{stderr}");
    assert!(!stderr.contains("initializing SDL"), "{stderr}");
    assert!(!stderr.contains("creating SDL window"), "{stderr}");
    assert!(!stderr.contains("audio unavailable"), "{stderr}");
    assert!(output.stdout.is_empty());
}

fn output_with_timeout(command: &mut Command, context: &str) -> Output {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "{context} timed out: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

#[test]
#[cfg(all(
    unix,
    not(target_os = "macos"),
    not(target_os = "ios"),
    not(target_os = "android")
))]
fn dummy_video_driver_fails_with_a_clear_error_before_graphics_startup() {
    let output = output_with_timeout(
        cli_without_display()
            .env("SDL_VIDEODRIVER", "dummy")
            .env_remove("WGPU_BACKEND")
            .args(["--no-audio", "--menus", "--frames", "1"]),
        "dummy SDL startup",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("SDL video driver"), "{stderr}");
    assert!(stderr.contains("dummy"), "{stderr}");
    assert!(
        stderr.contains("cannot provide a graphics surface"),
        "{stderr}"
    );
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(!stderr.contains("audio unavailable"), "{stderr}");
    assert!(
        output.stdout.is_empty(),
        "graphics startup unexpectedly ran"
    );
}

fn assert_window_smoke(menus: bool) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_skirmish-renderer"));
    command.args([
        "--no-audio",
        "--width",
        "320",
        "--height",
        "180",
        "--frames",
        "2",
    ]);
    if menus {
        command.arg("--menus");
    }
    let output = output_with_timeout(&mut command, "SDL window smoke test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Graphics adapter:"), "{stdout}");
    assert!(stdout.contains("SDL3 host:"), "{stdout}");
    assert!(stdout.contains("Presented 2 frames."), "{stdout}");
}

#[test]
#[ignore = "requires a window compositor and a graphics adapter"]
fn window_smoke_exits_after_presenting_the_requested_frames() {
    assert_window_smoke(false);
}

#[test]
#[ignore = "requires a window compositor and a graphics adapter"]
fn menu_window_smoke_exits_after_presenting_the_requested_frames() {
    assert_window_smoke(true);
}
