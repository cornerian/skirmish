//! End-to-end backward ground/aerial jumps and the aerial-jump Fall variant
//! in an explicitly synthetic native world.
#[path = "support/aerial.rs"]
mod aerial_support;
#[path = "support/escape_air.rs"]
mod escape_air_support;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_X, Controller, Match, State, data::MatchData,
};

/// Fighter 0 (facing +X) at the platform floor; fighter 1 idles out of every
/// attack's reach. `jump_backward_threshold` (`ftCommonData.x78`) is an
/// invented fixture value of 0.3, matching `tests/oracle/jump.c`'s
/// differential coverage; `max_jumps` is 2 with no `multi_jump`, so the
/// second jump is an ordinary double jump. The aerials and air-dodge
/// resources reuse `tests/game_air_dodge.rs`'s own combined fixture
/// composition, moved back to a grounded spawn so the ground jump launches
/// from the floor instead of a dropped mid-air start.
fn data() -> MatchData {
    let mut data = escape_air_support::profile(aerial_support::data());
    data.stage.spawns = [[0.0, 0.0], [20.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.locomotion.as_mut().unwrap().jump_backward_threshold = Some(0.3);
    }
    data
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

/// Two-frame JumpSquat (X held both frames, for a full hop) then the launch
/// frame with the given stick: the frame `ftCo_Jump_Enter`'s own direction
/// test (`ftCo_Jump.c:157-161`) consults.
fn full_hop(game: &mut Match, stick_x: f32) -> State {
    step(game, stick(BUTTON_X, [0.0, 0.0]));
    step(game, stick(BUTTON_X, [0.0, 0.0]));
    let launched = step(game, stick(0, [stick_x, 0.0]));
    assert_eq!(launched.fighters[0].action, Action::Jump);
    launched
}

/// X pressed then released before the launch frame, for a short hop.
fn short_hop(game: &mut Match, stick_x: f32) -> State {
    step(game, stick(BUTTON_X, [0.0, 0.0]));
    step(game, buttons(0));
    let launched = step(game, stick(0, [stick_x, 0.0]));
    assert_eq!(launched.fighters[0].action, Action::Jump);
    launched
}

/// A fresh X press with the given stick, immediately entering JumpAerial:
/// `ftCo_JumpAerial_Enter_Basic`'s own direction test (`ftCo_JumpAerial.c:
/// 169-171`) consults this same frame.
fn double_jump(game: &mut Match, stick_x: f32) -> State {
    let launched = step(game, stick(BUTTON_X, [stick_x, 0.0]));
    assert_eq!(launched.fighters[0].action, Action::JumpAerial);
    launched
}

/// Steps neutral frames until JumpAerial's own animation end enters Fall.
fn until_fall(game: &mut Match) -> State {
    for _ in 0..30 {
        let state = step(game, buttons(0));
        if state.fighters[0].action == Action::Fall {
            return state;
        }
    }
    unreachable!("JumpAerial must fall within thirty steps");
}

#[test]
fn ground_and_aerial_jump_direction_follows_the_launch_frames_stick_with_backward_on_equality() {
    // stick_x * facing == -x78 (0.3) is exactly the boundary; the source's
    // own comparison is strict `>`, so equality selects backward.
    let cases: [(f32, bool); 4] = [(1.0, false), (-1.0, true), (-0.3, true), (-0.29, false)];
    for (stick_x, backward) in cases {
        for full in [true, false] {
            let mut game = Match::new(data(), 42).unwrap();
            let launched = if full {
                full_hop(&mut game, stick_x)
            } else {
                short_hop(&mut game, stick_x)
            };
            assert_eq!(
                launched.fighters[0].locomotion.jump_backward, backward,
                "stick_x {stick_x} full {full}"
            );
            assert!(!launched.fighters[0].grounded);
        }
    }
}

#[test]
fn aerial_jump_direction_is_independent_of_the_ground_jumps_own_direction() {
    for (ground_stick, aerial_stick, aerial_backward) in [(1.0, -1.0, true), (-1.0, 1.0, false)] {
        let mut game = Match::new(data(), 42).unwrap();
        full_hop(&mut game, ground_stick);
        let launched = double_jump(&mut game, aerial_stick);
        assert_eq!(
            launched.fighters[0].locomotion.jump_backward,
            aerial_backward
        );
    }
}

#[test]
fn jump_backward_clears_on_landing_aerial_attack_and_air_dodge() {
    // Landing: the ground jump returns to earth without ever double jumping.
    let mut game = Match::new(data(), 42).unwrap();
    full_hop(&mut game, -1.0);
    let mut state = game.state().clone();
    let mut frames = 0;
    while state.fighters[0].action != Action::Landing {
        state = step(&mut game, buttons(0));
        frames += 1;
        assert!(frames < 60, "must land within sixty steps");
    }
    assert!(!state.fighters[0].locomotion.jump_backward);

    // Aerial attack: a fresh A from a backward JumpAerial.
    let mut game = Match::new(data(), 42).unwrap();
    full_hop(&mut game, -1.0);
    let aerial = double_jump(&mut game, -1.0);
    assert!(aerial.fighters[0].locomotion.jump_backward);
    let attacking = step(&mut game, buttons(BUTTON_A));
    assert_eq!(attacking.fighters[0].action, Action::AttackAirN);
    assert!(!attacking.fighters[0].locomotion.jump_backward);

    // Air dodge: a fresh L from a backward JumpAerial.
    let mut game = Match::new(data(), 42).unwrap();
    full_hop(&mut game, -1.0);
    let aerial = double_jump(&mut game, -1.0);
    assert!(aerial.fighters[0].locomotion.jump_backward);
    let dodging = step(&mut game, buttons(BUTTON_L));
    assert_eq!(dodging.fighters[0].action, Action::EscapeAir);
    assert!(!dodging.fighters[0].locomotion.jump_backward);
}

#[test]
fn fall_aerial_is_true_only_after_the_double_jumps_own_animation_ends() {
    // The double jump's own animation end: FallAerial.
    let mut game = Match::new(data(), 42).unwrap();
    full_hop(&mut game, 1.0);
    double_jump(&mut game, 1.0);
    let fallen = until_fall(&mut game);
    assert!(fallen.fighters[0].locomotion.fall_aerial);

    // An aerial attack ending into Fall (`aerial::update_animation`'s own
    // `super::simulation::enter(f, Action::Fall)`): ordinary, not aerial.
    let mut game = Match::new(data(), 42).unwrap();
    full_hop(&mut game, 1.0);
    double_jump(&mut game, 1.0);
    step(&mut game, buttons(BUTTON_A));
    let mut state = game.state().clone();
    let mut frames = 0;
    while state.fighters[0].action != Action::Fall {
        state = step(&mut game, buttons(0));
        frames += 1;
        assert!(
            frames < 20,
            "the aerial attack must fall within twenty steps"
        );
    }
    assert!(!state.fighters[0].locomotion.fall_aerial);

    // An air dodge from the aerial fall: the flag clears immediately, since
    // EscapeAir is a different action (the dodge itself always continues
    // into FallSpecial, never an ordinary Fall: `escape_air.rs`'s
    // `Action::EscapeAir` arm unconditionally enters FallSpecial).
    let mut game = Match::new(data(), 42).unwrap();
    full_hop(&mut game, 1.0);
    double_jump(&mut game, 1.0);
    let fallen = until_fall(&mut game);
    assert!(fallen.fighters[0].locomotion.fall_aerial);
    let dodging = step(&mut game, buttons(BUTTON_L));
    assert_eq!(dodging.fighters[0].action, Action::EscapeAir);
    assert!(!dodging.fighters[0].locomotion.fall_aerial);
}

#[test]
fn checkpoints_restore_all_three_jump_flag_phases() {
    // 0: mid-Jump with jump_backward set. 1: mid-JumpAerial with
    // jump_backward set. 2: mid-Fall with fall_aerial set.
    for phase in 0..3 {
        let mut game = Match::new(data(), 42).unwrap();
        full_hop(&mut game, -1.0);
        if phase >= 1 {
            double_jump(&mut game, -1.0);
        }
        if phase == 2 {
            until_fall(&mut game);
        }
        let checkpoint = game.checkpoint();
        let inputs: Vec<[Controller; 2]> = (0..10)
            .map(|frame| {
                [
                    match frame {
                        3 => buttons(BUTTON_A),
                        6 => buttons(BUTTON_L),
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
                "phase {phase}"
            );
        }
    }
}

#[test]
fn none_threshold_keeps_every_jump_forward() {
    let mut resource = data();
    for fighter in &mut resource.fighters {
        fighter.locomotion.as_mut().unwrap().jump_backward_threshold = None;
    }
    let mut game = Match::new(resource, 42).unwrap();
    let launched = full_hop(&mut game, -1.0);
    assert!(!launched.fighters[0].locomotion.jump_backward);
    let aerial = double_jump(&mut game, -1.0);
    assert!(!aerial.fighters[0].locomotion.jump_backward);
}

#[test]
fn invalid_jump_backward_threshold_is_rejected() {
    for threshold in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, -1.0] {
        let mut resource = data();
        resource.fighters[0]
            .locomotion
            .as_mut()
            .unwrap()
            .jump_backward_threshold = Some(threshold);
        assert!(Match::new(resource, 0).is_err(), "threshold {threshold}");
    }
    // Zero is the accepted inclusive boundary.
    let mut boundary = data();
    boundary.fighters[0]
        .locomotion
        .as_mut()
        .unwrap()
        .jump_backward_threshold = Some(0.0);
    assert!(Match::new(boundary, 0).is_ok());
}
