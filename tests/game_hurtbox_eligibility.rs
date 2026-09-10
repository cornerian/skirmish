//! Ordinary damage and grab eligibility for per-hurtbox source flags.
#[path = "support/grab.rs"]
mod grab_resources;

use skirmish::game::{
    BUTTON_A, BUTTON_Z, Controller, Event, Match,
    data::{Hurtbox, HurtboxState, MatchData},
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    let data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let mut data = grab_resources::profile(data);
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.stage.spawns = [[-0.5, 0.0], [0.5, 0.0]];
    data
}

fn contact(data: MatchData, button: u16, expected: impl Fn(&Event) -> bool) -> bool {
    let mut game = Match::new(data, 41).unwrap();
    let mut input = IDLE;
    input[0].buttons = button;
    game.step(input).unwrap();
    for _ in 0..8 {
        if game.state().events.iter().any(&expected) {
            return true;
        }
        game.step(IDLE).unwrap();
    }
    false
}

fn damages(data: MatchData) -> bool {
    contact(data, BUTTON_A, |event| {
        matches!(
            event,
            Event::Hit {
                attacker: 0,
                victim: 1,
                ..
            }
        )
    })
}

fn grabs(data: MatchData) -> bool {
    contact(data, BUTTON_Z, |event| {
        matches!(
            event,
            Event::Grabbed {
                holder: 0,
                victim: 1
            }
        )
    })
}

#[test]
fn legacy_hurtboxes_default_to_enabled_and_grabbable() {
    let data = data();
    assert!(
        data.fighters[1]
            .hurtboxes
            .iter()
            .all(|hurt| hurt.state == HurtboxState::Enabled && hurt.grabbable)
    );
    assert!(damages(data.clone()));
    assert!(grabs(data));
}

#[test]
fn ordinary_damage_ignores_disabled_and_intangible_hurtboxes() {
    for state in [HurtboxState::Disabled, HurtboxState::Intangible] {
        let mut resource = data();
        resource.fighters[1].hurtboxes[0].state = state;
        assert!(!damages(resource));
    }

    let mut resource = data();
    let enabled = resource.fighters[1].hurtboxes[0].clone();
    resource.fighters[1].hurtboxes[0].state = HurtboxState::Intangible;
    resource.fighters[1].hurtboxes.push(enabled);
    assert!(damages(resource));
}

#[test]
fn grabs_require_an_enabled_grabbable_hurtbox_and_scan_later_entries() {
    for state in [HurtboxState::Disabled, HurtboxState::Intangible] {
        let mut resource = data();
        resource.fighters[1].hurtboxes[0].state = state;
        assert!(!grabs(resource));
    }

    let mut resource = data();
    resource.fighters[1].hurtboxes[0].grabbable = false;
    assert!(!grabs(resource.clone()));
    let eligible = resource.fighters[1].hurtboxes[0].clone();
    resource.fighters[1].hurtboxes.push(Hurtbox {
        grabbable: true,
        ..eligible
    });
    assert!(grabs(resource));
}

#[test]
fn grabs_use_the_hurt_bone_matrix_for_directional_radius() {
    let directional = |center: [f32; 3]| {
        let mut resource = data();
        resource.stage.spawns = [[0.0, 0.0]; 2];
        resource.fighters[1].bones[0].scale = [4.0, 0.5, 1.0];
        let hurt = &mut resource.fighters[1].hurtboxes[0];
        hurt.bone = 0;
        hurt.start = [0.0; 3];
        hurt.end = [0.0; 3];
        hurt.radius = 1.0;
        for frame in &mut resource.fighters[0].grab.as_mut().unwrap().catch.frames {
            for grab in &mut frame.grabboxes {
                grab.bone = 0;
                grab.start = center;
                grab.end = center;
                grab.radius = 0.1;
            }
        }
        resource
    };
    assert!(grabs(directional([3.0, 0.0, 0.0])));
    assert!(!grabs(directional([0.0, 0.75, 0.0])));
}
