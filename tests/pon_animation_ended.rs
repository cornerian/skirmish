//! A complete source-defined action reaches the real special animation path.

#[path = "support/conformance.rs"]
mod conformance;

use skirmish::game::script::resources::Specials;
use skirmish::game::{BUTTON_B, Controller, Match, script::Program};

const SOURCE: &str = r#"
from skirmish import (ActionState, Button, Fighter, SpecialMove, SpecialMoves,
    action, custom_action, hook, register)

class State(ActionState):
    animation_ends: int = 0

class SpinningKong(SpecialMove):
    phase = action(custom_action("dk-test", "special_hi"), slippi_state=381)
    ground = phase
    air = phase

    @hook.input_pressed(Button.B)
    def start(self, fighter, ctx):
        fighter.change_action(self.phase)
        return True

    @hook.animation_end(phase)
    def end(self, fighter, ctx):
        fighter.action_state.animation_ends += 1

spinning_kong = SpinningKong()

@register
class TestFighter(Fighter):
    name = "test_fighter"
    action_state = State
    specials = SpecialMoves(
        Fighter.specials.neutral,
        Fighter.specials.side,
        spinning_kong,
        Fighter.specials.down,
    )
"#;

fn data() -> skirmish::game::data::MatchData {
    let mut data = conformance::data();
    let attack = data.fighters[0].jab.clone();
    let animation = serde_json::to_value(attack).unwrap();
    let specials: Specials = serde_json::from_value(serde_json::json!({
        "character": "test_fighter",
        "animations": {
            "331": {
                "animation_id": 331,
                "state_ids": [381],
                "status": "complete",
                "resource": animation
            }
        }
    }))
    .unwrap();
    for fighter in &mut data.fighters {
        fighter.script = Some(Program::new(SOURCE).unwrap());
        fighter.specials = Some(specials.clone());
        fighter.motion_states = Some(vec![skirmish::game::data::MotionStateProfile {
            state_id: 381,
            animation_id: 331,
            move_id: 20,
            flags: 0,
        }]);
    }
    data
}

#[test]
fn custom_dk_action_delivers_one_animation_end() {
    let mut game = Match::new(data(), 0).expect("custom action fixture registers");
    game.step([
        Controller {
            buttons: BUTTON_B,
            ..Default::default()
        },
        Controller::default(),
    ])
    .expect("special input enters custom action");

    for _ in 0..6 {
        game.step([Controller::default(); 2])
            .expect("custom action advances");
    }

    let state = game.state().fighters[0]
        .action_state
        .get("animation_ends")
        .expect("action-state counter");
    assert_eq!(state, &skirmish::game::script::LocalValue::Integer(1));
}
