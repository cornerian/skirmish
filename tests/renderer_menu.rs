use menus::input::pad;
use menus::{Action, Destination, Menu, MenuState, Scene, Unlocks};
use skirmish::renderer::menu::{FixedMenuClock, MenuEvent, MenuSession};
use skirmish::renderer::ui::{UI_SHADER, UiFrame};
use std::time::Duration;

fn ready(menu: Menu, selection: u16) -> MenuSession {
    MenuSession::from_state(MenuState::at(menu, selection, Unlocks::default()).unwrap())
}

fn ticks(session: &mut MenuSession, count: usize, buttons: u32) {
    for _ in 0..count {
        session.tick([buttons, 0, 0, 0]);
    }
}

#[test]
fn input_reaches_visible_rows_through_original_cooldown_and_controller_edges() {
    let mut session = MenuSession::default();
    ticks(&mut session, 19, 0);
    assert_eq!(session.snapshot().cooldown, 1);
    session.tick([pad::DOWN as u32, 0, 0, 0]);
    assert_eq!(session.view().selected_label, "1-P Mode");
    session.tick([0; 4]);
    session.tick([pad::DOWN as u32, 0, 0, 0]);
    assert_eq!(session.view().selected_label, "VS. Mode");
    assert_eq!(
        session.tick([pad::A as u32, 0, 0, 0]),
        Some(MenuEvent::Navigation(Action::Entered(Menu::Versus)))
    );
    assert_eq!(session.view().title, "VS. Mode");
    assert_eq!(session.view().selected_label, "Melee");
    ticks(&mut session, 120, pad::A as u32);
    assert_eq!(session.view().menu, Menu::Versus);
    assert_eq!(
        session.view().pending,
        None,
        "held Confirm must not open a leaf"
    );
}

#[test]
fn requested_leaf_is_visible_and_back_resumes_the_originating_selection() {
    let mut session = ready(Menu::RegularMatch, 1);
    assert_eq!(
        session.tick([0, 0, pad::A as u32, 0]),
        Some(MenuEvent::Navigation(Action::Requested(
            Destination::Scene(Scene::Adventure)
        )))
    );
    assert_eq!(
        session.view().pending,
        Some(Destination::Scene(Scene::Adventure))
    );
    assert_eq!(session.view().selected_label, "Adventure");
    assert_eq!(session.snapshot().controller_port, 2);
    ticks(&mut session, 50, pad::DOWN as u32);
    assert_eq!(session.view().selected_label, "Adventure");
    assert_eq!(
        session.tick([pad::B as u32, 0, 0, 0]),
        Some(MenuEvent::Resumed)
    );
    assert_eq!(session.view().pending, None);
    assert_eq!(session.view().menu, Menu::RegularMatch);
    assert_eq!(session.view().selected_label, "Adventure");
    assert_eq!(session.snapshot().cooldown, 5);
    ticks(&mut session, 120, pad::B as u32);
    assert_eq!(
        session.view().menu,
        Menu::RegularMatch,
        "held Back must not leave the resumed branch"
    );
    session.tick([0; 4]);
    session.tick([pad::B as u32, 0, 0, 0]);
    assert_eq!(session.view().menu, Menu::OnePlayer);
    assert_eq!(session.view().selected_label, "Regular Match");
}

#[test]
fn back_from_main_is_a_host_quit_request() {
    let mut session = ready(Menu::Main, 0);
    assert_eq!(
        session.tick([pad::B as u32, 0, 0, 0]),
        Some(MenuEvent::QuitRequested)
    );
}

#[test]
fn releasing_input_preserves_state_and_allows_fresh_confirm_on_any_port() {
    let mut session = ready(Menu::Main, 0);
    session.tick([pad::A as u32; 4]);
    assert_eq!(session.view().menu, Menu::OnePlayer);
    for _ in 0..5 {
        session.tick([pad::A as u32; 4]);
    }
    assert_eq!(session.snapshot().cooldown, 0);
    let before = session.snapshot();
    session.release_input();
    assert_eq!(session.snapshot(), before);
    assert_eq!(
        session.tick([0, 0, pad::A as u32, 0]),
        Some(MenuEvent::Navigation(Action::Entered(Menu::RegularMatch)))
    );
    assert_eq!(session.snapshot().controller_port, 2);
}

#[test]
fn releasing_input_preserves_pending_and_allows_fresh_back_after_resume() {
    let mut session = ready(Menu::RegularMatch, 1);
    session.tick([pad::A as u32, 0, 0, 0]);
    let pending = session.snapshot();
    assert_eq!(pending.pending, Some(Destination::Scene(Scene::Adventure)));
    assert_eq!(pending.cooldown, 5);
    session.release_input();
    assert_eq!(session.snapshot(), pending);
    assert_eq!(
        session.tick([pad::B as u32, 0, 0, 0]),
        Some(MenuEvent::Resumed)
    );
    ticks(&mut session, 5, pad::B as u32);
    let resumed = session.snapshot();
    assert_eq!(resumed.pending, None);
    assert_eq!(resumed.cooldown, 0);
    session.release_input();
    assert_eq!(session.snapshot(), resumed);
    assert_eq!(
        session.tick([pad::B as u32, 0, 0, 0]),
        Some(MenuEvent::Navigation(Action::Returned(Menu::OnePlayer)))
    );
}

#[test]
fn available_entries_and_selection_are_visible_on_every_branch() {
    for menu in Menu::ALL {
        let session = ready(menu, 0);
        let view = session.view();
        assert_eq!(view.title, menu.title());
        assert_eq!(view.rows.iter().filter(|row| row.selected).count(), 1);
        assert!(
            view.rows
                .iter()
                .all(|row| menu.is_available(row.index, Unlocks::default()))
        );
    }
    let regular = ready(Menu::RegularMatch, 0).view();
    assert_eq!(
        regular.rows.iter().map(|row| row.label).collect::<Vec<_>>(),
        ["Classic", "Adventure"]
    );
    let unlocked = MenuSession::from_state(
        MenuState::at(
            Menu::RegularMatch,
            2,
            Unlocks {
                all_star: true,
                sound_test: false,
            },
        )
        .unwrap(),
    )
    .view();
    assert_eq!(unlocked.selected_label, "All-Star");
    assert_eq!(unlocked.rows.len(), 3);
    assert!(
        ready(Menu::Data, 0)
            .view()
            .rows
            .iter()
            .all(|row| row.label != "Sound Test")
    );
}

#[test]
fn render_rate_does_not_change_menu_repeat_or_navigation() {
    fn replay(fps: u64) -> (Vec<menus::Snapshot>, usize) {
        let mut clock = FixedMenuClock::default();
        let mut session = ready(Menu::SpecialVersus, 0);
        let mut observations = Vec::new();
        let mut previous = Duration::ZERO;
        for frame in 1..=fps * 3 {
            let now = Duration::from_nanos(frame * 1_000_000_000 / fps);
            for _ in 0..clock.advance(now - previous) {
                session.tick([pad::DOWN as u32, 0, 0, 0]);
                observations.push(session.snapshot());
            }
            previous = now;
            // Rendering observes state without advancing it.
            let view = session.view();
            assert_eq!(
                view.selected_label,
                view.rows.iter().find(|row| row.selected).unwrap().label
            );
        }
        let count = observations.len();
        (observations, count)
    }
    let slow = replay(30);
    let fast = replay(144);
    assert_eq!(slow, fast);
    assert_eq!(slow.1, 180);
}

#[test]
fn stalled_clock_bounds_catchup_and_reset_discards_partial_time() {
    let mut clock = FixedMenuClock::default();
    assert_eq!(clock.advance(Duration::from_secs(10)), 8);
    assert_eq!(clock.advance(Duration::ZERO), 0);
    assert_eq!(clock.until_next_tick(), Duration::from_nanos(16_666_667));
    assert_eq!(clock.advance(Duration::from_millis(10)), 0);
    assert_eq!(clock.until_next_tick(), Duration::from_nanos(6_666_667));
    clock.reset();
    assert_eq!(clock.until_next_tick(), Duration::from_nanos(16_666_667));
}

#[test]
fn every_menu_layout_fits_landscape_portrait_and_small_windows() {
    for menu in Menu::ALL {
        for [width, height] in [[1280, 800], [1920, 1080], [640, 480], [360, 800]] {
            let view = ready(menu, 0).view();
            let frame = UiFrame::menu(&view, width, height);
            assert!(frame.vertices.len() > 500);
            assert_eq!(frame.vertices.len() % 6, 0);
            assert!(
                frame.vertices.iter().all(|vertex| vertex
                    .position
                    .into_iter()
                    .all(|n| n.is_finite() && (-1.0001..=1.0001).contains(&n))),
                "{menu:?} overflows {width}x{height}"
            );
        }
    }
    assert!(
        UiFrame::menu(&ready(Menu::Main, 0).view(), 0, 800)
            .vertices
            .is_empty()
    );
}

#[test]
fn linked_menu_wesl_validates_without_optional_gpu_capabilities() {
    let module = wgpu::naga::front::wgsl::parse_str(UI_SHADER).unwrap();
    wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
}
