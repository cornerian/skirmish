//! Synthetic integration checks for source-derived hitbox history and collision.
use skirmish::collision::bones::{Bone, LocalTransform, Pose};
use skirmish::game::{
    BUTTON_A, Controller, Error, Event, Match,
    data::{AttackFrame, Hitbox, MatchData},
    hitboxes::{Track, update_tracks},
};

const IDLE: [Controller; 2] = [Controller {
    cstick: [0.0; 2],
    trigger: 0.0,
    buttons: 0,
    stick: [0.0; 2],
}; 2];

fn hit(group: u8) -> Hitbox {
    Hitbox {
        shield_damage: 0,
        group,
        bone: 0,
        center: [0.0; 3],
        radius: 0.1,
        damage: 10,
        angle_degrees: 30.0,
        growth: 50,
        fixed: 0,
        base: 30,
    }
}

fn frame(groups: &[u8]) -> AttackFrame {
    AttackFrame {
        bones: vec![],
        hitboxes: groups.iter().copied().map(hit).collect(),
    }
}

fn pose(x: f32) -> Pose {
    Pose::evaluate(&[Bone {
        local: LocalTransform {
            translation: [x, 1.0, 0.0],
            ..LocalTransform::default()
        },
        ..Bone::default()
    }])
    .unwrap()
}

#[test]
fn enable_group_change_and_disable_preserve_slot_lifecycle() {
    let mut tracks = [Track::default(); 4];
    let a = update_tracks(&mut tracks, Some(&frame(&[0, 1])), &pose(-2.0)).unwrap();
    assert_eq!(a[0].unwrap().start, [-2.0, 1.0, 0.0]);
    assert_eq!(a[0].unwrap().start, a[0].unwrap().end);
    assert_eq!(tracks[1].group, Some(1));

    let a = update_tracks(&mut tracks, Some(&frame(&[0, 2])), &pose(6.0)).unwrap();
    assert_eq!(a[0].unwrap().start[0], -2.0);
    assert_eq!(a[0].unwrap().end[0], 6.0);
    assert_eq!(a[1].unwrap().start, a[1].unwrap().end);
    assert_eq!(tracks[1].group, Some(2));

    let a = update_tracks(&mut tracks, Some(&frame(&[0])), &pose(8.0)).unwrap();
    assert_eq!(a[0].unwrap().start[0], 6.0);
    assert_eq!(tracks[1], Track::default());
    assert!(a[1..].iter().all(Option::is_none));
    assert!(
        update_tracks(&mut tracks, None, &pose(8.0))
            .unwrap()
            .iter()
            .all(Option::is_none)
    );
    assert_eq!(tracks, [Track::default(); 4]);
    let a = update_tracks(&mut tracks, Some(&frame(&[0])), &pose(100.0)).unwrap();
    assert_eq!(a[0].unwrap().start, a[0].unwrap().end);
}

#[test]
fn frozen_pose_collapses_sweep_and_invalid_updates_are_atomic() {
    let mut tracks = [Track::default(); 4];
    update_tracks(&mut tracks, Some(&frame(&[0])), &pose(-2.0)).unwrap();
    update_tracks(&mut tracks, Some(&frame(&[0])), &pose(6.0)).unwrap();
    let frozen = update_tracks(&mut tracks, Some(&frame(&[0])), &pose(6.0)).unwrap();
    assert_eq!(frozen[0].unwrap().start, frozen[0].unwrap().end);
    let saved = tracks;
    let mut invalid = frame(&[0, 1]);
    invalid.hitboxes[1].bone = 10;
    assert!(matches!(
        update_tracks(&mut tracks, Some(&invalid), &pose(9.0)),
        Err(Error::Physics(_))
    ));
    assert_eq!(tracks, saved);
    assert!(matches!(
        update_tracks(&mut tracks, Some(&frame(&[0; 5])), &pose(9.0)),
        Err(Error::Data(_))
    ));
    assert_eq!(tracks, saved);
}

fn resource() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.knockback_speed = 0.0;
    let bones = data.fighters[0].bones.clone();
    let mut hitbox = hit(0);
    hitbox.bone = 1;
    let mut start = AttackFrame {
        bones: bones.clone(),
        hitboxes: vec![hitbox],
    };
    start.bones[1].translation = [0.0, 1.0, 0.0];
    start.bones[1].rotation = [0.0; 3];
    let mut end = start.clone();
    end.bones[1].translation[0] = 8.0;
    let idle = AttackFrame {
        bones,
        hitboxes: vec![],
    };
    data.fighters[0].jab.frames = vec![idle.clone(), start, end, idle.clone(), idle];
    data
}

fn active_start(mut game: Match) -> Match {
    let mut inputs = IDLE;
    inputs[0].buttons = BUTTON_A;
    game.step(inputs).unwrap();
    game.step(IDLE).unwrap();
    assert_eq!(game.state().fighters[1].percent, 0.0);
    assert_eq!(game.state().fighters[0].hitboxes[0].current[0], -2.0);
    game
}

#[test]
fn animated_hitbox_crossing_hits_with_both_frame_endpoints_outside_hurtbox() {
    let mut game = active_start(Match::new(resource(), 7).unwrap());
    game.step(IDLE).unwrap();
    assert!(matches!(
        game.state().events.as_slice(),
        [Event::Hit {
            attacker: 0,
            victim: 1,
            ..
        }]
    ));
    assert_eq!(game.state().fighters[1].percent, 10.0);
    let track = game.state().fighters[0].hitboxes[0];
    assert_eq!(track.previous[0], -2.0);
    assert_eq!(track.current[0], 6.0);
    // Victim's vertical centerline is at X=2 with radius .4, hit radius .1.
    assert!((track.previous[0] - 2.0).abs() > track.radius + 0.4);
    assert!((track.current[0] - 2.0).abs() > track.radius + 0.4);
}

#[test]
fn changed_group_or_inactive_gap_does_not_sweep_across_the_victim() {
    for gap in [false, true] {
        let mut data = resource();
        if gap {
            let empty = data.fighters[0].jab.frames[0].clone();
            data.fighters[0].jab.frames.insert(2, empty);
        } else {
            data.fighters[0].jab.frames[2].hitboxes[0].group = 1;
        }
        let mut game = active_start(Match::new(data, 7).unwrap());
        for _ in 0..4 {
            game.step(IDLE).unwrap();
            assert!(
                game.state()
                    .events
                    .iter()
                    .all(|event| !matches!(event, Event::Hit { .. }))
            );
            assert_eq!(game.state().fighters[1].percent, 0.0);
        }
    }
}

#[test]
fn checkpoint_preserves_pending_sweep_and_hitlag_clears_old_segment() {
    let mut game = active_start(Match::new(resource(), 7).unwrap());
    let checkpoint = game.checkpoint();
    let before = serde_json::to_vec(game.state()).unwrap();
    let expected: Vec<_> = (0..8)
        .map(|_| serde_json::to_vec(game.step(IDLE).unwrap()).unwrap())
        .collect();
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(serde_json::to_vec(game.state()).unwrap(), before);
    for (index, expected) in expected.into_iter().enumerate() {
        game.step(IDLE).unwrap();
        assert_eq!(serde_json::to_vec(game.state()).unwrap(), expected);
        if index == 1 {
            assert!(game.state().fighters[0].hitlag > 0.0);
            let track = game.state().fighters[0].hitboxes[0];
            assert_eq!(track.previous, track.current);
        }
    }
}
