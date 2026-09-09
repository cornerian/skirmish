//! Native match integration for the source-backed fighter push callback.
use skirmish::{
    collision::stage,
    fighter::nudge::Rules,
    game::{
        BUTTON_A, Controller, Match,
        data::{MatchData, StageGeometry},
        nudge::Attributes,
    },
};

fn data(spawns: [[f32; 2]; 2]) -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.spawns = spawns;
    data.rules.nudge = Some(Rules {
        horizontal_step: 0.25,
        depth_step: 0.125,
        depth_limit: 0.5,
        follower_depth_step: 0.375,
        follower_depth_limit: 1.0,
    });
    for fighter in &mut data.fighters {
        fighter.nudge = Some(Attributes {
            center_offset: 0.0,
            half_width: 1.0,
            nudge_disabled: false,
            overlap_disabled: false,
        });
    }
    data
}

fn step(game: &mut Match, inputs: [Controller; 2]) {
    game.step(inputs).unwrap();
}

#[test]
fn stable_order_samples_both_pushes_before_either_position_advances() {
    let mut game = Match::new(data([[0.0, 0.0], [1.75, 0.0]]), 7).unwrap();
    step(&mut game, [Controller::default(); 2]);
    let fighters = &game.state().fighters;
    assert_eq!(
        fighters.each_ref().map(|f| f.nudge),
        [[-0.25, -0.125], [0.25, 0.125]]
    );
    assert_eq!(fighters.each_ref().map(|f| f.position[0]), [-0.25, 2.0]);
    assert_eq!(fighters.each_ref().map(|f| f.depth), [-0.125, 0.125]);

    // At strict touching distance, horizontal push stops. Depth independently
    // recenters on the following frame using the same source callback.
    step(&mut game, [Controller::default(); 2]);
    let fighters = &game.state().fighters;
    assert_eq!(
        fighters.each_ref().map(|f| f.nudge),
        [[0.0, 0.125], [0.0, -0.125]]
    );
    assert_eq!(fighters.each_ref().map(|f| f.position[0]), [-0.25, 2.0]);
    assert_eq!(fighters.each_ref().map(|f| f.depth), [0.0; 2]);
}

#[test]
fn facing_relative_centers_and_disable_flags_are_native_resource_data() {
    let mut resource = data([[-1.0, 0.0], [1.0, 0.0]]);
    resource.fighters[0].nudge.as_mut().unwrap().center_offset = 0.25;
    resource.fighters[1].nudge.as_mut().unwrap().center_offset = 0.25;
    let mut enabled = Match::new(resource.clone(), 0).unwrap();
    step(&mut enabled, [Controller::default(); 2]);
    assert_eq!(
        enabled.state().fighters.each_ref().map(|f| f.nudge[0]),
        [-0.25, 0.25]
    );

    resource.fighters[0]
        .nudge
        .as_mut()
        .unwrap()
        .overlap_disabled = true;
    resource.fighters[1].nudge.as_mut().unwrap().nudge_disabled = true;
    let mut disabled = Match::new(resource, 0).unwrap();
    step(&mut disabled, [Controller::default(); 2]);
    assert_eq!(
        disabled.state().fighters.each_ref().map(|f| f.nudge),
        [[0.0; 2]; 2]
    );
}

#[test]
fn turn_animation_updates_facing_before_that_entitys_push_sample() {
    let mut resource = data([[0.0, 0.0], [2.0, 0.0]]);
    resource.rules.nudge.as_mut().unwrap().horizontal_step = 0.0;
    resource.rules.nudge.as_mut().unwrap().depth_limit = 0.5;
    let locomotion = serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap();
    for fighter in &mut resource.fighters {
        fighter.locomotion = Some(locomotion);
        let nudge = fighter.nudge.as_mut().unwrap();
        nudge.center_offset = 0.5;
        nudge.half_width = 0.75;
    }
    let mut game = Match::new(resource, 0).unwrap();
    let mut turn = [Controller::default(); 2];
    turn[0].stick[0] = -0.5;
    for _ in 0..4 {
        step(&mut game, turn);
    }
    assert_eq!(game.state().fighters[0].facing, 1.0);
    assert_eq!(game.state().fighters[0].depth, -0.5);
    step(&mut game, turn);
    let fighter = &game.state().fighters[0];
    assert_eq!(fighter.facing, -1.0);
    // The facing change makes the two facing-relative centers merely touch.
    // Therefore this frame recenters depth instead of adding overlap depth.
    assert_eq!(fighter.nudge, [0.0, 0.125]);
    assert_eq!(fighter.depth, -0.375);
}

#[test]
fn resolved_adjacent_floor_links_allow_push_across_a_stage_seam() {
    let mut linked = data([[-0.5, 0.0], [0.5, 0.0]]);
    let lines = vec![
        stage::Line {
            start: [-5.0, 0.0],
            end: [0.0, 0.0],
            flags: stage::FLOOR | stage::ENABLED,
            next: [None, Some(1)],
            ..Default::default()
        },
        stage::Line {
            start: [0.0, 0.0],
            end: [5.0, 0.0],
            flags: stage::FLOOR | stage::ENABLED,
            previous: [Some(0), None],
            ..Default::default()
        },
    ];
    linked.stage.geometry = Some(StageGeometry {
        lines,
        joints: vec![stage::Joint {
            id: 0,
            flags: stage::ENABLED,
            bounds_min: [-10.0, -10.0],
            bounds_max: [10.0, 10.0],
            floor: 0..2,
            ..Default::default()
        }],
    });
    let mut unlinked = linked.clone();
    let geometry = unlinked.stage.geometry.as_mut().unwrap();
    geometry.lines[0].next = [None; 2];
    geometry.lines[1].previous = [None; 2];
    let mut linked = Match::new(linked, 0).unwrap();
    let mut unlinked = Match::new(unlinked, 0).unwrap();
    step(&mut linked, [Controller::default(); 2]);
    step(&mut unlinked, [Controller::default(); 2]);
    assert_eq!(
        linked.state().fighters.each_ref().map(|f| f.nudge[0]),
        [-0.25, 0.25]
    );
    assert_eq!(
        unlinked.state().fighters.each_ref().map(|f| f.nudge),
        [[0.0; 2]; 2]
    );
}

#[test]
fn gameplay_depth_changes_bone_attached_hit_detection() {
    let mut separated = data([[-2.0, 0.0], [2.0, 0.0]]);
    separated.rules.nudge.as_mut().unwrap().horizontal_step = 0.0;
    separated.rules.nudge.as_mut().unwrap().depth_step = 1.0;
    separated.rules.nudge.as_mut().unwrap().depth_limit = 2.0;
    for fighter in &mut separated.fighters {
        fighter.nudge.as_mut().unwrap().half_width = 3.0;
    }
    let mut control = separated.clone();
    control.rules.nudge = None;
    for fighter in &mut control.fighters {
        fighter.nudge = None;
    }
    let mut separated = Match::new(separated, 0).unwrap();
    let mut control = Match::new(control, 0).unwrap();
    step(&mut separated, [Controller::default(); 2]);
    step(&mut control, [Controller::default(); 2]);
    let attack = [
        Controller {
            buttons: BUTTON_A,
            ..Default::default()
        },
        Controller::default(),
    ];
    step(&mut separated, attack);
    step(&mut control, attack);
    step(&mut separated, [Controller::default(); 2]);
    step(&mut control, [Controller::default(); 2]);
    assert_eq!(separated.state().fighters[1].percent, 0.0);
    assert!(control.state().fighters[1].percent > 0.0);
    assert_eq!(
        separated.state().fighters.each_ref().map(|f| f.depth),
        [-2.0, 2.0]
    );
}

#[test]
fn checkpoint_and_reset_preserve_depth_and_sampled_push_velocity() {
    let resource = data([[0.0, 0.0], [1.75, 0.0]]);
    let mut game = Match::new(resource.clone(), 19).unwrap();
    step(&mut game, [Controller::default(); 2]);
    let checkpoint = game.checkpoint();
    let expected = (0..6)
        .map(|_| {
            step(&mut game, [Controller::default(); 2]);
            serde_json::to_string(game.state()).unwrap()
        })
        .collect::<Vec<_>>();
    game.restore_checkpoint(&checkpoint).unwrap();
    let actual = (0..6)
        .map(|_| {
            step(&mut game, [Controller::default(); 2]);
            serde_json::to_string(game.state()).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    game.reset(19);
    let fresh = Match::new(resource, 19).unwrap();
    assert_eq!(game.state(), fresh.state());
}

#[test]
fn nudge_rules_and_fighter_attributes_must_be_enabled_together() {
    let mut resource = data([[0.0, 0.0], [2.0, 0.0]]);
    resource.fighters[0].nudge = None;
    assert!(Match::new(resource.clone(), 0).is_err());
    resource.rules.nudge = None;
    assert!(Match::new(resource.clone(), 0).is_err());
    for fighter in &mut resource.fighters {
        fighter.nudge = None;
    }
    assert!(Match::new(resource, 0).is_ok());
}
