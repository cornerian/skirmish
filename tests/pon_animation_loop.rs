//! Generic class-defined action animation samples wrap independently of the
//! native action frame clock.

#[path = "support/conformance.rs"]
mod conformance;

use skirmish::game::script::resources::{Resources, Specials};
use skirmish::game::{Action, BUTTON_B, Controller, Match, script::Program};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, Button, DefenseMoves, Fighter,
    GetupMoves, GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves,
    SpecialMoves, SpecialMove, TauntMoves, ThrowMoves, TiltMoves, action, hook,
    register)

class Loop(SpecialMove):
    resource = "neutral"
    start = action("special_n_start", attack="neutral", animation_loop=True)
    @hook.input_pressed(Button.B)
    def press(self, fighter, ctx):
        fighter.change_action(self.start)
        return True

loop = Loop()
ordinary = Move()

@register
class TestFighter(Fighter):
    name = "test_fighter"
    attributes = Attributes
    specials = SpecialMoves(loop, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary)
    smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary)
    throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

fn data(looping: bool) -> skirmish::game::data::MatchData {
    let mut data = conformance::data();
    let mut attack = data.fighters[0].jab.clone();
    attack.frames.truncate(2);
    assert_eq!(attack.frames.len(), 2);
    let hitbox = |y| {
        serde_json::from_value(serde_json::json!({
            "group": 0, "bone": 0, "center": [0.0, y, 0.0], "radius": 1.0,
            "damage": 1, "angle_degrees": 45.0, "growth": 1, "fixed": 1, "base": 1
        }))
        .unwrap()
    };
    attack.frames[0].hitboxes = vec![hitbox(1.0)];
    attack.frames[1].hitboxes = vec![hitbox(9.0)];
    let resources = Resources::new(std::collections::BTreeMap::from([(
        "neutral".to_owned(),
        serde_json::to_value(attack).unwrap(),
    )]))
    .unwrap();
    for fighter in &mut data.fighters {
        fighter.script = Some(
            Program::new(if looping {
                SOURCE.to_owned()
            } else {
                SOURCE.replace("animation_loop=True", "animation_loop=False")
            })
            .unwrap(),
        );
        fighter.specials = Some(Specials {
            character: "test_fighter".into(),
            special_attributes: None,
            animations: None,
            articles: None,
            resources: resources.clone(),
        });
    }
    data
}

#[test]
fn declared_loop_wraps_pose_and_hitbox_samples_while_action_frame_advances() {
    let mut game = Match::new(data(true), 0).expect("loop fixture registers");
    let press = [
        Controller {
            buttons: BUTTON_B,
            ..Default::default()
        },
        Controller::default(),
    ];
    let state = game.step(press).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);
    let first = state.fighters[0].hitboxes[0].current[1];
    assert!((first - 1.0).abs() < 2e-3, "first sampled hitbox: {first}");
    let state = game.step([Controller::default(); 2]).unwrap();
    let second = state.fighters[0].hitboxes[0].current[1];
    assert!(
        (second - 9.0).abs() < 2e-3,
        "second sampled hitbox: {second}"
    );
    let state = game.step([Controller::default(); 2]).unwrap();
    let third = state.fighters[0].hitboxes[0].current[1];
    assert!((third - 1.0).abs() < 2e-3, "third sampled hitbox: {third}");
    for _ in 0..2 {
        game.step([Controller::default(); 2]).unwrap();
    }
    let state = game.state();
    assert_eq!(state.fighters[0].action_frame, 5);
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);
    let wrapped = state.fighters[0].hitboxes[0].current[1];
    assert!(
        (wrapped - 1.0).abs() < 2e-3,
        "wrapped sampled hitbox: {wrapped}"
    );
}

#[test]
fn special_effect_ownership_is_monotonic_and_checkpoint_safe() {
    let source = SOURCE.replace(
        "fighter.change_action(self.start)\n        return True",
        "fighter.spawn_special_effect(\"neutral\", 0)\n        fighter.clear_special_effect()\n        fighter.spawn_special_effect(\"neutral\", 0)\n        return True",
    );
    let mut data = data(true);
    data.fighters[0].script = Some(Program::new(source).unwrap());
    let mut game = Match::new(data, 0).expect("effect fixture registers");
    let checkpoint = game.checkpoint();
    let state = game
        .step([
            Controller {
                buttons: BUTTON_B,
                ..Default::default()
            },
            Controller::default(),
        ])
        .unwrap();
    assert_eq!(state.fighters[0].effects.next_id, 3);
    assert_eq!(state.fighters[0].effects.owned.len(), 1);
    assert_eq!(state.fighters[0].effects.owned[0].id, 2);
    let expected = serde_json::to_vec(state).unwrap();
    game.restore_checkpoint(&checkpoint).unwrap();
    let replay = game
        .step([
            Controller {
                buttons: BUTTON_B,
                ..Default::default()
            },
            Controller::default(),
        ])
        .unwrap();
    assert_eq!(expected, serde_json::to_vec(replay).unwrap());
}

#[test]
fn native_effect_failure_rolls_back_owned_resources() {
    let source = SOURCE.replace(
        "fighter.change_action(self.start)\n        return True",
        "fighter.spawn_special_effect(\"neutral\", 0)\n        fighter.spawn_special_effect(\"neutral\", 9999)\n        return True",
    );
    let mut data = data(true);
    data.fighters[0].script = Some(Program::new(source).unwrap());
    let mut game = Match::new(data, 0).expect("effect failure fixture registers");
    let error = game
        .step([
            Controller {
                buttons: BUTTON_B,
                ..Default::default()
            },
            Controller::default(),
        ])
        .expect_err("out-of-range effect bone must fail");
    assert!(error.to_string().contains("outside the loaded skeleton"));
    assert!(game.state().fighters[0].effects.owned.is_empty());
    assert_eq!(game.state().fighters[0].effects.next_id, 1);
}

#[test]
fn undeclared_finite_action_keeps_out_of_range_error_behavior() {
    let mut game = Match::new(data(false), 0).expect("finite fixture registers");
    let press = [
        Controller {
            buttons: BUTTON_B,
            ..Default::default()
        },
        Controller::default(),
    ];
    game.step(press).unwrap();
    game.step([Controller::default(); 2]).unwrap();
    let error = game
        .step([Controller::default(); 2])
        .expect_err("finite sample must end");
    assert!(error.to_string().contains("outside the supplied animation"));
}
