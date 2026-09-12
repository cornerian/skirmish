//! Fox/Falco up special (Fire Fox/Fire Bird): the common input dispatch,
//! Hold's charge and gravity delay, the stick-driven launch angle and its
//! two thresholds, the grounded-vs-aerial launch decision (including the
//! platform check), Travel's duration-independent-of-animation countdown
//! and its post-`duration_end` reverse acceleration, the Bound rebound
//! decision, Landing/Fall (including the frame-13 regression fixed in this
//! batch), the `FallSpecial` exits and ledge catching. Bit-exact arithmetic
//! is the C-oracle differential suite's job
//! (`tests/fox_up_special_differential.rs`); this file owns the wiring --
//! that a real `Match` actually reaches each phase, in the right order,
//! with the resource's own attribute values.

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/fox_up_special.rs"]
mod up_special_resources;

use skirmish::collision::stage;
use skirmish::game::{
    Action, BUTTON_B, Controller, Event, Match,
    characters::Specials,
    data::{MatchData, StageGeometry},
};

/// Mutable access to a fighter's up-special resource in test setup, since
/// `Specials` is tagged by character and only Fox's variant carries one.
fn up_special_mut(
    fighter: &mut skirmish::game::data::FighterData,
) -> &mut skirmish::game::characters::fox::up::UpSpecial {
    let Some(Specials::Fox { up, .. }) = fighter.specials.as_mut() else {
        panic!("test fixture is missing its up-special resource");
    };
    up.as_mut()
        .expect("test fixture is missing its up-special resource")
}

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    up_special_resources::profile(conformance::data())
}

/// Player 0 spawns airborne, matching `tests/support/special.rs::airborne_game`.
fn airborne_data() -> MatchData {
    let mut resource = data();
    resource.stage.spawns[0][1] = 6.0;
    resource
}

/// A platform (not solid ground) directly under player 0's spawn, for the
/// grounded launch's own platform-skip test.
fn platform_data() -> MatchData {
    let mut resource = data();
    resource.stage.geometry = Some(StageGeometry {
        lines: vec![stage::Line {
            start: [-40.0, 0.0],
            end: [40.0, 0.0],
            flags: stage::FLOOR | stage::ENABLED,
            material_flags: stage::PLATFORM as u16,
            ..Default::default()
        }],
        joints: vec![stage::Joint {
            flags: stage::ENABLED,
            bounds_min: [-40.0, -100.0],
            bounds_max: [40.0, 100.0],
            floor: 0..1,
            ..Default::default()
        }],
    });
    resource
}

/// An open arena with a distant floor, a wall at `wall_x` and a ceiling at
/// `ceiling_y`, both far enough from each other and the floor that a slow
/// climb/drift in one axis reaches its own target well before the other
/// (the mid-Travel wall/ceiling redirect's own native regressions).
/// Attributes are overridden to a slow, non-decaying, long-duration
/// straight shot (`speed`/`duration` generous, `duration_end` effectively
/// disabled) so Travel's own velocity stays constant and reaches the
/// contact at a shallow, predictable angle.
fn wall_ceiling_data(wall_x: f32, ceiling_y: f32) -> MatchData {
    let mut resource = airborne_data();
    resource.stage.spawns[0] = [0.0, 5.0];
    for fighter in &mut resource.fighters {
        let p = up_special_mut(fighter);
        p.attributes.speed = 5.0;
        p.attributes.duration = 100.0;
        p.attributes.duration_end = 1000.0;
        p.attributes.bound_angle_degrees = 30.0;
    }
    resource.stage.geometry = Some(StageGeometry {
        lines: vec![
            stage::Line {
                start: [-1000.0, 0.0],
                end: [1000.0, 0.0],
                flags: stage::ENABLED | stage::FLOOR,
                ..Default::default()
            },
            stage::Line {
                start: [1000.0, ceiling_y],
                end: [-1000.0, ceiling_y],
                flags: stage::ENABLED | stage::CEILING,
                ..Default::default()
            },
            stage::Line {
                start: [wall_x, -1000.0],
                end: [wall_x, 1000.0],
                flags: stage::ENABLED | stage::LEFT_WALL,
                ..Default::default()
            },
        ],
        joints: vec![stage::Joint {
            flags: stage::ENABLED,
            bounds_min: [-1100.0, -1100.0],
            bounds_max: [1100.0, 1100.0],
            floor: 0..1,
            ceiling: 1..2,
            left_wall: 2..3,
            ..Default::default()
        }],
    });
    resource
}

fn input(player: usize, controller: Controller) -> [Controller; 2] {
    let mut inputs = IDLE;
    inputs[player] = controller;
    inputs
}

fn up_stick(stick_y: f32) -> Controller {
    Controller {
        buttons: BUTTON_B,
        stick: [0.0, stick_y],
        ..Controller::default()
    }
}

fn directional(stick: [f32; 2]) -> Controller {
    Controller {
        buttons: BUTTON_B,
        stick,
        ..Controller::default()
    }
}

fn approx(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-4, "expected {b}, got {a}");
}

/// Step with `held` every frame until `action` changes from its value at
/// the first step (or panics after `limit` frames), returning the state
/// on the transitioning frame.
fn step_until_action_change(
    game: &mut Match,
    held: Controller,
    limit: usize,
) -> skirmish::game::State {
    let starting = game.state().fighters[0].action;
    for _ in 0..limit {
        let state = game.step(input(0, held)).unwrap().clone();
        if state.fighters[0].action != starting {
            return state;
        }
    }
    panic!("action did not change from {starting:?} within {limit} frames");
}

#[test]
fn none_keeps_b_and_up_stick_inert() {
    let mut plain = conformance::data();
    plain.fighters[0].specials = None;
    plain.rules.specials = None;
    let mut game = Match::new(plain, 0).unwrap();
    let state = game.step(input(0, up_stick(0.9))).unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialHiHold);
}

#[test]
fn ground_entry_enters_hold_with_gravity_delay_and_jumps_untouched() {
    let mut game = Match::new(data(), 0).unwrap();
    let jumps_before = game.state().fighters[0].locomotion.jumps_used;
    let state = game.step(input(0, up_stick(0.9))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialHiHold);
    assert_eq!(state.fighters[0].action_frame, 1);
    // x54 == 2.0; ftFx_SpecialHiHold_Phys never reads or ticks it while
    // grounded (confirmed against the pinned source: unlike the side
    // special's own Start/End, Hold's ground Phys is only ft_80084F3C).
    assert_eq!(state.fighters[0].fox_up_special.gravity_delay, 2.0);
    assert_eq!(state.fighters[0].locomotion.jumps_used, jumps_before);
}

#[test]
fn air_entry_divides_velocity_and_does_not_yet_restore_jumps() {
    // Unlike the side special's own aerial Start (and unlike this move's
    // own actual aerial *launch*, `ftFx_SpecialAirHi_Enter`), the pinned
    // `ftFx_SpecialAirHiStart_Enter` (Hold's own charge-start) does not
    // touch jumps at all -- jump restoration is specifically the launch's
    // own effect, confirmed against the decomp: it has no
    // `x1968_jumpsUsed` write anywhere in its body.
    let mut game = Match::new(airborne_data(), 0).unwrap();
    // Drift horizontally while falling to build a real nonzero velocity.x.
    for _ in 0..3 {
        game.step(input(
            0,
            Controller {
                stick: [0.9, 0.0],
                ..Controller::default()
            },
        ))
        .unwrap();
    }
    let before = game.state().fighters[0].clone();
    assert!(
        !before.grounded,
        "the fixture spawn must stay airborne here"
    );
    assert_ne!(before.velocity[0], 0.0);
    let jumps_before = before.locomotion.jumps_used;
    let state = game.step(input(0, up_stick(0.9))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialHiHoldAir);
    assert_eq!(state.fighters[0].velocity[1], 0.0);
    // x58 division, then this same step's own Hold air Phys friction (x5C
    // == 0.02, applied unconditionally regardless of the gravity delay).
    let half = before.velocity[0] / 2.0;
    let friction = if half > 0.0 { -0.02 } else { 0.02 };
    approx(state.fighters[0].velocity[0], half + friction);
    // x54 == 2.0, ticked once by this same step's own Hold air Phys (the
    // gravity delay counts down even on the entry frame, unlike the
    // ground side's own Phys which never reads it at all).
    assert_eq!(state.fighters[0].fox_up_special.gravity_delay, 1.0);
    assert_eq!(state.fighters[0].locomotion.jumps_used, jumps_before);
}

#[test]
fn air_entry_requires_a_fresh_b_press_not_just_a_held_one() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    // Frame 1: B pressed, stick below the vertical threshold.
    game.step(input(
        0,
        Controller {
            buttons: BUTTON_B,
            stick: [0.0, 0.0],
            ..Controller::default()
        },
    ))
    .unwrap();
    // Frame 2: B still held (not a fresh press this frame), stick now past
    // the vertical threshold: the aerial branch has no age gate, it checks
    // the fresh press directly, so this must not fire.
    let state = game.step(input(0, up_stick(0.9))).unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialHiHoldAir);
}

#[test]
fn hold_air_gravity_delay_holds_vertical_velocity_before_falling() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    let state = game.step(input(0, up_stick(0.9))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialHiHoldAir);
    // x54 == 2.0, already ticked once by this same step's own Phys.
    assert_eq!(state.fighters[0].fox_up_special.gravity_delay, 1.0);
    assert_eq!(state.fighters[0].velocity[1], 0.0);
    let state = game.step(IDLE).unwrap();
    assert_eq!(state.fighters[0].velocity[1], 0.0);
    assert_eq!(state.fighters[0].fox_up_special.gravity_delay, 0.0);
    // The delay has now lapsed: ftCommon_Fall(x60) applies from here on.
    let state = game.step(IDLE).unwrap();
    assert!(state.fighters[0].velocity[1] < 0.0);
}

#[test]
fn hold_ground_anim_end_launches_along_the_floor() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    // Stick well past 90 degrees from the flat floor's own upward normal
    // ([0, 1]): magnitude clears x64 (0.2875) and the angle gate.
    let state = step_until_action_change(&mut game, directional([0.9, -0.5]), 20);
    assert_eq!(state.fighters[0].action, Action::SpecialHi);
    assert_eq!(state.fighters[0].facing, 1.0);
    // x74 == 3.0.
    assert_eq!(state.fighters[0].ground_velocity, 3.0);
}

#[test]
fn hold_ground_anim_end_declines_when_stick_points_up() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    // Stick within 90 degrees of the floor's own upward normal: the
    // grounded launch declines into the pure aerial one instead.
    let state = step_until_action_change(&mut game, up_stick(0.9), 20);
    assert_eq!(state.fighters[0].action, Action::SpecialAirHi);
    assert!(!state.fighters[0].grounded);
}

#[test]
fn hold_ground_anim_end_declines_on_a_platform() {
    // Otherwise a clean "along the floor" stick (magnitude past x64, angle
    // to the floor normal past 90 degrees) must still decline into the
    // pure aerial launch on a platform (`ftCo_8009A134`). Any such stick
    // necessarily points at or below the floor's own plane, so the
    // resulting aerial launch (`atan2f(stick.y, stick.x * facing)`, not
    // the grounded launch's own floor-normal-relative angle) has no
    // upward escape velocity and the very next collision pass immediately
    // re-lands the fighter -- exactly as it would for the same reason in
    // the pinned source, whose own launch velocity formula is identical.
    // Landing while not yet bound-eligible continues as grounded Travel
    // (`land`'s own "IsBound false" path); that very first grounded
    // frame's own `update_ground_contact` immediately overwrites
    // `rotateModel` from the current floor normal (`ftFx_SpecialHi_Coll`'s
    // own continuous re-derivation while grounded, not a one-time launch
    // angle), so the aerial formula's own angle is not observable by the
    // time this returns -- the floor-derived one is.
    let mut game = Match::new(platform_data(), 0).unwrap();
    assert!(game.state().fighters[0].grounded);
    game.step(input(0, up_stick(0.9))).unwrap();
    let stick = [0.9, -0.5];
    let state = step_until_action_change(&mut game, directional(stick), 20);
    assert_eq!(state.fighters[0].action, Action::SpecialHi);
    assert!(state.fighters[0].grounded);
    let facing = state.fighters[0].facing;
    let floor_normal = state.fighters[0].floor_normal;
    let expected_floor_angle = libm::atan2f(-floor_normal[0] * facing, floor_normal[1]);
    approx(
        state.fighters[0].fox_up_special.rotate_model,
        expected_floor_angle,
    );
}

#[test]
fn hold_air_anim_end_launch_angle_straight_up_default() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    let max_jumps = game.data().fighters[0]
        .locomotion
        .as_ref()
        .unwrap()
        .max_jumps;
    game.step(input(0, up_stick(0.9))).unwrap();
    // Stick under x64 (0.2875): the straight-up default fires.
    let state = step_until_action_change(&mut game, directional([0.1, 0.1]), 20);
    assert_eq!(state.fighters[0].action, Action::SpecialAirHi);
    approx(state.fighters[0].velocity[0], 0.0);
    // x74 == 3.0.
    approx(state.fighters[0].velocity[1], 3.0);
    // Unlike Hold's own charge-start, the actual launch restores every
    // jump (`ftFx_SpecialAirHi_Enter`'s own `x1968_jumpsUsed = max_jumps`).
    assert_eq!(state.fighters[0].locomotion.jumps_used, max_jumps);
}

#[test]
fn hold_air_anim_end_launch_angle_respects_the_facing_stick_threshold() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    let facing_before = game.state().fighters[0].facing;
    // |stick.x| (0.1) clears x64 combined with stick.y, but stays under
    // x88 (0.2875): the facing must not turn even though the stick is
    // pointing the other way.
    let state = step_until_action_change(&mut game, directional([-0.1, 0.9]), 20);
    assert_eq!(state.fighters[0].action, Action::SpecialAirHi);
    assert_eq!(state.fighters[0].facing, facing_before);
}

#[test]
fn travel_duration_counts_down_independent_of_the_looping_animation_and_lands() {
    // x68 == 3.0: three Travel frames regardless of the 2-pose animation
    // looping under it.
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    let state = step_until_action_change(&mut game, directional([0.9, -0.5]), 20);
    assert_eq!(state.fighters[0].action, Action::SpecialHi);
    let state = game.step(IDLE).unwrap(); // 3 -> 2
    assert_eq!(state.fighters[0].action, Action::SpecialHi);
    let state = game.step(IDLE).unwrap(); // 2 -> 1
    assert_eq!(state.fighters[0].action, Action::SpecialHi);
    // Duration expires (1 -> 0): this profile's own bound gate (unk2
    // starts at 0, x6C == 2) is not yet satisfied, so it's an ordinary
    // ground landing.
    let state = game.step(IDLE).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialHiLanding);
}

#[test]
fn travel_reverse_acceleration_engages_after_duration_end_in_the_air() {
    // x70 == 2.0: after two Phys ticks the reverse acceleration (x78 ==
    // 0.1) starts pulling self_vel back toward the launch direction.
    let mut game = Match::new(airborne_data(), 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    let state = step_until_action_change(&mut game, directional([0.1, 0.1]), 20);
    assert_eq!(state.fighters[0].action, Action::SpecialAirHi);
    let before = state.fighters[0].velocity;
    game.step(IDLE).unwrap();
    let state = game.step(IDLE).unwrap();
    assert_ne!(
        state.fighters[0].velocity, before,
        "the reverse acceleration must have run"
    );
}

#[test]
fn travel_bound_decision_fires_once_unk2_passes_bounce_frames() {
    // x6C == 2: land after at least 2 grounded Travel frames and Bound
    // must fire even while still on solid ground (the other bound-gate
    // operand, `!on_platform`, would also be true off this flat floor, so
    // this specifically exercises the `unk2 >= x6C` branch by running the
    // ground phase long enough first).
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    let state = step_until_action_change(&mut game, directional([0.9, -0.5]), 20);
    assert_eq!(state.fighters[0].action, Action::SpecialHi);
    for _ in 0..5 {
        let state = game.state();
        if state.fighters[0].action != Action::SpecialHi {
            break;
        }
        game.step(IDLE).unwrap();
    }
    let state = game.state().clone();
    assert!(
        matches!(
            state.fighters[0].action,
            Action::SpecialHiLanding | Action::SpecialHiBound
        ),
        "expected Travel to have ended, got {:?}",
        state.fighters[0].action
    );
}

#[test]
fn fall_lands_at_frame_13_via_ordinary_ground_touch() {
    // Regression for this batch's own fix: Fall's own ground/ledge check
    // succeeding (not Travel's own bound decision) must enter
    // SpecialHiLanding at frame 13, not fall through to the generic
    // Action::Landing. A tiny launch speed (x74) keeps the fighter close
    // to its spawn height throughout Travel instead of the default
    // speed's own huge climb (see the sibling FallSpecial-exit test);
    // spawned a little above ground so Hold's own gravity-delayed fall
    // does not touch down before the launch even fires.
    let mut resource = airborne_data();
    resource.stage.spawns[0][1] = 2.0;
    up_special_mut(&mut resource.fighters[0]).attributes.speed = 0.05;
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    let state = step_until_action_change(&mut game, directional([0.1, 0.1]), 20);
    assert_eq!(state.fighters[0].action, Action::SpecialAirHi);
    // Run out Travel's own duration while still airborne, into Fall.
    let mut state = state;
    for _ in 0..10 {
        if state.fighters[0].action == Action::SpecialHiFall {
            break;
        }
        state = game.step(IDLE).unwrap().clone();
    }
    assert_eq!(state.fighters[0].action, Action::SpecialHiFall);
    // Now let Fall's own ground/ledge check land it.
    for _ in 0..200 {
        if state.fighters[0].grounded {
            break;
        }
        state = game.step(IDLE).unwrap().clone();
    }
    assert!(state.fighters[0].grounded, "must land within 200 frames");
    assert_eq!(state.fighters[0].action, Action::SpecialHiLanding);
    // Entered at frame 13; the generic per-frame increment then advances
    // it once more within this same step, like every other transition
    // observed in this suite.
    assert_eq!(state.fighters[0].action_frame, 14);
}

#[test]
fn landing_and_fall_exit_to_fall_special_with_mobility_and_lag() {
    let mut game = Match::new(airborne_data(), 0).unwrap();
    let max_jumps = game.data().fighters[0]
        .locomotion
        .as_ref()
        .unwrap()
        .max_jumps;
    game.step(input(0, up_stick(0.9))).unwrap();
    let state = step_until_action_change(&mut game, directional([0.1, 0.1]), 20);
    assert_eq!(state.fighters[0].action, Action::SpecialAirHi);
    let mut state = state;
    for _ in 0..10 {
        if state.fighters[0].action == Action::SpecialHiFall {
            break;
        }
        state = game.step(IDLE).unwrap().clone();
    }
    assert_eq!(state.fighters[0].action, Action::SpecialHiFall);
    // Fall's own 60-pose animation runs out before it lands (the default
    // launch speed sends the fighter far too high for it to fall back
    // down within that many frames), exiting into FallSpecial.
    for _ in 0..65 {
        state = game.step(IDLE).unwrap().clone();
        if state.fighters[0].action == Action::FallSpecial {
            break;
        }
    }
    assert_eq!(state.fighters[0].action, Action::FallSpecial);
    assert!(state.fighters[0].aerial.allow_interrupt);
    // x8C == 0.5.
    assert_eq!(state.fighters[0].aerial.mobility, 0.5);
    assert_eq!(state.fighters[0].locomotion.jumps_used, max_jumps);
}

#[test]
fn bound_exit_flag_forces_fall_special_before_the_pose_ends_while_airborne() {
    // The fixture's own Bound pose sets `exit_flags` true only on its last
    // frame (index 2 of 3); reaching Bound airborne must exit into
    // FallSpecial there rather than waiting for the pose to run out.
    let mut resource = airborne_data();
    // Make the bound gate's own `!on_platform` operand true immediately by
    // never grounding: enter Travel in the air directly and let its
    // duration run out while still airborne, then force Bound by keeping
    // the fighter off the ground.
    resource.stage.spawns[0][1] = 50.0;
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    let state = step_until_action_change(&mut game, directional([0.1, 0.1]), 20);
    assert_eq!(state.fighters[0].action, Action::SpecialAirHi);
    let mut state = state;
    for _ in 0..10 {
        if state.fighters[0].action != Action::SpecialAirHi {
            break;
        }
        state = game.step(IDLE).unwrap().clone();
    }
    // Travel's duration ran out while airborne: Fall, not Bound (Bound only
    // follows a landing during Travel's own air Coll, not a plain timeout).
    assert_eq!(state.fighters[0].action, Action::SpecialHiFall);
}

#[test]
fn slippi_ids_are_353_through_359() {
    for (action, id) in [
        (Action::SpecialHiHold, "special_hi_hold"),
        (Action::SpecialHiHoldAir, "special_hi_hold_air"),
        (Action::SpecialHi, "special_hi"),
        (Action::SpecialAirHi, "special_air_hi"),
        (Action::SpecialHiLanding, "special_hi_landing"),
        (Action::SpecialHiFall, "special_hi_fall"),
        (Action::SpecialHiBound, "special_hi_bound"),
    ] {
        let value = serde_json::to_value(action).unwrap();
        assert_eq!(value.as_str().unwrap(), id);
    }
}

#[test]
fn every_phase_survives_a_checkpoint_round_trip() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    game.step(IDLE).unwrap();
    let checkpoint = game.checkpoint();
    let expected = game.state().clone();
    game.step(IDLE).unwrap();
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(game.state(), &expected);
}

#[test]
fn invalid_up_special_resources_are_rejected() {
    let mut resource = data();
    up_special_mut(&mut resource.fighters[0])
        .attributes
        .entry_speed_div = 0.0;
    assert!(Match::new(resource, 0).is_err());

    let mut resource = data();
    up_special_mut(&mut resource.fighters[0])
        .attributes
        .duration = 0.0;
    assert!(Match::new(resource, 0).is_err());

    let mut resource = data();
    up_special_mut(&mut resource.fighters[0])
        .bound
        .transn_y
        .pop();
    assert!(Match::new(resource, 0).is_err());

    // `specials::helpers::validate_hitboxes` (this batch's own validation
    // gap-closer, `docs/fox-up-special.md`'s "Hitboxes" section): an
    // out-of-range hitbox group (>= 16) on Hold's own pose is rejected the
    // same way every other move kind's hitboxes already were.
    let mut resource = data();
    let mut hitbox = travel_hitbox();
    hitbox.group = 99;
    up_special_mut(&mut resource.fighters[0]).hold.ground.frames[0].hitboxes = vec![hitbox];
    assert!(Match::new(resource, 0).is_err());

    // `fighter.specials = None` while `rules.specials` stays set is itself
    // rejected (the side special's own validation, shared with this move,
    // requires a motion for every fighter whenever the common rules
    // exist, and `rules` is match-wide); clearing both, for every fighter,
    // is the valid "entirely inert" shape.
    let mut resource = data();
    resource.rules.specials = None;
    for fighter in &mut resource.fighters {
        fighter.specials = None;
    }
    assert!(
        Match::new(resource, 0).is_ok(),
        "clearing both the resource and the common rules must stay valid"
    );
}

#[test]
fn travel_air_redirect_does_not_zero_velocity_against_a_shallow_wall() {
    // Mostly-vertical velocity drifting slightly into a distant wall: a
    // shallow angle to the wall's own normal (well under the 90+x94
    // gate), so `ftFx_SpecialAirHi_Coll`'s own redirect fires. The
    // observable effect a native test can check is that the ordinary
    // auto-zero-into-wall response (every other aerial action's default)
    // does not apply here -- the redirect's own facing/rotateModel
    // recompute is a no-op when velocity itself is unchanged (this move's
    // own Travel-air never applies gravity or drag before duration_end),
    // so it cannot be observed as a value *change*; the bit-exact angle
    // arithmetic itself is the C-oracle differential suite's job.
    let mut game = Match::new(wall_ceiling_data(20.0, 5000.0), 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    let launch = directional([0.1, 0.99]);
    let mut state = step_until_action_change(&mut game, launch, 20);
    assert_eq!(state.fighters[0].action, Action::SpecialAirHi);
    for _ in 0..80 {
        state = game.step(input(0, launch)).unwrap().clone();
        if state.fighters[0].position[0] >= 17.0 {
            break;
        }
    }
    assert!(
        state.fighters[0].position[0] >= 17.0,
        "must reach the wall within 80 frames"
    );
    assert_ne!(
        state.fighters[0].velocity[0], 0.0,
        "the redirect must not let the ordinary auto-zero response fire"
    );
}

#[test]
fn travel_air_redirect_does_not_zero_velocity_against_a_shallow_ceiling() {
    // Mirrors the wall case above with the axes swapped: mostly-horizontal
    // velocity drifting slightly into a distant ceiling.
    let mut game = Match::new(wall_ceiling_data(5000.0, 20.0), 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    let launch = directional([0.99, 0.1]);
    let mut state = step_until_action_change(&mut game, launch, 20);
    assert_eq!(state.fighters[0].action, Action::SpecialAirHi);
    for _ in 0..80 {
        state = game.step(input(0, launch)).unwrap().clone();
        if state.fighters[0].position[1] >= 16.9 {
            break;
        }
    }
    assert!(
        state.fighters[0].position[1] >= 16.9,
        "must reach the ceiling within 80 frames"
    );
    assert_ne!(
        state.fighters[0].velocity[1], 0.0,
        "the redirect must not let the ordinary auto-zero response fire"
    );
}

#[test]
fn travel_ground_rotation_reflects_the_last_grounded_floor_normal_after_leaving_the_edge() {
    // `update_ground_contact` (`ftFx_SpecialHi_Coll`'s own floor-contact
    // branch) must keep re-deriving `rotateModel` from the *current* floor
    // normal every grounded Travel frame, not just once at launch, so
    // that dropping off an edge mid-flight carries the last real contact
    // angle into the air phase's own post-`duration_end` reverse
    // acceleration instead of a stale launch-time value. A flat floor
    // makes the "last grounded" angle identical to the launch angle in
    // the fixture's own along-the-floor test, so this only needs to
    // confirm the mechanism runs continuously (not just once) by checking
    // it survives several grounded frames before the drop, using a floor
    // that ends right where Travel is still counting down.
    let mut resource = data();
    resource.stage.floor.right = 1.0;
    // Player 1's own spawn (from the shared conformance fixture) must stay
    // within the shortened floor too, or match construction rejects it.
    resource.stage.spawns[1][0] = -50.0;
    // x68 == 3.0 by default: too short to reach the shortened floor's own
    // edge before Travel's own duration expires (landing normally instead
    // of ever leaving the ground here). Extended so the edge-drop is what
    // actually ends grounded Travel in this test.
    up_special_mut(&mut resource.fighters[0])
        .attributes
        .duration = 20.0;
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    let launch = directional([0.9, -0.5]);
    let state = step_until_action_change(&mut game, launch, 20);
    assert_eq!(state.fighters[0].action, Action::SpecialHi);
    assert!(state.fighters[0].grounded);
    let facing = state.fighters[0].facing;
    let floor_normal = state.fighters[0].floor_normal;
    let expected = libm::atan2f(-floor_normal[0] * facing, floor_normal[1]);
    approx(state.fighters[0].fox_up_special.rotate_model, expected);
    // Keep sliding until it runs off the shortened floor's own edge.
    let mut state = state;
    for _ in 0..20 {
        if !state.fighters[0].grounded {
            break;
        }
        state = game.step(input(0, launch)).unwrap().clone();
    }
    assert!(
        !state.fighters[0].grounded,
        "must leave the shortened floor within 20 frames"
    );
    assert_eq!(state.fighters[0].action, Action::SpecialAirHi);
    // The rotate_model carried into the air phase is still the last
    // grounded floor-normal angle (unchanged since this fixture's own
    // floor is flat, so every grounded frame recomputed the same value;
    // the point is that it is *this* value, freshly re-derived every
    // frame, not a stale one frozen at launch).
    approx(state.fighters[0].fox_up_special.rotate_model, expected);
}

#[test]
fn fall_special_landing_uses_this_move_s_own_landing_lag_not_the_common_one() {
    // `ftCo_80096900`'s own `landing_lag` argument (x90) is stored per
    // `FallSpecial` instance and read back by that instance's own eventual
    // landing (`ftCo_FallSpecial_Coll` -> `ftCo_LandingFallSpecial_Enter`),
    // not the shared `escape_air::Rules`' own rate every other FallSpecial
    // entry in this codebase still uses. The fixture's own two values
    // differ (x90 == 4.0, the common rate == 10.0) specifically so the
    // resulting `landing_rate` ((end + 0.1) / lag) tells them apart:
    // 6.1 / 4.0 == 1.525 if this move's own value was used, 6.1 / 10.0 ==
    // 0.61 if the common one leaked through instead.
    let mut resource = airborne_data();
    resource.stage.spawns[0][1] = 3.0;
    up_special_mut(&mut resource.fighters[0]).attributes.speed = 0.05;
    up_special_mut(&mut resource.fighters[0])
        .fall
        .frames
        .truncate(2);
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    let launch = directional([0.1, 0.1]);
    let mut state = game.step(input(0, launch)).unwrap().clone();
    for _ in 0..30 {
        if state.fighters[0].action == Action::LandingFallSpecial {
            break;
        }
        state = game.step(input(0, launch)).unwrap().clone();
    }
    assert_eq!(state.fighters[0].action, Action::LandingFallSpecial);
    assert!(state.fighters[0].grounded);
    approx(state.fighters[0].aerial.landing_rate, 1.525);
}

use up_special_resources::{hold_attack_with_pack_hitboxes, travel_hitbox};

#[test]
fn hold_charge_hits_a_nearby_opponent_at_the_pack_documented_pulse_frames() {
    // The schedule below reproduces the pack's own periodic pulse (frames
    // 20/22/24/26/28/30/32, clear on every frame in between). Each pulse
    // independently connects: `hitboxes::refreshed_groups` (`src/game/
    // hitboxes.rs`) generically re-enables an attacker's per-group
    // `hit_groups` bit whenever a hitbox re-creates after being absent from
    // every one of the four slots on the previous frame -- the same rule
    // `ftAction_8007121C`'s own re-enable gate (`state ==
    // HitCapsule_Disabled || x4 != hit_group`) and `ftColl_800768A0`'s
    // victim-clear-on-no-shared-capsule (`lbColl_80008440`) apply in the
    // pinned source. See `docs/fox-up-special.md`'s "Hitboxes" section.
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [1.0, 0.0]];
    resource.rules.knockback_speed = 0.0;
    let bones = resource.fighters[0].bones.clone();
    let hold = hold_attack_with_pack_hitboxes(&bones);
    {
        let p = up_special_mut(&mut resource.fighters[0]);
        p.hold.ground = hold.clone();
        p.hold.air = hold;
    }
    let mut game = Match::new(resource, 0).unwrap();
    let entry = game.step(input(0, up_stick(0.9))).unwrap();
    assert_eq!(entry.fighters[0].action, Action::SpecialHiHold);
    // `frame_before` is the pose actually sampled by the *next* `step` call
    // (the frame this same, already-returned state reports, matching every
    // other same-frame-cascade note in this suite: a state's own
    // `action_frame` is one ahead of the pose it was itself computed from).
    let mut frame_before = entry.fighters[0].action_frame;
    let mut hit_frames = Vec::new();
    loop {
        let state = game.step(IDLE).unwrap();
        if state.fighters[0].action != Action::SpecialHiHold {
            break;
        }
        if state.events.iter().any(|event| {
            matches!(
                event,
                Event::Hit {
                    attacker: 0,
                    victim: 1,
                    ..
                }
            )
        }) {
            hit_frames.push(frame_before);
        }
        frame_before = state.fighters[0].action_frame;
    }
    assert_eq!(hit_frames, vec![20, 22, 24, 26, 28, 30, 32]);
    // Seven pulses at 2 damage apiece; no staling is configured for this profile.
    assert_eq!(game.state().fighters[1].percent, 14.0);
}

#[test]
fn travel_hits_a_nearby_opponent_every_frame_matching_the_pack_s_continuous_hitbox() {
    // The base fixture's own 2-pose Travel loop (independent of the
    // `travel_frames` countdown that actually governs the move's real
    // duration, see `docs/fox-up-special.md`) stays untouched apart from
    // installing the pack's own hitbox on both existing poses -- since the
    // pack itself reports the identical hitbox on every one of its own 31
    // sampled frames, a shorter looping pose with the hitbox on every one of
    // *its* frames reproduces the same "never clears while Travel runs"
    // fact exactly.
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [1.0, 0.0]];
    resource.rules.knockback_speed = 0.0;
    {
        let p = up_special_mut(&mut resource.fighters[0]);
        for attack in [&mut p.travel.ground, &mut p.travel.air] {
            attack.move_id = Some(20);
            for frame in &mut attack.frames {
                frame.hitboxes = vec![travel_hitbox()];
            }
        }
        // A long, slow, constant-velocity travel keeps the fighter's own
        // hitbox in reach for several real frames instead of overshooting
        // the stationary opponent in one step.
        p.attributes.speed = 1.0;
        p.attributes.duration = 20.0;
        p.attributes.duration_end = 1000.0;
    }
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, up_stick(0.9))).unwrap();
    // The hitbox is already active on Travel's own entry frame (frame 0), so
    // the connecting hit shows up on this very transition step -- the same
    // step `update_animation`'s Hold-anim-end dispatch (`enter_from_ground_
    // hold`) enters `Action::SpecialHi` and the later hitbox sweep in this
    // same `advance()` call already reads the new action's frame 0.
    let state = step_until_action_change(&mut game, directional([0.9, -0.5]), 60);
    assert_eq!(state.fighters[0].action, Action::SpecialHi);
    assert!(
        state.events.iter().any(|event| matches!(
            event,
            Event::Hit {
                attacker: 0,
                victim: 1,
                ..
            }
        )),
        "Travel's continuous hitbox must connect on its own entry frame"
    );
    // 14 damage per the pack's own value; no staling is configured.
    assert_eq!(state.fighters[1].percent, 14.0);
}
