//! Frame-sampled hurtbox eligibility through ordinary attacks, grabs and checkpoints.
#[path = "support/grab.rs"]
mod grab_resources;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_Z, Controller, Error, Event, Match, State,
    data::{HurtboxState, MatchData},
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.stage.spawns = [[-0.5, 0.0], [0.5, 0.0]];
    let victim_bones = data.fighters[1].bones.clone();
    for frame in &mut data.fighters[1].jab.frames {
        frame.bones.clone_from(&victim_bones);
        frame.hitboxes.clear();
    }
    data
}

fn buttons(first: u16, second: u16) -> [Controller; 2] {
    [
        Controller {
            buttons: first,
            ..Controller::default()
        },
        Controller {
            buttons: second,
            ..Controller::default()
        },
    ]
}

fn step(game: &mut Match, inputs: [Controller; 2]) -> State {
    game.step(inputs).unwrap().clone()
}

fn sample(data: &mut MatchData, frame: usize, state: HurtboxState) {
    let count = data.fighters[1].hurtboxes.len();
    data.fighters[1].jab.frames[frame].hurtbox_states = vec![state; count];
}

#[test]
fn disabled_and_intangible_attack_frames_delay_damage_until_enabled() {
    for blocked in [HurtboxState::Disabled, HurtboxState::Intangible] {
        let mut resource = data();
        sample(&mut resource, 1, blocked);
        sample(&mut resource, 2, HurtboxState::Enabled);
        let mut game = Match::new(resource, 41).unwrap();

        step(&mut game, buttons(BUTTON_A, BUTTON_A));
        let blocked_frame = step(&mut game, IDLE);
        assert_eq!(blocked_frame.fighters[1].action, Action::Jab);
        assert_eq!(blocked_frame.fighters[1].percent, 0.0);
        assert!(!blocked_frame.events.iter().any(|event| matches!(
            event,
            Event::Hit {
                attacker: 0,
                victim: 1,
                ..
            }
        )));

        let enabled_frame = step(&mut game, IDLE);
        assert_eq!(enabled_frame.fighters[1].percent, 10.0);
        assert!(enabled_frame.events.iter().any(|event| matches!(
            event,
            Event::Hit {
                attacker: 0,
                victim: 1,
                ..
            }
        )));
    }
}

#[test]
fn sampled_enabled_state_overrides_base_and_replays_from_checkpoint() {
    let mut resource = data();
    resource.fighters[1].hurtboxes[0].state = HurtboxState::Intangible;
    sample(&mut resource, 1, HurtboxState::Enabled);
    let mut game = Match::new(resource, 42).unwrap();

    step(&mut game, buttons(BUTTON_A, BUTTON_A));
    let checkpoint = game.checkpoint();
    let expected = step(&mut game, IDLE);
    assert_eq!(expected.fighters[1].percent, 10.0);

    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(step(&mut game, IDLE), expected);
}

#[test]
fn grabs_observe_the_targets_sampled_attack_state() {
    for blocked in [HurtboxState::Disabled, HurtboxState::Intangible] {
        let mut resource = grab_resources::profile(data());
        sample(&mut resource, 0, blocked);
        sample(&mut resource, 1, HurtboxState::Enabled);
        let mut game = Match::new(resource, 43).unwrap();

        let blocked_frame = step(&mut game, buttons(BUTTON_Z, BUTTON_A));
        assert_eq!(blocked_frame.fighters[0].action, Action::Catch);
        assert_eq!(blocked_frame.fighters[1].action, Action::Jab);
        assert!(!blocked_frame.events.contains(&Event::Grabbed {
            holder: 0,
            victim: 1,
        }));

        let enabled_frame = step(&mut game, IDLE);
        assert!(enabled_frame.events.contains(&Event::Grabbed {
            holder: 0,
            victim: 1,
        }));
    }
}

#[test]
fn empty_samples_inherit_base_state_and_partial_samples_are_rejected() {
    let mut legacy = data();
    legacy.fighters[1].hurtboxes[0].state = HurtboxState::Intangible;
    assert!(
        legacy.fighters[1]
            .jab
            .frames
            .iter()
            .all(|frame| frame.hurtbox_states.is_empty())
    );
    let mut game = Match::new(legacy, 44).unwrap();
    step(&mut game, buttons(BUTTON_A, BUTTON_A));
    step(&mut game, IDLE);
    let after_both_active_frames = step(&mut game, IDLE);
    assert_eq!(after_both_active_frames.fighters[1].percent, 0.0);

    let mut partial = data();
    partial.fighters[1].jab.frames[0].hurtbox_states =
        vec![HurtboxState::Enabled, HurtboxState::Disabled];
    assert!(
        matches!(Match::new(partial, 45), Err(Error::Data(message)) if
        message.contains("hurtbox state samples must be empty or complete"))
    );
}
