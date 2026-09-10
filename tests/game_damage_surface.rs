//! Match-level wall and ceiling damage reflection through ordinary inputs.
//! Exact mirror arithmetic is covered separately against the original C body.
use skirmish::{
    collision::{ecb, stage},
    game::{
        Action, BUTTON_A, Controller, Event, Match, State,
        damage::{FloorResponseRules, SurfaceResponseRules},
        data::{CollisionBox, MatchData, StageGeometry},
    },
};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn line(start: [f32; 2], end: [f32; 2], kind: u32) -> stage::Line {
    stage::Line {
        start,
        end,
        flags: stage::ENABLED | kind,
        ..stage::Line::default()
    }
}

fn profile() -> SurfaceResponseRules {
    SurfaceResponseRules {
        knockback_threshold: 0.1,
        velocity_multiplier: 0.8,
        lockout_frames: 2,
        wall_frames: 3,
        ceiling_frames: 4,
    }
}

fn data(angle: f32) -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9_999;
    data.rules.damage.floor_response = Some(FloorResponseRules {
        tumble_knockback_threshold: 1.0,
        tech_window: 20.0,
        tech_repeat_lockout: 40,
        passive_frames: 3,
        down_bound_frames: 4,
        down_wait_frames: 5,
        down_stand_frames: 3,
    });
    data.rules.damage.surface_response = Some(profile());
    data.stage.spawns = [[-2.0, 0.0], [2.0, 2.0]];
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.geometry = Some(StageGeometry {
        lines: vec![
            line([-100.0, 0.0], [100.0, 0.0], stage::FLOOR),
            line([100.0, 60.0], [-100.0, 60.0], stage::CEILING),
            line([5.0, -20.0], [5.0, 20.0], stage::LEFT_WALL),
        ],
        joints: vec![stage::Joint {
            id: 0,
            flags: stage::ENABLED,
            bounds_min: [-110.0, -30.0],
            bounds_max: [110.0, 80.0],
            floor: 0..1,
            ceiling: 1..2,
            left_wall: 2..3,
            ..stage::Joint::default()
        }],
    });
    for fighter in &mut data.fighters {
        fighter.collision_box = CollisionBox::Fixed {
            source: ecb::FixedSource {
                up: 3.0,
                down: 0.0,
                front: 1.5,
                back: 1.5,
                angle: 0.0,
            },
        };
    }
    data.fighters[1].movement.gravity = 0.0;
    for hit in data.fighters[0]
        .jab
        .frames
        .iter_mut()
        .flat_map(|frame| &mut frame.hitboxes)
    {
        hit.radius = 8.0;
        hit.damage = 40;
        hit.angle_degrees = angle;
    }
    data
}

fn attack() -> [Controller; 2] {
    let mut input = IDLE;
    input[0].buttons = BUTTON_A;
    input
}

fn step(game: &mut Match) -> State {
    game.step(IDLE).unwrap().clone()
}

fn until(game: &mut Match, condition: impl Fn(&State) -> bool) -> State {
    for _ in 0..240 {
        if condition(game.state()) {
            return game.state().clone();
        }
        step(game);
    }
    panic!("condition was not reached: {:?}", game.state());
}

fn hit(data: MatchData) -> Match {
    let mut game = Match::new(data, 71).unwrap();
    game.step(attack()).unwrap();
    until(&mut game, |state| state.fighters[1].percent > 0.0);
    assert!(game.state().fighters[1].tumbling);
    game
}

#[test]
fn horizontal_launch_reflects_from_wall_and_replays_from_checkpoint() {
    let mut game = hit(data(0.0));
    let checkpoint = game.checkpoint();
    let mut suffix = Vec::new();
    let reflected = loop {
        let state = step(&mut game);
        suffix.push(state.clone());
        if state.events.iter().any(|event| {
            matches!(
                event,
                Event::SurfaceReflected {
                    player: 1,
                    surface: stage::Surface::LeftWall,
                    line: 2
                }
            )
        }) {
            break state;
        }
        assert!(suffix.len() < 240);
    };
    let fighter = &reflected.fighters[1];
    assert_eq!(fighter.action, Action::FlyReflectWall);
    assert_eq!(fighter.contacts[2], Some(2));
    assert_eq!(fighter.velocity, [0.0; 2]);
    assert!(fighter.knockback[0] < 0.0);
    assert_eq!(fighter.facing, -1.0);
    assert_eq!(fighter.last_damage_surface, Some(stage::Surface::LeftWall));
    assert!(fighter.reflect_lockout <= profile().lockout_frames);

    game.restore_checkpoint(&checkpoint).unwrap();
    for expected in suffix {
        assert_eq!(step(&mut game), expected);
    }

    let mut samples = 1;
    while game.state().fighters[1].action == Action::FlyReflectWall {
        step(&mut game);
        samples += usize::from(game.state().fighters[1].action == Action::FlyReflectWall);
    }
    assert_eq!(samples, profile().wall_frames as usize);
    assert_eq!(game.state().fighters[1].action, Action::DamageFall);
}

#[test]
fn vertical_launch_reflects_from_ceiling_and_uses_its_duration() {
    let mut game = hit(data(90.0));
    let reflected = until(&mut game, |state| {
        state.events.iter().any(|event| {
            matches!(
                event,
                Event::SurfaceReflected {
                    player: 1,
                    surface: stage::Surface::Ceiling,
                    line: 1
                }
            )
        })
    });
    let fighter = &reflected.fighters[1];
    assert_eq!(fighter.action, Action::FlyReflectCeiling);
    assert_eq!(fighter.contacts[1], Some(1));
    assert_eq!(fighter.velocity, [0.0; 2]);
    assert!(fighter.knockback[1] < 0.0);
    assert_eq!(fighter.last_damage_surface, Some(stage::Surface::Ceiling));

    let mut samples = 1;
    while game.state().fighters[1].action == Action::FlyReflectCeiling {
        step(&mut game);
        samples += usize::from(game.state().fighters[1].action == Action::FlyReflectCeiling);
    }
    assert_eq!(samples, profile().ceiling_frames as usize);
    assert_eq!(game.state().fighters[1].action, Action::DamageFall);
}

#[test]
fn simultaneous_floor_contact_wins_over_wall_reflection() {
    let mut game = hit(data(315.0));
    let landed = until(&mut game, |state| {
        state.events.contains(&Event::Landed { player: 1 })
    });
    let fighter = &landed.fighters[1];
    assert!(fighter.grounded);
    assert_eq!(fighter.ground_line, Some(0));
    assert_eq!(fighter.action, Action::DownBound);
    assert_eq!(fighter.knockback, [0.0; 2]);
    assert_eq!(fighter.last_damage_surface, None);
    assert!(
        !landed
            .events
            .iter()
            .any(|event| matches!(event, Event::SurfaceReflected { .. }))
    );
}

#[test]
fn absent_or_unmet_profiles_stop_at_the_surface_without_reflecting() {
    for profile in [
        None,
        Some(SurfaceResponseRules {
            knockback_threshold: 1_000_000.0,
            ..profile()
        }),
    ] {
        let mut resource = data(0.0);
        resource.rules.damage.surface_response = profile;
        let mut game = hit(resource);
        let mut touched = false;
        for _ in 0..120 {
            let state = step(&mut game);
            touched |= state.fighters[1].contacts[2] == Some(2);
            assert!(
                !state
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::SurfaceReflected { .. }))
            );
            assert!(!matches!(
                state.fighters[1].action,
                Action::FlyReflectWall | Action::FlyReflectCeiling
            ));
        }
        assert!(touched);
    }
}

#[test]
fn malformed_surface_profiles_are_rejected_and_valid_profiles_roundtrip() {
    let encoded = serde_json::to_string(&data(0.0)).unwrap();
    let decoded: MatchData = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.rules.damage.surface_response, Some(profile()));

    for mutate in [
        |rules: &mut SurfaceResponseRules| rules.knockback_threshold = f32::NAN,
        |rules: &mut SurfaceResponseRules| rules.velocity_multiplier = 1.01,
        |rules: &mut SurfaceResponseRules| rules.wall_frames = 0,
        |rules: &mut SurfaceResponseRules| rules.ceiling_frames = 0,
    ] {
        let mut resource = data(0.0);
        mutate(resource.rules.damage.surface_response.as_mut().unwrap());
        assert!(Match::new(resource, 0).is_err());
    }
    let mut missing_tumble_rules = data(0.0);
    missing_tumble_rules.rules.damage.floor_response = None;
    assert!(Match::new(missing_tumble_rules, 0).is_err());
}
