//! Floor-end collision modes and the edge teeter: `game`-layer wiring around
//! the pure arithmetic in `crate::fighter::edge`. See `docs/edges.md` for the
//! per-action mode table (every `_Coll` callback cited) and the Ottotto/
//! OttottoWait entry, IASA and exit-distance sources.
use crate::collision::stage;
use crate::game::{
    Action, Error, Fighter,
    data::{Bone, FighterData, HurtboxState},
};
use serde::{Deserialize, Serialize};

pub use Side as EdgeSide;

/// Common edge/teeter data (`ftCommonData.x474`/`x478`/`x47C` plus the
/// mode-1 literal 0.75, supplied so the resource is explicit).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// mpcoll.c's mode-1 literal: exclusive facing-relative stick magnitude
    /// admitting Ottotto (`coll->lstick_x > -0.75` / `< 0.75`).
    pub teeter_stick_limit: f32,
    /// `+474`: inclusive facing-relative stick magnitude for the teeter
    /// walk override (`ftCo_Walk_CheckInput_Ottotto`).
    pub teeter_walk_threshold: f32,
    /// `+478`: exit-to-Wait distance from the current floor's facing-side end.
    pub teeter_exit_distance: f32,
    /// `+47C`: exit-to-Wait tolerance added to the distance above.
    pub teeter_exit_tolerance: f32,
}

/// Per-fighter Ottotto/OttottoWait poses: bones and hurtbox samples, with no
/// hitboxes since neither state can hit anything.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Teeter {
    /// One sample per Ottotto frame (`animation 210`); the last is used on
    /// the frame `action_frame` reaches it, immediately before OttottoWait.
    pub start: Vec<TeeterFrame>,
    /// OttottoWait's single, indefinitely held pose (`animation 211`).
    pub wait: TeeterFrame,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeeterFrame {
    pub bones: Vec<Bone>,
    /// Empty inherits each hurtbox's base state, exactly like `AttackFrame`.
    #[serde(default)]
    pub hurtbox_states: Vec<HurtboxState>,
}

pub(crate) fn validate(rules: &Rules, teeter: &Teeter, fighter: &FighterData) -> Result<(), Error> {
    if !(rules.teeter_stick_limit > 0.0 && rules.teeter_stick_limit <= 1.0)
        || !(0.0..=1.0).contains(&rules.teeter_walk_threshold)
        || !(rules.teeter_exit_distance.is_finite() && rules.teeter_exit_distance >= 0.0)
        || !(rules.teeter_exit_tolerance.is_finite() && rules.teeter_exit_tolerance >= 0.0)
    {
        return Err(Error::Data("invalid edge/teeter rules".into()));
    }
    if teeter.start.is_empty() || teeter.start.len() > 4096 {
        return Err(Error::Data(
            "Ottotto requires 1..4096 physics samples".into(),
        ));
    }
    for frame in teeter.start.iter().chain([&teeter.wait]) {
        crate::game::validation::validate_animation_pose(&frame.bones, fighter)?;
        if !frame.hurtbox_states.is_empty() && frame.hurtbox_states.len() != fighter.hurtboxes.len()
        {
            return Err(Error::Data(
                "teeter hurtbox state samples must be empty or complete".into(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(action, Action::Ottotto | Action::OttottoWait)
}

/// `inline2(coll, mode)`'s mode selection, derived from the fighter's
/// action. See docs/edges.md's per-action table for every `_Coll` callback
/// cited. `rules.edge` absent: mode 2 needs no data and still applies; mode
/// 1 (no teeter states to enter) degrades to mode 0.
pub(crate) fn mode_for_action(
    action: Action,
    data: &crate::game::data::FighterData,
    edge_rules_present: bool,
) -> Mode {
    use Action::*;
    if let Some(mode) = crate::fighter::specials::collision_mode(action, data) {
        return mode;
    }
    match action {
        Jab | Attack12 | Attack13 | Attack100Start | Attack100Loop | Attack100End | AttackS3Hi
        | AttackS3HiS | AttackS3S | AttackS3LwS | AttackS3Lw | AttackHi3 | AttackLw3
        | AttackS4Hi | AttackS4HiS | AttackS4S | AttackS4LwS | AttackS4Lw | AttackHi4
        | AttackLw4 | AttackDash | EscapeF | EscapeB | EscapeN | CatchCut | Catch | CatchDash
        | DownAttack | PassiveStandF | PassiveStandB | CliffClimb | RunTurn | Ottotto
        | OttottoWait | AppealSR | AppealSL => Mode::Clamp,
        Wait | Walk | Landing | RunBrake => {
            if edge_rules_present {
                Mode::Teeter
            } else {
                Mode::Plain
            }
        }
        _ => Mode::Plain,
    }
}

/// The current floor line's left/right endpoints, geometrically sorted
/// (`mpFloorGetLeft`/`mpFloorGetRight`, simplified: no separate
/// neighbor-extension, since `project_floor` already walked the chain and
/// only reaches this code at a genuine, unconnected terminal end).
pub(crate) fn line_ends(line: &stage::Line) -> ([f32; 2], [f32; 2]) {
    if line.start[0] <= line.end[0] {
        (line.start, line.end)
    } else {
        (line.end, line.start)
    }
}

/// `ftCo_8009A410`: `ChangeMotionState(Ottotto)`, zero `self_vel` and
/// `gr_vel`. The caller has already clamped the position.
pub(crate) fn enter(fighter: &mut Fighter) {
    crate::game::simulation::enter(fighter, Action::Ottotto);
    fighter.velocity = [0.0, 0.0];
    fighter.ground_velocity = 0.0;
}

/// `ftCo_Ottotto_Anim`: at the last supplied Ottotto sample, `ftCo_8009A6B8`
/// enters OttottoWait (sound unmodeled). OttottoWait has no Anim callback --
/// it holds its last pose indefinitely, so this only ever fires from Ottotto.
pub(crate) fn update_animation(fighter: &mut Fighter, data: &FighterData) -> Result<(), Error> {
    if fighter.action != Action::Ottotto {
        return Ok(());
    }
    let length = data
        .teeter
        .as_ref()
        .ok_or_else(|| Error::Data("Ottotto requires teeter poses".into()))?
        .start
        .len();
    if fighter.action_frame as usize >= length {
        crate::game::simulation::enter(fighter, Action::OttottoWait);
    }
    Ok(())
}

/// `ftCo_Ottotto_Coll`/`ftCo_OttottoWait_Coll`'s exit check, run once per
/// frame after collision settles: `ABS(fp->cur_pos.x - pos.x) > x478 + x47C`
/// against the current floor's end on the facing side (`mpFloorGetRight` for
/// `facing_dir > 0`, `mpFloorGetLeft` otherwise) enters Wait (`ft_8008A2BC`).
pub(crate) fn check_exit(fighter: &mut Fighter, rules: &Rules, floor_end_x: f32) {
    if exit_distance_exceeded(
        fighter.position[0],
        floor_end_x,
        rules.teeter_exit_distance,
        rules.teeter_exit_tolerance,
    ) {
        crate::game::simulation::enter(fighter, Action::Wait);
    }
}

/// Physics bones for the current Ottotto/OttottoWait sample.
pub(crate) fn pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a [Bone]> {
    let teeter = data.teeter.as_ref()?;
    match fighter.action {
        Action::Ottotto => teeter
            .start
            .get(fighter.action_frame as usize)
            .map(|frame| frame.bones.as_slice()),
        Action::OttottoWait => Some(teeter.wait.bones.as_slice()),
        _ => None,
    }
}

/// The current sample's complete hurtbox-state override, if any.
pub(crate) fn hurtbox_frame<'a>(
    fighter: &Fighter,
    data: &'a FighterData,
) -> Option<&'a [HurtboxState]> {
    let teeter = data.teeter.as_ref()?;
    match fighter.action {
        Action::Ottotto => teeter
            .start
            .get(fighter.action_frame as usize)
            .map(|frame| frame.hurtbox_states.as_slice()),
        Action::OttottoWait => Some(teeter.wait.hurtbox_states.as_slice()),
        _ => None,
    }
}

// Pure fighter arithmetic and predicates.
/// Which end of the current floor line a fighter's ECB bottom has passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
}

/// `inline2(coll, mode)`'s three floor-end rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// mode 0 (`mpColl_8004B108`): past the end, the fighter leaves the ground.
    Plain,
    /// mode 2 (`mpColl_8004B2DC`): always clamp; the fighter stays grounded.
    Clamp,
    /// mode 1 (`mpColl_8004B4B0`): clamp and admit Ottotto only when facing
    /// and stick allow it; otherwise the same as `Plain`.
    Teeter,
}

/// A successful floor-end clamp: which side, and (mode 1 only) whether the
/// facing/stick gate that additionally admits Ottotto passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub side: Side,
    pub enter_teeter: bool,
}

/// mpcoll's own `cur_pos.x <= edge.x` (left) then, only on that miss,
/// `cur_pos.x >= edge.x` (right) order. A point still between both ends
/// (should not occur once the caller's ordinary floor projection has
/// already failed, but kept total) reports neither side.
pub fn passed_side(bottom_x: f32, left_x: f32, right_x: f32) -> Option<Side> {
    if bottom_x <= left_x {
        Some(Side::Left)
    } else if bottom_x >= right_x {
        Some(Side::Right)
    } else {
        None
    }
}

/// `ft_80084280_inline`'s per-side teeter gate: no opposing wall-contact
/// flag (folded into `wall_blocked` by the caller alongside the geometric
/// wall check both modes share), the exact facing and the 0.75 stick
/// literal (supplied as `stick_limit`, `rules.edge.teeter_stick_limit`).
pub fn teeter_allowed(side: Side, facing: f32, stick_x: f32, stick_limit: f32) -> bool {
    match side {
        Side::Left => facing == -1.0 && stick_x > -stick_limit,
        Side::Right => facing == 1.0 && stick_x < stick_limit,
    }
}

/// Inputs to [`resolve`], grouped to stay under the lint-checked argument
/// count; every field is one of mpcoll's own `coll->` reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeQuery {
    pub bottom_x: f32,
    pub left_x: f32,
    pub right_x: f32,
    pub facing: f32,
    pub stick_x: f32,
    /// `rules.edge.teeter_stick_limit` (the 0.75 literal); unused by mode 2.
    pub stick_limit: f32,
}

/// The floor-end decision `mpColl_8004A45C_Floor`/`mpColl_8004A678_Floor`
/// make once a side is passed: `wall_blocked` folds in both
/// `mpCheckLeftWall`/`mpCheckRightWall` and, for mode 1, the
/// `Collide_*WallMask` flag check that precedes it in the source (the
/// caller computes both from stage geometry and fighter state).
pub fn resolve(mode: Mode, query: EdgeQuery, wall_blocked: bool) -> Option<Resolution> {
    let EdgeQuery {
        bottom_x,
        left_x,
        right_x,
        facing,
        stick_x,
        stick_limit,
    } = query;
    let side = passed_side(bottom_x, left_x, right_x)?;
    if wall_blocked {
        return None;
    }
    match mode {
        Mode::Plain => None,
        Mode::Clamp => Some(Resolution {
            side,
            enter_teeter: false,
        }),
        Mode::Teeter => teeter_allowed(side, facing, stick_x, stick_limit).then_some(Resolution {
            side,
            enter_teeter: true,
        }),
    }
}

/// `coll->cur_pos.{x,y} = edge.{x,y} - coll->ecb.bottom.{x,y}`.
pub fn clamped_position(edge: [f32; 2], ecb_bottom: [f32; 2]) -> [f32; 2] {
    [edge[0] - ecb_bottom[0], edge[1] - ecb_bottom[1]]
}

/// `ftCo_Walk_CheckInput_Ottotto`'s extra gate, ANDed by the caller with the
/// ordinary walk predicate (`ftWalkCommon_800DFC70`).
pub fn teeter_walk_allowed(stick_x: f32, facing: f32, threshold: f32) -> bool {
    stick_x * facing >= threshold
}

/// The exit-to-Wait check shared by `ftCo_Ottotto_Coll` and
/// `ftCo_OttottoWait_Coll`: `ABS(fp->cur_pos.x - pos.x) > x478 + x47C`.
pub fn exit_distance_exceeded(
    position_x: f32,
    floor_end_x: f32,
    distance: f32,
    tolerance: f32,
) -> bool {
    (position_x - floor_end_x).abs() > distance + tolerance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passed_side_prefers_left_and_requires_a_true_excursion() {
        assert_eq!(passed_side(-1.0, 0.0, 10.0), Some(Side::Left));
        assert_eq!(passed_side(0.0, 0.0, 10.0), Some(Side::Left));
        assert_eq!(passed_side(10.0, 0.0, 10.0), Some(Side::Right));
        assert_eq!(passed_side(11.0, 0.0, 10.0), Some(Side::Right));
        assert_eq!(passed_side(5.0, 0.0, 10.0), None);
    }

    #[test]
    fn teeter_gate_is_exact_facing_and_strict_stick_boundary() {
        assert!(teeter_allowed(Side::Left, -1.0, 0.0, 0.75));
        assert!(teeter_allowed(Side::Left, -1.0, -0.749, 0.75));
        assert!(!teeter_allowed(Side::Left, -1.0, -0.75, 0.75));
        assert!(!teeter_allowed(Side::Left, 1.0, 0.0, 0.75));
        assert!(teeter_allowed(Side::Right, 1.0, 0.0, 0.75));
        assert!(teeter_allowed(Side::Right, 1.0, 0.749, 0.75));
        assert!(!teeter_allowed(Side::Right, 1.0, 0.75, 0.75));
        assert!(!teeter_allowed(Side::Right, -1.0, 0.0, 0.75));
    }

    fn q(bottom_x: f32, facing: f32, stick_x: f32) -> EdgeQuery {
        EdgeQuery {
            bottom_x,
            left_x: 0.0,
            right_x: 10.0,
            facing,
            stick_x,
            stick_limit: 0.75,
        }
    }

    #[test]
    fn resolve_covers_every_mode() {
        assert_eq!(resolve(Mode::Plain, q(-1.0, -1.0, 0.0), false), None);
        assert_eq!(
            resolve(Mode::Clamp, q(-1.0, 1.0, 1.0), false),
            Some(Resolution {
                side: Side::Left,
                enter_teeter: false
            })
        );
        assert_eq!(resolve(Mode::Clamp, q(-1.0, 1.0, 1.0), true), None);
        assert_eq!(
            resolve(Mode::Teeter, q(-1.0, -1.0, 0.0), false),
            Some(Resolution {
                side: Side::Left,
                enter_teeter: true
            })
        );
        // Facing away falls, exactly like mode 0.
        assert_eq!(resolve(Mode::Teeter, q(-1.0, 1.0, 0.0), false), None);
        // Outward stick at the 0.75 literal falls too.
        assert_eq!(resolve(Mode::Teeter, q(11.0, 1.0, 0.75), false), None);
    }

    #[test]
    fn clamped_position_subtracts_ecb_bottom() {
        assert_eq!(clamped_position([10.0, 2.0], [1.0, -3.0]), [9.0, 5.0]);
    }

    #[test]
    fn teeter_walk_and_exit_distance() {
        assert!(teeter_walk_allowed(0.5, 1.0, 0.4));
        assert!(!teeter_walk_allowed(0.3, 1.0, 0.4));
        assert!(exit_distance_exceeded(10.0, 0.0, 5.0, 4.0));
        assert!(!exit_distance_exceeded(9.0, 0.0, 5.0, 4.0));
        assert!(exit_distance_exceeded(9.01, 0.0, 5.0, 4.0));
    }
}
