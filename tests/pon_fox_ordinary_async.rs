#![cfg(feature = "experimental-continuations")]

//! Acceptance coverage for an authored Fox ordinary move at the Match boundary.
//!
//! This uses the checked-in class based Fox module, rather than a synthetic
//! Move implementation, so the native action/resource linker is exercised by
//! a real aerial selection.

#[path = "support/aerial.rs"]
mod aerial;

use skirmish::game::{
    Action, BUTTON_A, Controller, Match,
    data::Hitbox,
    script::definition::{AssetStore, Definition},
};

const FOX: &str = include_str!("../scripts/fighters/fox.py");

fn input() -> [Controller; 2] {
    [
        Controller {
            buttons: BUTTON_A,
            ..Default::default()
        },
        Controller::default(),
    ]
}

#[test]
fn authored_fox_ordinary_aerial_runs_and_checkpoint_replays() {
    let mut data = aerial::data();
    // The shared aerial fixture intentionally has empty hitboxes. Add one
    // local, synthetic frame so this acceptance observes native hitbox track
    // creation and expiry without claiming authentic Fox DAT numerics.
    let hitbox = Hitbox {
        clank: false,
        rebound: false,
        element: Default::default(),
        group: 7,
        bone: 1,
        center: [0.0; 3],
        radius: 0.25,
        damage: 9,
        shield_damage: 0,
        angle_degrees: 30.0,
        growth: 50,
        fixed: 0,
        base: 30,
    };
    let aerial = &mut data.fighters[0].aerials.as_mut().unwrap().moves[0];
    aerial.attack.frames[0].hitboxes = vec![hitbox];
    let definition = Definition::load_registered(FOX, &AssetStore::builtins())
        .expect("authored Fox and its move modules compile");
    let program = (*definition.program).clone();
    let expected_owner = program
        .moves()
        .resolve("aerials", "neutral")
        .expect("Fox aerials.neutral owner is exported");
    let callback_keys = program
        .compiled()
        .expect("authored Fox Pon program is linked")
        .callback_keys()
        .expect("authored Fox callback table exports");
    data.fighters[0].script = Some(program);
    let mut game = Match::new(data, 0).expect("Fox aerial match loads");

    let entered = game.step(input()).expect("Fox ordinary aerial executes");
    assert_eq!(entered.fighters[0].action, Action::AttackAirN);
    let selected = entered.fighters[0]
        .script_events
        .active_move
        .expect("Fox aerial records selected move owner");
    assert_eq!(selected.behavior_index, expected_owner.behavior_index);
    assert_eq!(selected.action, Action::AttackAirN);
    assert!(!callback_keys.contains(&format!("move_{}.run", selected.behavior_index)));
    assert!(entered.fighters[0].script_events.pending_move.is_none());
    assert!(
        entered.fighters[0]
            .script_events
            .pending_move_timer
            .is_none()
    );
    assert!(entered.fighters[0].aerial.landing_lag_enabled);
    assert_eq!(entered.fighters[0].action_frame, 1);
    assert_eq!(entered.fighters[0].hitboxes[0].group, Some(7));
    assert_eq!(entered.fighters[0].hitboxes[0].radius, 0.25);

    let checkpoint = game.checkpoint();
    let expected = serde_json::to_vec(
        game.step([Controller::default(); 2])
            .expect("Fox aerial advances after run"),
    )
    .expect("serialize expected state");
    assert_eq!(game.state().fighters[0].hitboxes[0].group, None);

    game.restore_checkpoint(&checkpoint)
        .expect("restore Fox aerial checkpoint");
    let replayed = serde_json::to_vec(
        game.step([Controller::default(); 2])
            .expect("replay Fox aerial after run"),
    )
    .expect("serialize replayed state");
    assert_eq!(expected, replayed);
}
