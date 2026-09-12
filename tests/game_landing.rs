//! End-to-end ordinary Landing interrupt window (`ftCo_Landing_IASA`) in an
//! explicitly synthetic native world. `LandingFallSpecial` (air dodge and
//! other special landings) stays a full lockout and is covered separately in
//! `tests/game_air_dodge.rs`; this file only checks that it does not share
//! `landing_allow_interrupt` with the ordinary Landing this module adds.
#[path = "support/dash.rs"]
mod dash_support;
#[path = "support/escape_air.rs"]
mod escape_air_support;
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
};

#[derive(serde::Deserialize)]
struct ShieldProfile {
    rules: skirmish::game::shield::Rules,
    attributes: skirmish::game::shield::Attributes,
}

/// Fighter 0 (facing +X, far from fighter 1 at x=20) short-hops and lands;
/// fighter 1 idles out of every attack's reach so hitlag never freezes the
/// lander's frame count. The fixture's `landing_frames` is 2, too short for
/// a three-frame-deep interrupt window; this widens it to 6 and opens the
/// window at `normal_landing_lag = 3.0`, giving the ordinary Landing three
/// interruptible frames (`cur_anim_frame` 3, 4 and 5) before it ends.
fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let mut profile: ShieldProfile =
        serde_json::from_str(include_str!("fixtures/game/shield.json")).unwrap();
    // The default fixture's powershield_input_window is 0 (fresh presses can
    // never qualify); a fresh L needs a real window to open GuardReflect.
    profile.rules.powershield_input_window = 3;
    data.rules.shield = Some(profile.rules);
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.spawns = [[-5.0, 0.0], [20.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
        fighter.locomotion =
            Some(serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap());
        fighter.jab.move_id = Some(10);
        fighter.movement.landing_frames = 6;
        fighter.movement.normal_landing_lag = Some(3.0);
    }
    let data = jab_support::profile(smash_support::profile(tilt_support::profile(
        grab_support::profile(data),
    )));
    dash_support::profile(data)
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

fn cstick(cstick: [f32; 2]) -> Controller {
    Controller {
        cstick,
        ..Default::default()
    }
}

fn step(game: &mut Match, controller: Controller) -> State {
    game.step([controller, Controller::default()])
        .unwrap()
        .clone()
}

/// X press then release for a short hop (the released frame's release is
/// what selects the short hop over a full hop): steps until Landing, whose
/// entry frame is observed with `action_frame == 1` (`enter` resets it to 0,
/// then this same step's unconditional per-frame increment applies).
fn enter_landing(game: &mut Match) -> State {
    step(game, stick(BUTTON_X, [0.0, 0.0]));
    for _ in 0..40 {
        let state = step(game, buttons(0));
        if state.fighters[0].action == Action::Landing {
            assert_eq!(state.fighters[0].action_frame, 1);
            return state;
        }
    }
    unreachable!("the short hop must land within forty steps");
}

/// `steps` further neutral frames from the landing entry frame. Post-step
/// `action_frame` is already incremented, so the dispatch that consults
/// `cur_anim_frame` on the *next* step sees `steps + 1`: zero extra steps
/// leaves the next dispatch at `cur_anim_frame == 1` (still locked, since
/// `normal_landing_lag == 3.0`); two leaves it at the first interruptible
/// frame (`3`); three leaves it at the second (`4`).
fn hold_landing(game: &mut Match, steps: u32) {
    for _ in 0..steps {
        let state = step(game, buttons(0));
        assert_eq!(state.fighters[0].action, Action::Landing);
    }
}

#[test]
fn locked_frames_before_the_lag_ignore_every_interrupt() {
    // A fresh press of each of these opens some part of the Wait chain once
    // the lag has passed (see the interruptible-frame tests below); before
    // it, `ftCo_Landing_IASA`'s first RETURN_IF keeps every one of them from
    // being consulted at all. `tilt_x_age` (from the walk deadzone) is
    // exercised by the dash-stick and C-stick-smash cases exactly as it is
    // once the chain does open.
    let cases: [(&str, Controller); 7] = [
        ("A", buttons(BUTTON_A)),
        ("L", buttons(BUTTON_L)),
        ("X", buttons(BUTTON_X)),
        ("Z", buttons(BUTTON_Z)),
        ("stick down", stick(0, [0.0, -1.0])),
        ("dash stick", stick(0, [1.0, 0.0])),
        ("C-stick smash", cstick([1.0, 0.0])),
    ];
    for (label, input) in cases {
        // cur_anim_frame == 1.
        let mut game = Match::new(data(), 42).unwrap();
        enter_landing(&mut game);
        let state = step(&mut game, input);
        assert_eq!(
            state.fighters[0].action,
            Action::Landing,
            "{label} at frame 1"
        );

        // cur_anim_frame == 2, with a genuinely fresh press (held input is
        // not fresh, which would defeat this case).
        let mut game = Match::new(data(), 42).unwrap();
        enter_landing(&mut game);
        hold_landing(&mut game, 1);
        let state = step(&mut game, input);
        assert_eq!(
            state.fighters[0].action,
            Action::Landing,
            "{label} at frame 2"
        );
    }
}

#[test]
fn first_interruptible_frame_opens_the_complete_wait_chain_and_the_crouch() {
    // cur_anim_frame == 3 == normal_landing_lag: the lag gate opens, and
    // `cur_anim_frame < frame_speed_mul + normal_landing_lag` (3 < 1 + 3)
    // also still opens the squat entry (ordinary Landing's `anim_speed`, and
    // so `frame_speed_mul`, is always 1.0). `ftCo_Landing_IASA` (`ftCo_
    // Landing.c:146-147`) calls `ftCo_SquatWait_CheckInput` here, not
    // `ftCo_Squat_CheckInput`, so this crouch entry lands in SquatWait
    // directly, skipping Squat's own crouch-down animation.
    let cases: [(&str, Controller, Action); 8] = [
        ("jab", buttons(BUTTON_A), Action::Jab),
        ("tilt", stick(BUTTON_A, [0.6, 0.0]), Action::AttackS3S),
        ("catch", buttons(BUTTON_Z), Action::Catch),
        ("shield", buttons(BUTTON_L), Action::GuardReflect),
        ("jump", buttons(BUTTON_X), Action::JumpSquat),
        ("dash", stick(0, [1.0, 0.0]), Action::Dash),
        ("smash", cstick([1.0, 0.0]), Action::AttackS4S),
        ("crouch", stick(0, [0.0, -1.0]), Action::SquatWait),
    ];
    for (label, input, expected) in cases {
        let mut game = Match::new(data(), 42).unwrap();
        enter_landing(&mut game);
        hold_landing(&mut game, 2);
        let state = step(&mut game, input);
        assert_eq!(state.fighters[0].action, expected, "{label}");
    }
}

#[test]
fn first_interruptible_frame_reaches_turn_and_walk() {
    // Turn: an opposite stick past turn_threshold but short of dash_threshold;
    // it only flips facing once it resolves, matching the ordinary Wait chain.
    let mut game = Match::new(data(), 42).unwrap();
    enter_landing(&mut game);
    hold_landing(&mut game, 2);
    let state = step(&mut game, stick(0, [-0.5, 0.0]));
    assert_eq!(state.fighters[0].action, Action::Turn);
    assert_eq!(state.fighters[0].facing, 1.0);

    // Walk: forward stick past walk_threshold but short of turn/dash.
    let mut game = Match::new(data(), 42).unwrap();
    enter_landing(&mut game);
    hold_landing(&mut game, 2);
    let state = step(&mut game, stick(0, [0.3, 0.0]));
    assert_eq!(state.fighters[0].action, Action::Walk);
}

#[test]
fn second_interruptible_frame_accepts_everything_except_the_crouch() {
    // cur_anim_frame == 4: still >= normal_landing_lag (3), so the Wait
    // chain stays open, but 4 is no longer < 1 + 3, so the squat entry does
    // not even get consulted; the chain still reaches Turn and Walk below it.
    let cases: [(&str, Controller, Action); 6] = [
        ("jab", buttons(BUTTON_A), Action::Jab),
        ("catch", buttons(BUTTON_Z), Action::Catch),
        ("shield", buttons(BUTTON_L), Action::GuardReflect),
        ("jump", buttons(BUTTON_X), Action::JumpSquat),
        ("dash", stick(0, [1.0, 0.0]), Action::Dash),
        ("smash", cstick([1.0, 0.0]), Action::AttackS4S),
    ];
    for (label, input, expected) in cases {
        let mut game = Match::new(data(), 42).unwrap();
        enter_landing(&mut game);
        hold_landing(&mut game, 3);
        let state = step(&mut game, input);
        assert_eq!(state.fighters[0].action, expected, "{label}");
    }

    let mut game = Match::new(data(), 42).unwrap();
    enter_landing(&mut game);
    hold_landing(&mut game, 3);
    let state = step(&mut game, stick(0, [0.0, -1.0]));
    assert_eq!(
        state.fighters[0].action,
        Action::Landing,
        "crouch stays locked"
    );
}

#[test]
fn the_animation_still_ends_in_wait_without_input() {
    let mut game = Match::new(data(), 42).unwrap();
    enter_landing(&mut game);
    let mut state = game.state().clone();
    let mut frames = 0;
    while state.fighters[0].action == Action::Landing {
        state = step(&mut game, buttons(0));
        frames += 1;
        assert!(frames < 20, "landing must end within twenty steps");
    }
    assert_eq!(state.fighters[0].action, Action::Wait);
}

/// `ftCommon_8007D6A4` (reached from `ftCo_Landing_Enter` via
/// `ftCommon_8007D7FC`, the same callback every ordinary landing call site
/// in `ft_081B.c` reaches through `ftCo_Landing_Enter_Basic`) sets `gr_vel`
/// from `self_vel.x` on landing but never assigns `self_vel.y`: the
/// vertical self-velocity this same frame's own fall physics computed
/// survives the landing, exactly as `tests/fixtures/slippi/parity/
/// fox-fd-3.slp` records (P2's `velocities.self_y` at its own landing
/// frame, -2.53, one more gravity step past the previous frame's -2.3, not
/// the zero a full stop would report -- `docs/validation.md`).
#[test]
fn ordinary_landing_keeps_this_frames_fall_velocity_unzeroed() {
    let resource = data();
    let gravity = resource.fighters[0].movement.gravity;
    let mut game = Match::new(resource, 42).unwrap();
    step(&mut game, stick(BUTTON_X, [0.0, 0.0]));
    let mut falling = 0.0_f32;
    for _ in 0..40 {
        let state = step(&mut game, buttons(0));
        if state.fighters[0].action == Action::Landing {
            assert_eq!(state.fighters[0].action_frame, 1);
            assert_ne!(state.fighters[0].velocity[1], 0.0);
            assert_eq!(state.fighters[0].velocity[1], falling - gravity);
            return;
        }
        if !state.fighters[0].grounded {
            falling = state.fighters[0].velocity[1];
        }
    }
    unreachable!("the short hop must land within forty steps");
}

#[test]
fn landing_allow_interrupt_is_true_only_for_the_ordinary_landing() {
    let landed = enter_landing(&mut Match::new(data(), 42).unwrap());
    assert!(landed.fighters[0].landing_allow_interrupt);

    // Reach LandingFallSpecial (air dodge exhausting every jump, then the
    // special landing) the same way tests/game_air_dodge.rs does, and
    // confirm it never picks up the ordinary Landing's flag: it keeps its
    // own separate `aerial.allow_interrupt = false` full lockout unchanged.
    let mut game = Match::new(escape_air_support::profile(data()), 42).unwrap();
    step(&mut game, stick(BUTTON_X, [0.0, 0.0]));
    step(&mut game, stick(BUTTON_X, [0.0, 0.0])); // held: full hop.
    step(&mut game, buttons(0));
    let entry = step(&mut game, buttons(BUTTON_L));
    assert_eq!(entry.fighters[0].action, Action::EscapeAir);
    let mut frames = 0;
    while game.state().fighters[0].action == Action::EscapeAir {
        step(&mut game, buttons(0));
        frames += 1;
        assert!(frames < 40);
    }
    assert_eq!(game.state().fighters[0].action, Action::FallSpecial);
    let mut frames = 0;
    while game.state().fighters[0].action != Action::LandingFallSpecial {
        step(&mut game, buttons(BUTTON_X | BUTTON_A | BUTTON_L));
        frames += 1;
        assert!(frames < 60);
    }
    let landed = game.state().clone();
    assert_eq!(landed.fighters[0].action, Action::LandingFallSpecial);
    assert!(!landed.fighters[0].landing_allow_interrupt);
    assert!(!landed.fighters[0].aerial.allow_interrupt);
    for callback in 0..5 {
        let state = step(
            &mut game,
            buttons(BUTTON_X | BUTTON_A | BUTTON_L | BUTTON_Z),
        );
        assert_eq!(
            state.fighters[0].action,
            Action::LandingFallSpecial,
            "callback {callback}"
        );
    }
}

#[test]
fn checkpoints_restore_landing_in_every_interrupt_phase() {
    for held in [0, 1, 2, 3] {
        let mut game = Match::new(data(), 42).unwrap();
        enter_landing(&mut game);
        hold_landing(&mut game, held);
        let checkpoint = game.checkpoint();
        let inputs: Vec<[Controller; 2]> = (0..16)
            .map(|frame| {
                [
                    match frame {
                        2 => stick(BUTTON_A, [0.6, 0.0]),
                        5 => buttons(BUTTON_Z),
                        8 => stick(0, [0.0, -1.0]),
                        11 => buttons(BUTTON_L),
                        _ => buttons(0),
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
        for (input, expected) in inputs.into_iter().zip(expected) {
            assert_eq!(
                serde_json::to_vec(game.step(input).unwrap()).unwrap(),
                expected,
                "held {held}"
            );
        }
    }
}

#[test]
fn invalid_landing_resources_are_rejected() {
    for lag in [f32::NAN, -1.0, 7.0] {
        let mut resource = data();
        resource.fighters[0].movement.normal_landing_lag = Some(lag);
        assert!(Match::new(resource, 0).is_err(), "lag {lag}");
    }
    // The inclusive boundary (equal to landing_frames) is accepted.
    let mut boundary = data();
    boundary.fighters[0].movement.normal_landing_lag = Some(6.0);
    assert!(Match::new(boundary, 0).is_ok());
}

#[test]
fn none_keeps_a_chainless_landing() {
    let mut resource = data();
    for fighter in &mut resource.fighters {
        fighter.movement.normal_landing_lag = None;
    }
    let mut game = Match::new(resource, 42).unwrap();
    let mut state = enter_landing(&mut game);
    let mut frames = 1;
    while state.fighters[0].action == Action::Landing {
        // A fresh A press well past where the lag would otherwise have
        // opened the chain still does nothing without normal_landing_lag.
        let input = if frames == 3 {
            buttons(BUTTON_A)
        } else {
            buttons(0)
        };
        state = step(&mut game, input);
        if frames == 3 {
            assert_eq!(state.fighters[0].action, Action::Landing);
        }
        frames += 1;
        assert!(frames < 20);
    }
    assert_eq!(state.fighters[0].action, Action::Wait);
}
