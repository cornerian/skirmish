//! Native damage-floor scheduling with explicit synthetic state durations.
use skirmish::game::{Action, BUTTON_A, BUTTON_L, BUTTON_R, Controller, Event, Match, State};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn profile() -> skirmish::game::damage::FloorResponseRules {
    skirmish::game::damage::FloorResponseRules {
        tumble_knockback_threshold: 20.0,
        tech_window: 20.0,
        tech_repeat_lockout: 40,
        passive_frames: 3,
        down_bound_frames: 4,
        down_wait_frames: 5,
        down_stand_frames: 3,
    }
}

fn data() -> skirmish::game::data::MatchData {
    let mut data: skirmish::game::data::MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.rules.damage.floor_response = Some(profile());
    data.stage.spawns = [[-2.0, 0.0], [2.0, 2.0]];
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    for frame in &mut data.fighters[0].jab.frames {
        for hit in &mut frame.hitboxes {
            hit.damage = 40;
            hit.angle_degrees = 270.0;
        }
    }
    data
}

fn input(player: usize, buttons: u16) -> [Controller; 2] {
    let mut input = IDLE;
    input[player].buttons = buttons;
    input
}

fn step(game: &mut Match, input: [Controller; 2]) -> State {
    game.step(input).unwrap().clone()
}

fn until(game: &mut Match, condition: impl Fn(&State) -> bool) -> State {
    for _ in 0..240 {
        if condition(game.state()) {
            return game.state().clone();
        }
        step(game, IDLE);
    }
    panic!("condition was not reached: {:?}", game.state());
}

fn downward_hit(data: skirmish::game::data::MatchData) -> Match {
    let mut game = Match::new(data, 7).unwrap();
    step(&mut game, input(0, BUTTON_A));
    let hit = until(&mut game, |state| state.fighters[1].percent > 0.0);
    assert!(hit.fighters[1].tumbling);
    assert!(!hit.fighters[1].grounded);
    game
}

fn arm_tech(game: &mut Match) {
    while game.state().fighters[1].hitlag > 1.0 {
        step(game, IDLE);
    }
    step(game, input(1, BUTTON_L));
}

fn lingering_tumble_data() -> skirmish::game::data::MatchData {
    let mut resource = data();
    resource.stage.spawns[1][1] = 6.0;
    resource.rules.knockback_speed = 0.01;
    resource.rules.knockback_decay = 0.01;
    resource.rules.hitstun_scale = 0.01;
    for hit in resource.fighters[0]
        .jab
        .frames
        .iter_mut()
        .flat_map(|frame| &mut frame.hitboxes)
    {
        hit.radius = 8.0;
    }
    resource
}

#[test]
fn buffered_neutral_tech_stops_launch_and_recovers_for_input() {
    let mut game = downward_hit(data());
    arm_tech(&mut game);
    let landed = until(&mut game, |state| state.fighters[1].grounded);
    let fighter = &landed.fighters[1];
    assert_eq!(fighter.action, Action::Passive);
    assert_eq!(fighter.action_frame, 1);
    assert_eq!(fighter.velocity, [0.0; 2]);
    assert_eq!(fighter.knockback, [0.0; 2]);
    assert!(landed.events.contains(&Event::Landed { player: 1 }));

    let mut passive_samples = 1;
    while game.state().fighters[1].action == Action::Passive {
        step(&mut game, IDLE);
        passive_samples += usize::from(game.state().fighters[1].action == Action::Passive);
    }
    assert_eq!(passive_samples, profile().passive_frames as usize);
    assert_eq!(game.state().fighters[1].action, Action::Wait);
}

#[test]
fn unteched_tumble_runs_bound_wait_stand_and_complete_recovery() {
    let mut game = downward_hit(data());
    let landed = until(&mut game, |state| state.fighters[1].grounded);
    assert_eq!(landed.fighters[1].action, Action::DownBound);
    let mut transitions = vec![Action::DownBound];
    let mut counts = vec![1_usize];
    while game.state().fighters[1].action != Action::Wait {
        let action = step(&mut game, IDLE).fighters[1].action;
        if transitions.last() == Some(&action) {
            *counts.last_mut().unwrap() += 1;
        } else {
            transitions.push(action);
            counts.push(1);
        }
    }
    assert_eq!(
        transitions,
        [
            Action::DownBound,
            Action::DownWait,
            Action::DownStand,
            Action::Wait,
        ]
    );
    assert_eq!(
        &counts[..3],
        &[
            profile().down_bound_frames as usize,
            profile().down_wait_frames as usize,
            profile().down_stand_frames as usize,
        ]
    );
}

#[test]
fn a_second_recent_shoulder_press_fails_the_repeat_lockout() {
    let mut game = downward_hit(data());
    step(&mut game, input(1, BUTTON_L));
    step(&mut game, IDLE);
    while game.state().fighters[1].hitlag > 1.0 {
        step(&mut game, IDLE);
    }
    step(&mut game, input(1, BUTTON_R));
    let landed = until(&mut game, |state| state.fighters[1].grounded);
    assert_eq!(landed.fighters[1].action, Action::DownBound);
    assert!(
        landed.fighters[1].locomotion.previous_tech_press_age < profile().tech_repeat_lockout as u8
    );
}

#[test]
fn non_tumbling_damage_does_not_enter_the_tumble_floor_graph() {
    let mut resource = data();
    resource
        .rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .tumble_knockback_threshold = 100.0;
    let mut game = Match::new(resource, 7).unwrap();
    step(&mut game, input(0, BUTTON_A));
    until(&mut game, |state| state.fighters[1].percent > 0.0);
    assert!(!game.state().fighters[1].tumbling);
    let mut seen_floor_action = None;
    for _ in 0..120 {
        let state = step(&mut game, IDLE);
        if state.fighters[1].grounded {
            seen_floor_action = Some(state.fighters[1].action);
        }
        assert!(!matches!(
            state.fighters[1].action,
            Action::Passive | Action::DownBound | Action::DownWait | Action::DownStand
        ));
        if state.fighters[1].action == Action::Wait {
            break;
        }
    }
    assert!(seen_floor_action.is_some());
    assert_eq!(game.state().fighters[1].action, Action::Wait);
}

#[test]
fn damagefall_preserves_tumble_until_a_later_floor_contact() {
    let mut game = downward_hit(lingering_tumble_data());
    let mut saw_damagefall = false;
    while !game.state().fighters[1].grounded {
        let state = step(&mut game, IDLE);
        saw_damagefall |= state.fighters[1].action == Action::DamageFall;
    }
    assert!(saw_damagefall);
    assert_eq!(game.state().fighters[1].action, Action::DownBound);
}

#[test]
fn damagefall_uses_ordinary_air_drift_and_fresh_fastfall_input() {
    let mut game = downward_hit(lingering_tumble_data());
    until(&mut game, |state| {
        state.fighters[1].action == Action::DamageFall
    });
    let before = game.state().fighters[1].clone();
    let mut controls = IDLE;
    controls[1].stick = [1.0, -1.0];
    let after = step(&mut game, controls).fighters[1].clone();
    assert!(after.fast_fall);
    assert!(after.velocity[0] > before.velocity[0]);
    assert!(after.velocity[1] < before.velocity[1]);
    assert!(!after.grounded);
}

#[test]
fn checkpoint_and_reset_preserve_tech_history_and_floor_suffixes() {
    let mut game = downward_hit(data());
    arm_tech(&mut game);
    let checkpoint = game.checkpoint();
    let expected = (0..20).map(|_| step(&mut game, IDLE)).collect::<Vec<_>>();
    game.restore_checkpoint(&checkpoint).unwrap();
    for state in expected {
        assert_eq!(step(&mut game, IDLE), state);
    }
    let reset = game.reset(99);
    assert_eq!(reset.fighters[1].locomotion.tech_press_age, 255);
    assert_eq!(reset.fighters[1].locomotion.previous_tech_press_age, 255);
    assert!(!reset.fighters[1].tumbling);
}

#[test]
fn malformed_floor_profiles_are_rejected_transactionally() {
    let mut cases = Vec::new();
    let mut bad = data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .tech_window = f32::NAN;
    cases.push(bad);
    let mut bad = data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .tech_repeat_lockout = -1;
    cases.push(bad);
    let mut bad = data();
    bad.rules
        .damage
        .floor_response
        .as_mut()
        .unwrap()
        .passive_frames = 0;
    cases.push(bad);
    for resource in cases {
        assert!(Match::new(resource, 0).is_err());
    }
}
