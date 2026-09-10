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

/// `ftCo_800986B0`: buffered physical-L/R tech eligibility. The current byte
/// age is promoted to float; the previous byte and repeat boundary stay ints.
pub fn can_tech(
    input_locked: bool,
    press_age: u8,
    previous_press_age: u8,
    window: f32,
    repeat_lockout: i32,
) -> bool {
    !input_locked
        && f32::from(press_age) < window
        && i32::from(previous_press_age) >= repeat_lockout
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TechRoll {
    Forward,
    Backward,
}

/// Directional portion of `ftCo_80098928`. The caller owns the already-tested
/// `ftCo_800986B0` tech predicate and the subsequent action transition.
pub fn tech_roll_direction(stick_x: f32, facing: f32, threshold: f32) -> Option<TechRoll> {
    (super::compat::comparison_abs(stick_x) >= threshold).then_some(if stick_x * facing >= 0.0 {
        TechRoll::Forward
    } else {
        TechRoll::Backward
    })
}

/// `ftCo_800DF644`: the C-stick's Y sample newly crosses the upward threshold.
pub fn fresh_up_cstick(previous_y: f32, current_y: f32, threshold: f32) -> bool {
    previous_y < threshold && current_y >= threshold
}

/// `ftCo_800DF678`: a new horizontal C-stick excursion whose angle is below
/// the common get-up vertical boundary.
pub fn fresh_horizontal_cstick(
    previous: [f32; 2],
    current: [f32; 2],
    horizontal_threshold: f32,
    vertical_angle: f32,
) -> bool {
    super::compat::comparison_abs(previous[0]) < horizontal_threshold
        && super::compat::comparison_abs(current[0]) >= horizontal_threshold
        && super::aerial::stick_angle(current) < vertical_angle
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnockdownInput {
    pub main: [f32; 2],
    pub cstick: [f32; 2],
    pub previous_cstick: [f32; 2],
    pub facing: f32,
    pub attack_pressed: bool,
    pub shoulder_pressed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnockdownRules {
    pub horizontal_stick_threshold: f32,
    pub stand_stick_threshold: f32,
    pub vertical_angle_radians: f32,
    pub attack_cstick_threshold: f32,
    pub bound_attack_window: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnockdownOption {
    Attack,
    Forward,
    Backward,
    Stand,
}

/// Refactored DownWait IASA composition. The original callback gives get-up
/// attack priority, then a fresh C-stick or held main-stick roll, then stand.
pub fn knockdown_option(input: KnockdownInput, rules: &KnockdownRules) -> Option<KnockdownOption> {
    if input.attack_pressed
        || fresh_up_cstick(
            input.previous_cstick[1],
            input.cstick[1],
            rules.attack_cstick_threshold,
        )
    {
        return Some(KnockdownOption::Attack);
    }
    if let Some(option) = knockdown_roll(input, rules) {
        return Some(option);
    }
    ((input.main[1] >= rules.stand_stick_threshold
        && super::aerial::stick_angle(input.main) >= rules.vertical_angle_radians)
        || input.shoulder_pressed)
        .then_some(KnockdownOption::Stand)
}

/// DownBound's animation-end input branch. Fresh A/B ages are reset when the
/// bound begins; either buffered button or a fresh upward C-stick beats a roll.
pub fn down_bound_option(
    input: KnockdownInput,
    attack_ages: [u8; 2],
    rules: &KnockdownRules,
) -> Option<KnockdownOption> {
    if attack_ages
        .into_iter()
        .any(|age| f32::from(age) < rules.bound_attack_window)
        || fresh_up_cstick(
            input.previous_cstick[1],
            input.cstick[1],
            rules.attack_cstick_threshold,
        )
    {
        Some(KnockdownOption::Attack)
    } else {
        knockdown_roll(input, rules)
    }
}

fn knockdown_roll(input: KnockdownInput, rules: &KnockdownRules) -> Option<KnockdownOption> {
    let stick_x = if fresh_horizontal_cstick(
        input.previous_cstick,
        input.cstick,
        rules.horizontal_stick_threshold,
        rules.vertical_angle_radians,
    ) {
        Some(input.cstick[0])
    } else if super::compat::comparison_abs(input.main[0]) >= rules.horizontal_stick_threshold
        && super::aerial::stick_angle(input.main) < rules.vertical_angle_radians
    {
        Some(input.main[0])
    } else {
        None
    };
    stick_x.map(|stick_x| {
        if stick_x * input.facing >= 0.0 {
            KnockdownOption::Forward
        } else {
            KnockdownOption::Backward
        }
    })
}

/// `ftCo_800C1E0C`: a recent X/Y press or an upward stick at the inclusive
/// threshold upgrades a wall tech to its jump variant.
pub fn wall_tech_jumps(
    jump_press_age: u8,
    stick_y: f32,
    input_window: f32,
    stick_threshold: f32,
) -> bool {
    f32::from(jump_press_age) < input_window || stick_y >= stick_threshold
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reflection {
    pub knockback: [f32; 2],
    pub facing: f32,
}

/// Velocity portion of `ftCo_800C18A8`: combine self/knockback velocity,
/// mirror it across the contact plane, scale it, and choose reflected facing.
pub fn reflect_velocity(
    self_velocity: [f32; 2],
    knockback: [f32; 2],
    normal: [f32; 2],
    multiplier: f32,
) -> Reflection {
    let mut reflected = [
        self_velocity[0] + knockback[0],
        self_velocity[1] + knockback[1],
    ];
    let projection = (normal[0] * reflected[0] + normal[1] * reflected[1]) * -2.0;
    reflected[0] += normal[0] * projection;
    reflected[1] += normal[1] * projection;
    reflected[0] *= multiplier;
    reflected[1] *= multiplier;
    Reflection {
        knockback: reflected,
        facing: if reflected[0] < 0.0 { -1.0 } else { 1.0 },
    }
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

    #[test]
    fn tech_gate_uses_strict_window_and_inclusive_repeat_boundary() {
        assert!(can_tech(false, 2, 7, 3.0, 7));
        assert!(!can_tech(false, 3, 7, 3.0, 7));
        assert!(!can_tech(false, 2, 6, 3.0, 7));
        assert!(!can_tech(true, 0, 255, 3.0, 7));
    }

    #[test]
    fn wall_tech_jump_uses_strict_timer_and_inclusive_stick_boundaries() {
        assert!(wall_tech_jumps(2, 0.0, 3.0, 0.8));
        assert!(!wall_tech_jumps(3, 0.799, 3.0, 0.8));
        assert!(wall_tech_jumps(3, 0.8, 3.0, 0.8));
    }

    #[test]
    fn floor_tech_roll_uses_inclusive_threshold_and_relative_facing() {
        assert_eq!(tech_roll_direction(0.699, 1.0, 0.7), None);
        assert_eq!(tech_roll_direction(0.7, 1.0, 0.7), Some(TechRoll::Forward));
        assert_eq!(
            tech_roll_direction(0.7, -1.0, 0.7),
            Some(TechRoll::Backward)
        );
        assert_eq!(
            tech_roll_direction(-0.7, -1.0, 0.7),
            Some(TechRoll::Forward)
        );
    }

    #[test]
    fn exact_knockdown_cstick_predicates_preserve_source_boundaries() {
        assert!(!fresh_up_cstick(0.8, 0.9, 0.8));
        assert!(fresh_up_cstick(0.799, 0.8, 0.8));
        assert!(!fresh_horizontal_cstick([0.7, 0.0], [1.0, 0.0], 0.7, 0.8));
        assert!(fresh_horizontal_cstick([0.699, 0.0], [-0.7, 0.0], 0.7, 0.8));
        assert!(!fresh_horizontal_cstick([0.0, 0.0], [0.7, 1.0], 0.7, 0.8));
    }

    #[test]
    fn knockdown_option_keeps_attack_roll_stand_priority_and_relative_facing() {
        let rules = KnockdownRules {
            horizontal_stick_threshold: 0.7,
            stand_stick_threshold: 0.7,
            vertical_angle_radians: 0.8,
            attack_cstick_threshold: 0.8,
            bound_attack_window: 4.0,
        };
        let mut input = KnockdownInput {
            main: [-0.7, 0.0],
            cstick: [0.0, 0.0],
            previous_cstick: [0.0, 0.0],
            facing: -1.0,
            attack_pressed: false,
            shoulder_pressed: true,
        };
        assert_eq!(
            knockdown_option(input, &rules),
            Some(KnockdownOption::Forward)
        );
        input.attack_pressed = true;
        assert_eq!(
            knockdown_option(input, &rules),
            Some(KnockdownOption::Attack)
        );
        input.attack_pressed = false;
        input.main = [0.0, 0.7];
        assert_eq!(
            knockdown_option(input, &rules),
            Some(KnockdownOption::Stand)
        );
        input.main = [0.0, 0.0];
        input.shoulder_pressed = false;
        assert_eq!(knockdown_option(input, &rules), None);

        input.main = [-0.7, 0.0];
        assert_eq!(
            down_bound_option(input, [3, 255], &rules),
            Some(KnockdownOption::Attack)
        );
        assert_eq!(
            down_bound_option(input, [4, 255], &rules),
            Some(KnockdownOption::Forward)
        );
        input.main = [0.0, 0.7];
        assert_eq!(down_bound_option(input, [4, 255], &rules), None);
    }

    #[test]
    fn reflection_combines_velocities_before_mirroring_and_chooses_facing() {
        assert_eq!(
            reflect_velocity([1.0, 2.0], [3.0, -1.0], [-1.0, 0.0], 0.8),
            Reflection {
                knockback: [-3.2, 0.8],
                facing: -1.0,
            }
        );
        assert_eq!(
            reflect_velocity([-0.0; 2], [0.0; 2], [0.0, -1.0], 1.0).facing,
            1.0
        );
    }
}
