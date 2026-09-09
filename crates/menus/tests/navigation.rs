use menus::{Action, Destination, Menu, MenuState, Panel, Scene, Unlocks, input::*};

fn drain(state: &mut MenuState) {
    while state.snapshot().cooldown != 0 {
        assert_eq!(state.step(0), None);
    }
}

fn confirm(state: &mut MenuState) -> Action {
    drain(state);
    state.step(CONFIRM).unwrap()
}

#[test]
fn stadium_path_resumes_at_the_selected_entry_and_returns_to_each_parent() {
    let mut state = MenuState::new(Unlocks::default());
    assert_eq!(state.snapshot().cooldown, 20);
    assert_eq!(confirm(&mut state), Action::Entered(Menu::OnePlayer));
    drain(&mut state);
    state.step(DOWN);
    state.step(DOWN);
    // Hidden 1-P slot two is skipped, while its original indices are retained.
    assert_eq!(state.snapshot().selection, 3);
    assert_eq!(confirm(&mut state), Action::Entered(Menu::Stadium));
    drain(&mut state);
    state.step(UP);
    assert_eq!(state.snapshot().selection, 2);
    assert_eq!(
        confirm(&mut state),
        Action::Requested(Destination::Panel(Panel::MultiMan))
    );
    let pending = state.snapshot();
    for _ in 0..20 {
        assert_eq!(state.step(BACK | DOWN), None);
    }
    assert_eq!(state.snapshot(), pending);
    assert!(state.resume());
    assert!(!state.resume());
    assert_eq!(state.snapshot().selection, 2);
    drain(&mut state);
    assert_eq!(state.step(BACK), Some(Action::Returned(Menu::OnePlayer)));
    assert_eq!(state.snapshot().selection, 3);
    drain(&mut state);
    assert_eq!(state.step(BACK), Some(Action::Returned(Menu::Main)));
    assert_eq!(state.snapshot().selection, 0);
}

#[test]
fn transition_cooldown_consumes_exactly_five_polls_without_buffering_input() {
    let mut state = MenuState::at(Menu::Main, 1, Unlocks::default()).unwrap();
    assert_eq!(state.step(CONFIRM), Some(Action::Entered(Menu::Versus)));
    for expected in (0..5).rev() {
        assert_eq!(state.step(CONFIRM | DOWN), None);
        assert_eq!(state.snapshot().cooldown, expected);
        assert_eq!(state.snapshot().buttons, 0);
        assert_eq!(state.snapshot().selection, 0);
    }
    assert_eq!(state.step(0), None);
    assert_eq!(state.step(DOWN), Some(Action::Moved));
    assert_eq!(state.snapshot().selection, 1);
}

#[test]
fn simultaneous_inputs_preserve_callback_priority() {
    let mut state = MenuState::at(Menu::Main, 1, Unlocks::default()).unwrap();
    assert_eq!(
        state.step(CONFIRM | BACK | UP | DOWN),
        Some(Action::Entered(Menu::Versus))
    );
    drain(&mut state);
    assert_eq!(
        state.step(BACK | UP | DOWN),
        Some(Action::Returned(Menu::Main))
    );
    drain(&mut state);
    assert_eq!(state.step(UP | DOWN), Some(Action::Moved));
    assert_eq!(state.snapshot().selection, 0);
}

#[test]
fn every_navigation_cycle_visits_each_available_entry_once() {
    for unlocks in [
        Unlocks::default(),
        Unlocks {
            all_star: true,
            sound_test: true,
        },
    ] {
        for menu in Menu::ALL {
            let available: Vec<_> = menu
                .entries()
                .iter()
                .filter(|entry| menu.is_available(entry.index, unlocks))
                .map(|entry| entry.index)
                .collect();
            let mut state = MenuState::at(menu, available[0], unlocks).unwrap();
            for &selection in available.iter().cycle().skip(1).take(available.len()) {
                assert_eq!(state.step(DOWN), Some(Action::Moved));
                assert_eq!(state.snapshot().selection, selection, "{menu:?}");
            }
            for &selection in available.iter().rev() {
                assert_eq!(state.step(UP), Some(Action::Moved));
                assert_eq!(state.snapshot().selection, selection, "{menu:?}");
            }
        }
    }
}

#[test]
fn external_requests_preserve_confirming_port_only_where_upstream_sets_it() {
    for (menu, selection, destination, expected_port, expected_cooldown) in [
        (
            Menu::RegularMatch,
            1,
            Destination::Scene(Scene::Adventure),
            3,
            5,
        ),
        (
            Menu::Stadium,
            0,
            Destination::Scene(Scene::TargetTest),
            3,
            5,
        ),
        (
            Menu::Stadium,
            1,
            Destination::Scene(Scene::HomeRunContest),
            3,
            5,
        ),
        (Menu::Stadium, 2, Destination::Panel(Panel::MultiMan), 0, 5),
        (
            Menu::OnePlayer,
            4,
            Destination::Scene(Scene::Training),
            3,
            0,
        ),
        (Menu::Data, 0, Destination::Panel(Panel::Snapshots), 0, 0),
        (Menu::Versus, 0, Destination::Scene(Scene::Versus), 0, 5),
    ] {
        let mut state = MenuState::at(menu, selection, Unlocks::default()).unwrap();
        assert_eq!(
            state.step_for_port(CONFIRM, 3),
            Some(Action::Requested(destination))
        );
        assert_eq!(state.snapshot().controller_port, expected_port);
        assert_eq!(state.snapshot().cooldown, expected_cooldown);
        assert_eq!(state.snapshot().menu, menu);
        assert_eq!(state.snapshot().selection, selection);
    }
}

#[test]
fn invalid_and_locked_initial_states_are_rejected() {
    for menu in Menu::ALL {
        assert!(MenuState::at(menu, menu.selection_count(), Unlocks::default()).is_err());
        assert!(MenuState::at(menu, u16::MAX, Unlocks::default()).is_err());
    }
    for (menu, selection) in [
        (Menu::OnePlayer, 2),
        (Menu::Trophies, 2),
        (Menu::Options, 3),
        (Menu::RegularMatch, 2),
        (Menu::Data, 2),
    ] {
        assert!(MenuState::at(menu, selection, Unlocks::default()).is_err());
    }
    let all = Unlocks {
        all_star: true,
        sound_test: true,
    };
    assert!(MenuState::at(Menu::RegularMatch, 2, all).is_ok());
    assert!(MenuState::at(Menu::Data, 2, all).is_ok());
}
