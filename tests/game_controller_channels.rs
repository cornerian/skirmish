//! Processed controller channels reach the real match and checkpoint state.
use skirmish::game::{
    BUTTON_A, BUTTON_L, BUTTON_R, Controller, Match, damage::HitlagDisplacementRules,
    data::MatchData,
};

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.damage.displacement = Some(HitlagDisplacementRules {
        axis_thresholds: [0.5; 2],
        minimum_stick_magnitude: 0.5,
        sdi_window: 3,
        sdi_distance: 2.0,
        asdi_distance: 0.75,
    });
    data
}

#[test]
fn cstick_asdi_has_inclusive_priority_but_does_not_change_main_stick_di() {
    let idle = [Controller::default(); 2];
    let mut base = Match::new(data(), 0).unwrap();
    let mut attack = idle;
    attack[0].buttons = BUTTON_A;
    base.step(attack).unwrap();
    base.step(idle).unwrap();
    let position = base.state().fighters[1].position;
    let mut results = Vec::new();
    for cstick in [[0.5_f32.next_down(), 0.0], [0.5, 0.0], [-1.0, 0.0]] {
        let mut game = base.clone();
        let mut input = idle;
        input[1].stick = [0.0, 0.25];
        input[1].cstick = cstick;
        while game.state().fighters[1].hitlag > 1.0 {
            assert_eq!(
                game.step(input).unwrap().fighters[1].position,
                position,
                "C-stick does not cause SDI on intermediate hitlag ticks"
            );
        }
        let checkpoint = game.checkpoint();
        let after = game.step(input).unwrap().clone();
        game.restore_checkpoint(&checkpoint).unwrap();
        assert_eq!(game.step(input).unwrap(), &after);
        results.push(after.fighters[1].clone());
    }
    assert_eq!(
        results[0].position, position,
        "both sticks are below the ASDI magnitude threshold"
    );
    assert_eq!(results[1].position, [position[0] + 0.375, position[1]]);
    assert_eq!(results[2].position, [position[0] - 0.75, position[1]]);
    assert_eq!(
        results[0].knockback.map(f32::to_bits),
        results[1].knockback.map(f32::to_bits)
    );
    assert_eq!(
        results[1].knockback.map(f32::to_bits),
        results[2].knockback.map(f32::to_bits)
    );
}

#[test]
fn digital_trigger_override_preserves_analog_channel_and_prior_input_bits() {
    let mut game = Match::new(data(), 0).unwrap();
    for buttons in [0, BUTTON_L, BUTTON_R, BUTTON_L | BUTTON_R] {
        let input = Controller {
            buttons,
            trigger: 0.25,
            cstick: [-0.0, 0.5],
            ..Default::default()
        };
        assert_eq!(
            input.shield_pressure(),
            if buttons == 0 { 0.25 } else { 1.0 }
        );
        let recorded =
            game.step([input, Controller::default()]).unwrap().fighters[0].previous_input;
        assert_eq!(recorded.trigger.to_bits(), 0.25_f32.to_bits());
        assert_eq!(
            recorded.cstick.map(f32::to_bits),
            [(-0.0_f32).to_bits(), 0.5_f32.to_bits()]
        );
        assert_eq!(recorded.buttons, buttons);
    }
}

#[test]
fn invalid_new_channels_leave_match_and_input_history_unchanged() {
    let mut game = Match::new(data(), 7).unwrap();
    let before = serde_json::to_vec(game.state()).unwrap();
    for input in [
        Controller {
            cstick: [f32::NAN, 0.0],
            ..Default::default()
        },
        Controller {
            cstick: [0.0, 1.01],
            ..Default::default()
        },
        Controller {
            trigger: f32::INFINITY,
            ..Default::default()
        },
        Controller {
            trigger: -0.01,
            ..Default::default()
        },
        Controller {
            trigger: 1.01,
            ..Default::default()
        },
    ] {
        assert!(game.step([input, Controller::default()]).is_err());
        assert_eq!(serde_json::to_vec(game.state()).unwrap(), before);
    }
}
