//! Luau fighter hook contract tests.
//!
//! These exercise the public script boundary directly. Match-level tests use
//! the same programs once the simulation wires dispatch into collision.
use skirmish::game::script::{Command, Error, FighterView, HitView, Hook, LocalValue, Program};
use skirmish::game::{Controller, Error as MatchError, Event, Match, data::MatchData};
use std::collections::BTreeMap;

fn fighter() -> FighterView {
    FighterView {
        id: 1,
        action: "parry".into(),
        action_frame: 3,
        velocity: [1.25, -0.5],
        grounded: false,
        percent: 12.0,
        hitlag: 0.0,
        hitstun: 0,
        flags: BTreeMap::from([("double_jump_armor".into(), true)]),
    }
}

fn hit() -> HitView {
    HitView {
        frame: 17,
        attacker: 0,
        defender: 1,
        damage: 7.0,
        angle: 45.0,
        base_knockback: 20,
        knockback_growth: 100,
        knockback: 80.0,
        hitbox_group: 2,
        projectile: false,
        max_damage: 0,
    }
}

const IDLE: [Controller; 2] = [
    Controller {
        buttons: 0,
        stick: [0.0; 2],
        cstick: [0.0; 2],
        trigger: 0.0,
    },
    Controller {
        buttons: 0,
        stick: [0.0; 2],
        cstick: [0.0; 2],
        trigger: 0.0,
    },
];

fn jab_input() -> [Controller; 2] {
    [
        Controller {
            buttons: skirmish::game::BUTTON_A,
            ..Controller::default()
        },
        IDLE[1],
    ]
}

fn match_data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data
}

fn patch(statements: &str) -> String {
    format!("before_receive_hit = function(self, ctx) {statements} end")
}

#[test]
fn incoming_hit_can_be_cancelled_without_engine_specific_parry_api() {
    let program = Program::new(patch("ctx.cancelled = true")).unwrap();
    let result = program
        .dispatch(
            Hook::BeforeReceiveHit,
            &fighter(),
            Some(&hit()),
            &Default::default(),
        )
        .unwrap();
    assert!(result.hit.unwrap().cancelled);
}

#[test]
fn defender_script_reads_generic_flags_when_deciding_to_cancel() {
    let program = Program::new(
        "before_receive_hit = function(self, ctx) if self.flags.parry_window then ctx.cancelled = true end end",
    ).unwrap();
    let mut open = fighter();
    open.flags.insert("parry_window".into(), true);
    let closed = fighter();
    assert!(
        program
            .dispatch(
                Hook::BeforeReceiveHit,
                &open,
                Some(&hit()),
                &BTreeMap::new()
            )
            .unwrap()
            .hit
            .unwrap()
            .cancelled
    );
    assert!(
        !program
            .dispatch(
                Hook::BeforeReceiveHit,
                &closed,
                Some(&hit()),
                &BTreeMap::new()
            )
            .unwrap()
            .hit
            .unwrap()
            .cancelled
    );
}

#[test]
fn armor_can_keep_damage_while_suppressing_knockback() {
    let program = Program::new(patch("ctx.apply_knockback = false")).unwrap();
    let result = program
        .dispatch(
            Hook::BeforeReceiveHit,
            &fighter(),
            Some(&hit()),
            &Default::default(),
        )
        .unwrap();
    let hit = result.hit.unwrap();
    assert!(hit.apply_damage);
    assert!(!hit.apply_knockback);
}

#[test]
fn hitlag_and_hitstun_are_independent_resolution_gates() {
    let program =
        Program::new(patch("ctx.apply_hitlag = false; ctx.apply_hitstun = true")).unwrap();
    let result = program
        .dispatch(
            Hook::BeforeReceiveHit,
            &fighter(),
            Some(&hit()),
            &Default::default(),
        )
        .unwrap();
    let hit = result.hit.unwrap();
    assert!(!hit.apply_hitlag);
    assert!(hit.apply_hitstun);
}

#[test]
fn locals_are_explicit_state_and_survive_dispatch_checkpoint_round_trip() {
    let program = Program::new(
        "on_frame = function(self, ctx) self.locals.count = (self.locals.count or 0) + 1; self.locals.mode = 'armed' end",
    )
    .unwrap();
    let initial = BTreeMap::new();
    let checkpoint = initial.clone();
    let first = program
        .dispatch(Hook::OnFrame, &fighter(), None, &initial)
        .unwrap();
    assert_eq!(first.locals.get("count"), Some(&LocalValue::Integer(1)));
    assert_eq!(
        first.locals.get("mode"),
        Some(&LocalValue::String("armed".into()))
    );
    let replay = program
        .dispatch(Hook::OnFrame, &fighter(), None, &checkpoint)
        .unwrap();
    assert_eq!(first, replay);
}

#[test]
fn invalid_hook_return_is_rejected_and_script_error_is_transactional() {
    let program = Program::new("before_hit = function() return 42 end").unwrap();
    let locals = BTreeMap::from([("kept".into(), LocalValue::Bool(true))]);
    let error = program
        .dispatch(Hook::BeforeHit, &fighter(), None, &locals)
        .unwrap_err();
    assert!(matches!(error, Error::Invalid(_)));
    assert_eq!(locals.get("kept"), Some(&LocalValue::Bool(true)));
}

#[test]
fn generic_state_commands_are_available_to_character_scripts() {
    let program = Program::new(
        "on_frame = function(self, ctx) self:set_action('wait'); self:set_velocity(1.5, -2.0); return { commands = { { kind = 'apply_hitlag', fighter = 1, frames = 6 } } } end",
    ).unwrap();
    let result = program
        .dispatch(Hook::OnFrame, &fighter(), None, &BTreeMap::new())
        .unwrap();
    assert_eq!(
        result.commands,
        [
            Command::SetAction("wait".into()),
            Command::SetVelocity([1.5, -2.0]),
            Command::ApplyHitlag {
                fighter: 1,
                frames: 6
            },
        ]
    );
}

#[test]
fn infinite_hook_is_stopped_by_instruction_budget() {
    let program = Program::new("on_frame = function() while true do end end").unwrap();
    let error = program
        .dispatch(Hook::OnFrame, &fighter(), None, &BTreeMap::new())
        .unwrap_err();
    assert!(matches!(error, Error::Runtime(message) if message.contains("instruction budget")));
}

#[test]
fn non_finite_hit_values_are_rejected() {
    let program = Program::new(patch("ctx.damage = 0 / 0")).unwrap();
    let error = program
        .dispatch(
            Hook::BeforeReceiveHit,
            &fighter(),
            Some(&hit()),
            &BTreeMap::new(),
        )
        .unwrap_err();
    assert!(matches!(error, Error::Invalid(_) | Error::Runtime(_)));
}

#[test]
fn no_op_fighter_script_preserves_the_no_script_match_baseline() {
    let mut plain = Match::new(match_data(), 9).unwrap();
    let mut scripted_data = match_data();
    scripted_data.fighters[0].script = Some(Program::new("-- intentionally empty").unwrap());
    let mut scripted = Match::new(scripted_data, 9).unwrap();
    for input in [IDLE, jab_input(), IDLE] {
        assert_eq!(plain.step(input).unwrap(), scripted.step(input).unwrap());
    }
}

#[test]
fn no_op_scripts_preserve_a_simultaneous_trade_baseline() {
    let mut plain = Match::new(match_data(), 91).unwrap();
    let mut scripted_data = match_data();
    let no_op =
        Program::new("before_hit = function() end; before_receive_hit = function() end").unwrap();
    scripted_data.fighters[0].script = Some(no_op.clone());
    scripted_data.fighters[1].script = Some(no_op);
    let mut scripted = Match::new(scripted_data, 91).unwrap();
    let trade = [
        Controller {
            buttons: skirmish::game::BUTTON_A,
            ..Controller::default()
        },
        Controller {
            buttons: skirmish::game::BUTTON_A,
            ..Controller::default()
        },
    ];
    let mut trade_hits = 0;
    for input in [trade, IDLE, IDLE] {
        let left = plain.step(input).unwrap();
        let right = scripted.step(input).unwrap();
        trade_hits += left
            .events
            .iter()
            .filter(|event| matches!(event, Event::Hit { .. }))
            .count();
        assert_eq!(left, right);
    }
    assert_eq!(trade_hits, 2);
}

#[test]
fn match_defender_script_can_cancel_a_real_jab_hit() {
    let mut data = match_data();
    data.fighters[1].script = Some(
        Program::new("before_receive_hit = function(self, ctx) ctx.cancelled = true end").unwrap(),
    );
    let mut game = Match::new(data, 11).unwrap();
    game.step(jab_input()).unwrap();
    let state = game.step(IDLE).unwrap();
    assert!(
        !state
            .events
            .iter()
            .any(|event| matches!(event, Event::Hit { .. }))
    );
    assert_eq!(state.fighters[1].percent, 0.0);
}

#[test]
fn cancelled_hit_keeps_script_locals_commands_and_targeted_hitlag() {
    let mut data = match_data();
    data.fighters[1].script = Some(Program::new(
        "before_receive_hit = function(self, ctx) self.locals.parries = (self.locals.parries or 0) + 1; self:set_velocity(3, 4); ctx.cancelled = true; return { commands = { { kind = 'apply_hitlag', fighter = 0, frames = 6 } } } end",
    ).unwrap());
    let mut game = Match::new(data, 111).unwrap();
    game.step(jab_input()).unwrap();
    let state = game.step(IDLE).unwrap();
    assert!(
        !state
            .events
            .iter()
            .any(|event| matches!(event, Event::Hit { .. }))
    );
    assert_eq!(state.fighters[1].percent, 0.0);
    assert_eq!(
        state.fighters[1].script_state.get("parries"),
        Some(&LocalValue::Integer(1))
    );
    assert_eq!(state.fighters[1].velocity, [3.0, 4.0]);
    assert_eq!(state.fighters[0].hitlag, 6.0);
}

#[test]
fn fighter_script_locals_are_restored_by_match_checkpoint_replay() {
    let mut data = match_data();
    data.fighters[0].script = Some(
        Program::new(
            "before_hit = function(self, ctx) self.locals.hits = (self.locals.hits or 0) + 1 end",
        )
        .unwrap(),
    );
    let mut game = Match::new(data, 92).unwrap();
    let checkpoint = game.checkpoint();
    game.step(jab_input()).unwrap();
    let expected = game.step(IDLE).unwrap().clone();
    assert_eq!(
        expected.fighters[0].script_state.get("hits"),
        Some(&LocalValue::Integer(1))
    );
    game.restore_checkpoint(&checkpoint).unwrap();
    game.step(jab_input()).unwrap();
    assert_eq!(game.step(IDLE).unwrap(), &expected);
}

#[test]
fn match_script_can_keep_damage_while_suppressing_knockback() {
    let mut data = match_data();
    data.fighters[1].script = Some(
        Program::new("before_receive_hit = function(self, ctx) ctx.apply_knockback = false; ctx.apply_hitstun = false end")
            .unwrap(),
    );
    let mut game = Match::new(data, 12).unwrap();
    game.step(jab_input()).unwrap();
    let state = game.step(IDLE).unwrap();
    assert!(
        state
            .events
            .iter()
            .any(|event| matches!(event, Event::Hit { .. }))
    );
    assert_eq!(state.fighters[1].percent, 10.0);
    assert_eq!(state.fighters[1].action, skirmish::game::Action::Wait);
    assert_eq!(state.fighters[1].hitstun, 0);
    assert!(state.fighters[0].hitlag > 0.0);
    assert!(state.fighters[1].hitlag > 0.0);
}

#[test]
fn attacker_hook_runs_before_defender_hook() {
    let mut data = match_data();
    data.fighters[0].script = Some(
        Program::new("before_hit = function(self, ctx) ctx.damage = ctx.damage + 3 end").unwrap(),
    );
    data.fighters[1].script = Some(
        Program::new("before_receive_hit = function(self, ctx) ctx.damage = ctx.damage * 2 end")
            .unwrap(),
    );
    let mut game = Match::new(data, 13).unwrap();
    game.step(jab_input()).unwrap();
    let state = game.step(IDLE).unwrap();
    assert!(
        state
            .events
            .iter()
            .any(|event| matches!(event, Event::Hit { .. }))
    );
    assert_eq!(state.fighters[1].percent, 26.0);
}

#[test]
fn match_script_can_gate_hitlag_without_gating_hitstun() {
    let mut data = match_data();
    data.fighters[1].script = Some(
        Program::new("before_receive_hit = function(self, ctx) ctx.apply_hitlag = false end")
            .unwrap(),
    );
    let mut game = Match::new(data, 14).unwrap();
    game.step(jab_input()).unwrap();
    let state = game.step(IDLE).unwrap();
    assert!(state.fighters[1].hitstun > 0);
    assert_eq!(state.fighters[1].hitlag, 0.0);
    assert_eq!(state.fighters[0].hitlag, 0.0);
}

#[test]
fn differing_script_resources_reject_checkpoint_restore() {
    let mut first_data = match_data();
    first_data.fighters[0].script = Some(Program::new("-- first").unwrap());
    let mut second_data = match_data();
    second_data.fighters[0].script = Some(Program::new("-- second").unwrap());
    let first = Match::new(first_data, 15).unwrap();
    let mut second = Match::new(second_data, 15).unwrap();
    assert!(matches!(
        second.restore_checkpoint(&first.checkpoint()),
        Err(MatchError::Resources)
    ));
}
