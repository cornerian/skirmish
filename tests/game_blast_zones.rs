//! Ordinary ftCo_800D3158 eligibility, exercised through native match inputs.
//! The synthetic rules do not model scripted death exemptions or star/screen
//! selection. No original executable or extracted resources are needed.
use skirmish::game::{BUTTON_A, BUTTON_X, Controller, Event, Match, data::MatchData};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
}; 2];

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.top_ko_min_knockback = Some(0.5);
    data.stage.blast[3] = 5.0;
    data
}

#[test]
fn self_velocity_can_cross_top_and_return_without_losing_a_stock() {
    let mut game = Match::new(data(), 7).unwrap();
    let mut input = IDLE;
    input[0].buttons = BUTTON_X;
    let mut crossed = false;
    let mut returned = false;
    for _ in 0..40 {
        let state = game.step(input).unwrap();
        let fighter = &state.fighters[0];
        crossed |= fighter.position[1] > 5.0;
        returned |= crossed && fighter.grounded;
        assert_eq!(fighter.stocks, 2);
        assert_eq!(fighter.knockback, [0.0; 2]);
        assert!(
            !state
                .events
                .iter()
                .any(|event| matches!(event, Event::Knockout { .. }))
        );
    }
    assert!(crossed && returned);
}

#[test]
fn upward_knockback_threshold_and_top_boundary_are_both_strict() {
    // Zero growth and base=2 produce exactly 2 units of upward knockback.
    // Account for the source floor projection's 0.0001 separation bias.
    let heights = [
        0.0001_f32 + 2.0,
        (0.0001_f32 + 2.0) + 2.0,
        ((0.0001_f32 + 2.0) + 2.0) + 2.0,
    ];
    for (minimum, loses_stock) in [
        (2.0_f32.next_down(), true),
        (2.0, false),
        (2.0_f32.next_up(), false),
    ] {
        let mut data = data();
        data.stage.blast[3] = heights[1];
        data.rules.top_ko_min_knockback = Some(minimum);
        data.rules.knockback_decay = 0.0;
        data.rules.knockback_speed = 1.0;
        data.rules.hitlag.base = 0.0;
        data.rules.hitlag.damage_scale = 0.0;
        for fighter in &mut data.fighters {
            fighter.movement.gravity = 0.0;
        }
        for frame in &mut data.fighters[0].jab.frames {
            for hit in &mut frame.hitboxes {
                hit.angle_degrees = 90.0;
                hit.growth = 0;
                hit.base = 2;
            }
        }
        let mut game = Match::new(data, 3).unwrap();
        let mut attack = IDLE;
        attack[0].buttons = BUTTON_A;
        game.step(attack).unwrap();
        game.step(IDLE).unwrap();
        assert_eq!(game.state().fighters[1].knockback[1], 2.0);
        let checkpoint = game.checkpoint();
        for y in &heights[..2] {
            let fighter = &game.step(IDLE).unwrap().fighters[1];
            assert_eq!(fighter.position[1], *y);
            assert_eq!(fighter.stocks, 2, "a strict top crossing is required");
        }
        let expected = game.step(IDLE).unwrap().clone();
        assert_eq!(expected.fighters[1].position[1], heights[2]);
        assert_eq!(expected.fighters[1].stocks, if loses_stock { 1 } else { 2 });
        assert_eq!(
            expected.events.contains(&Event::Knockout {
                player: 1,
                stocks: 1
            }),
            loses_stock
        );
        game.restore_checkpoint(&checkpoint).unwrap();
        for _ in 0..3 {
            game.step(IDLE).unwrap();
        }
        assert_eq!(game.state(), &expected);
    }
}

#[test]
fn top_eligibility_does_not_protect_side_or_bottom_crossings() {
    for bottom_crossing in [false, true] {
        let mut data = data();
        data.stage.floor.left = -5.0;
        data.stage.floor.right = 5.0;
        data.stage.spawns = [[4.0, 0.0], [-4.0, 0.0]];
        data.stage.blast[1] = if bottom_crossing { 200.0 } else { 6.0 };
        data.rules.top_ko_min_knockback = Some(1000.0);
        let mut game = Match::new(data, 0).unwrap();
        let mut input = IDLE;
        input[0].stick[0] = 1.0;
        let mut knocked_out = false;
        for _ in 0..40 {
            let state = game.step(input).unwrap();
            if state.events.contains(&Event::Knockout {
                player: 0,
                stocks: 1,
            }) {
                let fighter = &state.fighters[0];
                assert!(if bottom_crossing {
                    fighter.position[1] < -8.0
                } else {
                    fighter.position[0] > 6.0
                });
                knocked_out = true;
                break;
            }
        }
        assert!(knocked_out);
    }
}

#[test]
fn invalid_top_threshold_is_rejected_before_a_match_starts() {
    for minimum in [-1.0, f32::NAN, f32::INFINITY] {
        let mut data = data();
        data.rules.top_ko_min_knockback = Some(minimum);
        assert!(Match::new(data, 0).is_err());
    }
}

#[test]
fn grounded_fighter_crossing_top_does_not_require_knockback() {
    use skirmish::collision::stage;
    use skirmish::game::data::StageGeometry;
    let mut data = data();
    data.stage.spawns = [[-2.0, 0.0]; 2];
    data.rules.top_ko_min_knockback = Some(1000.0);
    data.stage.geometry = Some(StageGeometry {
        lines: vec![stage::Line {
            start: [-2.0, 0.0],
            end: [8.0, 10.0],
            flags: stage::FLOOR | stage::ENABLED,
            ..Default::default()
        }],
        joints: vec![stage::Joint {
            flags: stage::ENABLED,
            bounds_min: [-2.0, 0.0],
            bounds_max: [8.0, 10.0],
            floor: 0..1,
            ..Default::default()
        }],
    });
    let mut game = Match::new(data, 0).unwrap();
    let mut input = IDLE;
    input[0].stick[0] = 0.5;
    for _ in 0..40 {
        let state = game.step(input).unwrap();
        let fighter = &state.fighters[0];
        assert_eq!(fighter.knockback, [0.0; 2]);
        if state.events.contains(&Event::Knockout {
            player: 0,
            stocks: 1,
        }) {
            assert!(fighter.grounded && fighter.position[1] > 5.0);
            return;
        }
    }
    panic!("grounded traversal never crossed the upper boundary");
}
