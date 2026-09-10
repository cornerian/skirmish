use extraction::Source;
use menus::{Menu, MenuState, Unlocks, input::pad};
use skirmish::renderer::{
    asset_menu::{AssetImportMenu, ImportAction},
    menu::{MenuEvent, MenuSession},
    ui::UiFrame,
};
use std::time::{Duration, Instant};

#[test]
fn main_menu_exposes_import_and_returns_without_replaying_close_input() {
    let mut menu =
        MenuSession::from_state(MenuState::at(Menu::Main, 0, Unlocks::default()).unwrap());
    menu.enable_asset_import();
    menu.tick([0, pad::UP as u32, 0, 0]);
    assert_eq!(menu.view().selected_label, "Import Game Assets");
    assert_eq!(
        menu.view().rows.iter().filter(|row| row.selected).count(),
        1
    );
    assert_eq!(
        menu.tick([0, pad::A as u32, 0, 0]),
        Some(MenuEvent::ImportAssetsRequested)
    );
    assert_eq!(menu.tick([0, pad::A as u32, 0, 0]), None);
    menu.synchronize_input([0, pad::B as u32, 0, 0]);
    assert_eq!(menu.tick([0, pad::B as u32, 0, 0]), None);
    assert_eq!(menu.view().selected_label, "Import Game Assets");
    menu.tick([0; 4]);
    menu.tick([pad::DOWN as u32, 0, 0, 0]);
    assert_eq!(menu.view().selected_label, "1-P Mode");
}

#[test]
fn importer_supports_controller_file_picker_cancel_and_error_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let mut screen = AssetImportMenu::new(Ok(temp.path().join("assets")), vec![temp.path().into()]);
    screen.tick([0, 0, pad::DOWN as u32, 0]);
    assert_eq!(screen.view().selected_label, "Choose ISO file...");
    assert_eq!(
        screen.tick([0, 0, pad::A as u32, 0]),
        Some(ImportAction::Browse)
    );
    let callback = screen.dialog_callback();
    assert!(screen.busy());
    callback(Err(sdl3::dialog::DialogError::Canceled), None);
    assert!(screen.poll());
    assert!(!screen.busy());
    assert!(
        screen
            .view()
            .status
            .unwrap()
            .lines
            .join("")
            .contains("cancelled")
    );
    let callback = screen.dialog_callback();
    callback(Ok(vec![temp.path().join("missing.iso")]), None);
    screen.poll();
    let deadline = Instant::now() + Duration::from_secs(5);
    while screen.busy() && Instant::now() < deadline {
        screen.poll();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!screen.busy());
    assert!(
        screen
            .view()
            .status
            .unwrap()
            .lines
            .join("")
            .contains("Opening ISO")
    );
    assert!(!temp.path().join("assets").exists());
    assert_eq!(
        screen.tick([pad::B as u32, 0, 0, 0]),
        Some(ImportAction::Back)
    );
}

#[test]
fn import_screen_layout_and_busy_controls_fit_small_and_portrait_windows() {
    let temp = tempfile::tempdir().unwrap();
    let mut screen = AssetImportMenu::new(Ok(temp.path().join("assets")), vec![]);
    for busy in [false, true] {
        if busy {
            screen.start(Source::Search(vec![]));
        }
        for [width, height] in [[1280, 720], [360, 800], [320, 180]] {
            let frame = UiFrame::menu(&screen.view(), width, height);
            assert!(frame.vertices.len() > 500);
            assert!(frame.vertices.iter().all(|vertex| {
                vertex
                    .position
                    .iter()
                    .all(|value| value.is_finite() && (-1.0001..=1.0001).contains(value))
            }));
        }
    }
    assert_eq!(
        screen.tick([pad::B as u32, 0, 0, 0]),
        Some(ImportAction::Cancel)
    );
    screen.cancel();
}

#[test]
fn failed_automatic_search_selects_manual_browsing() {
    let temp = tempfile::tempdir().unwrap();
    let mut screen = AssetImportMenu::new(Ok(temp.path().join("assets")), vec![temp.path().into()]);
    screen.start_search();
    let deadline = Instant::now() + Duration::from_secs(5);
    while screen.busy() && Instant::now() < deadline {
        screen.poll();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!screen.busy());
    assert_eq!(screen.view().selected_label, "Choose ISO file...");
    assert!(
        screen
            .view()
            .status
            .unwrap()
            .lines
            .join("")
            .contains("No valid Melee")
    );
    assert_eq!(
        screen.tick([pad::A as u32, 0, 0, 0]),
        Some(ImportAction::Browse)
    );
}
