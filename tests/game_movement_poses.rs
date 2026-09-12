//! End-to-end per-frame movement poses (`docs/movement-poses.md`,
//! `game::data::MovementPoses`) in an explicitly synthetic native world.
//! `tests/fixtures/game/integration-match.json`'s two-bone skeleton (bone 0
//! is the untranslated root; bone 1 is its child, offset `[0.0, 1.0, 0.0]`
//! in the rest pose) is repurposed with a `CollisionBox::Bones` config that
//! samples bone 1 alone (never bone 0), so the environmental collision box's
//! bottom genuinely follows whichever pose supplies bone 1's translation,
//! matching `ecb::load_joints`/`mpColl_LoadECB_JObj`'s own six-joint
//! min/max sweep -- unlike the real `fox-fd.slp` pack, whose Fox always
//! includes bone 0 (offset `[0,0,0]` in every exported pose, `docs/
//! movement-poses.md`) among its own six `collision_box.indices`, pinning
//! its own ECB bottom to exactly `position.y` regardless of animation (see
//! `docs/parity.md`'s measured first divergence for that specific,
//! upstream-ECB-batch limitation). This fixture avoids that limitation on
//! purpose, to exercise the pose-selection wiring this batch adds on its
//! own merits.
use skirmish::collision::ecb::JointParameters;
use skirmish::game::{
    Action, Match,
    data::{Bone, CollisionBox, MatchData, MovementPoses},
};

const GRAVITY: f32 = 0.2;
const TERMINAL_VELOCITY: f32 = 2.0;
/// `collision_box.parameters.width_threshold`/`height_threshold`: 0.0 so the
/// four-unit minimum clamp (`ecb::load_joints`) is the only floor, never
/// this fixture's own value.
const NO_WIDTH_OR_HEIGHT_FLOOR: f32 = 0.0;

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    // Spawn fighter 0 well above the floor (`y = 0`), unsupported at frame
    // 0: `collision::initialize` finds no floor contact and assigns
    // `Action::Fall` from the very first frame, exactly like a real
    // mid-air drop -- no jump mechanics needed to reach Fall.
    data.stage.spawns[0] = [-5.0, 8.0];
    data.rules.countdown_frames = 0;
    data.fighters[0].movement.gravity = GRAVITY;
    data.fighters[0].movement.terminal_velocity = TERMINAL_VELOCITY;
    data.fighters[0].collision_box = CollisionBox::Bones {
        indices: [1, 1, 1, 1, 1, 1],
        parameters: JointParameters {
            side_y_offset: 0.0,
            height_threshold: NO_WIDTH_OR_HEIGHT_FLOOR,
            width_threshold: NO_WIDTH_OR_HEIGHT_FLOOR,
        },
        flags: 0,
    };
    data
}

fn bone(y: f32) -> Bone {
    Bone {
        parent: Some(0),
        classical_scale: false,
        translation: [0.0, y, 0.0],
        rotation: [0.0, 0.0, 0.0],
        scale: [1.0, 1.0, 1.0],
    }
}

fn root() -> Bone {
    Bone {
        parent: None,
        classical_scale: false,
        translation: [0.0, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0],
        scale: [1.0, 1.0, 1.0],
    }
}

/// Steps fighter 0 (fighter 1's own idle Wait never interacts) until it
/// lands, returning the 1-based frame count. Panics past a generous bound,
/// since a fixture bug should fail loudly rather than loop forever.
fn frames_until_landing(game: &mut Match) -> u32 {
    for frame in 1..=200 {
        let state = game.step([Default::default(), Default::default()]).unwrap();
        if state.fighters[0].action == Action::Landing {
            return frame;
        }
    }
    unreachable!("fixture must land within two hundred frames");
}

#[test]
fn a_fall_pose_whose_sampled_bone_sits_above_the_rest_pose_lands_later() {
    let baseline = data();
    assert_eq!(
        Match::new(baseline.clone(), 0).unwrap().state().fighters[0].action,
        Action::Fall
    );
    let baseline_frame = frames_until_landing(&mut Match::new(baseline, 0).unwrap());

    let mut with_pose = data();
    // Bone 1 sits six units above the root every Fall frame (vs. the rest
    // pose's one unit): `ecb::load_joints`'s own four-unit padding
    // (`flags & 4 == 0`) then leaves a genuinely positive ECB bottom
    // (6.0 - 2.0 = 4.0) instead of the rest pose's (1.0 - 2.0 = -1.0,
    // clamped to 0.0 by the same "if bottom < 0.0" rule
    // `mpColl_LoadECB_JObj` uses, `mpcoll.c:425-433`) -- so this fighter's
    // collision-box bottom sweeps across the floor a full four units of
    // fall distance later than the rest pose's, landing on a later frame.
    let fall_frames = vec![vec![root(), bone(6.0)]; 4];
    with_pose.fighters[0].movement_poses = Some(MovementPoses {
        fall: Some(fall_frames),
        ..Default::default()
    });
    let with_pose_frame = frames_until_landing(&mut Match::new(with_pose, 0).unwrap());

    assert!(
        with_pose_frame > baseline_frame,
        "pose-driven ECB bottom (4.0 above the rest pose's 0.0, post-clamp) \
         should delay ground contact, but baseline landed at frame \
         {baseline_frame} and the posed fighter landed at frame {with_pose_frame}"
    );
}

#[test]
fn an_empty_movement_poses_resource_keeps_the_rest_pose_fallback() {
    let baseline_frame = frames_until_landing(&mut Match::new(data(), 0).unwrap());

    let mut explicit_default = data();
    explicit_default.fighters[0].movement_poses = Some(MovementPoses::default());
    let default_frame = frames_until_landing(&mut Match::new(explicit_default, 0).unwrap());

    assert_eq!(
        baseline_frame, default_frame,
        "movement_poses present but every field absent must behave exactly \
         like movement_poses: None"
    );
}

#[test]
fn validation_rejects_an_empty_frame_list() {
    let mut invalid = data();
    invalid.fighters[0].movement_poses = Some(MovementPoses {
        fall: Some(vec![]),
        ..Default::default()
    });
    assert!(Match::new(invalid, 0).is_err());
}

#[test]
fn validation_rejects_a_bone_count_that_does_not_match_the_skeleton() {
    let mut invalid = data();
    // The fixture's skeleton has two bones (root and its one child); one
    // bone short must be rejected.
    invalid.fighters[0].movement_poses = Some(MovementPoses {
        fall: Some(vec![vec![root()]]),
        ..Default::default()
    });
    assert!(Match::new(invalid, 0).is_err());
}
