//! Launch angles, accumulated knockback and hitlag-exit directional influence.
//!
//! These are isolated arithmetic routines, not the complete damage scheduler.
//! Coefficients and game-state decisions are explicit inputs. libm replaces the
//! target transcendental library; host C comparisons use numerical tolerances.

const DEGREES_TO_RADIANS: f32 = f32::from_bits(0x3c8e_fa35);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaunchAngleRules {
    /// ftCommonData::x144_radians: airborne 361 angle.
    pub airborne_radians: f32,
    /// ftCommonData::x148.
    pub grounded_max_degrees: f32,
    /// ftCommonData::x14C.
    pub grounded_low_knockback: f32,
    /// ftCommonData::x150.
    pub grounded_high_knockback: f32,
    /// Inclusive ftCommonData::unk_kb_angle_min/max range, excluding 361.
    pub special_angle_min: u32,
    pub special_angle_max: u32,
    /// ftCommonData::x7F0, narrowed to the original byte storage.
    pub special_timer: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaunchAngle {
    pub radians: f32,
    /// Some(timer) requests damage.x1A=1 and damage.x1B=timer. None leaves both
    /// fields untouched, preserving ftCo_Damage_CalcAngle's conditional writes.
    pub special_timer: Option<u8>,
}

/// ftCo_Damage_CalcAngle plus ftColl_8007AC68's inclusive angle predicate.
/// At the grounded low threshold, angle 361 starts at one degree, then clamps
/// to the supplied maximum. No game/common-data defaults are inferred.
pub fn launch_angle(
    angle: i32,
    knockback: f32,
    airborne: bool,
    rules: &LaunchAngleRules,
) -> LaunchAngle {
    if angle != 361 {
        let unsigned_angle = angle as u32;
        return LaunchAngle {
            radians: angle as f32 * DEGREES_TO_RADIANS,
            special_timer: (rules.special_angle_min <= unsigned_angle
                && unsigned_angle <= rules.special_angle_max)
                .then_some(rules.special_timer as u8),
        };
    }
    let radians = if airborne {
        rules.airborne_radians
    } else if knockback < rules.grounded_low_knockback {
        0.0
    } else {
        let mut result = (rules.grounded_max_degrees
            * ((knockback - rules.grounded_low_knockback)
                / (rules.grounded_high_knockback - rules.grounded_low_knockback))
            + 1.0)
            * DEGREES_TO_RADIANS;
        let cap = rules.grounded_max_degrees * DEGREES_TO_RADIANS;
        if result > cap {
            result = cap;
        }
        result
    };
    LaunchAngle {
        radians,
        special_timer: None,
    }
}

/// ftCo_Damage_CalcVel. During xFC's replacement window both components are
/// replaced; afterward opposite signs add and a larger same-sign input wins.
/// Equality retains the existing component, including its zero sign.
pub fn merge_knockback(
    current: [f32; 2],
    incoming: [f32; 2],
    time_since_hit: i32,
    replace_window: i32,
) -> [f32; 2] {
    if time_since_hit < replace_window {
        return incoming;
    }
    core::array::from_fn(|axis| {
        let (old, new) = (current[axis], incoming[axis]);
        if old * new < 0.0 {
            old + new
        } else if new.abs() > old.abs() {
            new
        } else {
            old
        }
    })
}

/// ftCo_8008E5A4, called from damage's post-hitlag callback after ASDI.
/// The supplied stick is already calibrated. This function neither clamps it
/// nor implements SDI/ASDI, crouch cancellation, or LR/vcancel scaling.
/// The scalar cross-product sign substitutes for PSVECCrossProduct's z output.
pub fn directional_influence(
    velocity: [f32; 2],
    stick: [f32; 2],
    max_angle_degrees: f32,
) -> [f32; 2] {
    let [x, y] = velocity;
    if stick[0] != 0.0 || stick[1] != 0.0 {
        let negative_x = -x;
        let squared_magnitude = negative_x * negative_x + y * y;
        if squared_magnitude.partial_cmp(&0.00001) != Some(core::cmp::Ordering::Less) {
            let perpendicular = y * stick[0] + negative_x * stick[1];
            let mut deflection = perpendicular * perpendicular / squared_magnitude;
            if x * stick[1] - y * stick[0] < 0.0 {
                deflection = -deflection;
            }
            let mut angle = libm::atan2f(y, x);
            let magnitude = libm::sqrtf(x * x + y * y);
            let scale = max_angle_degrees * DEGREES_TO_RADIANS;
            angle += scale * deflection;
            return [magnitude * libm::cosf(angle), magnitude * libm::sinf(angle)];
        }
    }
    velocity
}

/// Ordinary airborne branch of Fighter_procUpdate: no x2228_b2 per-axis mode,
/// no shield recoil, no grounded friction. The caller separately maintains its
/// ground-knockback accumulator. Strict length < decay matches the source;
/// equality still passes through trigonometry and can retain a tiny residual.
pub fn decay_air_knockback(velocity: [f32; 2], decay: f32) -> [f32; 2] {
    let [x, y] = velocity;
    if x != 0.0 || y != 0.0 {
        let angle = libm::atan2f(y, x);
        if libm::sqrtf(x * x + y * y) < decay {
            [0.0; 2]
        } else {
            [x - decay * libm::cosf(angle), y - decay * libm::sinf(angle)]
        }
    } else {
        velocity
    }
}

/// The per-axis x670/x671 input-timer branches in Fighter's input callback.
/// A new excursion or a sign reversal resets the timer. Holding increments its
/// original byte storage before clamping to 254; a corrupt 255 wraps to zero.
pub fn tilt_timer(timer: u8, current: f32, previous: f32, threshold: f32) -> u8 {
    let held = if current >= threshold {
        previous >= threshold
    } else if current <= -threshold {
        previous <= -threshold
    } else {
        return 254;
    };
    if held {
        timer.wrapping_add(1).min(254)
    } else {
        0
    }
}

/// ftCo_Damage_OnEveryHitlag. The magnitude threshold is inclusive, the timer
/// window is exclusive, and a displacement consumes both axis timers. Ground
/// response belongs to the caller; the callback itself adds both coordinates.
pub fn smash_displacement(
    position: &mut [f32; 2],
    stick: [f32; 2],
    timers: &mut [u8; 2],
    allowed: bool,
    minimum_magnitude: f32,
    window: i32,
    distance: f32,
) -> bool {
    if allowed
        && stick[0] * stick[0] + stick[1] * stick[1] >= minimum_magnitude * minimum_magnitude
        && (i32::from(timers[0]) < window || i32::from(timers[1]) < window)
    {
        position[0] += stick[0] * distance;
        position[1] += stick[1] * distance;
        *timers = [254; 2];
        true
    } else {
        false
    }
}

/// The main-stick branch of ftCo_Damage_OnExitHitlag, before DI. C-stick
/// priority, the collision flag callback and LR scaling are outside this helper.
/// Unlike SDI, this branch does not inspect or consume fresh-tilt timers.
pub fn automatic_displacement(
    position: &mut [f32; 2],
    stick: [f32; 2],
    minimum_magnitude: f32,
    distance: f32,
) -> bool {
    if stick[0] * stick[0] + stick[1] * stick[1] >= minimum_magnitude * minimum_magnitude {
        position[0] += stick[0] * distance;
        position[1] += stick[1] * distance;
        true
    } else {
        false
    }
}

/// Ordinary ftCo_Damage_CalcKnockback: no squat, ice, charging, scale or metal
/// modifiers. Nonzero knockback loses the greater armor channel then clamps to
/// the explicit minimum. The source's early return preserves either zero sign.
pub fn subtract_armor(knockback: f32, armor: [f32; 2], minimum: f32) -> f32 {
    if knockback == 0.0 {
        return knockback;
    }
    let armor = if armor[0] > armor[1] {
        armor[0]
    } else {
        armor[1]
    };
    let reduced = knockback - armor;
    if reduced < minimum { minimum } else { reduced }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_threshold_opposite_signs_and_equal_magnitudes() {
        assert_eq!(
            merge_knockback([4.0, -3.0], [-1.0, -2.0], 2, 3),
            [-1.0, -2.0]
        );
        assert_eq!(
            merge_knockback([4.0, -3.0], [-1.0, -2.0], 3, 3),
            [3.0, -3.0]
        );
        assert_eq!(
            merge_knockback([4.0, -3.0], [-4.0, -4.0], 3, 3),
            [0.0, -4.0]
        );
        assert_eq!(
            merge_knockback([-0.0, 0.0], [0.0, -0.0], 3, 3).map(f32::to_bits),
            [(-0.0_f32).to_bits(), 0]
        );
    }

    #[test]
    fn di_turns_toward_perpendicular_stick_and_preserves_noop_bits() {
        let upward = directional_influence([4.0, 0.0], [0.0, 1.0], 20.0);
        let downward = directional_influence([4.0, 0.0], [0.0, -1.0], 20.0);
        assert!(upward[1] > 0.0 && downward[1] < 0.0);
        assert!(
            (libm::sqrtf(upward[0] * upward[0] + upward[1] * upward[1]) - 4.0).abs() < 0.000001
        );
        assert_eq!(
            directional_influence([-0.0, 0.0], [1.0, 1.0], 20.0).map(f32::to_bits),
            [(-0.0_f32).to_bits(), 0]
        );
        assert_eq!(
            decay_air_knockback([-0.0, 0.0], 1.0).map(f32::to_bits),
            [(-0.0_f32).to_bits(), 0]
        );
        assert_eq!(decay_air_knockback([0.5, 0.0], 1.0), [0.0; 2]);
    }
}
