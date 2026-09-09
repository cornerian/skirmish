//! The shared source input timers advance once per frame, including hitlag.
use skirmish::game::{
    Action, BUTTON_A, Controller, Match, damage::HitlagDisplacementRules, data::MatchData,
};

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let parameters = serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap();
    for fighter in &mut data.fighters {
        fighter.locomotion = Some(parameters);
    }
    data.rules.countdown_frames = 0;
    data.rules.hitlag.base = 6.0;
    data.rules.hitlag.damage_scale = 0.0;
    data.rules.knockback_speed = 0.0;
    data.rules.hitstun_scale = 0.0;
    data
}

#[test]
fn input_held_through_hitlag_ages_once_per_frame_and_cannot_become_a_fresh_jump() {
    let idle = [Controller::default(); 2];
    let mut game = Match::new(data(), 0).unwrap();
    let mut attack = idle;
    attack[0].buttons = BUTTON_A;
    game.step(attack).unwrap();
    game.step(idle).unwrap();
    assert_eq!(game.state().fighters[1].hitlag, 6.0);
    assert_eq!(game.state().fighters[1].locomotion.tilt_y_age, 254);
    let mut held = idle;
    held[1].stick[1] = 1.0;
    for age in 0..6 {
        let fighter = &game.step(held).unwrap().fighters[1];
        assert_eq!(fighter.locomotion.tilt_y_age, age);
        assert_eq!(fighter.hitlag, f32::from(5 - age));
    }
    let checkpoint = game.checkpoint();
    for age in 6..10 {
        let fighter = &game.step(held).unwrap().fighters[1];
        assert_eq!(fighter.locomotion.tilt_y_age, age);
        assert!(!matches!(
            fighter.action,
            Action::JumpSquat | Action::Jump | Action::JumpAerial
        ));
    }
    assert_eq!(game.state().fighters[1].action, Action::Wait);
    let expected = game.state().clone();
    game.restore_checkpoint(&checkpoint).unwrap();
    for _ in 0..4 {
        game.step(held).unwrap();
    }
    assert_eq!(game.state(), &expected);
    game.step(idle).unwrap();
    assert_eq!(
        game.step(held).unwrap().fighters[1].action,
        Action::JumpSquat
    );
}

#[test]
fn conflicting_thresholds_for_the_same_input_history_are_rejected() {
    let mut data = data();
    data.rules.damage.displacement = Some(HitlagDisplacementRules {
        axis_thresholds: [0.3, 0.5],
        minimum_stick_magnitude: 0.5,
        sdi_window: 3,
        sdi_distance: 2.0,
        asdi_distance: 0.75,
    });
    let error = Match::new(data.clone(), 0).unwrap_err();
    assert!(error.to_string().contains("shared stick-age thresholds"));
    data.rules
        .damage
        .displacement
        .as_mut()
        .unwrap()
        .axis_thresholds = [0.3; 2];
    assert!(Match::new(data, 0).is_ok());
}
