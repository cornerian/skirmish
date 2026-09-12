//! Exact stick-region decision retained from `ftCo_8009AAFC`, plus the real
//! per-frame ledge-catch box query retained from `mpColl_80044164`/
//! `mpColl_800443C4`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickOption {
    Climb,
    Drop,
}

/// Select a main/C-stick ledge option after the caller applies the magnitude gate.
pub fn stick_option(
    main_stick: bool,
    input_ready: bool,
    stick_x: f32,
    angle: f32,
    facing: f32,
    angle_threshold: f32,
) -> Option<StickOption> {
    if angle > angle_threshold || (angle > -angle_threshold && stick_x * facing >= 0.0) {
        (main_stick && input_ready).then_some(StickOption::Climb)
    } else {
        input_ready.then_some(StickOption::Drop)
    }
}

/// `mpColl_80044164`'s bounding box (`mp/mpcoll.c:1250-1266`), reached every
/// airborne frame that isn't already grounded from
/// `mpColl_80046904`'s `!touched_floor && CollisionFlagAir_CanGrabLedge`
/// branch (`mpcoll.c:2516-2547`) when the fighter faces right
/// (`Collide_LeftLedgeGrab`, the stage's left-hand ledge). Branch structure
/// (rather than `min`/`max`) is retained on purpose: with an exotic (e.g.
/// NaN) position or previous position, `f32::min`/`max` and the source's
/// strict `<` branch disagree about which operand survives.
fn snap_box_left(
    position: [f32; 2],
    previous_position: [f32; 2],
    ecb_right_x: f32,
    snap_x: f32,
    snap_y: f32,
    snap_height: f32,
) -> [f32; 4] {
    let half_height = 0.5 * snap_height;
    let (left, right) = if previous_position[0] < position[0] {
        (previous_position[0], snap_x + (position[0] + ecb_right_x))
    } else {
        (position[0], snap_x + (previous_position[0] + ecb_right_x))
    };
    let (bottom, top) = if previous_position[1] < position[1] {
        (
            (previous_position[1] + snap_y) - half_height,
            half_height + (position[1] + snap_y),
        )
    } else {
        (
            (position[1] + snap_y) - half_height,
            half_height + (previous_position[1] + snap_y),
        )
    };
    [left, bottom, right, top]
}

/// `mpColl_800443C4`'s mirrored bounding box (`mp/mpcoll.c:1326-1344`),
/// tested when the fighter faces left (`Collide_RightLedgeGrab`, the
/// stage's right-hand ledge). See `snap_box_left` for the branch-vs-`min`/
/// `max` note.
fn snap_box_right(
    position: [f32; 2],
    previous_position: [f32; 2],
    ecb_left_x: f32,
    snap_x: f32,
    snap_y: f32,
    snap_height: f32,
) -> [f32; 4] {
    let half_height = 0.5 * snap_height;
    let negated_snap_x = -snap_x;
    let (left, right) = if previous_position[0] > position[0] {
        (
            negated_snap_x + (position[0] + ecb_left_x),
            previous_position[0],
        )
    } else {
        (
            negated_snap_x + (previous_position[0] + ecb_left_x),
            position[0],
        )
    };
    let (bottom, top) = if previous_position[1] < position[1] {
        (
            (previous_position[1] + snap_y) - half_height,
            half_height + (position[1] + snap_y),
        )
    } else {
        (
            (position[1] + snap_y) - half_height,
            half_height + (previous_position[1] + snap_y),
        )
    };
    [left, bottom, right, top]
}

/// `mpColl_80044164` (`mp/mpcoll.c:1250-1292`): does the fighter catch the
/// ledge at `edge` (a stage's left-hand/`Side::Left` endpoint, `mpFloorGetLeft`'s
/// point)? The source's own floor-database box search
/// (`mpLib_80051BA8_Floor`) and its two paired `mpCheckMultiple`
/// line-of-sight checks exist to reject a *different*, obstructed floor the
/// box might otherwise contain; `game::ledge::scan` already knows it is
/// testing one single, isolated, disconnected ledge endpoint (an eligible
/// stage line has no connected neighbor), so that candidate's box can only
/// ever contain its own tip, with nothing to obstruct line of sight to it.
/// Under that reduction the source's `contact.x - edge.x < 5.0F` tolerance
/// against the (elsewhere-searched) contact point is always true (contact
/// and edge coincide), leaving only the box-containment and the two direct
/// position/ECB-vs-edge comparisons below. `tests/oracle/ledge_snap.c` pins
/// the full original function under a forced no-obstruction scenario and
/// checks both this box and this reduction against it.
#[allow(clippy::too_many_arguments)]
pub fn snap_catch_left(
    position: [f32; 2],
    previous_position: [f32; 2],
    ecb_right_x: f32,
    ecb_bottom: [f32; 2],
    snap_x: f32,
    snap_y: f32,
    snap_height: f32,
    edge: [f32; 2],
) -> bool {
    let [left, bottom, right, top] = snap_box_left(
        position,
        previous_position,
        ecb_right_x,
        snap_x,
        snap_y,
        snap_height,
    );
    (left..=right).contains(&edge[0])
        && (bottom..=top).contains(&edge[1])
        && position[0] + ecb_bottom[0] < edge[0]
        && position[1] + ecb_bottom[1] < edge[1]
}

/// `mpColl_800443C4` (`mp/mpcoll.c:1326-1368`), mirroring `snap_catch_left`
/// for a stage's right-hand/`Side::Right` endpoint (`mpFloorGetRight`'s
/// point). See `snap_catch_left` for the box-search/obstruction reduction
/// this retains.
#[allow(clippy::too_many_arguments)]
pub fn snap_catch_right(
    position: [f32; 2],
    previous_position: [f32; 2],
    ecb_left_x: f32,
    ecb_bottom: [f32; 2],
    snap_x: f32,
    snap_y: f32,
    snap_height: f32,
    edge: [f32; 2],
) -> bool {
    let [left, bottom, right, top] = snap_box_right(
        position,
        previous_position,
        ecb_left_x,
        snap_x,
        snap_y,
        snap_height,
    );
    (left..=right).contains(&edge[0])
        && (bottom..=top).contains(&edge[1])
        && position[0] + ecb_bottom[0] > edge[0]
        && position[1] + ecb_bottom[1] < edge[1]
}

#[cfg(any(test, feature = "c-oracle"))]
pub use box_bounds::{box_left, box_right};

#[cfg(any(test, feature = "c-oracle"))]
mod box_bounds {
    //! Test-only access to the box arithmetic, kept private in ordinary
    //! builds since `snap_catch_left`/`_right` are the only real callers.
    pub fn box_left(
        position: [f32; 2],
        previous_position: [f32; 2],
        ecb_right_x: f32,
        snap_x: f32,
        snap_y: f32,
        snap_height: f32,
    ) -> [f32; 4] {
        super::snap_box_left(
            position,
            previous_position,
            ecb_right_x,
            snap_x,
            snap_y,
            snap_height,
        )
    }

    pub fn box_right(
        position: [f32; 2],
        previous_position: [f32; 2],
        ecb_left_x: f32,
        snap_x: f32,
        snap_y: f32,
        snap_height: f32,
    ) -> [f32; 4] {
        super::snap_box_right(
            position,
            previous_position,
            ecb_left_x,
            snap_x,
            snap_y,
            snap_height,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_regions_preserve_strict_angle_and_main_stick_climb_gate() {
        let threshold = 0.7;
        assert_eq!(
            stick_option(true, true, 1.0, 0.0, 1.0, threshold),
            Some(StickOption::Climb)
        );
        assert_eq!(
            stick_option(true, true, 0.0, -1.0, 1.0, threshold),
            Some(StickOption::Drop)
        );
        assert_eq!(stick_option(false, true, 1.0, 0.0, 1.0, threshold), None);
        assert_eq!(
            stick_option(true, false, 1.0, threshold, 1.0, threshold),
            None
        );
    }

    #[test]
    fn falling_toward_a_left_ledge_catches_only_within_its_box() {
        // ecb.right.x = 8, ecb.bottom = (-6, -30); one unit of fall this
        // frame, with a wide left-to-right sweep so the box's left bound
        // (the previous position) sits well left of the ECB-bottom crossing
        // threshold below.
        let position = [0.0, -1.0];
        let previous_position = [-20.0, 0.0];
        let ecb_right_x = 8.0;
        let ecb_bottom = [-6.0, -30.0];
        let (snap_x, snap_y, snap_height) = (10.0, 4.0, 8.0);

        // The tip sits just past the widened right bound and just below the
        // snap band's top: caught.
        assert!(snap_catch_left(
            position,
            previous_position,
            ecb_right_x,
            ecb_bottom,
            snap_x,
            snap_y,
            snap_height,
            [17.0, 3.0],
        ));

        // Same tip height and comfortably inside the box, but the fighter's
        // ECB bottom (position.x + ecb_bottom.x = -6) has already crossed
        // past it (no longer strictly outside the platform): rejected.
        assert!(!snap_catch_left(
            position,
            previous_position,
            ecb_right_x,
            ecb_bottom,
            snap_x,
            snap_y,
            snap_height,
            [-10.0, 3.0],
        ));

        // Far outside the horizontal box: rejected.
        assert!(!snap_catch_left(
            position,
            previous_position,
            ecb_right_x,
            ecb_bottom,
            snap_x,
            snap_y,
            snap_height,
            [100.0, 3.0],
        ));
    }

    #[test]
    fn falling_toward_a_right_ledge_mirrors_the_left_box() {
        let position = [0.0, -1.0];
        let previous_position = [0.0, 0.0];
        let ecb_left_x = -8.0;
        let ecb_bottom = [6.0, -30.0];
        let (snap_x, snap_y, snap_height) = (10.0, 4.0, 8.0);

        assert!(snap_catch_right(
            position,
            previous_position,
            ecb_left_x,
            ecb_bottom,
            snap_x,
            snap_y,
            snap_height,
            [-17.0, 3.0],
        ));
        assert!(!snap_catch_right(
            position,
            previous_position,
            ecb_left_x,
            ecb_bottom,
            snap_x,
            snap_y,
            snap_height,
            [10.0, 3.0],
        ));
    }

    #[test]
    fn a_nan_endpoint_of_the_frame_never_reports_a_catch() {
        // Range containment against a NaN edge is false either way, matching
        // the source's own strict comparisons never being true against NaN.
        assert!(!snap_catch_left(
            [0.0, -1.0],
            [0.0, 0.0],
            8.0,
            [-6.0, -30.0],
            10.0,
            4.0,
            8.0,
            [f32::NAN, 3.0],
        ));
    }
}
