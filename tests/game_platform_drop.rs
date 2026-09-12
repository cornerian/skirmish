//! Synthetic input-to-collision regressions for ftCo_Pass.c, ftCo_Squat.c,
//! mpUpdateFloorSkip and Fighter_ChangeMotionState's floor-skip clearing.
use skirmish::collision::stage;
use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, Controller, Match, State,
    data::{MatchData, StageGeometry},
};

#[derive(serde::Deserialize)]
struct ShieldProfile {
    rules: skirmish::game::shield::Rules,
    attributes: skirmish::game::shield::Attributes,
}

const IDLE: [Controller; 2] = [Controller {
    cstick: [0.0; 2],
    trigger: 0.0,
    buttons: 0,
    stick: [0.0; 2],
}; 2];

fn down() -> [Controller; 2] {
    let mut input = IDLE;
    input[0].stick[1] = -1.0;
    input
}

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.stage.floor.left = -40.0;
    data.stage.floor.right = 40.0;
    data.stage.floor.y = 0.0;
    data.stage.spawns = [[0.0, 6.0], [20.0, 6.0]];
    data.stage.blast = [-100.0, 100.0, -100.0, 100.0];
    data.stage.geometry = Some(StageGeometry {
        lines: [6.0, 0.0]
            .map(|y| stage::Line {
                start: [-40.0, y],
                end: [40.0, y],
                flags: stage::FLOOR | stage::ENABLED,
                material_flags: stage::PLATFORM as u16,
                ..Default::default()
            })
            .to_vec(),
        joints: vec![stage::Joint {
            flags: stage::ENABLED,
            bounds_min: [-40.0, -100.0],
            bounds_max: [40.0, 100.0],
            floor: 0..2,
            ..Default::default()
        }],
    });
    let mut parameters: skirmish::game::locomotion::Parameters =
        serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap();
    parameters.pass_stick_threshold = 0.7;
    parameters.pass_window = 3;
    parameters.pass_delay = 1.0;
    parameters.pass_velocity = -0.5;
    parameters.pass_animation_frames = 60;
    for fighter in &mut data.fighters {
        fighter.locomotion = Some(parameters);
    }
    let shield: ShieldProfile =
        serde_json::from_str(include_str!("fixtures/game/shield.json")).unwrap();
    data.rules.shield = Some(shield.rules);
    for fighter in &mut data.fighters {
        fighter.shield = Some(shield.attributes.clone());
    }
    data
}

fn until(game: &mut Match, input: [Controller; 2], condition: impl Fn(&State) -> bool) -> State {
    for _ in 0..60 {
        let state = game.step(input).unwrap();
        if condition(state) {
            return state.clone();
        }
    }
    assert!(
        condition(game.state()),
        "condition not reached: {:?}",
        game.state()
    );
    game.state().clone()
}

#[test]
fn dropping_skips_only_the_supporting_line_and_lands_on_the_lower_platform() {
    let mut game = Match::new(data(), 0).unwrap();
    assert_eq!(game.state().fighters[0].ground_line, Some(0));
    let passing = until(&mut game, down(), |s| s.fighters[0].action == Action::Pass);
    assert!(!passing.fighters[0].grounded);
    assert!(passing.fighters[0].position[1] < 6.0);
    assert_eq!(passing.fighters[0].skip_floor, Some(0));
    assert!(passing.fighters[0].ecb.bottom_locked && passing.fighters[0].ecb_lock > 0);
    let landed = until(&mut game, down(), |s| s.fighters[0].ground_line == Some(1));
    assert!((landed.fighters[0].position[1] - 0.0001).abs() < 0.000001);
    assert_eq!(landed.fighters[0].skip_floor, None);
    assert_eq!(landed.fighters[0].ecb_lock, 0);
    assert!(!landed.fighters[0].ecb.bottom_locked);
    // Consumed tilt history prevents holding down from dropping through every
    // platform in a stack; releasing and tilting down again is a new request.
    for _ in 0..10 {
        game.step(down()).unwrap();
    }
    assert_eq!(game.state().fighters[0].ground_line, Some(1));
    game.step(IDLE).unwrap();
    let second = until(&mut game, down(), |s| !s.fighters[0].grounded);
    assert_eq!(second.fighters[0].skip_floor, Some(1));
}

#[test]
fn a_solid_floor_refuses_the_same_drop_input() {
    let mut data = data();
    data.stage.geometry.as_mut().unwrap().lines[0].material_flags = 0;
    let mut game = Match::new(data, 0).unwrap();
    for _ in 0..20 {
        let state = game.step(down()).unwrap();
        assert!(state.fighters[0].grounded);
        assert_eq!(state.fighters[0].ground_line, Some(0));
        assert_eq!(state.fighters[0].skip_floor, None);
        assert_ne!(state.fighters[0].action, Action::Pass);
    }
}

#[test]
fn fast_drop_substeps_still_collide_with_the_next_platform() {
    let mut data = data();
    let fighter = &mut data.fighters[0];
    fighter.locomotion.as_mut().unwrap().pass_velocity = -20.0;
    fighter.movement.terminal_velocity = 100.0;
    fighter.movement.fast_fall_velocity = 100.0;
    let mut game = Match::new(data, 0).unwrap();
    let landed = until(&mut game, down(), |s| s.fighters[0].ground_line == Some(1));
    assert!(landed.fighters[0].grounded);
    assert!((landed.fighters[0].position[1] - 0.0001).abs() < 0.000001);
    assert_eq!(landed.fighters[0].skip_floor, None);
}

#[test]
fn old_held_down_input_does_not_drop_on_landing_but_a_new_tilt_does() {
    let mut data = data();
    data.stage.spawns[0][1] = 10.0;
    let mut game = Match::new(data, 0).unwrap();
    until(&mut game, down(), |s| s.fighters[0].ground_line == Some(0));
    for _ in 0..10 {
        game.step(down()).unwrap();
    }
    assert_eq!(game.state().fighters[0].ground_line, Some(0));
    assert_eq!(game.state().fighters[0].skip_floor, None);
    game.step(IDLE).unwrap();
    let passing = until(&mut game, down(), |s| s.fighters[0].action == Action::Pass);
    assert_eq!(passing.fighters[0].skip_floor, Some(0));
}

#[test]
fn shielding_drops_immediately_through_a_platform_for_digital_and_analog_shoulders() {
    for shield in [
        Controller {
            buttons: BUTTON_L,
            ..Default::default()
        },
        Controller {
            trigger: 0.5,
            ..Default::default()
        },
    ] {
        let mut game = Match::new(data(), 0).unwrap();
        let mut input = IDLE;
        input[0] = shield;
        assert_eq!(
            game.step(input).unwrap().fighters[0].action,
            Action::GuardOn
        );

        input[0].stick[1] = -1.0;
        let passing = game.step(input).unwrap().fighters[0].clone();
        assert_eq!(passing.action, Action::Pass);
        assert!(!passing.grounded);
        assert_eq!(passing.ground_line, None);
        assert_eq!(passing.skip_floor, Some(0));
        assert!(passing.velocity[1] < 0.0);
        assert_eq!(passing.locomotion.tilt_y_age, 254);
    }
}

/// `fox-bf.slp` (`docs/parity.md`): P4's `GuardOn` converts into `Pass` on
/// its only frame, and no shield regeneration lands on that conversion
/// frame despite the fighter no longer being `GuardOn`/`Guard` by the time
/// the frame's shield-health update runs. `begin_pass`'s own
/// (`ftCo_8009A184`/`ftCo_8009A228`) `Fighter_ChangeMotionState` call
/// clears `x221A_b7` -- the flag `Fighter_ProcessHit_8006D1EC` gates
/// regeneration on (`fighter.c:1048,2821`) -- the same as any other
/// transition out of `GuardOn`/`Guard`. A steady digital press (unaffected
/// by the separate entry-frame-trigger fix covered elsewhere, since the
/// digital override reads the same value every frame either way) isolates
/// this: with this fixture's own rules (`drain_rate` `0.2`, `drain_scales`
/// `[0.5, 1.0]`, `maximum_health` `50.0`, `regeneration` `0.1`), the entry
/// frame itself never runs the shield-owning Anim arm at all (`update_actions`,
/// priority 3, only enters `GuardOn` after priority 1's Anim already ran
/// this frame against the old, non-shielding action), so health stays
/// `50.0`; the next, still-held frame drains against the entry frame's own
/// press (`strength` `1.0`, loss `0.2`, `50.0 -> 49.8`); the conversion
/// frame drains the same way again (`49.8 -> 49.6`) with regeneration
/// correctly withheld -- an unfixed regeneration gate would add `0.1` back,
/// landing on `49.7` instead.
#[test]
fn no_regeneration_lands_on_the_frame_guard_on_converts_into_pass() {
    let mut game = Match::new(data(), 0).unwrap();
    let mut input = IDLE;
    input[0].buttons = BUTTON_L;
    assert_eq!(
        game.step(input).unwrap().fighters[0].action,
        Action::GuardOn
    );
    assert_eq!(game.state().fighters[0].shield.health, 50.0);
    assert_eq!(game.step(input).unwrap().fighters[0].shield.health, 49.8);

    input[0].stick[1] = -1.0;
    let passing = game.step(input).unwrap().fighters[0].clone();
    assert_eq!(passing.action, Action::Pass);
    assert_eq!(passing.shield.health, 49.6);
}

#[test]
fn shield_entry_does_not_chain_a_drop_and_a_solid_floor_refuses_it() {
    let mut combined = IDLE;
    combined[0] = Controller {
        buttons: BUTTON_L,
        stick: [0.0, -1.0],
        ..Default::default()
    };
    let mut game = Match::new(data(), 0).unwrap();
    let entered = game.step(combined).unwrap();
    assert_eq!(entered.fighters[0].action, Action::GuardOn);
    assert!(entered.fighters[0].grounded);

    let mut solid = data();
    solid.stage.geometry.as_mut().unwrap().lines[0].material_flags = 0;
    let mut game = Match::new(solid, 0).unwrap();
    let mut neutral_shield = combined;
    neutral_shield[0].stick[1] = 0.0;
    game.step(neutral_shield).unwrap();
    for _ in 0..3 {
        let state = game.step(combined).unwrap();
        assert!(state.fighters[0].grounded);
        assert_eq!(state.fighters[0].skip_floor, None);
        assert!(matches!(
            state.fighters[0].action,
            Action::GuardOn | Action::Guard
        ));
    }
}

#[test]
fn checkpoint_restores_skip_and_input_history_during_the_drop() {
    let mut game = Match::new(data(), 13).unwrap();
    until(&mut game, down(), |s| s.fighters[0].action == Action::Pass);
    let checkpoint = game.checkpoint();
    let before = serde_json::to_vec(game.state()).unwrap();
    let expected: Vec<_> = (0..16)
        .map(|_| serde_json::to_vec(game.step(down()).unwrap()).unwrap())
        .collect();
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(serde_json::to_vec(game.state()).unwrap(), before);
    assert_eq!(game.state().fighters[0].skip_floor, Some(0));
    for state in expected {
        assert_eq!(
            serde_json::to_vec(game.step(down()).unwrap()).unwrap(),
            state
        );
    }
    assert_eq!(game.state().fighters[0].ground_line, Some(1));
}

#[test]
fn pass_animation_end_clears_the_source_line_skip_before_landing() {
    let mut data = data();
    data.fighters[0]
        .locomotion
        .as_mut()
        .unwrap()
        .pass_animation_frames = 2;
    data.stage.geometry.as_mut().unwrap().lines[1].start[1] = -30.0;
    data.stage.geometry.as_mut().unwrap().lines[1].end[1] = -30.0;
    let mut game = Match::new(data, 0).unwrap();
    until(&mut game, down(), |s| s.fighters[0].action == Action::Pass);
    let falling = until(&mut game, IDLE, |s| s.fighters[0].action == Action::Fall);
    assert!(!falling.fighters[0].grounded && falling.fighters[0].position[1] < 6.0);
    assert_eq!(falling.fighters[0].skip_floor, None);
}

#[test]
fn being_hit_during_pass_does_not_freeze_the_ecb_map_countdown() {
    let mut data = data();
    data.stage.spawns[1][0] = 2.5;
    data.fighters[1].jab.frames = vec![data.fighters[1].jab.frames[1].clone()];
    let mut game = Match::new(data, 0).unwrap();
    until(&mut game, down(), |s| s.fighters[0].action == Action::Pass);
    let mut input = IDLE;
    input[1].buttons = BUTTON_A;
    let hit = game.step(input).unwrap().fighters[0].clone();
    assert_eq!(hit.action, Action::Damage);
    assert!(hit.hitlag > 1.0 && hit.ecb_lock > 1 && hit.ecb.bottom_locked);
    assert_eq!(
        hit.skip_floor, None,
        "Damage clears the previous action's skip"
    );
    let frozen = &game.step(IDLE).unwrap().fighters[0];
    assert!(frozen.hitlag > 0.0);
    assert_eq!(frozen.position, hit.position);
    // Fighter_procMap's lock decrement is outside the hitlag guard. The frozen
    // collision pose does not freeze this separate map-callback counter.
    assert_eq!(frozen.ecb_lock, hit.ecb_lock - 1);
}
