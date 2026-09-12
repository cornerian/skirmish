//! End-to-end floor-end collision modes (`docs/edges.md`, "Ground collision
//! modes") and the edge teeter (Ottotto/OttottoWait) in an explicitly
//! synthetic native world with invented escape, tilt, smash, dash and jab
//! profiles.
#[path = "support/dash.rs"]
mod dash_support;
#[path = "support/edge.rs"]
mod edge_support;
#[path = "support/escape.rs"]
mod escape_support;
#[path = "support/grab.rs"]
mod grab_support;
#[path = "support/jab.rs"]
mod jab_support;
#[path = "support/smash.rs"]
mod smash_support;
#[path = "support/tilt.rs"]
mod tilt_support;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_X, BUTTON_Z, Controller, Match, State, data::MatchData,
    edge::EdgeSide,
};

#[derive(serde::Deserialize)]
struct ShieldProfile {
    rules: skirmish::game::shield::Rules,
    attributes: skirmish::game::shield::Attributes,
}

/// Fighter 0 (facing +X) approaches the right floor end; fighter 1 spawns far
/// away so it never interferes (hitlag, nudge or reach).
fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let profile: ShieldProfile =
        serde_json::from_str(include_str!("fixtures/game/shield.json")).unwrap();
    data.rules.shield = Some(profile.rules);
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.rules.knockback_speed = 0.0;
    data.stage.floor.left = -30.0;
    data.stage.floor.right = 30.0;
    data.stage.blast = [-500.0, 500.0, -500.0, 500.0];
    data.stage.spawns = [[-2.0, 0.0], [-29.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
        fighter.locomotion =
            Some(serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap());
        fighter.jab.move_id = Some(10);
    }
    let data = smash_support::profile(tilt_support::profile(escape_support::profile(
        grab_support::profile(data),
    )));
    edge_support::profile(dash_support::profile(jab_support::profile(data)))
}

fn stick(buttons: u16, stick: [f32; 2]) -> Controller {
    Controller {
        buttons,
        stick,
        ..Default::default()
    }
}

fn buttons(buttons: u16) -> Controller {
    stick(buttons, [0.0, 0.0])
}

fn step(game: &mut Match, controller: Controller) -> State {
    game.step([controller, Controller::default()])
        .unwrap()
        .clone()
}

fn run_frames(game: &mut Match, controller: Controller, frames: u32) -> State {
    let mut state = step(game, controller);
    for _ in 1..frames {
        state = step(game, controller);
    }
    state
}

#[test]
fn smash_root_motion_reaching_the_right_end_clamps_and_stays_grounded() {
    // Attack collision is `ft_80084104` (mode 2, `mpColl_8004A45C_Floor`,
    // mpcoll.c:3584-3652): the straight forward smash's own TransN root
    // motion carries it past the edge, but the position stops there instead
    // of sliding through.
    let mut resource = data();
    resource.stage.floor.right = -1.0;
    let mut game = Match::new(resource, 42).unwrap();
    let entry = step(&mut game, stick(BUTTON_A, [1.0, 0.0]));
    assert_eq!(entry.fighters[0].action, Action::AttackS4S);
    let mut last = entry;
    for _ in 0..9 {
        last = step(&mut game, Controller::default());
        if last.fighters[0].edge_contact.is_some() {
            break;
        }
    }
    assert_eq!(last.fighters[0].edge_contact, Some(EdgeSide::Right));
    assert!(last.fighters[0].grounded);
    assert_eq!(last.fighters[0].ground_line, Some(0));
    assert_eq!(last.fighters[0].position, [-1.0, 0.0]);
    // The clamp repeats every remaining frame of the smash without sliding
    // through, then the smash still ends normally back in Wait.
    for _ in 0..9 {
        let state = step(&mut game, Controller::default());
        assert!(state.fighters[0].grounded);
        assert_eq!(state.fighters[0].position[0], -1.0);
        if state.fighters[0].action == Action::Wait {
            return;
        }
    }
    panic!("the smash should return to Wait");
}

#[test]
fn dash_attack_root_motion_reaching_the_right_end_clamps_and_stays_grounded() {
    // AttackDash's own collision callback is also mode 2 (`ft_80084104`,
    // "AttackDash" in docs/edges.md's table). Supply an explicit root-motion
    // dash attack (unlike `dash_support::profile`'s default) so it walks
    // into the edge deterministically.
    let mut resource = data();
    for fighter in &mut resource.fighters {
        let bones = fighter.bones.clone();
        fighter.dash_attack = Some(dash_support::attack(
            &bones,
            Some(vec![0.0, 0.0, 1.0, 1.5, 1.5, 1.0, 0.0, 0.0]),
        ));
    }
    resource.stage.floor.right = 5.0;
    let mut game = Match::new(resource, 42).unwrap();
    let dash = step(&mut game, stick(0, [1.0, 0.0]));
    assert_eq!(dash.fighters[0].action, Action::Dash);
    run_frames(&mut game, buttons(0), 3);
    let mut last = step(&mut game, stick(BUTTON_A, [1.0, 0.0]));
    assert_eq!(last.fighters[0].action, Action::AttackDash);
    for _ in 0..12 {
        last = step(&mut game, Controller::default());
        if last.fighters[0].edge_contact.is_some() {
            break;
        }
    }
    assert_eq!(last.fighters[0].edge_contact, Some(EdgeSide::Right));
    assert!(last.fighters[0].grounded);
    assert_eq!(last.fighters[0].position[0], 5.0);
}

#[test]
fn rolling_into_the_right_end_clamps_instead_of_falling() {
    // Escape's collision callback is also `ft_80084104`; mirrors
    // `tests/game_escape.rs`'s own corrected expectation.
    let mut resource = data();
    resource.stage.floor.right = 0.0;
    let mut game = Match::new(resource, 42).unwrap();
    assert_eq!(
        step(&mut game, buttons(BUTTON_L)).fighters[0].action,
        Action::GuardOn
    );
    let entry = step(&mut game, stick(BUTTON_L, [1.0, 0.0]));
    assert_eq!(entry.fighters[0].action, Action::EscapeF);
    let mut last = entry;
    for _ in 0..8 {
        last = step(&mut game, Controller::default());
        if last.fighters[0].edge_contact.is_some() {
            break;
        }
    }
    assert_eq!(last.fighters[0].edge_contact, Some(EdgeSide::Right));
    assert!(last.fighters[0].grounded);
    assert_eq!(last.fighters[0].position[0], 0.0);
}

#[test]
fn walking_toward_the_edge_within_the_teeter_limit_enters_ottotto_then_ottotto_wait() {
    let mut resource = data();
    resource.stage.floor.right = -0.5;
    let mut game = Match::new(resource, 42).unwrap();
    let mut state = step(&mut game, stick(0, [0.5, 0.0]));
    let mut reached = false;
    for _ in 0..40 {
        if state.fighters[0].action == Action::Ottotto {
            reached = true;
            break;
        }
        state = step(&mut game, stick(0, [0.5, 0.0]));
    }
    assert!(reached, "the fighter should walk into Ottotto");
    assert_eq!(state.fighters[0].velocity, [0.0, 0.0]);
    assert_eq!(state.fighters[0].ground_velocity, 0.0);
    assert!(state.fighters[0].grounded);
    assert_eq!(state.fighters[0].edge_contact, Some(EdgeSide::Right));
    let entry_position = state.fighters[0].position;
    // Holding the same admissible stick keeps it in Ottotto until the
    // supplied start poses are spent, then OttottoWait, never moving.
    let mut reached_wait = false;
    for _ in 0..(edge_support::START_FRAMES + 10) {
        state = step(&mut game, stick(0, [0.5, 0.0]));
        assert!(state.fighters[0].grounded);
        assert_eq!(state.fighters[0].position[0], entry_position[0]);
        assert!(matches!(
            state.fighters[0].action,
            Action::Ottotto | Action::OttottoWait
        ));
        if state.fighters[0].action == Action::OttottoWait {
            reached_wait = true;
            break;
        }
    }
    assert!(reached_wait);
    // OttottoWait holds indefinitely with no physics of its own.
    for _ in 0..10 {
        state = step(&mut game, stick(0, [0.5, 0.0]));
        assert_eq!(state.fighters[0].action, Action::OttottoWait);
        assert_eq!(state.fighters[0].position[0], entry_position[0]);
    }
}

#[test]
fn outward_stick_at_the_teeter_limit_falls_off_the_edge() {
    // The 0.75 literal is exclusive: exactly at it, mpColl_8004A678_Floor's
    // gate fails and the fighter falls, exactly like mode 0.
    let mut resource = data();
    resource.stage.floor.right = -0.5;
    let mut game = Match::new(resource, 42).unwrap();
    let mut state = step(&mut game, stick(0, [0.75, 0.0]));
    for _ in 0..40 {
        if !state.fighters[0].grounded {
            break;
        }
        state = step(&mut game, stick(0, [0.75, 0.0]));
    }
    assert_eq!(state.fighters[0].action, Action::Fall);
    assert!(!state.fighters[0].grounded);
    assert!(state.fighters[0].position[0] > -0.5);
}

#[test]
fn facing_away_from_the_edge_falls_off_it() {
    // A fighter standing in Wait (facing +1, away from the left end) pushed
    // there by an overlap nudge -- not by walking, which would flip its own
    // facing -- fails the teeter's facing gate and falls, exactly like mode 0.
    let mut resource = data();
    resource.stage.floor.left = -1.0;
    resource.stage.spawns = [[0.5, 0.0], [1.3, 0.0]];
    resource.rules.nudge = Some(skirmish::fighter::nudge::Rules {
        horizontal_step: 0.3,
        depth_step: 0.0,
        depth_limit: 0.0,
        follower_depth_step: 0.0,
        follower_depth_limit: 0.0,
    });
    for fighter in &mut resource.fighters {
        fighter.nudge = Some(skirmish::game::nudge::Attributes {
            center_offset: 0.0,
            half_width: 2.0,
            nudge_disabled: false,
            overlap_disabled: false,
        });
    }
    let mut game = Match::new(resource, 42).unwrap();
    assert_eq!(game.state().fighters[0].facing, 1.0);
    let mut state = step(&mut game, Controller::default());
    for _ in 0..40 {
        if !state.fighters[0].grounded {
            break;
        }
        state = step(&mut game, Controller::default());
    }
    assert_eq!(state.fighters[0].action, Action::Fall);
    assert!(!state.fighters[0].grounded);
    assert!(state.fighters[0].position[0] < -1.0);
}

#[test]
fn dash_and_run_past_the_end_fall_off_it() {
    // Dash/Run are mode 0 (`ft_800844EC`); unaffected by `rules.edge`.
    for (label, floor_right) in [("dash", -1.5), ("run", 6.0)] {
        let mut resource = data();
        let air_drift_max = resource.fighters[0].movement.air_drift_max;
        resource.stage.floor.right = floor_right;
        let mut game = Match::new(resource, 42).unwrap();
        let mut state = step(&mut game, stick(0, [1.0, 0.0]));
        assert_eq!(state.fighters[0].action, Action::Dash, "{label}");
        for _ in 0..40 {
            if !state.fighters[0].grounded {
                break;
            }
            state = step(&mut game, stick(0, [1.0, 0.0]));
        }
        assert_eq!(state.fighters[0].action, Action::Fall, "{label}");
        assert!(!state.fighters[0].grounded, "{label}");
        // ftCo_Fall_Enter (ftCo_Fall.c:63) unconditionally calls
        // ftCommon_ClampAirDrift right after the motion-state change: the
        // ground speed carried into this exact frame (well above
        // air_drift_max here) must already be clamped down to it, not left
        // at the dash/run speed.
        let velocity_x = state.fighters[0].velocity[0];
        assert!(
            (velocity_x - air_drift_max).abs() < 1e-4,
            "{label}: velocity.x {velocity_x} not clamped to air_drift_max {air_drift_max}"
        );
    }
}

/// Walk the fighter into Ottotto against `floor.right`, returning the state
/// once it has entered (still Ottotto, not yet OttottoWait).
fn enter_ottotto(game: &mut Match) -> State {
    let mut state = step(game, stick(0, [0.5, 0.0]));
    for _ in 0..40 {
        if state.fighters[0].action == Action::Ottotto {
            return state;
        }
        state = step(game, stick(0, [0.5, 0.0]));
    }
    panic!("the fighter should have entered Ottotto");
}

/// Keep holding an admissible stick until Ottotto's supplied poses run out
/// and it reaches OttottoWait.
fn advance_to_ottotto_wait(game: &mut Match, input: Controller) -> State {
    let mut state;
    for _ in 0..(edge_support::START_FRAMES + 10) {
        state = step(game, input);
        if state.fighters[0].action == Action::OttottoWait {
            return state;
        }
        assert_eq!(state.fighters[0].action, Action::Ottotto);
    }
    panic!("Ottotto should have reached OttottoWait");
}

type Scenario = (&'static str, Controller, fn(Action) -> bool);

#[test]
fn from_ottotto_the_exposed_wait_chain_still_works() {
    // catch, smash, shield and dash all reach Ottotto through
    // `tilt::interrupt_chain` returning `Chain::Wait`, exactly as Wait itself
    // exposes them.
    let scenarios: [Scenario; 4] = [
        ("catch", stick(BUTTON_A | BUTTON_Z, [0.0, 0.0]), |a| {
            a == Action::Catch
        }),
        ("smash", stick(BUTTON_A, [-1.0, 0.0]), |a| {
            a == Action::AttackS4S
        }),
        ("shield", buttons(BUTTON_L), |a| a == Action::GuardOn),
        ("jump", buttons(BUTTON_X), |a| a == Action::JumpSquat),
    ];
    for (label, input, matches_expected) in scenarios {
        let mut resource = data();
        resource.stage.floor.right = -0.5;
        let mut game = Match::new(resource, 42).unwrap();
        enter_ottotto(&mut game);
        let state = step(&mut game, input);
        assert!(
            matches_expected(state.fighters[0].action),
            "{label}: got {:?}",
            state.fighters[0].action
        );
    }
    // Dash also reaches through the same chain. A fresh full-magnitude stick
    // in the same direction as the current facing (+1, toward the edge being
    // teetered) restarts Dash directly (`try_dash`'s same-direction branch),
    // exactly like from Wait -- and, since teetering only ever faces the
    // near edge, that Dash immediately carries it off the cliff on the very
    // same frame (mode 0, unaffected by `rules.edge`), correctly ungrounding
    // it instead of leaving it frozen in Ottotto.
    let mut resource = data();
    resource.stage.floor.right = -0.5;
    let mut game = Match::new(resource, 42).unwrap();
    enter_ottotto(&mut game);
    let state = step(&mut game, stick(0, [1.0, 0.0]));
    assert!(!state.fighters[0].grounded);
    assert_ne!(state.fighters[0].action, Action::Ottotto);
    assert_ne!(state.fighters[0].action, Action::OttottoWait);
}

#[test]
fn walking_toward_the_edge_below_the_teeter_walk_threshold_stays_in_ottotto() {
    // Ordinary walk_threshold (0.2) admits this stick, but the teeter
    // override (0.4, `ftCo_Walk_CheckInput_Ottotto`) does not.
    let mut resource = data();
    resource.stage.floor.right = -0.5;
    let mut game = Match::new(resource, 42).unwrap();
    enter_ottotto(&mut game);
    advance_to_ottotto_wait(&mut game, stick(0, [0.5, 0.0]));
    let position = game.state().fighters[0].position;
    for _ in 0..5 {
        let state = step(&mut game, stick(0, [0.3, 0.0]));
        assert_eq!(state.fighters[0].action, Action::OttottoWait);
        assert_eq!(state.fighters[0].position[0], position[0]);
    }
}

#[test]
fn turn_is_reachable_from_ottotto() {
    // `ftCo_Turn_CheckInput` is one of the checks `ftCo_Ottotto_IASA` shares
    // with Wait's own chain (docs/edges.md, "Ottotto"). The exit-to-Wait
    // distance check itself (`ftCo_Ottotto_Coll`/`ftCo_OttottoWait_Coll`:
    // `ABS(cur_pos.x - floor_end.x) > x478 + x47C`) is covered directly by
    // `fighter::edge::exit_distance_exceeded`'s unit tests, and implicitly by
    // every other Ottotto/OttottoWait test here, which hold position exactly
    // (distance 0) for many frames without a spurious exit; engineering an
    // external displacement of a grounded, zero-velocity Ottotto fighter
    // (nudge only resolves overlap up to the fighters' combined half-width,
    // not an arbitrary distance, and no wind/moving-platform primitive
    // exists here) without disrupting the approach that reaches Ottotto in
    // the first place is not covered by an integration test.
    let mut resource = data();
    resource.stage.floor.right = -0.5;
    let mut game = Match::new(resource, 42).unwrap();
    enter_ottotto(&mut game);
    let state = step(&mut game, stick(0, [-1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::Turn);
}

#[test]
fn checkpoints_restore_ottotto_and_ottotto_wait() {
    for target_wait in [false, true] {
        let mut resource = data();
        resource.stage.floor.right = -0.5;
        let mut game = Match::new(resource, 42).unwrap();
        enter_ottotto(&mut game);
        if target_wait {
            advance_to_ottotto_wait(&mut game, stick(0, [0.5, 0.0]));
            assert_eq!(game.state().fighters[0].action, Action::OttottoWait);
        }
        let expected_action = game.state().fighters[0].action;
        let checkpoint = game.checkpoint();
        let inputs: Vec<_> = (0..10)
            .map(|frame| {
                [
                    if frame % 4 == 0 {
                        buttons(BUTTON_X)
                    } else {
                        Controller::default()
                    },
                    Controller::default(),
                ]
            })
            .collect();
        let expected: Vec<_> = inputs
            .iter()
            .map(|&input| serde_json::to_vec(game.step(input).unwrap()).unwrap())
            .collect();
        game.restore_checkpoint(&checkpoint).unwrap();
        assert_eq!(game.state().fighters[0].action, expected_action);
        for (input, expected) in inputs.into_iter().zip(expected) {
            assert_eq!(
                serde_json::to_vec(game.step(input).unwrap()).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn invalid_edge_resources_are_rejected() {
    fn rejected(edit: impl FnOnce(&mut MatchData)) {
        let mut resource = data();
        edit(&mut resource);
        assert!(Match::new(resource, 0).is_err());
    }
    assert!(Match::new(data(), 0).is_ok());
    for limit in [0.0, -0.1, 1.5, f32::NAN] {
        rejected(|d| d.rules.edge.as_mut().unwrap().teeter_stick_limit = limit);
    }
    for threshold in [-0.1, 1.5, f32::NAN] {
        rejected(|d| d.rules.edge.as_mut().unwrap().teeter_walk_threshold = threshold);
    }
    for value in [-0.1, f32::NAN, f32::INFINITY] {
        rejected(|d| d.rules.edge.as_mut().unwrap().teeter_exit_distance = value);
        rejected(|d| d.rules.edge.as_mut().unwrap().teeter_exit_tolerance = value);
    }
    rejected(|d| d.fighters[0].teeter.as_mut().unwrap().start.clear());
    rejected(|d| d.fighters[0].teeter = None);
    rejected(|d| d.rules.edge = None);
}

#[test]
fn rules_edge_none_keeps_wait_falling_but_a_smash_still_clamps() {
    // With `rules.edge` absent, mode 2 clamping still applies (it needs no
    // data), but mode 1 degrades to mode 0 -- Wait/Walk simply fall.
    let mut resource = data();
    resource.rules.edge = None;
    for fighter in &mut resource.fighters {
        fighter.teeter = None;
    }
    resource.stage.floor.right = -0.5;
    let mut game = Match::new(resource.clone(), 42).unwrap();
    let mut state = step(&mut game, stick(0, [0.5, 0.0]));
    for _ in 0..40 {
        if !state.fighters[0].grounded {
            break;
        }
        state = step(&mut game, stick(0, [0.5, 0.0]));
    }
    assert_eq!(state.fighters[0].action, Action::Fall);
    assert!(!state.fighters[0].grounded);

    resource.stage.floor.right = -1.0;
    let mut game = Match::new(resource, 42).unwrap();
    let entry = step(&mut game, stick(BUTTON_A, [1.0, 0.0]));
    assert_eq!(entry.fighters[0].action, Action::AttackS4S);
    let mut last = entry;
    for _ in 0..9 {
        last = step(&mut game, Controller::default());
        if last.fighters[0].edge_contact.is_some() {
            break;
        }
    }
    assert_eq!(last.fighters[0].edge_contact, Some(EdgeSide::Right));
    assert!(last.fighters[0].grounded);
}

#[test]
fn jump_squat_overwrites_walk_only_after_it_wins_not_before() {
    // ftCo_Wait_IASA (and every other Wait-chain IASA) is a RETURN_IF chain:
    // `RETURN_IF(ftCo_Jump_CheckInput(gobj))` returns immediately once a
    // jump-squat entry fires, before `ftCo_Dash_CheckInput`/`ftCo_800D5FB0`/
    // `ftCo_Turn_CheckInput`/the walk check ever run. Regression test for a
    // bug where `locomotion::update_actions` continued into the shared
    // Wait/Walk arm on the very same frame (using its `interruptible_tilt`,
    // computed before the jump-squat entry, so it did not know the action
    // had just changed) and immediately overwrote the fresh JumpSquat with
    // Walk whenever the jump press came with an admissible walk stick.
    // An interruptible smash pose.
    let mut resource = data();
    resource.stage.floor.left = -30.0;
    resource.stage.floor.right = 30.0;
    let mut game = Match::new(resource, 42).unwrap();
    let entry = step(&mut game, stick(BUTTON_A, [1.0, 0.0]));
    assert_eq!(entry.fighters[0].action, Action::AttackS4S);
    // smash_support's straight forward smash opens its interrupt at frame 7.
    for _ in 0..6 {
        step(&mut game, Controller::default());
    }
    let state = step(&mut game, stick(BUTTON_X, [0.5, 0.0]));
    assert_eq!(state.fighters[0].action, Action::JumpSquat);

    // An interruptible jab pose (allow_interrupt from frame 3).
    let mut resource = data();
    resource.stage.floor.left = -30.0;
    resource.stage.floor.right = 30.0;
    let mut game = Match::new(resource, 42).unwrap();
    let entry = step(&mut game, buttons(BUTTON_A));
    assert_eq!(entry.fighters[0].action, Action::Jab);
    for _ in 0..2 {
        step(&mut game, Controller::default());
    }
    let state = step(&mut game, stick(BUTTON_X, [0.5, 0.0]));
    assert_eq!(state.fighters[0].action, Action::JumpSquat);

    // From Ottotto itself.
    let mut resource = data();
    resource.stage.floor.right = -0.5;
    let mut game = Match::new(resource, 42).unwrap();
    enter_ottotto(&mut game);
    let state = step(&mut game, stick(BUTTON_X, [0.5, 0.0]));
    assert_eq!(state.fighters[0].action, Action::JumpSquat);
}
