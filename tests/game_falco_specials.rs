//! Falco's registration as a playable character
//! (`game::script::resources::Specials` with the Falco character key): a fighter
//! carrying that resource
//! loads into a real `Match`, dispatches to the exact same side/up/down
//! special-move code Fox's own variant uses (see that enum's own doc for
//! why the decomp makes this a data-only difference: `ftFc_Init_
//! MotionStateTable`, `ftfalco.c:23-370`, points every one of Falco's
//! `ftFx_MS_Special*` entries at the identical Fox callbacks), and resolves
//! its own external Slippi character id (20) through
//! `game::script::definition::builtin_slippi_ids`.
//!
//! This intentionally reuses the same invented numeric fixtures the Fox
//! wiring tests already use (`fox-side-special.json`/`fox-up-special.json`/
//! `fox-down-special.json`): the point here is that the `Falco` variant
//! dispatches to the shared move code at all, not a second bit-exact
//! numeric check of Falco's own attributes (that is `real_parity_falco_
//! fox_fd.rs`'s job, against the real exported pack and a real recording).
//!
//! `neutral` is Falco's own Laser, wired for the first time this batch
//! (`docs/falco.md`): `fighters/falco.json` (gameplay export v10) carries
//! his own `specials.neutral`, exporter-confirmed with real, and in one
//! respect genuinely different, attributes/hitbox data from Fox's own (see
//! `neutral` resource in `scripts/fighters/fox.luau`).
//! `tests/game_falco_neutral_special.rs` covers that move end to end; this
//! file only confirms the wiring reaches it, matching this file's own
//! existing side/up/down coverage style.

#[path = "support/conformance.rs"]
mod conformance;

use skirmish::fighter::escape_air::{Parameters as EscapeAirParameters, Rules as EscapeAirRules};
use skirmish::game::{
    Action, BUTTON_B, Controller, Match,
    data::MatchData,
    script::resources::{Resources, Specials},
};

#[derive(serde::Deserialize)]
struct SideFixture {
    rules: skirmish::fighter::specials::Rules,
    parameters: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct UpFixture {
    parameters: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct DownFixture {
    parameters: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct EscapeAirFixture {
    rules: EscapeAirRules,
    parameters: EscapeAirParameters,
}

#[derive(serde::Deserialize)]
struct NeutralFixture {
    parameters: serde_json::Value,
}

/// Both fighters play `Specials` with the Falco character key, with all four moves now that
/// Falco's own Laser is wired (`fighters/falco.json`'s own `specials.
/// neutral`, gameplay export v10; see this file's own doc comment).
fn data() -> MatchData {
    let mut data = conformance::data();
    let escape_air: EscapeAirFixture =
        serde_json::from_str(include_str!("fixtures/game/escape-air.json")).unwrap();
    let side: SideFixture =
        serde_json::from_str(include_str!("fixtures/game/fox-side-special.json")).unwrap();
    let up: UpFixture =
        serde_json::from_str(include_str!("fixtures/game/fox-up-special.json")).unwrap();
    let down: DownFixture =
        serde_json::from_str(include_str!("fixtures/game/fox-down-special.json")).unwrap();
    let neutral: NeutralFixture =
        serde_json::from_str(include_str!("fixtures/game/falco-neutral-special.json")).unwrap();
    data.rules.escape_air = Some(escape_air.rules);
    data.rules.specials = Some(side.rules);
    for fighter in &mut data.fighters {
        fighter.escape_air = Some(escape_air.parameters.clone());
        let mut values = std::collections::BTreeMap::new();
        values.insert("neutral".into(), neutral.parameters.clone());
        values.insert("side".into(), side.parameters.clone());
        values.insert("up".into(), up.parameters.clone());
        values.insert("down".into(), down.parameters.clone());
        fighter.specials = Some(Specials {
            character: "falco".into(),
            special_attributes: None,
            animations: None,
            articles: None,
            resources: Resources::new(values).unwrap(),
        });
    }
    data
}

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn input(player: usize, controller: Controller) -> [Controller; 2] {
    let mut inputs = IDLE;
    inputs[player] = controller;
    inputs
}

fn side(stick_x: f32) -> Controller {
    Controller {
        buttons: BUTTON_B,
        stick: [stick_x, 0.0],
        ..Controller::default()
    }
}

fn vertical(stick_y: f32) -> Controller {
    Controller {
        buttons: BUTTON_B,
        stick: [0.0, stick_y],
        ..Controller::default()
    }
}

#[test]
fn a_falco_fighter_loads_into_a_real_match() {
    let game = Match::new(data(), 0).unwrap();
    assert_eq!(game.state().fighters[0].action, Action::Wait);
}

#[test]
fn falco_plays_the_shared_side_special() {
    let mut game = Match::new(data(), 0).unwrap();
    let state = game.step(input(0, side(0.6))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialSStart);
}

#[test]
fn falco_plays_the_shared_up_special() {
    let mut game = Match::new(data(), 0).unwrap();
    let state = game.step(input(0, vertical(0.9))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialHiHold);
}

#[test]
fn falco_plays_the_shared_down_special() {
    let mut game = Match::new(data(), 0).unwrap();
    let state = game.step(input(0, vertical(-0.8))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialLwStart);
}

#[test]
fn falco_plays_his_own_neutral_special() {
    let game = Match::new(data(), 0).unwrap();
    assert!(
        game.data().fighters[0]
            .specials
            .as_ref()
            .unwrap()
            .lookup("neutral")
            .is_some()
    );
    let mut game = Match::new(data(), 0).unwrap();
    // A centered-stick B press (`vertical(0.0)`: `BUTTON_B`, stick `[0.0,
    // 0.0]`), matching this file's own `side`/`vertical` helper style; the
    // side/up/down tests above each bias the stick toward their own move,
    // so a plain neutral press is the one this file does not already have
    // a named helper for.
    let state = game.step(input(0, vertical(0.0))).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);
}

/// External Slippi character ids (`crates/cli/src/initialization.rs`'s
/// `CHARACTER_EXTERNAL_IDS`): Fox is 2, Falco is 20, and both resolve
/// through the same table since Falco's own `ftFc_Init_MotionStateTable`
/// gives it the identical `ftFx_MS_Special*` state ids Fox's own table
/// uses (`game::script::definition::builtin_slippi_ids`'s own docs). Falco's
/// *internal* fighter kind,
/// `FTKIND_FALCO` (`ft/forward.h:112`), is a different number (22) from his
/// external CSS id and must not be confused with it.
#[test]
fn falco_and_fox_resolve_the_same_slippi_special_ids() {
    for action in [
        Action::SpecialNStart,
        Action::SpecialAirNStart,
        Action::SpecialSStart,
        Action::SpecialHiHold,
        Action::SpecialLwStart,
    ] {
        let fox_ids = skirmish::game::script::definition::builtin_slippi_ids(Some(2), action);
        let falco_ids = skirmish::game::script::definition::builtin_slippi_ids(Some(20), action);
        assert!(fox_ids.is_some());
        assert_eq!(fox_ids, falco_ids);
    }
    // An unregistered external id (Dr. Mario, 22 -- not to be confused with
    // Falco's own internal kind, also numbered 22) still resolves nothing.
    assert_eq!(
        skirmish::game::script::definition::builtin_slippi_ids(Some(22), Action::SpecialNStart),
        None
    );
}
