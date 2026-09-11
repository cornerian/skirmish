//! Pure floor-end clamp arithmetic from `mpColl_8004A45C_Floor` (mode 2,
//! always clamp) and `mpColl_8004A678_Floor` (mode 1, teeter), plus the
//! teeter walk predicate from `ftCo_Walk_CheckInput_Ottotto` and the shared
//! exit-distance check from `ftCo_Ottotto_Coll`/`ftCo_OttottoWait_Coll`.
//! Wall-blocking and stage geometry queries are the caller's job (they need
//! `crate::collision::stage`, which this module does not depend on); this
//! module only makes the floor-end decision `inline2(coll, mode)` selects.
//! See `docs/edges.md` for every source line.

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
