use super::{BUTTON_A, Controller, Match, data::MatchData, script::Program};
use crate::fighter::jab::{Attributes, FrameFlags, JabAttack, Parameters, Script};

const SOURCE: &str = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, action, register)

class Actionless(Move):
    async def run(self, action):
        await action.wait(1)

class Canonical(Move):
    action = "jab"
    async def run(self, action):
        await action.wait(1)

actionless = Actionless()
canonical = Canonical()
ordinary = Move()

@register
class LifetimeExhaustionFighter(Fighter):
    name = "move_lifetime_exhaustion_test"
    attributes = Attributes
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(actionless, canonical, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary)
    smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary)
    throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;

fn setup(source: &str) -> Match {
    let mut data: MatchData = serde_json::from_str(include_str!(
        "../../tests/fixtures/game/integration-match.json"
    ))
    .expect("integration fixture decodes");
    for fighter in &mut data.fighters {
        data_jab_combo(fighter);
    }
    data.rules.countdown_frames = 0;
    data.fighters[0].script = Some(Program::new(source).expect("lifetime fixture compiles"));
    for fighter in &mut data.fighters {
        fighter.locomotion = Some(
            serde_json::from_str(include_str!("../../tests/fixtures/game/locomotion.json"))
                .expect("locomotion fixture decodes"),
        );
    }
    Match::new(data, 0).expect("integration match resources load")
}

fn data_jab_combo(fighter: &mut super::data::FighterData) {
    let second_frames = fighter.jab.frames.len();
    let second = JabAttack {
        attack: fighter.jab.clone(),
        script: Script {
            flags: vec![FrameFlags::default(); second_frames],
            root_translations: None,
        },
    };
    fighter.jab_combo = Some(Parameters {
        attributes: Attributes {
            second_window: 6.0,
            third_window: 6.0,
            rapid_window: 3,
        },
        first: Script {
            flags: vec![
                FrameFlags {
                    allow_interrupt: true,
                    ..Default::default()
                };
                fighter.jab.frames.len()
            ],
            root_translations: None,
        },
        second: Some(second),
        third: None,
        rapid: None,
    });
}

fn assert_exhausted(source: &str) {
    let mut game = setup(source);
    assert!(
        game.state().fighters[0].grounded,
        "fixture must spawn grounded"
    );
    game.state.fighters[0].script_events.next_move_lifetime = u64::MAX;
    let before = serde_json::to_vec(game.state()).expect("state serializes");
    let error = match game.step([
        Controller {
            buttons: BUTTON_A,
            ..Default::default()
        },
        Controller::default(),
    ]) {
        Err(error) => error,
        Ok(_) => panic!("exhausted move lifetime unexpectedly committed a step"),
    };
    assert!(
        error.to_string().contains("native move lifetime exhausted"),
        "unexpected error: {error}"
    );
    assert_eq!(
        serde_json::to_vec(game.state()).expect("state serializes"),
        before,
        "failed step must leave committed state unchanged"
    );
}

#[test]
fn exhausted_actionless_authored_move_rejects_step_without_commit() {
    assert_exhausted(SOURCE);
}

#[test]
fn exhausted_canonical_authored_move_rejects_step_without_commit() {
    let source = SOURCE.replace(
        "GroundedMoves(actionless, canonical, ordinary)",
        "GroundedMoves(canonical, actionless, ordinary)",
    );
    assert_exhausted(&source);
}
