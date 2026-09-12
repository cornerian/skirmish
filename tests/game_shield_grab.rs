//! End-to-end grabs dispatched from a raised shield (ftCo_Catch_CheckInput and
//! the ftCo_800D8B9C dash-grab buffer) in an explicitly synthetic native world.
#[path = "support/escape.rs"]
mod escape_support;
#[path = "support/grab.rs"]
mod grab_support;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_X, BUTTON_Z, Controller, Event, Match, State,
    data::MatchData, grab::ShieldGrabRules, shield,
};

#[derive(serde::Deserialize)]
struct ShieldProfile {
    rules: shield::Rules,
    attributes: shield::Attributes,
}

const BUFFER: ShieldGrabRules = ShieldGrabRules {
    dash_buffer_frames: 3.0,
    dash_buffer_frame_limit: 4.0,
};

/// Fighter 0 (facing +X at -1) shields and grabs; fighter 1 stands at +1.
fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let profile: ShieldProfile =
        serde_json::from_str(include_str!("fixtures/game/shield.json")).unwrap();
    data.rules.shield = Some(profile.rules);
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.spawns = [[-1.0, 0.0], [1.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
        fighter.locomotion =
            Some(serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap());
    }
    let mut data = grab_support::profile(escape_support::profile(data));
    data.rules.grab.as_mut().unwrap().shield_grab = Some(BUFFER);
    data
}

fn long_raise() -> MatchData {
    let mut data = data();
    for fighter in &mut data.fighters {
        fighter.shield.as_mut().unwrap().raise_frames = 10.0;
    }
    data
}

fn buttons(buttons: u16) -> Controller {
    Controller {
        buttons,
        ..Default::default()
    }
}

fn stick(buttons: u16, stick: [f32; 2]) -> Controller {
    Controller {
        buttons,
        stick,
        ..Default::default()
    }
}

fn step(game: &mut Match, grabber: Controller) -> State {
    game.step([grabber, Controller::default()]).unwrap().clone()
}

fn grabbed(state: &State) -> bool {
    state.events.contains(&Event::Grabbed {
        holder: 0,
        victim: 1,
    })
}

/// Dash for `frames` frames, then raise the shield while still dashing or
/// running. Returns the shield-entry state.
fn dash_then_shield(game: &mut Match, frames: u32) -> State {
    for _ in 0..frames {
        let state = step(game, stick(0, [1.0, 0.0]));
        assert!(matches!(
            state.fighters[0].action,
            Action::Dash | Action::Run
        ));
    }
    step(game, stick(BUTTON_L, [1.0, 0.0]))
}

#[test]
fn a_or_z_while_shielding_grabs_from_every_raised_guard_state() {
    let mut on = Match::new(data(), 42).unwrap();
    assert_eq!(
        step(&mut on, buttons(BUTTON_L)).fighters[0].action,
        Action::GuardOn
    );
    let caught = step(&mut on, buttons(BUTTON_L | BUTTON_A));
    assert_eq!(caught.fighters[0].action, Action::CatchPull);
    assert!(grabbed(&caught));
    assert_eq!(caught.fighters[0].shield.dash_grab_buffer, 0.0);

    let mut guard = Match::new(data(), 42).unwrap();
    for _ in 0..3 {
        step(&mut guard, buttons(BUTTON_L));
    }
    assert_eq!(guard.state().fighters[0].action, Action::Guard);
    let caught = step(&mut guard, buttons(BUTTON_L | BUTTON_A));
    assert!(grabbed(&caught));

    let mut reflect_data = data();
    let rules = reflect_data.rules.shield.as_mut().unwrap();
    rules.powershield_input_window = 3;
    rules.powershield_reflect_frames = 3.0;
    rules.powershield_frames = 2.0;
    let mut reflect = Match::new(reflect_data, 42).unwrap();
    assert_eq!(
        step(&mut reflect, buttons(BUTTON_L)).fighters[0].action,
        Action::GuardReflect
    );
    let caught = step(&mut reflect, buttons(BUTTON_L | BUTTON_A));
    assert!(grabbed(&caught));
    assert!(!caught.fighters[0].shield.reflecting);
    assert!(!caught.fighters[0].shield.powershield_just_started);
    assert!(caught.fighters[0].shield.powershield);

    // Physical Z supplies both the logical shoulder and the fresh A press,
    // even after the trigger itself was released inside the minimum hold.
    let mut z = Match::new(data(), 42).unwrap();
    step(&mut z, buttons(BUTTON_L));
    let released = step(&mut z, buttons(0));
    assert_eq!(released.fighters[0].action, Action::GuardOn);
    assert!(released.fighters[0].shield.release_latched);
    assert!(grabbed(&step(&mut z, buttons(BUTTON_Z))));

    // A without a held shoulder is not a shield grab.
    let mut bare = Match::new(data(), 42).unwrap();
    step(&mut bare, buttons(BUTTON_L));
    step(&mut bare, buttons(0));
    let idle = step(&mut bare, buttons(BUTTON_A));
    assert!(matches!(
        idle.fighters[0].action,
        Action::GuardOn | Action::Guard
    ));
    assert!(!grabbed(&idle));
}

#[test]
fn shield_grab_follows_the_escapes_and_precedes_the_jump_dispatchers() {
    for (input, expected) in [
        (stick(BUTTON_L | BUTTON_A, [0.0, -1.0]), Action::EscapeN),
        (stick(BUTTON_L | BUTTON_A, [1.0, 0.0]), Action::EscapeF),
        (buttons(BUTTON_L | BUTTON_A | BUTTON_X), Action::CatchPull),
        (
            Controller {
                buttons: BUTTON_L | BUTTON_A,
                cstick: [0.0, 1.0],
                ..Default::default()
            },
            Action::CatchPull,
        ),
        (buttons(BUTTON_L | BUTTON_X), Action::JumpSquat),
    ] {
        let mut game = Match::new(data(), 42).unwrap();
        step(&mut game, buttons(BUTTON_L));
        assert_eq!(step(&mut game, input).fighters[0].action, expected);
    }
}

#[test]
fn run_and_late_dash_shields_arm_the_dash_grab_buffer_for_guard_on_frames() {
    // Dash frames at or before the x4C limit raise a shield without a buffer.
    let mut early = Match::new(long_raise(), 42).unwrap();
    let entered = dash_then_shield(&mut early, 3);
    assert_eq!(entered.fighters[0].action, Action::GuardOn);
    assert_eq!(entered.fighters[0].shield.dash_grab_buffer, 0.0);
    assert_eq!(
        step(&mut early, buttons(BUTTON_L | BUTTON_A)).fighters[0].action,
        Action::Catch
    );

    // Later dash frames arm x68 frames that count down on GuardOn callbacks.
    let mut late = Match::new(long_raise(), 42).unwrap();
    let entered = dash_then_shield(&mut late, 4);
    assert_eq!(entered.fighters[0].action, Action::GuardOn);
    assert_eq!(entered.fighters[0].shield.dash_grab_buffer, 3.0);
    assert_eq!(
        step(&mut late, buttons(BUTTON_L | BUTTON_A)).fighters[0].action,
        Action::CatchDash
    );

    let mut expiring = Match::new(long_raise(), 42).unwrap();
    dash_then_shield(&mut expiring, 4);
    for remaining in [2.0, 1.0, 0.0] {
        let state = step(&mut expiring, buttons(BUTTON_L));
        assert_eq!(state.fighters[0].action, Action::GuardOn);
        assert_eq!(state.fighters[0].shield.dash_grab_buffer, remaining);
    }
    assert_eq!(
        step(&mut expiring, buttons(BUTTON_L | BUTTON_A)).fighters[0].action,
        Action::Catch
    );

    let mut last_frame = Match::new(long_raise(), 42).unwrap();
    dash_then_shield(&mut last_frame, 4);
    step(&mut last_frame, buttons(BUTTON_L));
    step(&mut last_frame, buttons(BUTTON_L));
    let state = step(&mut last_frame, buttons(BUTTON_L | BUTTON_A));
    assert_eq!(state.fighters[0].action, Action::CatchDash);
    assert_eq!(state.fighters[0].shield.dash_grab_buffer, 1.0);

    // Running always arms the buffer, including through a powershield entry.
    let mut run = Match::new(long_raise(), 42).unwrap();
    let entered = dash_then_shield(&mut run, 9);
    assert_eq!(entered.fighters[0].shield.dash_grab_buffer, 3.0);
    assert_eq!(
        step(&mut run, buttons(BUTTON_L | BUTTON_A)).fighters[0].action,
        Action::CatchDash
    );

    let mut reflect_data = long_raise();
    let rules = reflect_data.rules.shield.as_mut().unwrap();
    rules.powershield_input_window = 3;
    rules.powershield_reflect_frames = 3.0;
    rules.powershield_frames = 2.0;
    let mut reflect = Match::new(reflect_data, 42).unwrap();
    let entered = dash_then_shield(&mut reflect, 9);
    assert_eq!(entered.fighters[0].action, Action::GuardReflect);
    assert_eq!(entered.fighters[0].shield.dash_grab_buffer, 3.0);
    assert_eq!(
        step(&mut reflect, buttons(BUTTON_L | BUTTON_A)).fighters[0].action,
        Action::CatchDash
    );

    // Guard itself never consults or drains the buffer.
    let mut guard = Match::new(data(), 42).unwrap();
    dash_then_shield(&mut guard, 4);
    step(&mut guard, buttons(BUTTON_L));
    let state = step(&mut guard, buttons(BUTTON_L | BUTTON_A));
    assert_eq!(state.fighters[0].action, Action::Catch);
    assert_eq!(state.fighters[0].shield.dash_grab_buffer, 2.0);
}

#[test]
fn a_fresh_shield_from_wait_clears_a_stale_buffer() {
    let mut game = Match::new(long_raise(), 42).unwrap();
    dash_then_shield(&mut game, 4);
    assert_eq!(game.state().fighters[0].shield.dash_grab_buffer, 3.0);
    let mut frames = 0;
    while game.state().fighters[0].action != Action::Wait {
        step(&mut game, buttons(0));
        frames += 1;
        assert!(frames < 40);
    }
    assert_eq!(
        step(&mut game, buttons(BUTTON_L)).fighters[0].action,
        Action::GuardOn
    );
    let state = step(&mut game, buttons(BUTTON_L | BUTTON_A));
    assert_eq!(state.fighters[0].action, Action::Catch);
    assert_eq!(state.fighters[0].shield.dash_grab_buffer, 0.0);
}

#[test]
fn shield_stun_and_guard_off_do_not_grab() {
    let mut stunned = Match::new(data(), 42).unwrap();
    stunned
        .step([buttons(BUTTON_L), buttons(BUTTON_A)])
        .unwrap();
    let contact = stunned
        .step([buttons(BUTTON_L), buttons(0)])
        .unwrap()
        .clone();
    assert_eq!(contact.fighters[0].action, Action::GuardSetOff);
    while stunned.state().fighters[0].hitlag > 0.0 {
        step(&mut stunned, buttons(BUTTON_L));
    }
    let blocked = step(&mut stunned, buttons(BUTTON_L | BUTTON_A));
    assert_eq!(blocked.fighters[0].action, Action::GuardSetOff);
    assert!(!grabbed(&blocked));

    let mut off = Match::new(data(), 42).unwrap();
    step(&mut off, buttons(BUTTON_L));
    step(&mut off, buttons(0));
    step(&mut off, buttons(BUTTON_L));
    assert_eq!(
        step(&mut off, buttons(BUTTON_L)).fighters[0].action,
        Action::GuardOff
    );
    let idle = step(&mut off, buttons(BUTTON_L | BUTTON_A));
    assert_eq!(idle.fighters[0].action, Action::GuardOff);
    assert!(!grabbed(&idle));
}

#[test]
fn checkpoints_restore_the_dash_grab_buffer_and_its_branch() {
    let mut game = Match::new(long_raise(), 42).unwrap();
    dash_then_shield(&mut game, 4);
    let checkpoint = game.checkpoint();
    let inputs = [
        buttons(BUTTON_L),
        buttons(BUTTON_L | BUTTON_A),
        buttons(0),
        buttons(0),
        buttons(BUTTON_Z),
        buttons(0),
    ];
    let expected: Vec<_> = inputs
        .iter()
        .map(|&input| {
            let state = step(&mut game, input);
            (
                state.fighters[0].action,
                serde_json::to_vec(&state).unwrap(),
            )
        })
        .collect();
    // The dash carried the grabber past its target, so the buffered
    // CatchDash whiffs and the branch simply returns to Wait.
    assert_eq!(expected[1].0, Action::CatchDash);
    assert_eq!(game.state().fighters[0].action, Action::Wait);
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(game.state().fighters[0].shield.dash_grab_buffer, 3.0);
    for (input, (_, expected)) in inputs.into_iter().zip(expected) {
        assert_eq!(
            serde_json::to_vec(&step(&mut game, input)).unwrap(),
            expected
        );
    }
    game.restore_checkpoint(&checkpoint).unwrap();
    for _ in 0..3 {
        step(&mut game, buttons(BUTTON_L));
    }
    assert_eq!(
        step(&mut game, buttons(BUTTON_L | BUTTON_A)).fighters[0].action,
        Action::Catch
    );
}

#[test]
fn shield_grabs_require_explicit_rules_and_reject_invalid_ones() {
    let mut without = data();
    without.rules.grab.as_mut().unwrap().shield_grab = None;
    let mut game = Match::new(without, 42).unwrap();
    step(&mut game, buttons(BUTTON_L));
    let idle = step(&mut game, buttons(BUTTON_L | BUTTON_A));
    assert_eq!(idle.fighters[0].action, Action::GuardOn);
    assert!(!grabbed(&idle));
    let idle = step(&mut game, buttons(BUTTON_L | BUTTON_Z));
    assert_eq!(idle.fighters[0].action, Action::Guard);
    assert!(!grabbed(&idle));

    for frames in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let mut resource = data();
        resource.rules.grab.as_mut().unwrap().shield_grab = Some(ShieldGrabRules {
            dash_buffer_frames: frames,
            ..BUFFER
        });
        assert!(Match::new(resource, 0).is_err(), "frames {frames}");
    }
    for limit in [-1.0, f32::NAN, f32::INFINITY, 2_000_000.0] {
        let mut resource = data();
        resource.rules.grab.as_mut().unwrap().shield_grab = Some(ShieldGrabRules {
            dash_buffer_frame_limit: limit,
            ..BUFFER
        });
        assert!(Match::new(resource, 0).is_err(), "limit {limit}");
    }
}
