//! Native input→aerial→physics/contact→landing scenarios. The sampled resources
//! are deliberately synthetic; source paths justify lifecycle expectations only.
#[path = "support/aerial.rs"]
mod support;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_R, BUTTON_X, Controller, Event, Match,
    data::{Hitbox, MatchData},
};
use support::{ATTACKS, LANDINGS, STICKS, data, game, input, step};

fn attack(stick: [f32; 2]) -> Controller {
    Controller {
        buttons: BUTTON_A,
        stick,
        ..Default::default()
    }
}

// ftCo_AttackAir_GetMsidFromCStick: neutral deadzone, angle, then facing.
#[test]
fn all_five_main_stick_attacks_and_fresh_cstick_directions_respect_facing() {
    for player in 0..2 {
        for (index, mut stick) in STICKS.into_iter().enumerate() {
            let mut game = game();
            stick[0] *= game.state().fighters[player].facing;
            let fighter = &game.step(input(player, attack(stick))).unwrap().fighters[player];
            assert_eq!(fighter.action, ATTACKS[index]);
            assert!(!fighter.grounded);
            for _ in 0..8 {
                game.step([Controller::default(); 2]).unwrap();
            }
            assert_eq!(game.state().fighters[player].action, Action::Fall);
            if index == 0 {
                continue;
            }
            let mut game = support::game();
            let controller = Controller {
                cstick: stick,
                stick: [-stick[0], -stick[1]],
                ..Default::default()
            };
            assert_eq!(
                game.step(input(player, controller)).unwrap().fighters[player].action,
                ATTACKS[index]
            );
        }
    }
}

// ftCo_800DF478 tests each old C-stick magnitude below its deadzone. Direct
// opposite input remains held, and only returning through neutral rearms it.
#[test]
fn held_or_opposite_cstick_does_not_repeat_after_attack_end_but_neutral_rearms() {
    let mut game = game();
    let right = Controller {
        cstick: [1.0, 0.0],
        ..Default::default()
    };
    let left = Controller {
        cstick: [-1.0, 0.0],
        ..Default::default()
    };
    assert_eq!(
        step(&mut game, right).fighters[0].action,
        Action::AttackAirF
    );
    for frame in 1..8 {
        let state = step(&mut game, if frame % 2 == 0 { right } else { left });
        assert_eq!(state.fighters[0].action, Action::AttackAirF);
        assert_eq!(state.fighters[0].action_frame, frame + 1);
    }
    assert_eq!(step(&mut game, right).fighters[0].action, Action::Fall);
    assert_eq!(step(&mut game, left).fighters[0].action, Action::Fall);
    step(&mut game, Controller::default());
    assert_eq!(step(&mut game, left).fighters[0].action, Action::AttackAirB);
}

#[test]
fn fresh_a_uses_main_stick_when_cstick_is_held_and_neutral_does_not_start_attacks() {
    let mut resource = data();
    for movement in &mut resource.fighters[0].aerials.as_mut().unwrap().moves {
        for flags in &mut movement.flags {
            flags.allow_interrupt = true;
        }
    }
    let mut game = Match::new(resource, 0).unwrap();
    let held = Controller {
        cstick: [1.0, 0.0],
        ..Default::default()
    };
    step(&mut game, held);
    let state = step(
        &mut game,
        Controller {
            buttons: BUTTON_A,
            stick: [0.0, 1.0],
            ..held
        },
    );
    assert_eq!(state.fighters[0].action, Action::AttackAirHi);
    let mut game = support::game();
    for _ in 0..3 {
        assert_eq!(
            step(&mut game, Controller::default()).fighters[0].action,
            Action::Fall
        );
    }
}

// ftCo_AttackAir_Phys delegates to the ordinary air physics callback.
#[test]
fn attack_preserves_air_drift_gravity_and_fast_fall() {
    let mut attacking = game();
    let mut falling = game();
    let control = Controller {
        stick: [1.0, 0.0],
        ..Default::default()
    };
    step(&mut attacking, attack(control.stick));
    step(&mut falling, control);
    for game in [&attacking, &falling] {
        assert_eq!(game.state().fighters[0].velocity, [0.15, -0.2]);
    }
    for controller in [
        control,
        Controller {
            stick: [0.0, -1.0],
            ..Default::default()
        },
        Controller::default(),
    ] {
        let a = step(&mut attacking, controller);
        let b = step(&mut falling, controller);
        assert_eq!(a.fighters[0].position, b.fighters[0].position);
        assert_eq!(a.fighters[0].velocity, b.fighters[0].velocity);
    }
    assert!(attacking.state().fighters[0].fast_fall);
    assert_eq!(attacking.state().fighters[0].velocity[1], -3.0);
}

// AttackAir DO_IASA checks the attack branch before the ordinary aerial jump.
#[test]
fn interrupt_window_allows_new_attacks_or_double_jump_with_attack_priority() {
    for (buttons, expected) in [
        (BUTTON_A, Action::AttackAirHi),
        (BUTTON_X, Action::JumpAerial),
        (BUTTON_A | BUTTON_X, Action::AttackAirHi),
    ] {
        let mut resource = data();
        resource.fighters[0].aerials.as_mut().unwrap().moves[0].flags[2].allow_interrupt = true;
        let mut game = Match::new(resource, 0).unwrap();
        step(&mut game, attack([0.0; 2]));
        assert_eq!(
            step(&mut game, Controller::default()).fighters[0].action,
            Action::AttackAirN
        );
        let state = step(
            &mut game,
            Controller {
                buttons,
                stick: if buttons & BUTTON_A != 0 {
                    [0.0, 1.0]
                } else {
                    [0.0; 2]
                },
                ..Default::default()
            },
        );
        assert_eq!(state.fighters[0].action, expected);
        assert_eq!(
            state.fighters[0].locomotion.jumps_used,
            if expected == Action::JumpAerial { 2 } else { 1 }
        );
    }
    let mut game = game();
    step(&mut game, attack([0.0; 2]));
    let state = step(
        &mut game,
        Controller {
            buttons: BUTTON_X,
            ..Default::default()
        },
    );
    assert_eq!(state.fighters[0].action, Action::AttackAirN);
    assert_eq!(state.fighters[0].locomotion.jumps_used, 1);
    let mut game = support::game();
    let state = step(
        &mut game,
        Controller {
            buttons: BUTTON_A | BUTTON_X,
            ..Default::default()
        },
    );
    assert_eq!(state.fighters[0].action, Action::AttackAirN);
    assert_eq!(state.fighters[0].locomotion.jumps_used, 1);
}

// KneeBend_Anim enters Jump before this frame's Jump_IASA dispatch. A fresh
// launch-frame aerial must run immediately, ahead of a simultaneous air jump.
#[test]
fn launch_frame_a_or_cstick_enters_an_aerial_after_ground_jump_animation() {
    for (controller, expected) in [
        (attack([0.0; 2]), Action::AttackAirN),
        (
            Controller {
                cstick: [1.0, 0.0],
                ..Default::default()
            },
            Action::AttackAirF,
        ),
        (
            Controller {
                buttons: BUTTON_A | BUTTON_X,
                ..Default::default()
            },
            Action::AttackAirN,
        ),
    ] {
        let mut resource = data();
        resource.stage.spawns[0][1] = 0.0;
        let mut game = Match::new(resource, 0).unwrap();
        let squat = step(
            &mut game,
            Controller {
                buttons: BUTTON_X,
                ..Default::default()
            },
        );
        assert_eq!(squat.fighters[0].action, Action::JumpSquat);
        let ready = step(&mut game, Controller::default());
        assert_eq!(ready.fighters[0].action, Action::JumpSquat);
        assert_eq!(ready.fighters[0].action_frame, 2);
        let launch = step(&mut game, controller);
        assert_eq!(launch.fighters[0].action, expected);
        assert!(!launch.fighters[0].grounded);
        assert!(launch.fighters[0].velocity[1] > 0.0);
        assert_eq!(launch.fighters[0].locomotion.jumps_used, 1);
    }
}

// ftCheckThrowB3 clears the event. The same frozen animation sample must not
// reverse facing again; the sampled bone drives the actual hurtbox contact.
#[test]
fn animated_hitbox_reverses_once_through_hitlag_and_checkpoint_replay() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 100.0], [2.0, 100.0]];
    resource.rules.hitlag.base = 3.0;
    resource.rules.hitlag.damage_scale = 0.0;
    resource.rules.knockback_speed = 0.0;
    let movement = &mut resource.fighters[0].aerials.as_mut().unwrap().moves[0];
    for (frame, x) in [(0, -6.0), (1, -2.0), (2, -2.0)] {
        movement.attack.frames[frame].bones[1].translation = [x, 1.0, 0.0];
        movement.attack.frames[frame].hitboxes = vec![Hitbox {
            clank: false,
            rebound: false,
            element: Default::default(),
            shield_damage: 0,
            group: 0,
            bone: 1,
            center: [0.0; 3],
            radius: 0.25,
            damage: 9,
            angle_degrees: 30.0,
            growth: 50,
            fixed: 0,
            base: 30,
        }];
    }
    movement.flags[1].reverse_facing = true;
    let mut game = Match::new(resource, 0).unwrap();
    let first = step(&mut game, attack([0.0; 2]));
    assert_eq!(first.fighters[0].hitboxes[0].current[0], -6.0);
    assert_eq!(first.fighters[1].percent, 0.0);
    let hit = step(&mut game, Controller::default());
    assert!(hit.events.iter().any(|event| matches!(
        event,
        Event::Hit {
            attacker: 0,
            victim: 1,
            damage: 9.0,
            ..
        }
    )));
    assert_eq!(hit.fighters[0].hitboxes[0].current[0], 2.0);
    assert_eq!(hit.fighters[0].facing, -1.0);
    assert_eq!(hit.fighters[0].action_frame, 1);
    let checkpoint = game.checkpoint();
    let mut expected = vec![];
    for _ in 0..4 {
        let state = step(&mut game, Controller::default()).clone();
        assert_eq!(state.fighters[0].facing, -1.0);
        assert_eq!(state.fighters[1].percent, 9.0);
        assert!(
            !state
                .events
                .iter()
                .any(|event| matches!(event, Event::Hit { .. }))
        );
        expected.push(state);
    }
    assert_eq!(expected[0].fighters[0].action_frame, 1);
    assert_eq!(expected[2].fighters[0].action_frame, 1);
    assert_eq!(expected[3].fighters[0].action_frame, 2);
    game.restore_checkpoint(&checkpoint).unwrap();
    for state in expected {
        assert_eq!(step(&mut game, Controller::default()), &state);
    }
}

fn landing_game(index: usize, autocancel: bool, triggers: impl Fn(usize) -> Controller) -> Match {
    let mut resource = data();
    resource.stage.spawns[0][1] = 4.0;
    if autocancel {
        for flags in &mut resource.fighters[0].aerials.as_mut().unwrap().moves[index].flags {
            flags.landing_lag = false;
        }
    }
    let mut game = Match::new(resource, 0).unwrap();
    let mut attack_instance = 0;
    for frame in 0..8 {
        let mut controller = triggers(frame);
        if frame == 0 {
            controller.buttons |= BUTTON_A;
            controller.stick = STICKS[index];
        }
        let state = step(&mut game, controller);
        if frame == 0 {
            attack_instance = state.fighters[0].action_instance.id;
        }
        if state.fighters[0].grounded {
            assert_eq!(
                frame, 5,
                "the explicit four-unit fall must land on the sixth step"
            );
            assert_eq!(
                state.fighters[0].action_instance.id == attack_instance,
                !autocancel,
                "aerial and matching landing retain one nonzero motion identity"
            );
            return game;
        }
    }
    unreachable!("the explicit falling scenario must contact its floor");
}

fn remaining_landing_frames(game: &mut Match) -> usize {
    for frames in 1..=20 {
        let state = step(game, Controller::default());
        if state.fighters[0].action == Action::Wait {
            return frames;
        }
        assert!(state.fighters[0].grounded);
    }
    unreachable!("explicit landing lag must expire within twenty steps");
}

// ftCo_LandingAir_EnterWithLag selects a separate landing action for all five
// attacks when cmd_vars[0] is set; otherwise it enters the basic landing state.
#[test]
fn all_five_landing_actions_use_their_own_lag_and_autocancel_uses_basic_landing() {
    for (index, expected) in LANDINGS.into_iter().enumerate() {
        let mut game = landing_game(index, false, |_| Controller::default());
        assert_eq!(game.state().fighters[0].action, expected);
        assert_eq!(remaining_landing_frames(&mut game), 8 + 2 * index);
        let mut game = landing_game(index, true, |_| Controller::default());
        assert_eq!(game.state().fighters[0].action, Action::Landing);
        assert_eq!(remaining_landing_frames(&mut game), 2);
    }
}

// LandingAir's age comparison is strict. Fighter input history combines
// analog and digital shoulders, so changing L to R cannot make held input fresh.
#[test]
fn l_cancel_requires_fresh_combined_trigger_history_strictly_inside_the_window() {
    for (index, _) in LANDINGS.into_iter().enumerate() {
        for analog in [false, true] {
            for (pressed_frame, cancelled) in [(0, false), (2, false), (3, true), (5, true)] {
                let mut game = landing_game(index, false, |frame| {
                    if frame < pressed_frame {
                        Controller::default()
                    } else if analog {
                        Controller {
                            trigger: 0.01,
                            ..Default::default()
                        }
                    } else {
                        Controller {
                            buttons: BUTTON_L,
                            ..Default::default()
                        }
                    }
                });
                assert_eq!(
                    game.state().fighters[0].locomotion.trigger_age,
                    5 - pressed_frame as u8
                );
                let lag = 8 + 2 * index;
                assert_eq!(
                    remaining_landing_frames(&mut game),
                    if cancelled { lag / 2 } else { lag }
                );
            }
        }
    }
    for analog in [false, true] {
        let mut game = landing_game(0, false, |frame| {
            if frame >= 3 {
                Controller {
                    buttons: BUTTON_R,
                    ..Default::default()
                }
            } else if analog {
                Controller {
                    trigger: 0.01,
                    ..Default::default()
                }
            } else {
                Controller {
                    buttons: BUTTON_L,
                    ..Default::default()
                }
            }
        });
        assert_eq!(game.state().fighters[0].locomotion.trigger_age, 5);
        assert_eq!(remaining_landing_frames(&mut game), 8);
    }
    let mut rearmed = landing_game(0, false, |frame| Controller {
        buttons: match frame {
            0 | 1 => BUTTON_L,
            2 => 0,
            _ => BUTTON_R,
        },
        ..Default::default()
    });
    assert_eq!(rearmed.state().fighters[0].locomotion.trigger_age, 2);
    let checkpoint = rearmed.checkpoint();
    assert_eq!(remaining_landing_frames(&mut rearmed), 4);
    let expected = rearmed.state().clone();
    rearmed.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(remaining_landing_frames(&mut rearmed), 4);
    assert_eq!(rearmed.state(), &expected);
}

#[test]
fn the_command_sample_on_the_landing_frame_controls_autocancel() {
    for lag_on_landing in [false, true] {
        let mut resource = data();
        resource.stage.spawns[0][1] = 4.0;
        let movement = &mut resource.fighters[0].aerials.as_mut().unwrap().moves[0];
        for flags in &mut movement.flags {
            flags.landing_lag = !lag_on_landing;
        }
        movement.flags[5].landing_lag = lag_on_landing;
        let mut game = Match::new(resource, 0).unwrap();
        step(&mut game, attack([0.0; 2]));
        for _ in 0..5 {
            step(&mut game, Controller::default());
        }
        assert_eq!(
            game.state().fighters[0].action,
            if lag_on_landing {
                Action::LandingAirN
            } else {
                Action::Landing
            }
        );
    }
}

#[test]
fn l_cancel_history_ages_through_real_hitlag_and_checkpoint_restore() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 4.0], [2.0, 4.0]];
    resource.rules.hitlag.base = 3.0;
    resource.rules.hitlag.damage_scale = 0.0;
    resource.rules.knockback_speed = 0.0;
    let frame = &mut resource.fighters[0].aerials.as_mut().unwrap().moves[0]
        .attack
        .frames[0];
    frame.bones[1].translation[0] = 2.0;
    frame.hitboxes.push(Hitbox {
        clank: false,
        rebound: false,
        element: Default::default(),
        shield_damage: 0,
        group: 0,
        bone: 1,
        center: [0.0; 3],
        radius: 0.25,
        damage: 9,
        angle_degrees: 30.0,
        growth: 50,
        fixed: 0,
        base: 30,
    });
    let mut game = Match::new(resource, 0).unwrap();
    let hit = step(&mut game, attack([0.0; 2]));
    assert_eq!(hit.fighters[0].hitlag, 3.0);
    assert_eq!(hit.fighters[1].percent, 9.0);
    let frozen_position = hit.fighters[0].position;
    let checkpoint = game.checkpoint();
    let mut expected = vec![];
    for frame in 0..8 {
        let controller = Controller {
            buttons: if frame == 0 { BUTTON_L } else { BUTTON_R },
            ..Default::default()
        };
        let state = step(&mut game, controller).clone();
        assert_eq!(state.fighters[0].locomotion.trigger_age, frame);
        if frame < 3 {
            assert_eq!(state.fighters[0].hitlag, f32::from(2 - frame));
            assert_eq!(state.fighters[0].position, frozen_position);
        }
        expected.push((controller, state));
    }
    assert_eq!(game.state().fighters[0].action, Action::LandingAirN);
    assert_eq!(game.state().fighters[0].locomotion.trigger_age, 7);
    assert_eq!(remaining_landing_frames(&mut game), 8);
    game.restore_checkpoint(&checkpoint).unwrap();
    for (controller, state) in expected {
        assert_eq!(step(&mut game, controller), &state);
    }
    assert_eq!(remaining_landing_frames(&mut game), 8);
}

#[test]
fn incomplete_or_invalid_aerial_resources_fail_before_the_first_frame() {
    type InvalidResource = (&'static str, fn(&mut MatchData));
    let cases: &[InvalidResource] = &[
        ("missing command samples", |d| {
            d.fighters[0].aerials.as_mut().unwrap().moves[0]
                .flags
                .clear()
        }),
        ("mismatched command samples", |d| {
            d.fighters[0].aerials.as_mut().unwrap().moves[0].flags.pop();
        }),
        ("zero selection threshold", |d| {
            d.fighters[0].aerials.as_mut().unwrap().selection.thresholds[0] = 0.0
        }),
        ("nonfinite selection angle", |d| {
            d.fighters[0]
                .aerials
                .as_mut()
                .unwrap()
                .selection
                .vertical_angle = f32::NAN
        }),
        ("zero cancel divisor", |d| {
            d.fighters[0].aerials.as_mut().unwrap().l_cancel_divisor = 0.0
        }),
        ("cancel division overflow", |d| {
            d.fighters[0].aerials.as_mut().unwrap().l_cancel_divisor = f32::from_bits(1)
        }),
        ("nonfinite landing lag", |d| {
            d.fighters[0].aerials.as_mut().unwrap().moves[0].landing_lag = f32::INFINITY
        }),
        ("landing animation rate overflow", |d| {
            d.fighters[0].aerials.as_mut().unwrap().moves[0].landing_lag = f32::from_bits(1)
        }),
        ("uncovered landing end", |d| {
            d.fighters[0].aerials.as_mut().unwrap().moves[0].landing_animation_end = 11.0
        }),
        ("changed landing topology", |d| {
            d.fighters[0].aerials.as_mut().unwrap().moves[0].landing_poses[0][1].parent = None
        }),
        ("changed attack topology", |d| {
            d.fighters[0].aerials.as_mut().unwrap().moves[0]
                .attack
                .frames[0]
                .bones[1]
                .parent = None
        }),
        ("nonfinite landing pose", |d| {
            d.fighters[0].aerials.as_mut().unwrap().moves[0].landing_poses[0][1].translation[0] =
                f32::NAN
        }),
        ("nonfinite attack pose", |d| {
            d.fighters[0].aerials.as_mut().unwrap().moves[0]
                .attack
                .frames[0]
                .bones[1]
                .translation[0] = f32::INFINITY
        }),
        ("nonfinite aerial hitbox", |d| {
            let mut hit = d.fighters[0].jab.frames[1].hitboxes[0].clone();
            hit.radius = f32::NAN;
            d.fighters[0].aerials.as_mut().unwrap().moves[0]
                .attack
                .frames[0]
                .hitboxes
                .push(hit);
        }),
    ];
    for (description, invalidate) in cases {
        let mut resource = data();
        invalidate(&mut resource);
        assert!(Match::new(resource, 0).is_err(), "accepted {description}");
    }
}
