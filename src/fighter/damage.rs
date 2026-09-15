// Damage arithmetic, callback scheduling, and native pose/resource handling.
// Coefficients and game-state decisions are explicit inputs. libm replaces the
// target transcendental library; host C comparisons use numerical tolerances.
use serde::{Deserialize, Serialize};

use crate::game::{
    Action, Error, Fighter,
    data::{Bone, FighterData},
};
const DEGREES_TO_RADIANS: f32 = f32::from_bits(0x3c8e_fa35);
const RADIANS_TO_DEGREES: f32 = f32::from_bits(0x4265_2ee1);

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
        // `x148 * ratio + 1` is a single Gekko `fmadds`
        // (`tools/ppc_fma_audit.py ftCo_Damage_CalcAngle`; `docs/math.md`),
        // computed with `f32::mul_add` for its one rounding; the
        // degrees-to-radians conversion is a separate, unfused multiply.
        let mut result = rules.grounded_max_degrees.mul_add(
            (knockback - rules.grounded_low_knockback)
                / (rules.grounded_high_knockback - rules.grounded_low_knockback),
            1.0,
        ) * DEGREES_TO_RADIANS;
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
            let mut angle = crate::compat::math::trig::atan2f(y, x);
            let magnitude = libm::sqrtf(x * x + y * y);
            let scale = max_angle_degrees * DEGREES_TO_RADIANS;
            angle += scale * deflection;
            return [
                magnitude * crate::compat::math::trig::cosf(angle),
                magnitude * crate::compat::math::trig::sinf(angle),
            ];
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
        let angle = crate::compat::math::trig::atan2f(y, x);
        if libm::sqrtf(x * x + y * y) < decay {
            [0.0; 2]
        } else {
            [
                x - decay * crate::compat::math::trig::cosf(angle),
                y - decay * crate::compat::math::trig::sinf(angle),
            ]
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

/// `ftCommon_CheckFallFast` (`ftcommon.c:492-503`): `!fp->fall_fast &&
/// fp->self_vel.y < 0 && fp->input.lstick[0].y <= -p_ftCommonData->x88
/// (fast_fall_threshold) && fp->x671_timer_lstick_tilt_y <
/// p_ftCommonData->x8C`. `x8C` (an `int`, `types.h:88`, immediately after
/// `x88`/`fast_fall_threshold`) is the same per-frame timer `tilt_timer`
/// (above) already maintains as `fighter.locomotion.tilt_y_age`; `window`
/// is that constant, read directly from the retail disc's own
/// `PlCo.dat`/`ftCommonData` for this batch (`+0x8C` = `4`, a plain `int`
/// like its already-named neighbors `dash_smash_window`/`tap_jump_window`
/// at `+0x40`/`+0x74`, both also small integers).
pub fn fast_fall_trigger(
    already_fast_falling: bool,
    velocity_y: f32,
    stick_y: f32,
    threshold: f32,
    tilt_y_age: u8,
    window: u32,
) -> bool {
    !already_fast_falling
        && velocity_y < 0.0
        && stick_y <= -threshold
        && u32::from(tilt_y_age) < window
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

/// HurtCapsule::height, retained as the source's low/middle/high selector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HurtHeight {
    Low,
    #[default]
    Middle,
    High,
}

impl HurtHeight {
    pub const fn index(self) -> usize {
        match self {
            Self::Low => 0,
            Self::Middle => 1,
            Self::High => 2,
        }
    }
}

/// The 15 distinct ordinary Damage motions in `ftCo_803C5520`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DamageMotion {
    Ground { level: u8, height: HurtHeight },
    Air { level: u8 },
    Fly { height: HurtHeight },
}

/// `ftCo_8008DCE0`'s ordinary knockback-level and motion-table selection.
/// Multiplication happens before the strict comparisons; NaN therefore falls
/// through to the fly row exactly as in the source.
pub fn damage_motion(
    knockback: f32,
    scale: f32,
    thresholds: [f32; 3],
    airborne: bool,
    height: HurtHeight,
) -> DamageMotion {
    let scaled = knockback * scale;
    let level = if scaled < thresholds[0] {
        0
    } else if scaled < thresholds[1] {
        1
    } else if scaled < thresholds[2] {
        2
    } else {
        3
    };
    match (airborne, level) {
        (_, 3) => DamageMotion::Fly { height },
        (true, level) => DamageMotion::Air { level },
        (false, level) => DamageMotion::Ground { level, height },
    }
}

/// Fighter-attacker branch of `ftColl_8007A06C`. The victim faces toward the
/// attacker; equal X, signed zero and unordered comparisons select +1.
pub fn fighter_hit_direction(victim_x: f32, attacker_x: f32) -> f32 {
    if victim_x > attacker_x { -1.0 } else { 1.0 }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionalLaunch {
    pub direction: f32,
    pub angle_degrees: i32,
}

/// Special hitbox angle 362 in `ftColl_8007A06C`. The hurt-capsule midpoint
/// points away from the narrow-phase surface contact. The source truncates the
/// signed degree result toward zero and treats horizontal separations below
/// 1e-5 as angle zero. Inputs are finite evaluated collision geometry.
pub fn positional_launch(
    hurt_start: [f32; 3],
    hurt_end: [f32; 3],
    contact: [f32; 3],
) -> PositionalLaunch {
    let dx = 0.5 * (hurt_start[0] + hurt_end[0]) - contact[0];
    let dy = 0.5 * (hurt_start[1] + hurt_end[1]) - contact[1];
    let direction = if dx < 0.0 { 1.0 } else { -1.0 };
    let abs_dx = if dx < 0.0 { -dx } else { dx };
    let angle_degrees = if abs_dx < 1e-5 {
        0
    } else {
        (crate::compat::math::trig::atanf(dy / abs_dx) * RADIANS_TO_DEGREES) as i32
    };
    PositionalLaunch {
        direction,
        angle_degrees,
    }
}

/// Captured-victim direction assigned by `ftCo_800DDDE4` before throw damage.
pub fn throw_hit_direction(attacker_facing: f32) -> f32 {
    -attacker_facing
}

/// Source motion ID for differential testing and resource-extraction tooling.
pub const fn damage_motion_id(motion: DamageMotion) -> u16 {
    match motion {
        DamageMotion::Ground { level, height } => {
            [[81, 78, 75], [82, 79, 76], [83, 80, 77]][level as usize][height.index()]
        }
        DamageMotion::Air { level } => [84, 85, 86][level as usize],
        DamageMotion::Fly { height } => [89, 88, 87][height.index()],
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundLaunchParameters {
    /// Common x1E8: additional angle past pi/2 before a fly launch bounces.
    pub fly_bounce_angle_radians: f32,
    /// Common x1EC: vertical multiplier applied by that bounce.
    pub fly_bounce_vertical_multiplier: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundLaunch {
    pub airborne: bool,
    pub knockback: [f32; 2],
    pub ground_knockback: f32,
    pub bounced: bool,
}

/// `lbVector_Angle`, including its tiny-vector fallthrough and cosine clamp.
/// The original is three-dimensional; the damage transition supplies z = 0.
/// `acosf` uses `crate::compat::math::trig::acosf`, seeded from a real reciprocal-sqrt
/// estimate rather than this decompilation project's own placeholder-derived
/// (and non-convergent) one; `sqrt` is still `f32::sqrt` (`libm::sqrtf`'s
/// hardware `frsqrte`-estimate equivalent is out of scope for this batch;
/// see `docs/math.md`).
#[allow(clippy::manual_clamp)] // Two ordered source comparisons preserve NaN.
pub fn vector_angle(a: [f32; 2], b: [f32; 2]) -> f32 {
    let length_product = (a[0] * a[0] + a[1] * a[1]).sqrt() * (b[0] * b[0] + b[1] * b[1]).sqrt();
    if length_product > 0.0000000001_f32 {
        let mut cosine = (a[0] * b[0] + a[1] * b[1]) / length_product;
        if cosine > 1.0 {
            cosine = 1.0;
        }
        if cosine < -1.0 {
            cosine = -1.0;
        }
        crate::compat::math::trig::acosf(cosine)
    } else {
        0.0
    }
}

/// Grounded portion of `ftCo_8008DCE0`. Low-level launch at least pi/2 from
/// the floor normal remains grounded and is projected onto the floor tangent.
/// Fly launch always leaves ground and may reverse/scale its vertical component.
pub fn ground_launch(
    knockback: [f32; 2],
    floor_normal: [f32; 2],
    fly: bool,
    rules: &GroundLaunchParameters,
) -> GroundLaunch {
    let floor_angle = vector_angle(floor_normal, knockback);
    if floor_angle < core::f32::consts::FRAC_PI_2 {
        GroundLaunch {
            airborne: true,
            knockback,
            ground_knockback: 0.0,
            bounced: false,
        }
    } else if fly {
        let bounced = f64::from(floor_angle)
            > core::f64::consts::FRAC_PI_2 + f64::from(rules.fly_bounce_angle_radians);
        GroundLaunch {
            airborne: true,
            knockback: if bounced {
                [
                    knockback[0],
                    -knockback[1] * rules.fly_bounce_vertical_multiplier,
                ]
            } else {
                knockback
            },
            ground_knockback: 0.0,
            bounced,
        }
    } else {
        GroundLaunch {
            airborne: false,
            knockback: [
                floor_normal[1] * knockback[0],
                -floor_normal[0] * knockback[0],
            ],
            ground_knockback: knockback[0],
            bounced: false,
        }
    }
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
    (crate::compat::source_ops::comparison_abs(stick_x) >= threshold).then_some(
        if stick_x * facing >= 0.0 {
            TechRoll::Forward
        } else {
            TechRoll::Backward
        },
    )
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
    crate::compat::source_ops::comparison_abs(previous[0]) < horizontal_threshold
        && crate::compat::source_ops::comparison_abs(current[0]) >= horizontal_threshold
        && crate::fighter::aerial::stick_angle(current) < vertical_angle
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
pub struct KnockdownParameters {
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

/// `ftCo_80097570` plus the caller's per-fighter inversion flag. The source
/// selects one column from the evaluated HipN matrix and uses a strict sign
/// test to choose the face-up recovery family.
pub fn prone_face_up(hip_matrix: &[[f32; 4]; 3], use_z_axis: bool, invert: bool) -> bool {
    let face_up = hip_matrix[1][if use_z_axis { 2 } else { 1 }] > 0.0;
    face_up != invert
}

/// `ftCo_8009F0F0` and its DownDamageU/DownDamageD selector, expressed without
/// fighter pointers. The source uses a strict pending-damage threshold and has
/// a notable selector quirk: only DownWaitU chooses the face-up reaction;
/// DownBoundU and a repeated DownDamageU choose DownDamageD.
pub fn down_damage_face_up(
    prone_action: bool,
    face_up_wait: bool,
    forced: bool,
    pending_damage: f32,
    threshold: i32,
) -> Option<bool> {
    (prone_action && (forced || pending_damage < threshold as f32)).then_some(face_up_wait)
}

/// Refactored DownWait IASA composition. The original callback gives get-up
/// attack priority, then a fresh C-stick or held main-stick roll, then stand.
pub fn knockdown_option(
    input: KnockdownInput,
    rules: &KnockdownParameters,
) -> Option<KnockdownOption> {
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
        && crate::fighter::aerial::stick_angle(input.main) >= rules.vertical_angle_radians)
        || input.shoulder_pressed)
        .then_some(KnockdownOption::Stand)
}

/// DownBound's animation-end input branch. Fresh A/B ages are reset when the
/// bound begins; either buffered button or a fresh upward C-stick beats a roll.
pub fn down_bound_option(
    input: KnockdownInput,
    attack_ages: [u8; 2],
    rules: &KnockdownParameters,
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

fn knockdown_roll(input: KnockdownInput, rules: &KnockdownParameters) -> Option<KnockdownOption> {
    let stick_x = if fresh_horizontal_cstick(
        input.previous_cstick,
        input.cstick,
        rules.horizontal_stick_threshold,
        rules.vertical_angle_radians,
    ) {
        Some(input.cstick[0])
    } else if crate::compat::source_ops::comparison_abs(input.main[0])
        >= rules.horizontal_stick_threshold
        && crate::fighter::aerial::stick_angle(input.main) < rules.vertical_angle_radians
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

// Explicit damage rules and the experimental scheduler's damage integration.
// Native helpers preserve selected source arithmetic; action ordering and
// remaining callback ordering stays the documented match-slice policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatRules {
    pub angle_361_airborne_radians: f32,
    pub angle_361_grounded_max_degrees: f32,
    pub angle_361_low_knockback: f32,
    pub angle_361_high_knockback: f32,
    pub di_max_degrees: f32,
    pub special_angle_min: u32,
    pub special_angle_max: u32,
    pub special_angle_timer: i32,
    pub knockback_replace_window: i32,
    pub combo: crate::fighter::combo::Rules,
    /// None retains the explicitly incomplete legacy match profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub displacement: Option<HitlagDisplacementRules>,
    /// Explicit damage-floor state profile. None preserves the legacy slice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_response: Option<FloorResponseRules>,
    /// Optional ordinary wall/ceiling damage reflection profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_response: Option<SurfaceResponseRules>,
    /// Optional wall/ceiling tech timing and input profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_tech: Option<SurfaceTechRules>,
    /// Optional ordinary Damage motion thresholds (common x158/x15C/x160).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_motion: Option<DamageMotionRules>,
    /// Optional grounded launch projection and friction profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_launch: Option<GroundLaunchRules>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageMotionRules {
    pub thresholds: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundLaunchRules {
    pub fly_bounce_angle_radians: f32,
    pub fly_bounce_vertical_multiplier: f32,
    /// Common x200, multiplied by each fighter's ground friction.
    pub ground_knockback_friction_multiplier: f32,
}

impl GroundLaunchRules {
    pub(crate) fn physics(&self) -> GroundLaunchParameters {
        GroundLaunchParameters {
            fly_bounce_angle_radians: self.fly_bounce_angle_radians,
            fly_bounce_vertical_multiplier: self.fly_bounce_vertical_multiplier,
        }
    }
}

/// Complete physics-pose samples for the source's 15 ordinary Damage motions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamagePoseAttributes {
    /// One source low/middle/high selector per fighter hurtbox.
    pub hurtbox_heights: Vec<self::HurtHeight>,
    /// Levels 1..3, then hurt height low/middle/high.
    pub ground: [[Vec<Vec<Bone>>; 3]; 3],
    /// Air levels 1..3; hurt height is ignored by the source table.
    pub air: [Vec<Vec<Bone>>; 3],
    /// Fly hurt height low/middle/high.
    pub fly: [Vec<Vec<Bone>>; 3],
}

impl DamagePoseAttributes {
    fn motion(&self, motion: self::DamageMotion) -> &Vec<Vec<Bone>> {
        match motion {
            self::DamageMotion::Ground { level, height } => {
                &self.ground[level as usize][height.index()]
            }
            self::DamageMotion::Air { level } => &self.air[level as usize],
            self::DamageMotion::Fly { height } => &self.fly[height.index()],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorResponseRules {
    pub tumble_knockback_threshold: f32,
    pub tech_window: f32,
    pub tech_repeat_lockout: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tech_roll: Option<FloorTechRules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knockdown_options: Option<KnockdownRules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_invincibility: Option<RecoveryInvincibilityRules>,
    /// Optional low-damage reaction while the fighter is prone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down_damage: Option<DownDamageRules>,
    pub passive_frames: u32,
    /// Shared DownBound duration. The source drives this from
    /// `ftAnim_IsFramesRemaining` against whichever of DownBoundU/D
    /// (`ftCo_DownBound.c`) the fighter's motion state selected, so the two
    /// orientations' animations are free to run different lengths. This
    /// field is the fallback when a fighter/pack supplies neither override
    /// below.
    pub down_bound_frames: u32,
    /// Face-up override for `down_bound_frames` (DownBoundU). `None` reuses
    /// the shared value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down_bound_frames_face_up: Option<u32>,
    /// Face-down override for `down_bound_frames` (DownBoundD). `None`
    /// reuses the shared value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down_bound_frames_face_down: Option<u32>,
    /// Shared DownWait duration, same per-orientation caveat as
    /// `down_bound_frames`: Fox's DownWaitU (sub-motion 184) runs 70 frames
    /// and DownWaitD (192) runs 90 (`ftmotionstates.c`, `ftCo_Down.c`).
    pub down_wait_frames: u32,
    /// Face-up override for `down_wait_frames` (DownWaitU).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down_wait_frames_face_up: Option<u32>,
    /// Face-down override for `down_wait_frames` (DownWaitD).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down_wait_frames_face_down: Option<u32>,
    /// Shared DownStand duration, same per-orientation caveat
    /// (`ftCo_DownStand.c` selects DownStandU/D from `fp->motion_id`).
    pub down_stand_frames: u32,
    /// Face-up override for `down_stand_frames` (DownStandU).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down_stand_frames_face_up: Option<u32>,
    /// Face-down override for `down_stand_frames` (DownStandD).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down_stand_frames_face_down: Option<u32>,
}

impl FloorResponseRules {
    /// Effective DownBound duration for `orientation`, falling back to the
    /// shared field when no per-orientation override (or no orientation,
    /// i.e. a fighter without `KnockdownAttributes`) applies.
    pub(crate) fn down_bound_frames_for(&self, orientation: Option<ProneOrientation>) -> u32 {
        match orientation {
            Some(ProneOrientation::FaceUp) => self
                .down_bound_frames_face_up
                .unwrap_or(self.down_bound_frames),
            Some(ProneOrientation::FaceDown) => self
                .down_bound_frames_face_down
                .unwrap_or(self.down_bound_frames),
            None => self.down_bound_frames,
        }
    }

    /// Effective DownWait duration for `orientation`. See
    /// `down_bound_frames_for`.
    pub(crate) fn down_wait_frames_for(&self, orientation: Option<ProneOrientation>) -> u32 {
        match orientation {
            Some(ProneOrientation::FaceUp) => self
                .down_wait_frames_face_up
                .unwrap_or(self.down_wait_frames),
            Some(ProneOrientation::FaceDown) => self
                .down_wait_frames_face_down
                .unwrap_or(self.down_wait_frames),
            None => self.down_wait_frames,
        }
    }

    /// Effective DownStand duration for `orientation`. See
    /// `down_bound_frames_for`.
    pub(crate) fn down_stand_frames_for(&self, orientation: Option<ProneOrientation>) -> u32 {
        match orientation {
            Some(ProneOrientation::FaceUp) => self
                .down_stand_frames_face_up
                .unwrap_or(self.down_stand_frames),
            Some(ProneOrientation::FaceDown) => self
                .down_stand_frames_face_down
                .unwrap_or(self.down_stand_frames),
            None => self.down_stand_frames,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownDamageRules {
    /// Strict upper bound for the current hit's pending damage.
    pub pending_damage_threshold: i32,
    /// Number of supplied DownDamage animation/physics samples.
    pub frames: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnockdownRules {
    pub horizontal_stick_threshold: f32,
    pub stand_stick_threshold: f32,
    pub vertical_angle_radians: f32,
    pub attack_cstick_threshold: f32,
    pub bound_attack_window: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryInvincibilityRules {
    pub passive_frames: u32,
    pub tech_roll_frames: u32,
    pub missed_roll_frames: u32,
    pub stand_frames: u32,
    pub attack_frames: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnockdownAttributes {
    pub passive_poses: Vec<Vec<Bone>>,
    pub orientation: ProneOrientationRules,
    pub face_up: ProneRecoveryAttributes,
    pub face_down: ProneRecoveryAttributes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProneOrientation {
    FaceUp,
    FaceDown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProneOrientationRules {
    pub hip_bone: usize,
    pub use_z_axis: bool,
    pub invert: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProneRecoveryAttributes {
    pub bound_poses: Vec<Vec<Bone>>,
    pub wait_poses: Vec<Vec<Bone>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_poses: Option<Vec<Vec<Bone>>>,
    pub forward: FloorTechMotion,
    pub backward: FloorTechMotion,
    pub stand_poses: Vec<Vec<Bone>>,
    pub attack: crate::game::data::Attack,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorTechRules {
    pub stick_threshold: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorTechAttributes {
    pub forward: FloorTechMotion,
    pub backward: FloorTechMotion,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorTechMotion {
    /// One headless physics/pose sample per action frame.
    pub frames: Vec<FloorTechFrame>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloorTechFrame {
    pub bones: Vec<Bone>,
    /// Source TransN delta along local forward for this frame.
    pub root_translation: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceResponseRules {
    pub knockback_threshold: f32,
    pub velocity_multiplier: f32,
    pub lockout_frames: u8,
    pub wall_frames: u32,
    pub ceiling_frames: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceResponseAttributes {
    /// Complete fighter-specific FlyReflectWall physics poses.
    pub wall_poses: Vec<Vec<Bone>>,
    /// Complete fighter-specific FlyReflectCeiling physics poses.
    pub ceiling_poses: Vec<Vec<Bone>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceTechRules {
    pub wall_freeze_frames: u32,
    pub wall_frames: u32,
    pub wall_jump_frames: u32,
    pub ceiling_frames: u32,
    pub ceiling_horizontal_frame: u32,
    pub jump_stick_threshold: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceTechAttributes {
    pub passive_wall_velocity: f32,
    pub wall_jump_horizontal_velocity: f32,
    pub wall_jump_vertical_velocity: f32,
    pub passive_ceiling_velocity: f32,
    /// Complete fighter-specific PassiveWall physics poses.
    pub passive_wall_poses: Vec<Vec<Bone>>,
    /// Complete fighter-specific damage-tech PassiveWallJump physics poses.
    pub passive_wall_jump_poses: Vec<Vec<Bone>>,
    /// Complete fighter-specific PassiveCeiling physics poses.
    pub passive_ceiling_poses: Vec<Vec<Bone>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SurfaceTechState {
    pub timer: u32,
    pub jump_queued: bool,
    pub ceiling_velocity_applied: bool,
}

/// Native common-data coefficients. Main-stick SDI/ASDI only; the current
/// controller contract has no C-stick channel or LR/vcancel response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitlagDisplacementRules {
    pub axis_thresholds: [f32; 2],
    pub minimum_stick_magnitude: f32,
    pub sdi_window: u8,
    pub sdi_distance: f32,
    pub asdi_distance: f32,
}

/// Ordinary persistent armor channels; dynamic metal/state modifiers remain
/// separate. Minimum knockback is required explicitly, never guessed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Armor {
    pub armor0: f32,
    pub armor1: f32,
    pub minimum_knockback: f32,
}

pub(crate) fn validate_armor(armor: &Armor) -> Result<(), Error> {
    if [armor.armor0, armor.armor1, armor.minimum_knockback]
        .into_iter()
        .any(|value| !value.is_finite() || !(0.0..=1_000_000.0).contains(&value))
    {
        return Err(Error::Data("invalid explicit armor parameters".into()));
    }
    Ok(())
}

pub(crate) fn validate_surface_tech_attributes(
    attributes: &SurfaceTechAttributes,
    profile: &SurfaceTechRules,
    fighter: &FighterData,
) -> Result<(), Error> {
    if ![
        attributes.passive_wall_velocity,
        attributes.wall_jump_horizontal_velocity,
        attributes.wall_jump_vertical_velocity,
        attributes.passive_ceiling_velocity,
    ]
    .into_iter()
    .all(|value| value.is_finite() && (0.0..=1_000_000.0).contains(&value))
    {
        return Err(Error::Data(
            "invalid fighter damage-surface tech attributes".into(),
        ));
    }
    for (poses, frames) in [
        (&attributes.passive_wall_poses, profile.wall_frames),
        (
            &attributes.passive_wall_jump_poses,
            profile.wall_jump_frames,
        ),
        (&attributes.passive_ceiling_poses, profile.ceiling_frames),
    ] {
        if poses.len() != frames as usize {
            return Err(Error::Data(
                "damage-surface tech poses must match the configured duration".into(),
            ));
        }
        for pose in poses {
            crate::game::validation::validate_animation_pose(pose, fighter)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_surface_response_attributes(
    attributes: &SurfaceResponseAttributes,
    profile: &SurfaceResponseRules,
    fighter: &FighterData,
) -> Result<(), Error> {
    for (poses, frames) in [
        (&attributes.wall_poses, profile.wall_frames),
        (&attributes.ceiling_poses, profile.ceiling_frames),
    ] {
        if poses.len() != frames as usize {
            return Err(Error::Data(
                "damage-surface response poses must match the configured duration".into(),
            ));
        }
        for pose in poses {
            crate::game::validation::validate_animation_pose(pose, fighter)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_damage_pose_attributes(
    attributes: &DamagePoseAttributes,
    fighter: &FighterData,
) -> Result<(), Error> {
    if attributes.hurtbox_heights.len() != fighter.hurtboxes.len() {
        return Err(Error::Data(
            "damage poses require one height per hurtbox".into(),
        ));
    }
    for motion in attributes
        .ground
        .iter()
        .flatten()
        .chain(&attributes.air)
        .chain(&attributes.fly)
    {
        if motion.is_empty() || motion.len() > 4096 {
            return Err(Error::Data(
                "damage motions require 1..4096 physics samples".into(),
            ));
        }
        for pose in motion {
            crate::game::validation::validate_animation_pose(pose, fighter)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_floor_tech_attributes(
    attributes: &FloorTechAttributes,
    fighter: &FighterData,
    invincibility_frames: u32,
) -> Result<(), Error> {
    for motion in [&attributes.forward, &attributes.backward] {
        validate_ground_motion(motion, fighter)?;
        if invincibility_frames > motion.frames.len() as u32 {
            return Err(Error::Data(
                "floor-tech invincibility exceeds the supplied motion".into(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_knockdown_attributes(
    attributes: &KnockdownAttributes,
    fighter: &FighterData,
    profile: &FloorResponseRules,
) -> Result<(), Error> {
    if attributes.orientation.hip_bone >= fighter.bones.len() {
        return Err(Error::Data(
            "prone orientation requires a valid hip bone".into(),
        ));
    }
    validate_poses(&attributes.passive_poses, profile.passive_frames, fighter)?;
    for (orientation, variant) in [
        (ProneOrientation::FaceUp, &attributes.face_up),
        (ProneOrientation::FaceDown, &attributes.face_down),
    ] {
        validate_ground_motion(&variant.forward, fighter)?;
        validate_ground_motion(&variant.backward, fighter)?;
        for (poses, frames) in [
            (
                &variant.bound_poses,
                profile.down_bound_frames_for(Some(orientation)),
            ),
            (
                &variant.wait_poses,
                profile.down_wait_frames_for(Some(orientation)),
            ),
            (
                &variant.stand_poses,
                profile.down_stand_frames_for(Some(orientation)),
            ),
        ] {
            validate_poses(poses, frames, fighter)?;
        }
        match (&profile.down_damage, &variant.damage_poses) {
            (Some(rules), Some(poses)) => validate_poses(poses, rules.frames, fighter)?,
            (Some(_), None) => {
                return Err(Error::Data(
                    "down-damage rules require poses for both orientations".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(Error::Data(
                    "down-damage poses require explicit common rules".into(),
                ));
            }
            (None, None) => {}
        }
        if let Some(invincibility) = &profile.recovery_invincibility
            && (invincibility.missed_roll_frames > variant.forward.frames.len() as u32
                || invincibility.missed_roll_frames > variant.backward.frames.len() as u32
                || invincibility.attack_frames > variant.attack.frames.len() as u32)
        {
            return Err(Error::Data(
                "knockdown invincibility exceeds the supplied action".into(),
            ));
        }
    }
    Ok(())
}

fn validate_poses(poses: &[Vec<Bone>], frames: u32, fighter: &FighterData) -> Result<(), Error> {
    if poses.len() != frames as usize {
        return Err(Error::Data(
            "floor-recovery poses must match the configured duration".into(),
        ));
    }
    for pose in poses {
        crate::game::validation::validate_animation_pose(pose, fighter)?;
    }
    Ok(())
}

fn validate_ground_motion(motion: &FloorTechMotion, fighter: &FighterData) -> Result<(), Error> {
    if motion.frames.is_empty() || motion.frames.len() > 4096 {
        return Err(Error::Data(
            "ground recovery requires 1..4096 physics samples".into(),
        ));
    }
    for frame in &motion.frames {
        if !frame.root_translation.is_finite() || frame.root_translation.abs() > 1_000_000.0 {
            return Err(Error::Data(
                "invalid ground-recovery root translation".into(),
            ));
        }
        crate::game::validation::validate_animation_pose(&frame.bones, fighter)?;
    }
    Ok(())
}

impl FloorTechAttributes {
    pub(crate) fn motion(&self, action: Action) -> Option<&FloorTechMotion> {
        match action {
            Action::PassiveStandF => Some(&self.forward),
            Action::PassiveStandB => Some(&self.backward),
            _ => None,
        }
    }
}

impl KnockdownAttributes {
    pub(crate) fn variant(
        &self,
        orientation: Option<ProneOrientation>,
    ) -> Option<&ProneRecoveryAttributes> {
        match orientation? {
            ProneOrientation::FaceUp => Some(&self.face_up),
            ProneOrientation::FaceDown => Some(&self.face_down),
        }
    }

    pub(crate) fn motion(
        &self,
        action: Action,
        orientation: Option<ProneOrientation>,
    ) -> Option<&FloorTechMotion> {
        let variant = self.variant(orientation)?;
        match action {
            Action::DownForward => Some(&variant.forward),
            Action::DownBack => Some(&variant.backward),
            _ => None,
        }
    }
}

impl CombatRules {
    pub(crate) fn angle_rules(&self) -> self::LaunchAngleRules {
        self::LaunchAngleRules {
            airborne_radians: self.angle_361_airborne_radians,
            grounded_max_degrees: self.angle_361_grounded_max_degrees,
            grounded_low_knockback: self.angle_361_low_knockback,
            grounded_high_knockback: self.angle_361_high_knockback,
            special_angle_min: self.special_angle_min,
            special_angle_max: self.special_angle_max,
            special_timer: self.special_angle_timer,
        }
    }
}

pub(crate) fn validate_rules(rules: &CombatRules) -> Result<(), Error> {
    if ![
        rules.angle_361_airborne_radians,
        rules.angle_361_grounded_max_degrees,
        rules.angle_361_low_knockback,
        rules.angle_361_high_knockback,
        rules.di_max_degrees,
    ]
    .into_iter()
    .all(|value| value.is_finite())
        || !(0.0..=core::f32::consts::TAU).contains(&rules.angle_361_airborne_radians)
        || !(0.0..=360.0).contains(&rules.angle_361_grounded_max_degrees)
        || rules.angle_361_low_knockback < 0.0
        || rules.angle_361_low_knockback >= rules.angle_361_high_knockback
        || rules.angle_361_high_knockback > 1_000_000.0
        || !(0.0..=180.0).contains(&rules.di_max_degrees)
        || rules.special_angle_min > rules.special_angle_max
        || !(0..=255).contains(&rules.special_angle_timer)
        || !(0..1_000_000).contains(&rules.knockback_replace_window)
        || rules.combo.push_count <= 0
        || rules.combo.strong_push_count < rules.combo.push_count
        || rules.combo.push_frames == 0
        || !rules
            .combo
            .push_distance
            .into_iter()
            .all(|distance| distance.is_finite() && (0.0..=1_000_000.0).contains(&distance))
    {
        return Err(Error::Data("invalid explicit damage rules".into()));
    }
    if let Some(profile) = &rules.displacement
        && (profile
            .axis_thresholds
            .into_iter()
            .any(|value| !value.is_finite() || value <= 0.0 || value > 1.0)
            || !profile.minimum_stick_magnitude.is_finite()
            || !(0.0..=2.0).contains(&profile.minimum_stick_magnitude)
            || profile.sdi_window == 0
            || profile.sdi_window == 255
            || [profile.sdi_distance, profile.asdi_distance]
                .into_iter()
                .any(|value| !value.is_finite() || !(0.0..=1_000_000.0).contains(&value)))
    {
        return Err(Error::Data(
            "invalid explicit hitlag displacement rules".into(),
        ));
    }
    if let Some(profile) = &rules.damage_motion
        && (!profile
            .thresholds
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1_000_000.0).contains(&value))
            || !(profile.thresholds[0] < profile.thresholds[1]
                && profile.thresholds[1] < profile.thresholds[2]))
    {
        return Err(Error::Data(
            "invalid explicit damage-motion thresholds".into(),
        ));
    }
    if let Some(profile) = &rules.ground_launch
        && (!(0.0..=core::f32::consts::FRAC_PI_2).contains(&profile.fly_bounce_angle_radians)
            || ![
                profile.fly_bounce_vertical_multiplier,
                profile.ground_knockback_friction_multiplier,
            ]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1_000_000.0).contains(&value)))
    {
        return Err(Error::Data("invalid explicit grounded-launch rules".into()));
    }
    if rules.ground_launch.is_some() && rules.damage_motion.is_none() {
        return Err(Error::Data(
            "grounded launch requires damage-motion thresholds".into(),
        ));
    }
    if let Some(profile) = &rules.floor_response
        && (![profile.tumble_knockback_threshold, profile.tech_window]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1_000_000.0).contains(&value))
            || profile.tech_window > 255.0
            || !(0..=255).contains(&profile.tech_repeat_lockout)
            || [
                profile.passive_frames,
                profile.down_bound_frames,
                profile.down_wait_frames,
                profile.down_stand_frames,
            ]
            .into_iter()
            .any(|frames| frames == 0 || frames >= 1_000_000)
            || [
                profile.down_bound_frames_face_up,
                profile.down_bound_frames_face_down,
                profile.down_wait_frames_face_up,
                profile.down_wait_frames_face_down,
                profile.down_stand_frames_face_up,
                profile.down_stand_frames_face_down,
            ]
            .into_iter()
            .flatten()
            .any(|frames| frames == 0 || frames >= 1_000_000))
    {
        return Err(Error::Data(
            "invalid explicit damage-floor response rules".into(),
        ));
    }
    if let Some(profile) = rules
        .floor_response
        .as_ref()
        .and_then(|profile| profile.tech_roll.as_ref())
        && (!profile.stick_threshold.is_finite()
            || profile.stick_threshold <= 0.0
            || profile.stick_threshold > 1.0)
    {
        return Err(Error::Data("invalid explicit floor-tech roll rules".into()));
    }
    if let Some(profile) = rules
        .floor_response
        .as_ref()
        .and_then(|profile| profile.knockdown_options.as_ref())
        && (![
            profile.horizontal_stick_threshold,
            profile.stand_stick_threshold,
        ]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0 && value <= 1.0)
            || !profile.attack_cstick_threshold.is_finite()
            || !(0.0..=1.0).contains(&profile.attack_cstick_threshold)
            || !profile.vertical_angle_radians.is_finite()
            || !(0.0..=core::f32::consts::FRAC_PI_2).contains(&profile.vertical_angle_radians)
            || !profile.bound_attack_window.is_finite()
            || !(0.0..=255.0).contains(&profile.bound_attack_window))
    {
        return Err(Error::Data(
            "invalid explicit knockdown-option rules".into(),
        ));
    }
    if let Some(profile) = rules
        .floor_response
        .as_ref()
        .and_then(|profile| profile.down_damage.as_ref())
        && (!(0..=1_000_000).contains(&profile.pending_damage_threshold)
            || profile.frames == 0
            || profile.frames >= 1_000_000)
    {
        return Err(Error::Data("invalid explicit down-damage rules".into()));
    }
    if let Some(invincibility) = rules
        .floor_response
        .as_ref()
        .and_then(|profile| profile.recovery_invincibility.as_ref())
    {
        let floor = rules.floor_response.as_ref().unwrap();
        if [
            invincibility.passive_frames,
            invincibility.tech_roll_frames,
            invincibility.missed_roll_frames,
            invincibility.stand_frames,
            invincibility.attack_frames,
        ]
        .into_iter()
        .any(|frames| frames >= 1_000_000)
            || invincibility.passive_frames > floor.passive_frames
            || invincibility.stand_frames
                > floor.down_stand_frames_for(Some(ProneOrientation::FaceUp))
            || invincibility.stand_frames
                > floor.down_stand_frames_for(Some(ProneOrientation::FaceDown))
            || floor.tech_roll.is_none() && invincibility.tech_roll_frames != 0
            || floor.knockdown_options.is_none()
                && (invincibility.missed_roll_frames != 0
                    || invincibility.stand_frames != 0
                    || invincibility.attack_frames != 0)
        {
            return Err(Error::Data(
                "invalid explicit floor-recovery invincibility".into(),
            ));
        }
    }
    if let Some(profile) = &rules.surface_response
        && ((!profile.knockback_threshold.is_finite()
            || !(0.0..=1_000_000.0).contains(&profile.knockback_threshold))
            || !profile.velocity_multiplier.is_finite()
            || !(0.0..=1.0).contains(&profile.velocity_multiplier)
            || profile.wall_frames == 0
            || profile.ceiling_frames == 0
            || profile.wall_frames >= 1_000_000
            || profile.ceiling_frames >= 1_000_000)
    {
        return Err(Error::Data(
            "invalid explicit damage-surface response rules".into(),
        ));
    }
    if rules.surface_response.is_some() && rules.floor_response.is_none() {
        return Err(Error::Data(
            "damage-surface response requires explicit tumble rules".into(),
        ));
    }
    if let Some(profile) = &rules.surface_tech
        && (!profile.jump_stick_threshold.is_finite()
            || !(0.0..=1.0).contains(&profile.jump_stick_threshold)
            || profile.jump_stick_threshold == 0.0
            || profile.wall_freeze_frames == 0
            || profile.wall_freeze_frames >= profile.wall_frames
            || profile.wall_freeze_frames >= profile.wall_jump_frames
            || profile.ceiling_horizontal_frame >= profile.ceiling_frames
            || [
                profile.wall_frames,
                profile.wall_jump_frames,
                profile.ceiling_frames,
            ]
            .into_iter()
            .any(|frames| frames == 0 || frames >= 1_000_000))
    {
        return Err(Error::Data(
            "invalid explicit damage-surface tech rules".into(),
        ));
    }
    if rules.surface_tech.is_some() && rules.floor_response.is_none() {
        return Err(Error::Data(
            "damage-surface tech requires explicit tumble rules".into(),
        ));
    }
    Ok(())
}

/// Priority-1 portions of the ordinary tumble, knockdown and neutral-tech
/// graph. Durations are explicit resources because animation data is not yet
/// available for every fighter.
pub(crate) fn update_animation(
    fighter: &mut Fighter,
    data: &crate::game::data::FighterData,
    rules: &CombatRules,
    input: crate::game::Controller,
) {
    fighter.reflect_lockout = fighter.reflect_lockout.saturating_sub(1);
    if fighter.action == Action::DownDamage
        && let Some(profile) = rules
            .floor_response
            .as_ref()
            .and_then(|floor| floor.down_damage.as_ref())
    {
        if fighter.action_frame < profile.frames {
            fighter.down_timer = fighter.down_timer.saturating_sub(1);
        }
        if fighter.action_frame >= profile.frames {
            if fighter.grounded {
                let action = if fighter.down_timer == 0 {
                    Action::DownStand
                } else {
                    Action::DownWait
                };
                enter_recovery_registered(
                    fighter,
                    data,
                    action,
                    rules.floor_response.as_ref().unwrap(),
                );
            } else {
                crate::game::simulation::enter(fighter, Action::Fall);
            }
            return;
        }
    }
    if let Some(motion) = ground_motion(fighter, data)
        && fighter.action_frame as usize >= motion.frames.len()
    {
        crate::game::simulation::enter(fighter, Action::Wait);
        return;
    }
    if fighter.action == Action::DownAttack
        && data.knockdown.as_ref().is_some_and(|attributes| {
            attributes
                .variant(fighter.prone)
                .is_some_and(|variant| fighter.action_frame as usize >= variant.attack.frames.len())
        })
    {
        crate::game::simulation::enter(fighter, Action::Wait);
        return;
    }
    if fighter.action == Action::DownBound
        && let Some(floor) = &rules.floor_response
        && fighter.action_frame >= floor.down_bound_frames_for(fighter.prone)
    {
        let action = floor
            .knockdown_options
            .as_ref()
            .and_then(|rules| {
                self::down_bound_option(
                    knockdown_input(fighter, input),
                    [
                        fighter.locomotion.attack_a_age,
                        fighter.locomotion.attack_b_age,
                    ],
                    &knockdown_rules(rules),
                )
            })
            .map(knockdown_action)
            .unwrap_or(Action::DownWait);
        if action == Action::DownWait {
            fighter.down_timer = floor.down_wait_frames_for(fighter.prone);
        }
        enter_recovery_registered(fighter, data, action, floor);
        return;
    }
    if let (Some(profile), Some(attributes)) = (&rules.surface_tech, &data.surface_tech) {
        let duration = match fighter.action {
            Action::PassiveWall | Action::PassiveWallJump if !fighter.wall_jump.active => {
                if fighter.surface_tech.timer != 0 {
                    fighter.surface_tech.timer -= 1;
                    if fighter.surface_tech.timer == 0 {
                        if fighter.action == Action::PassiveWall && fighter.surface_tech.jump_queued
                        {
                            let frame = fighter.action_frame;
                            crate::game::simulation::enter(fighter, Action::PassiveWallJump);
                            fighter.action_frame = frame;
                            fighter.surface_tech.jump_queued = false;
                        }
                        if fighter.action == Action::PassiveWall {
                            fighter.velocity[0] = fighter.facing * attributes.passive_wall_velocity;
                        } else {
                            fighter.velocity = [
                                fighter.facing * attributes.wall_jump_horizontal_velocity,
                                attributes.wall_jump_vertical_velocity,
                            ];
                        }
                    }
                }
                Some(if fighter.action == Action::PassiveWall {
                    profile.wall_frames
                } else {
                    profile.wall_jump_frames
                })
            }
            Action::PassiveCeiling => {
                if !fighter.surface_tech.ceiling_velocity_applied
                    && fighter.action_frame >= profile.ceiling_horizontal_frame
                {
                    fighter.velocity[0] = input.stick[0] * attributes.passive_ceiling_velocity;
                    fighter.surface_tech.ceiling_velocity_applied = true;
                }
                Some(profile.ceiling_frames)
            }
            _ => None,
        };
        if duration.is_some_and(|duration| fighter.action_frame >= duration) {
            crate::game::simulation::enter(fighter, Action::Fall);
            return;
        }
    }
    let surface_next = rules
        .surface_response
        .as_ref()
        .and_then(|response| match fighter.action {
            Action::FlyReflectWall if fighter.action_frame >= response.wall_frames => {
                Some(Action::DamageFall)
            }
            Action::FlyReflectCeiling if fighter.action_frame >= response.ceiling_frames => {
                Some(Action::DamageFall)
            }
            _ => None,
        });
    if let Some(action) = surface_next {
        crate::game::simulation::enter(fighter, action);
        return;
    }
    let motion_ended = fighter.damage_motion.is_none_or(|motion| {
        data.damage_poses
            .as_ref()
            .is_none_or(|poses| fighter.action_frame as usize >= poses.motion(motion).len())
    });
    let Some(profile) = &rules.floor_response else {
        if fighter.action == Action::Damage && fighter.hitstun == 0 && motion_ended {
            crate::game::simulation::enter(
                fighter,
                if fighter.grounded {
                    Action::Wait
                } else {
                    Action::Fall
                },
            );
        }
        return;
    };
    let next = match fighter.action {
        Action::Damage if fighter.hitstun == 0 && motion_ended => Some(if fighter.grounded {
            Action::Wait
        } else if fighter.tumbling {
            Action::DamageFall
        } else {
            Action::Fall
        }),
        Action::Passive if fighter.action_frame >= profile.passive_frames => Some(Action::Wait),
        Action::DownWait => {
            fighter.down_timer = fighter.down_timer.saturating_sub(1);
            (fighter.down_timer == 0).then_some(Action::DownStand)
        }
        Action::DownStand
            if fighter.action_frame >= profile.down_stand_frames_for(fighter.prone) =>
        {
            Some(Action::Wait)
        }
        _ => None,
    };
    if let Some(action) = next {
        enter_recovery_registered(fighter, data, action, profile);
    }
}

/// Selected ordinary Damage physics pose. If hitstun outlasts its animation,
/// the last supplied pose remains sampled until the action can exit.
pub(crate) fn damage_pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a Vec<Bone>> {
    let motion = fighter.damage_motion?;
    let frames = data.damage_poses.as_ref()?.motion(motion);
    frames
        .get(fighter.action_frame as usize)
        .or_else(|| frames.last())
}

/// DownWait's source priority: get-up attack, roll, then stand. This owns the
/// action even when no option is selected so unrelated state machines cannot
/// consume input while the fighter remains knocked down.
pub(crate) fn update_actions(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: &CombatRules,
    input: crate::game::Controller,
) -> bool {
    if matches!(
        fighter.action,
        Action::PassiveWall | Action::PassiveWallJump
    ) && !fighter.wall_jump.active
        && fighter.surface_tech.timer != 0
    {
        if fighter.action == Action::PassiveWall
            && self::wall_tech_jumps(
                fighter.locomotion.jump_press_age,
                input.stick[1],
                rules
                    .floor_response
                    .as_ref()
                    .expect("validated surface-tech rules require floor response")
                    .tech_window,
                rules
                    .surface_tech
                    .as_ref()
                    .expect("surface-tech action requires rules")
                    .jump_stick_threshold,
            )
        {
            fighter.surface_tech.jump_queued = true;
        }
        return true;
    }
    if fighter.action != Action::DownWait {
        return false;
    }
    // DownBound's animation callback owns its final input decision. DownWait's
    // IASA begins on the following frame if that decision selected no option.
    if fighter.action_frame == 0 {
        return true;
    }
    let Some(floor) = &rules.floor_response else {
        return true;
    };
    let Some(rules) = &floor.knockdown_options else {
        return true;
    };
    let action = self::knockdown_option(knockdown_input(fighter, input), &knockdown_rules(rules))
        .map(knockdown_action);
    if let Some(action) = action {
        enter_recovery_registered(fighter, data, action, floor);
    }
    true
}

pub(crate) fn wall_tech_interruptible(fighter: &Fighter) -> bool {
    matches!(
        fighter.action,
        Action::PassiveWall | Action::PassiveWallJump
    ) && fighter.surface_tech.timer == 0
        && fighter.wall_jump.startup_timer == 0
}

/// DamageFall and `ftCo_DamageFly_IASA` delegate to the ordinary airborne input
/// graph once hitstun ends. Reflected actions share the latter callback.
pub(crate) fn damage_air_interruptible(fighter: &Fighter) -> bool {
    !fighter.grounded
        && fighter.hitstun == 0
        && matches!(
            fighter.action,
            Action::Damage
                | Action::DamageFall
                | Action::FlyReflectWall
                | Action::FlyReflectCeiling
        )
}

pub(crate) fn can_reflect(
    fighter: &Fighter,
    surface: crate::collision::stage::Surface,
    rules: &CombatRules,
) -> bool {
    use crate::collision::stage::Surface;
    let Some(profile) = &rules.surface_response else {
        return false;
    };
    if !fighter.tumbling
        || fighter.last_damage_surface == Some(surface)
        || !matches!(
            fighter.action,
            Action::Damage
                | Action::DamageFall
                | Action::DownDamage
                | Action::FlyReflectWall
                | Action::FlyReflectCeiling
        )
    {
        return false;
    }
    if matches!(surface, Surface::LeftWall | Surface::RightWall)
        && fighter.action == Action::FlyReflectWall
        && fighter.reflect_lockout != 0
    {
        return false;
    }
    match surface {
        Surface::LeftWall => fighter.knockback[0] > profile.knockback_threshold,
        Surface::RightWall => fighter.knockback[0] < -profile.knockback_threshold,
        Surface::Ceiling => fighter.knockback[1] > profile.knockback_threshold,
        Surface::Floor => false,
    }
}

pub(crate) fn can_surface_tech(
    fighter: &Fighter,
    surface: crate::collision::stage::Surface,
    rules: &CombatRules,
) -> bool {
    use crate::collision::stage::Surface;
    let (Some(_), Some(floor)) = (&rules.surface_tech, &rules.floor_response) else {
        return false;
    };
    fighter.tumbling
        && !matches!(surface, Surface::Floor)
        && matches!(
            fighter.action,
            Action::Damage
                | Action::DamageFall
                | Action::DownDamage
                | Action::FlyReflectWall
                | Action::FlyReflectCeiling
        )
        && !(matches!(surface, Surface::LeftWall | Surface::RightWall)
            && fighter.action == Action::FlyReflectWall
            && fighter.reflect_lockout != 0)
        && self::can_tech(
            false,
            fighter.locomotion.tech_press_age,
            fighter.locomotion.previous_tech_press_age,
            floor.tech_window,
            floor.tech_repeat_lockout,
        )
}

pub(crate) fn surface_tech(
    fighter: &mut Fighter,
    surface: crate::collision::stage::Surface,
    rules: &CombatRules,
    input: crate::game::Controller,
) -> bool {
    use crate::collision::stage::Surface;
    let profile = rules.surface_tech.as_ref().unwrap();
    fighter.velocity = [0.0; 2];
    fighter.knockback = [0.0; 2];
    fighter.ground_knockback = 0.0;
    fighter.ground_velocity = 0.0;
    fighter.grounded = false;
    fighter.ground_line = None;
    let jump = matches!(surface, Surface::LeftWall | Surface::RightWall)
        && self::wall_tech_jumps(
            fighter.locomotion.jump_press_age,
            input.stick[1],
            rules.floor_response.as_ref().unwrap().tech_window,
            profile.jump_stick_threshold,
        );
    if matches!(surface, Surface::LeftWall | Surface::RightWall) {
        fighter.facing = if surface == Surface::LeftWall {
            -1.0
        } else {
            1.0
        };
        fighter.locomotion.tilt_x_age = 254;
        fighter.locomotion.tilt_y_age = 254;
    }
    crate::game::simulation::enter(
        fighter,
        match (surface, jump) {
            (Surface::Ceiling, _) => Action::PassiveCeiling,
            (_, true) => Action::PassiveWallJump,
            _ => Action::PassiveWall,
        },
    );
    fighter.surface_tech = SurfaceTechState {
        timer: if surface == Surface::Ceiling {
            0
        } else {
            profile.wall_freeze_frames
        },
        jump_queued: false,
        ceiling_velocity_applied: false,
    };
    jump
}

pub(crate) fn surface_tech_pose<'a>(
    fighter: &Fighter,
    data: &'a FighterData,
) -> Option<&'a Vec<Bone>> {
    let attributes = data.surface_tech.as_ref()?;
    let poses = match fighter.action {
        Action::PassiveWall => &attributes.passive_wall_poses,
        Action::PassiveWallJump if !fighter.wall_jump.active => &attributes.passive_wall_jump_poses,
        Action::PassiveCeiling => &attributes.passive_ceiling_poses,
        _ => return None,
    };
    poses.get(fighter.action_frame as usize)
}

pub(crate) fn surface_response_pose<'a>(
    fighter: &Fighter,
    data: &'a FighterData,
) -> Option<&'a Vec<Bone>> {
    let attributes = data.surface_response.as_ref()?;
    let poses = match fighter.action {
        Action::FlyReflectWall => &attributes.wall_poses,
        Action::FlyReflectCeiling => &attributes.ceiling_poses,
        _ => return None,
    };
    poses.get(fighter.action_frame as usize)
}

pub(crate) fn reflect(
    fighter: &mut Fighter,
    surface: crate::collision::stage::Surface,
    normal: [f32; 3],
    rules: &CombatRules,
) {
    let profile = rules.surface_response.as_ref().unwrap();
    let reflected = self::reflect_velocity(
        fighter.velocity,
        fighter.knockback,
        [normal[0], normal[1]],
        profile.velocity_multiplier,
    );
    fighter.velocity = [0.0; 2];
    fighter.knockback = reflected.knockback;
    fighter.ground_knockback = 0.0;
    fighter.ground_velocity = 0.0;
    fighter.facing = reflected.facing;
    fighter.grounded = false;
    fighter.ground_line = None;
    fighter.last_damage_surface = Some(surface);
    fighter.reflect_lockout = profile.lockout_frames;
    crate::game::simulation::enter(
        fighter,
        if matches!(surface, crate::collision::stage::Surface::Ceiling) {
            Action::FlyReflectCeiling
        } else {
            Action::FlyReflectWall
        },
    );
}

/// Damage-floor callback shared by Damage and DamageFall. False leaves a
/// non-tumbling or profile-free damage action under its existing policy.
pub(crate) fn land(
    fighter: &mut Fighter,
    data: &FighterData,
    pose: &crate::collision::bones::Pose,
    rules: &CombatRules,
    input: crate::game::Controller,
) -> Result<bool, Error> {
    if fighter.action == Action::DownDamage {
        // DownDamage's airborne collision callback converts to ground without
        // entering the ordinary tech/missed-tech landing graph.
        fighter.tumbling = false;
        return Ok(true);
    }
    let Some(profile) = &rules.floor_response else {
        return Ok(false);
    };
    if !fighter.tumbling {
        return Ok(false);
    }
    let action = if self::can_tech(
        false,
        fighter.locomotion.tech_press_age,
        fighter.locomotion.previous_tech_press_age,
        profile.tech_window,
        profile.tech_repeat_lockout,
    ) {
        profile
            .tech_roll
            .as_ref()
            .and_then(|roll| {
                self::tech_roll_direction(input.stick[0], fighter.facing, roll.stick_threshold)
            })
            .map_or(Action::Passive, |direction| match direction {
                self::TechRoll::Forward => Action::PassiveStandF,
                self::TechRoll::Backward => Action::PassiveStandB,
            })
    } else {
        Action::DownBound
    };
    enter_recovery(fighter, action, profile);
    if action == Action::DownBound {
        if let Some(attributes) = &data.knockdown {
            let matrix = pose
                .world_matrix(attributes.orientation.hip_bone)
                .map_err(physics)?;
            fighter.prone = Some(
                if self::prone_face_up(
                    matrix,
                    attributes.orientation.use_z_axis,
                    attributes.orientation.invert,
                ) {
                    ProneOrientation::FaceUp
                } else {
                    ProneOrientation::FaceDown
                },
            );
        }
        fighter.locomotion.attack_a_age = 255;
        fighter.locomotion.attack_b_age = 255;
    }
    Ok(true)
}

pub(crate) fn ground_recovery_pose<'a>(
    fighter: &Fighter,
    data: &'a FighterData,
) -> Option<&'a [Bone]> {
    if let Some(attributes) = &data.knockdown {
        if fighter.action == Action::Passive {
            return attributes
                .passive_poses
                .get(fighter.action_frame as usize)
                .map(Vec::as_slice);
        }
        let variant = attributes.variant(fighter.prone)?;
        let poses = match fighter.action {
            Action::DownBound => Some(&variant.bound_poses),
            Action::DownWait => Some(&variant.wait_poses),
            Action::DownDamage => variant.damage_poses.as_ref(),
            Action::DownStand => Some(&variant.stand_poses),
            _ => None,
        };
        if let Some(poses) = poses {
            return poses.get(fighter.action_frame as usize).map(Vec::as_slice);
        }
    }
    ground_motion(fighter, data)?
        .frames
        .get(fighter.action_frame as usize)
        .map(|frame| frame.bones.as_slice())
}

pub(crate) fn ground_recovery_velocity(fighter: &Fighter, data: &FighterData) -> Option<f32> {
    let frame = ground_motion(fighter, data)?
        .frames
        .get(fighter.action_frame as usize)?;
    Some(frame.root_translation * fighter.facing)
}

fn ground_motion<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a FloorTechMotion> {
    data.floor_tech
        .as_ref()
        .and_then(|attributes| attributes.motion(fighter.action))
        .or_else(|| {
            data.knockdown
                .as_ref()
                .and_then(|attributes| attributes.motion(fighter.action, fighter.prone))
        })
}

fn knockdown_input(fighter: &Fighter, input: crate::game::Controller) -> self::KnockdownInput {
    let pressed = input.buttons & !fighter.previous_input.buttons;
    self::KnockdownInput {
        main: input.stick,
        cstick: input.cstick,
        previous_cstick: fighter.previous_input.cstick,
        facing: fighter.facing,
        attack_pressed: pressed & (crate::game::BUTTON_A | crate::game::BUTTON_B) != 0,
        shoulder_pressed: pressed & (crate::game::BUTTON_L | crate::game::BUTTON_R) != 0,
    }
}

fn knockdown_rules(rules: &KnockdownRules) -> self::KnockdownParameters {
    self::KnockdownParameters {
        horizontal_stick_threshold: rules.horizontal_stick_threshold,
        stand_stick_threshold: rules.stand_stick_threshold,
        vertical_angle_radians: rules.vertical_angle_radians,
        attack_cstick_threshold: rules.attack_cstick_threshold,
        bound_attack_window: rules.bound_attack_window,
    }
}

fn knockdown_action(option: self::KnockdownOption) -> Action {
    match option {
        self::KnockdownOption::Attack => Action::DownAttack,
        self::KnockdownOption::Forward => Action::DownForward,
        self::KnockdownOption::Backward => Action::DownBack,
        self::KnockdownOption::Stand => Action::DownStand,
    }
}

fn enter_recovery(fighter: &mut Fighter, action: Action, floor: &FloorResponseRules) {
    crate::game::simulation::enter(fighter, action);
    apply_recovery_invincibility(fighter, action, floor);
}

fn enter_recovery_registered(
    fighter: &mut Fighter,
    data: &FighterData,
    action: Action,
    floor: &FloorResponseRules,
) {
    let slot = match action {
        Action::DownStand => crate::game::script::move_registry::MoveSlot::Neutral,
        Action::DownForward => crate::game::script::move_registry::MoveSlot::RollForward,
        Action::DownBack => crate::game::script::move_registry::MoveSlot::RollBack,
        Action::DownAttack => crate::game::script::move_registry::MoveSlot::Attack,
        _ => {
            enter_recovery(fighter, action, floor);
            return;
        }
    };
    match crate::game::script::move_selection::select_native_move(
        fighter,
        data,
        crate::game::script::move_registry::MoveGroup::Getup,
        slot,
        action,
    ) {
        crate::game::script::move_selection::NativeMoveSelection::Entered(destination)
        | crate::game::script::move_selection::NativeMoveSelection::Unbound(destination)
            if matches!(
                destination,
                Action::DownStand | Action::DownForward | Action::DownBack | Action::DownAttack
            ) =>
        {
            apply_recovery_invincibility(fighter, destination, floor)
        }
        crate::game::script::move_selection::NativeMoveSelection::Entered(_)
        | crate::game::script::move_selection::NativeMoveSelection::Unbound(_)
        | crate::game::script::move_selection::NativeMoveSelection::CallbackDriven { .. } => {}
    }
}

fn apply_recovery_invincibility(fighter: &mut Fighter, action: Action, floor: &FloorResponseRules) {
    let frames = floor
        .recovery_invincibility
        .as_ref()
        .map_or(0, |rules| match action {
            Action::Passive => rules.passive_frames,
            Action::PassiveStandF | Action::PassiveStandB => rules.tech_roll_frames,
            Action::DownForward | Action::DownBack => rules.missed_roll_frames,
            Action::DownStand => rules.stand_frames,
            Action::DownAttack => rules.attack_frames,
            _ => 0,
        });
    fighter.invincibility = fighter.invincibility.max(frames);
}

/// Called on frozen damage frames after the timer decrement, while it remains
/// positive. The last tick runs only the exit callback. Collision response runs
/// afterward using the displaced position and frozen collision pose. Input is
/// already sampled into the same x670/x671 ages used by ordinary locomotion.
pub(crate) fn during_hitlag(
    fighter: &mut Fighter,
    stick: [f32; 2],
    rules: &CombatRules,
) -> Result<(), Error> {
    if let Some(profile) = &rules.displacement
        && fighter.di_pending
    {
        let mut timers = [fighter.locomotion.tilt_x_age, fighter.locomotion.tilt_y_age];
        self::smash_displacement(
            &mut fighter.position,
            stick,
            &mut timers,
            true,
            profile.minimum_stick_magnitude,
            i32::from(profile.sdi_window),
            profile.sdi_distance,
        );
        [fighter.locomotion.tilt_x_age, fighter.locomotion.tilt_y_age] = timers;
        finite_position(fighter)?;
    }
    Ok(())
}

/// Called once when positive hitlag reaches zero, before normal motion resumes.
/// Main-stick ASDI precedes DI, including when the stick has been held throughout
/// hitlag. Attacker hitlag does not install this damage callback.
pub(crate) fn exit_hitlag(
    fighter: &mut Fighter,
    input: crate::game::Controller,
    rules: &CombatRules,
) -> Result<(), Error> {
    if fighter.di_pending {
        if let Some(profile) = &rules.displacement {
            // ftCo_Damage_OnExitHitlag gives a held C-stick priority for ASDI.
            // Main-stick DI below remains independent of that choice.
            let [x, y] = input.cstick;
            let stick = if x * x + y * y
                >= profile.minimum_stick_magnitude * profile.minimum_stick_magnitude
            {
                input.cstick
            } else {
                input.stick
            };
            self::automatic_displacement(
                &mut fighter.position,
                stick,
                profile.minimum_stick_magnitude,
                profile.asdi_distance,
            );
            finite_position(fighter)?;
        }
        let influenced =
            self::directional_influence(fighter.knockback, input.stick, rules.di_max_degrees);
        if influenced.into_iter().any(|value| !value.is_finite()) {
            return Err(Error::NonFinite);
        }
        fighter.knockback = influenced;
        fighter.di_pending = false;
    }
    Ok(())
}

fn finite_position(fighter: &Fighter) -> Result<(), Error> {
    if fighter.position.into_iter().any(|value| !value.is_finite()) {
        Err(Error::NonFinite)
    } else {
        Ok(())
    }
}

fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
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

    /// `ftCo_Damage_CalcAngle`'s `x148 * ratio + 1` is a single Gekko
    /// `fmadds` (`tools/ppc_fma_audit.py ftCo_Damage_CalcAngle`;
    /// `docs/math.md`); `launch_angle`'s grounded branch computes it with
    /// `f32::mul_add`. These inputs (a negative `ratio`, reached here via
    /// `grounded_high_knockback < grounded_low_knockback` rather than a
    /// negative knockback, since a knockback below `grounded_low_knockback`
    /// would otherwise take the early-return-zero branch instead) were
    /// chosen so the fused and naive roundings of that one expression
    /// disagree, and the disagreement survives the subsequent
    /// degrees-to-radians multiply as two different final bit patterns.
    /// Hand-verified against `libm`'s `fmaf` outside Rust.
    #[test]
    fn launch_angle_matches_the_hardware_fused_rounding_not_naive_two_rounding() {
        let rules = LaunchAngleRules {
            airborne_radians: 0.0,
            grounded_max_degrees: f32::from_bits(0x4295_3de4), // 74.62088012695312
            grounded_low_knockback: 0.0,
            grounded_high_knockback: -1.0,
            special_angle_min: 0,
            special_angle_max: 0,
            special_timer: 0,
        };
        // `special_angle_min`/`special_angle_max`/`special_timer` above are
        // unused on this path: they only apply when `angle != 361`.
        let knockback = f32::from_bits(0x3c4b_7a98); // 0.012419365346431732
        let result = launch_angle(361, knockback, false, &rules);
        assert_eq!(result.radians.to_bits(), 0x3aa7_9552);
        assert_eq!(result.special_timer, None);
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
    fn fast_fall_trigger_uses_the_tilt_timer_not_previous_input() {
        // `fox-bf.slp` (`docs/parity.md`'s "A first Battlefield recording"
        // section), window = 4 (read directly from the retail disc's own
        // `PlCo.dat`/`ftCommonData+0x8C`).
        const WINDOW: u32 = 4;
        const THRESHOLD: f32 = 0.6625;

        // Frame -27, P4: a platform pass just reset tilt_y_age to the 254
        // sentinel (`ftCo_8009A228`'s own `x671_timer_lstick_tilt_y = 0xFE`)
        // on the same continuously-held down-stick that triggered the
        // pass; the recording shows no fast-fall this frame (ordinary
        // gravity, `pass_velocity + gravity`), unlike the previous, approximate
        // `previous_input` heuristic, which would fire here (the previous
        // frame's raw stick was already past threshold too).
        assert!(!fast_fall_trigger(
            false, -0.5, -0.663, THRESHOLD, 254, WINDOW
        ));

        // Frame -21, P1: an ordinary fresh down-press one frame after
        // crossing the smash deadzone (tilt_y_age = 1, well inside the
        // window) -- the recording's own next fall shows an immediate
        // fast_fall_velocity-magnitude drop this exact frame.
        assert!(fast_fall_trigger(false, -0.9, -0.738, THRESHOLD, 1, WINDOW));

        // The window's own boundary is exclusive, matching decomp's `<`
        // (not `<=`): held exactly `window` frames no longer qualifies.
        assert!(fast_fall_trigger(false, -0.9, -0.738, THRESHOLD, 3, WINDOW));
        assert!(!fast_fall_trigger(
            false, -0.9, -0.738, THRESHOLD, 4, WINDOW
        ));

        // Every other precondition still gates the trigger: already
        // fast-falling, rising (or stationary) velocity, and a stick short
        // of the threshold all refuse it regardless of tilt_y_age.
        assert!(!fast_fall_trigger(true, -0.9, -0.738, THRESHOLD, 0, WINDOW));
        assert!(!fast_fall_trigger(false, 0.1, -0.738, THRESHOLD, 0, WINDOW));
        assert!(!fast_fall_trigger(false, -0.9, -0.5, THRESHOLD, 0, WINDOW));
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
        let rules = KnockdownParameters {
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
    fn prone_orientation_uses_the_selected_hip_axis_strict_sign_and_inversion() {
        let mut hip = [[0.0; 4]; 3];
        hip[1][1] = f32::from_bits(0x8000_0000);
        hip[1][2] = 1.0;
        assert!(!prone_face_up(&hip, false, false));
        assert!(prone_face_up(&hip, false, true));
        assert!(prone_face_up(&hip, true, false));
        hip[1][1] = f32::NAN;
        assert!(!prone_face_up(&hip, false, false));
    }

    #[test]
    fn down_damage_retains_strict_threshold_and_face_up_wait_quirk() {
        assert_eq!(down_damage_face_up(true, true, false, 4.0, 5), Some(true));
        assert_eq!(down_damage_face_up(true, false, false, 4.0, 5), Some(false));
        assert_eq!(down_damage_face_up(true, true, false, 5.0, 5), None);
        assert_eq!(
            down_damage_face_up(true, false, true, f32::NAN, 0),
            Some(false)
        );
        assert_eq!(down_damage_face_up(false, true, true, 0.0, 1), None);
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

    #[test]
    fn damage_motion_preserves_strict_levels_nan_fallthrough_and_source_table() {
        let thresholds = [10.0, 20.0, 30.0];
        for (knockback, level) in [(9.0, 0), (10.0, 1), (20.0, 2)] {
            for height in [HurtHeight::Low, HurtHeight::Middle, HurtHeight::High] {
                assert_eq!(
                    damage_motion(knockback, 1.0, thresholds, false, height),
                    DamageMotion::Ground { level, height }
                );
                assert_eq!(
                    damage_motion(knockback, 1.0, thresholds, true, height),
                    DamageMotion::Air { level }
                );
            }
        }
        for knockback in [30.0, f32::NAN] {
            assert_eq!(
                damage_motion(knockback, 1.0, thresholds, false, HurtHeight::High),
                DamageMotion::Fly {
                    height: HurtHeight::High
                }
            );
        }
        let expected = [
            [[81, 78, 75], [82, 79, 76], [83, 80, 77], [89, 88, 87]],
            [[84, 84, 84], [85, 85, 85], [86, 86, 86], [89, 88, 87]],
        ];
        for airborne in [false, true] {
            for (level, knockback) in [5.0, 15.0, 25.0, 35.0].into_iter().enumerate() {
                for height in [HurtHeight::Low, HurtHeight::Middle, HurtHeight::High] {
                    let motion = damage_motion(knockback, 1.0, thresholds, airborne, height);
                    assert_eq!(
                        damage_motion_id(motion),
                        expected[usize::from(airborne)][level][height.index()]
                    );
                }
            }
        }
    }

    #[test]
    fn fighter_and_throw_hit_directions_preserve_source_boundaries() {
        assert_eq!(fighter_hit_direction(1.0, 0.0), -1.0);
        assert_eq!(fighter_hit_direction(-1.0, 0.0), 1.0);
        assert_eq!(fighter_hit_direction(0.0, 0.0), 1.0);
        assert_eq!(fighter_hit_direction(-0.0, 0.0), 1.0);
        assert_eq!(fighter_hit_direction(f32::NAN, 0.0), 1.0);
        assert_eq!(throw_hit_direction(1.0), -1.0);
        assert_eq!(throw_hit_direction(-1.0), 1.0);
        assert_eq!(throw_hit_direction(0.0).to_bits(), (-0.0_f32).to_bits());
    }

    #[test]
    fn positional_launch_preserves_quadrants_truncation_and_vertical_threshold() {
        let point = |x, y| [x, y, 0.0];
        assert_eq!(
            positional_launch(point(0.0, 0.0), point(0.0, 0.0), point(-1.0, -1.0)),
            PositionalLaunch {
                direction: -1.0,
                angle_degrees: 45,
            }
        );
        assert_eq!(
            positional_launch(point(0.0, 0.0), point(0.0, 0.0), point(1.0, 0.5)),
            PositionalLaunch {
                direction: 1.0,
                angle_degrees: -26,
            }
        );
        for dx in [0.0, -0.0, 0.000009] {
            assert_eq!(
                positional_launch(point(0.0, 1.0), point(0.0, 1.0), point(-dx, 0.0)),
                PositionalLaunch {
                    direction: -1.0,
                    angle_degrees: 0,
                }
            );
        }
        assert_eq!(
            positional_launch(
                point(0.0, 0.00001),
                point(0.0, 0.00001),
                point(-0.00001, 0.0),
            )
            .angle_degrees,
            45
        );
    }

    #[test]
    fn ground_launch_preserves_floor_and_fly_boundaries() {
        let rules = GroundLaunchParameters {
            fly_bounce_angle_radians: 0.2,
            fly_bounce_vertical_multiplier: 0.5,
        };
        let grounded = ground_launch([4.0, 0.0], [0.0, 1.0], false, &rules);
        assert!(!grounded.airborne);
        assert_eq!(
            grounded.knockback.map(f32::to_bits),
            [4.0_f32.to_bits(), (-0.0_f32).to_bits()]
        );
        assert_eq!(grounded.ground_knockback, 4.0);

        let rising = ground_launch([4.0, 1.0], [0.0, 1.0], false, &rules);
        assert!(rising.airborne && !rising.bounced);
        assert_eq!(rising.knockback, [4.0, 1.0]);

        let fly_boundary = ground_launch([4.0, 0.0], [0.0, 1.0], true, &rules);
        assert!(fly_boundary.airborne && !fly_boundary.bounced);
        let bounced = ground_launch([4.0, -2.0], [0.0, 1.0], true, &rules);
        assert!(bounced.airborne && bounced.bounced);
        assert_eq!(bounced.knockback, [4.0, 1.0]);
    }

    #[test]
    fn ground_launch_projects_onto_slopes_and_tiny_vectors_take_angle_zero() {
        let rules = GroundLaunchParameters {
            fly_bounce_angle_radians: 0.0,
            fly_bounce_vertical_multiplier: 1.0,
        };
        assert_eq!(vector_angle([0.0, 1.0], [0.0; 2]), 0.0);
        assert_eq!(vector_angle([0.0, 1.0], [f32::NAN, 0.0]), 0.0);
        assert!(ground_launch([1.0e-12, 0.0], [0.0, 1.0], false, &rules).airborne);

        let launch = ground_launch([5.0, 0.0], [-0.6, 0.8], false, &rules);
        assert!(!launch.airborne);
        assert_eq!(launch.knockback, [4.0, 3.0]);
        assert_eq!(launch.ground_knockback, 5.0);
    }
}
