//! Fox/Falco up special (Fire Fox / Fire Bird): resource shape, validation
//! and the phase table genuinely specific to this move. Shared phase
//! behaviours (gravity-delayed fall, ground/air frame-preserving
//! conversion, the `FallSpecial` exit) come from `game::specials::helpers`
//! instead of being re-derived here, exactly like the side special. Gated
//! purely on resource presence.
//!
//! See `docs/fox-up-special.md` for the phase table, the decomp sources
//! pinned for the oracle, the wall/ceiling mid-Travel redirect, the
//! shallow-floor-graze sub-case of the landing bound decision, ground
//! Travel's own per-frame `rotateModel` re-derivation, `FallSpecial`'s own
//! stored `landing_lag` argument, Hold's/Travel's own script-embedded
//! hitboxes (its own "Hitboxes" section), and what stays unmodeled (the
//! visual model-rotation bone itself and the already-unreachable `x21F8`
//! callback).

use super::side;
use crate::{
    fighter::{Movement, edge::Mode},
    game::{
        Action, BUTTON_B, Controller, Error, Fighter,
        data::{Attack, FighterData, Rules as MatchRules},
        simulation,
        specials::{SpecialMove, helpers},
    },
};
use serde::{Deserialize, Serialize};

/// `ftfoxspecialhi.c`'s own `HALF_PI32` literal (the straight-up default
/// launch angle), kept bit-exact rather than reconstructed from
/// `core::f32::consts::FRAC_PI_2`.
#[allow(clippy::excessive_precision)]
const HALF_PI: f32 = 1.5707963705062866;

/// `fp->dat_attrs`'s up-special block (`ftFox/types.h:109-132`,
/// `x54..x94`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    /// `x54_FOX_FIREFOX_GRAVITY_DELAY`.
    pub gravity_delay: f32,
    /// `x58_FOX_FIREFOX_VEL_X`: divides the ground `gr_vel`/air `self_vel.x`
    /// carried into Hold.
    pub entry_speed_div: f32,
    /// `x5C_FOX_FIREFOX_AIR_MOMENTUM_PRESERVE_X`: Hold air's aerial friction.
    pub hold_air_friction: f32,
    /// `x60_FOX_FIREFOX_FALL_ACCEL`: Hold air's `ftCommon_Fall` gravity.
    pub hold_fall_accel: f32,
    /// `x64_FOX_FIREFOX_DIRECTION_STICK_RANGE_MIN`: the launch direction's
    /// stick-magnitude gate (`|stick.x| + |stick.y|`).
    pub direction_stick_min: f32,
    /// `x68_FOX_FIREFOX_DURATION`: Travel's initial frame countdown.
    pub duration: f32,
    /// `x6C_FOX_FIREFOX_BOUNCE_VAR`: the grounded-frame count (during
    /// ground Travel) past which an aerial landing always bounds.
    pub bounce_frames: i32,
    /// `x70_FOX_FIREFOX_DURATION_END`: the Travel frame count past which
    /// ground friction/air reverse-acceleration engages.
    pub duration_end: f32,
    /// `x74_FOX_FIREFOX_SPEED`: the launch speed.
    pub speed: f32,
    /// `x78_FOX_FIREFOX_REVERSE_ACCEL`: ground friction and air
    /// acceleration applied past `duration_end`.
    pub reverse_accel: f32,
    /// `x7C_FOX_FIREFOX_GROUND_MOMENTUM_END`: Landing's ground friction.
    pub landing_friction: f32,
    /// `x84_FOX_FIREFOX_BOUND_VEL_X`: Bound's entry `self_vel.x` multiplier.
    pub bound_speed_mul: f32,
    /// `x88_FOX_FIREFOX_FACING_STICK_RANGE_MIN`: the aerial launch's own
    /// turn-around gate, independent of the grounded launch's unconditional
    /// `ftCommon_UpdateFacing`.
    pub facing_stick_min: f32,
    /// `x8C_FOX_FIREFOX_FREEFALL_MOBILITY`: every `FallSpecial` exit's
    /// mobility multiplier.
    pub freefall_mobility: f32,
    /// `x90_FOX_FIREFOX_LANDING_LAG`: this move's own `ftCo_80096900`
    /// `landing_lag` argument, threaded through every `FallSpecial` exit
    /// (`aerial::State::landing_lag`) and consulted by that instance's own
    /// eventual landing instead of the shared `escape_air::Rules`' rate.
    pub landing_lag: f32,
    /// `x94_FOX_FIREFOX_BOUND_ANGLE`: the wall/ceiling redirect and landing
    /// bound decisions' angle gate, in degrees.
    pub bound_angle_degrees: f32,
    /// `ftCommonData.x1FC`: `ftCommon_8007CF58`'s over-max drift
    /// acceleration, used by Bound's air phase. Common data in the source,
    /// modeled per-fighter here since `SpecialMove::air_physics` has no
    /// match-wide `Rules` access.
    pub air_drift_clamp_accel: f32,
}

/// Bound's single pose set (`ftFx_MS_SpecialHiBound`; unlike Hold/Travel it
/// has no separate air motion id, shared by its own ground/air `Phys`/`Coll`
/// branches instead).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bound {
    pub pose: Attack,
    /// `ft_800851C0`: per-pose TransN y, written unconditionally into
    /// `self_vel.y` while airborne.
    pub transn_y: Vec<f32>,
    /// `cmd_vars[0]`: per-pose script flag. The first airborne frame this
    /// is set forces the `FallSpecial` exit ahead of the animation's own end.
    pub exit_flags: Vec<bool>,
}

/// `Fighter::up_special` (`fp->x1C_actionStateList` slice for
/// `ftFx_MS_SpecialHiHold..=ftFx_MS_SpecialHiBound`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpSpecial {
    pub hold: side::Phase,
    /// Travel's own hit is a real script-embedded hitbox (`docs/
    /// fox-up-special.md`'s "Hitboxes" section): a continuous bone-58
    /// capsule active on every sampled frame, dealing the pack's own 14
    /// damage. The pinned source itself (`ftfoxspecialhi.c`) never creates
    /// it directly -- it belongs to the animation script, like every other
    /// attack in this codebase.
    pub travel: side::Phase,
    pub landing: Attack,
    pub fall: Attack,
    pub bound: Bound,
    pub attributes: Attributes,
}

pub(crate) fn validate(
    specials_rules: &side::Rules,
    parameters: &UpSpecial,
    fighter: &FighterData,
    rules: &MatchRules,
) -> Result<(), Error> {
    side::validate_rules(specials_rules)?;
    let finite = |v: f32| v.is_finite() && v.abs() <= 1_000_000.0;
    let a = &parameters.attributes;
    if !finite(a.gravity_delay)
        || a.gravity_delay < 0.0
        || !finite(a.entry_speed_div)
        || a.entry_speed_div == 0.0
        || !finite(a.hold_air_friction)
        || a.hold_air_friction < 0.0
        || !finite(a.hold_fall_accel)
        || !finite(a.direction_stick_min)
        || a.direction_stick_min < 0.0
        || !finite(a.duration)
        || a.duration <= 0.0
        || a.bounce_frames < 0
        || !finite(a.duration_end)
        || a.duration_end < 0.0
        || !finite(a.speed)
        || !finite(a.reverse_accel)
        || a.reverse_accel < 0.0
        || !finite(a.landing_friction)
        || a.landing_friction < 0.0
        || !finite(a.bound_speed_mul)
        || !finite(a.facing_stick_min)
        || a.facing_stick_min < 0.0
        || !finite(a.freefall_mobility)
        || a.freefall_mobility < 0.0
        || !finite(a.landing_lag)
        || a.landing_lag <= 0.0
        || !finite(a.bound_angle_degrees)
        || !finite(a.air_drift_clamp_accel)
        || a.air_drift_clamp_accel < 0.0
    {
        return Err(Error::Data("invalid up-special attributes".into()));
    }
    for attack in [
        &parameters.hold.ground,
        &parameters.hold.air,
        &parameters.travel.ground,
        &parameters.travel.air,
        &parameters.landing,
        &parameters.fall,
        &parameters.bound.pose,
    ] {
        if attack.frames.is_empty() || attack.frames.len() > 4096 {
            return Err(Error::Data(
                "up special requires 1..4096 physics samples per phase".into(),
            ));
        }
        // Hold's charge pulse and Travel's own continuous hit are real
        // script-embedded hitboxes (`docs/fox-up-special.md`'s own
        // gameplay-export citation); every phase is checked uniformly since
        // the shape is the same `Attack`/`AttackFrame` either way.
        helpers::validate_hitboxes(attack, fighter, rules)?;
    }
    if parameters.bound.transn_y.len() != parameters.bound.pose.frames.len()
        || parameters.bound.exit_flags.len() != parameters.bound.pose.frames.len()
    {
        return Err(Error::Data(
            "up-special bound TransN/exit flags must cover every bound pose".into(),
        ));
    }
    if parameters.bound.transn_y.iter().any(|y| !finite(*y)) {
        return Err(Error::Data("up-special bound TransN must be finite".into()));
    }
    Ok(())
}

/// Persistent per-fighter state (`mv.fx.SpecialHi`).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub gravity_delay: f32,
    /// `rotateModel`: the launch/redirect direction in radians. Drives the
    /// Travel air phase's own post-`duration_end` acceleration; the
    /// source's additional use rotating a visual bone
    /// (`ftFox_SpecialHi_RotateModel`) stays unmodeled (rendering only).
    pub rotate_model: f32,
    /// `travelFrames`: counts down from `duration` (x68).
    pub travel_frames: f32,
    /// `unk`: Travel's duration-end counter against `duration_end` (x70).
    pub unk: f32,
    /// `unk2`: the grounded-frame counter Travel's ground Coll accumulates,
    /// read by the aerial landing's bound-eligibility gate.
    pub unk2: f32,
}

pub(crate) fn attack(action: Action, parameters: &UpSpecial) -> Option<&Attack> {
    match action {
        Action::SpecialHiHold => Some(&parameters.hold.ground),
        Action::SpecialHiHoldAir => Some(&parameters.hold.air),
        Action::SpecialHi => Some(&parameters.travel.ground),
        Action::SpecialAirHi => Some(&parameters.travel.air),
        Action::SpecialHiLanding => Some(&parameters.landing),
        Action::SpecialHiFall => Some(&parameters.fall),
        Action::SpecialHiBound => Some(&parameters.bound.pose),
        _ => None,
    }
}

/// `sqrtf_accurate` (`MSL/math_ppc.h`): four fused Newton-Raphson
/// iterations, in double precision, refining a reciprocal-square-root
/// estimate that the real hardware seeds from `__frsqrte` (a Gekko
/// instruction with no portable equivalent). Each iteration's `3.0 -
/// guess * guess * x` is a single Gekko `fnmsub` (`tools/ppc_fma_audit.py
/// lbVector_AngleXY`; `docs/math.md`, where the same iteration appears
/// inlined into `lbVector_AngleXY` itself), so it is computed here with
/// `f64::mul_add` for that one rounding rather than two.
///
/// This does not need to start from a bit-exact `__frsqrte` estimate to
/// land on the same answer: Newton's method for `1/sqrt(x)` has a single,
/// stable fixed point with quadratic convergence, so any starting guess
/// already accurate to a handful of bits reaches the fixed point (to full
/// double precision, given `__frsqrte`'s documented ~12-bit accuracy
/// doubling on each of these four iterations to 12, 24, 48, then 96 bits)
/// well before the fourth iteration, and further iterations from that
/// point are idempotent up to rounding. Seeding from `1.0 / sqrt(x)`
/// (`f64::sqrt` is correctly rounded, i.e. far more accurate than
/// `__frsqrte`'s own estimate, not less) converges to the identical fixed
/// point the real hardware's four iterations do.
fn sqrt_accurate(x: f32) -> f32 {
    if x > 0.0 {
        let x64 = f64::from(x);
        let mut guess = 1.0 / x64.sqrt();
        for _ in 0..4 {
            let refined = x64.mul_add(-(guess * guess), 3.0);
            guess = (0.5 * guess) * refined;
        }
        (x64 * guess) as f32
    } else {
        x
    }
}

/// `lbVector_AngleXY`: the clamped angle between two XY vectors (Z ignored).
fn angle_xy(a: [f32; 3], b: [f32; 2]) -> f32 {
    let len_a = sqrt_accurate(a[0] * a[0] + a[1] * a[1]);
    let len_b = sqrt_accurate(b[0] * b[0] + b[1] * b[1]);
    let product = len_a * len_b;
    // `if (lena_lenb)`: a C truthy (non-zero) check, not `> 0.0`. The two
    // agree for every ordinary finite product (both lengths are
    // non-negative, so the product can never be negative), but diverge for
    // a NaN product (a near-zero vector paired with one large enough to
    // overflow `len_b` to infinity, `0.0 * inf = NaN`): C's `!= 0` is true
    // for NaN and falls into `acosf(NaN)` (propagating NaN, which the
    // caller's own `!(angle < threshold)` then treats as satisfied), while
    // `> 0.0` is false for NaN and would have silently substituted 0.0.
    if product != 0.0 {
        // `a.x * b.x + a.y * b.y` is a single Gekko `fmadds`
        // (`tools/ppc_fma_audit.py lbVector_AngleXY`; `docs/math.md`).
        let dot = a[1].mul_add(b[1], a[0] * b[0]);
        let cosine = (dot / product).clamp(-1.0, 1.0);
        crate::compat::math::trig::acosf(cosine)
    } else {
        0.0
    }
}

/// `ftCommon_UpdateFacing`: face the stick's horizontal sign, unconditional.
fn face_stick(stick_x: f32) -> f32 {
    if stick_x >= 0.0 { 1.0 } else { -1.0 }
}

/// `0.01745329238474369f * (90.0f + x94)`: the pinned source's own
/// degrees-to-radians literal applied to the bound-angle attribute, kept
/// bit-exact rather than reconstructed. Shared by the landing bound
/// decision's own approach-angle gate and the mid-Travel wall/ceiling
/// redirect's identical one.
#[allow(clippy::excessive_precision)]
fn bound_angle_gate(bound_angle_degrees: f32) -> f32 {
    0.01745329238474369_f32 * (90.0 + bound_angle_degrees)
}

/// `ftFx_SpecialAirHi_Coll`'s own "facingDir" block: recompute facing from
/// the velocity's own horizontal sign, and the model-rotation angle from
/// that same velocity (independent of whatever contact normal triggered
/// this). Shared by the mid-Travel wall/ceiling redirect and the landing
/// bound decision's own shallow-floor-angle graze.
fn redirect_from_velocity(velocity: [f32; 2]) -> (f32, f32) {
    let facing = face_stick(velocity[0]);
    (
        facing,
        crate::compat::math::trig::atan2f(velocity[1], velocity[0] * facing),
    )
}

/// `ftFx_SpecialAirHi_AirToGround`, 418-482: the Hold-ground anim-end
/// decision between launching along the current floor and leaving the
/// ground for the aerial launch.
fn enter_from_ground_hold(
    fighter: &mut Fighter,
    data: &FighterData,
    p: &UpSpecial,
    input: Controller,
    on_platform: bool,
) {
    let a = &p.attributes;
    let stick = input.stick;
    let magnitude = stick[0].abs() + stick[1].abs();
    // `ftFx_SpecialAirHi_AirToGround` writes both gates as negated
    // less-than (`!(magnitude < x64)`, `!(angle < HALF_PI32)`), unlike
    // `ftFx_SpecialAirHi_Enter`'s own direct `>=`; kept as the exact
    // negated form rather than folded into `>=`, since the two diverge for
    // a NaN operand (`angle_xy` can legitimately return NaN, see its own
    // comment) -- `!(NaN < x)` is true, but `NaN >= x` is false.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    let along_ground = !(magnitude < a.direction_stick_min)
        && !(angle_xy(fighter.floor_normal, stick) < HALF_PI)
        && !on_platform;
    if along_ground {
        fighter.facing = face_stick(stick[0]);
        let floor_normal = fighter.floor_normal;
        simulation::enter(fighter, Action::SpecialHi);
        fighter.fox_up_special.travel_frames = a.duration;
        fighter.ground_velocity = a.speed * fighter.facing;
        fighter.fox_up_special.rotate_model =
            crate::compat::math::trig::atan2f(-floor_normal[0] * fighter.facing, floor_normal[1]);
    } else {
        // ftCommon_8007D60C's behaviorally-observable core: leave the
        // ground under the fighter's own decision, not a physical loss of
        // contact (the ECB-lock/hazard-light bookkeeping it also touches
        // stays unmodeled, matching every other manual leave-ground site
        // in this codebase, e.g. `wall_jump::enter`).
        fighter.grounded = false;
        fighter.ground_line = None;
        fighter.ground_velocity = 0.0;
        enter_aerial_launch(fighter, data, p, input);
    }
}

/// `ftFx_SpecialAirHi_Enter`, 418-482 (the pure aerial launch: fresh from
/// Hold air's own anim end, or the grounded launch's declined branch above).
fn enter_aerial_launch(
    fighter: &mut Fighter,
    data: &FighterData,
    p: &UpSpecial,
    input: Controller,
) {
    let a = &p.attributes;
    let stick = input.stick;
    let magnitude = stick[0].abs() + stick[1].abs();
    let angle = if magnitude >= a.direction_stick_min {
        if stick[0].abs() > a.facing_stick_min {
            fighter.facing = face_stick(stick[0]);
        }
        crate::compat::math::trig::atan2f(stick[1], stick[0] * fighter.facing)
    } else {
        HALF_PI
    };
    let facing = fighter.facing;
    simulation::enter(fighter, Action::SpecialAirHi);
    fighter.fox_up_special.rotate_model = angle;
    fighter.fox_up_special.travel_frames = a.duration;
    // `ftFx_SpecialAirHi_Enter`: `facing_dir * (x74 * cosf(rotateModel))`, not
    // `(facing_dir * x74) * cosf(rotateModel)` -- f32 multiplication is not
    // associative, so the grouping is kept bit-exact to the source.
    fighter.velocity = [
        facing * (a.speed * crate::compat::math::trig::cosf(angle)),
        a.speed * crate::compat::math::trig::sinf(angle),
    ];
    helpers::max_out_jumps(fighter, data);
}

/// `ftFx_SpecialHiBound_Enter`: makes the same extra, explicit
/// `ftAnim_8006EBA4(gobj)` call the Hold entries above do (see their own
/// comment), so `action_frame` is 1 (not 0) from this frame on.
fn enter_bound(fighter: &mut Fighter, p: &UpSpecial) {
    simulation::enter(fighter, Action::SpecialHiBound);
    fighter.action_frame = 1;
    fighter.velocity[0] *= p.attributes.bound_speed_mul;
}

/// This move's handle in Fox's registry (`MOVES` in `characters::fox`).
pub(crate) struct Move;

pub(crate) const MOVE: Move = Move;

impl SpecialMove for Move {
    fn owns(&self, action: Action) -> bool {
        matches!(
            action,
            Action::SpecialHiHold
                | Action::SpecialHiHoldAir
                | Action::SpecialHi
                | Action::SpecialAirHi
                | Action::SpecialHiLanding
                | Action::SpecialHiFall
                | Action::SpecialHiBound
        )
    }

    fn attack<'a>(&self, action: Action, data: &'a FighterData) -> Option<&'a Attack> {
        attack(action, data.specials.as_ref()?.fox_up()?)
    }

    /// Grounded: `ftCo_Attack100_CheckInput` (checked after `SpecialS`, so
    /// Fox's registry lists this move after `side::MOVE`). Aerial:
    /// `ftCo_SpecialAir_CheckInput`'s Hi branch, checked ahead of Lw/S/N --
    /// the side special's own air branch already defers whenever the stick
    /// clears the vertical threshold, so list order alone reproduces both
    /// priorities correctly.
    fn update_actions(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        rules: &MatchRules,
        ground: bool,
        air: bool,
        input: Controller,
    ) -> bool {
        let (Some(rules), Some(parameters)) = (
            rules.specials.as_ref(),
            data.specials.as_ref().and_then(|s| s.fox_up()),
        ) else {
            return false;
        };
        if self.owns(fighter.action) {
            return true;
        }
        if ground {
            if fighter.locomotion.up_special_b_age != 0 {
                return false;
            }
            fighter.ground_velocity /= parameters.attributes.entry_speed_div;
            simulation::enter(fighter, Action::SpecialHiHold);
            // `ftFx_SpecialHi_Enter` makes an extra, explicit
            // `ftAnim_8006EBA4(gobj)` call immediately after
            // `Fighter_ChangeMotionState` lands `cur_anim_frame` on `0.0`
            // -- the same second advance `ftCo_Dash_Enter`/`ftCo_Turn_Enter`
            // make (`locomotion::start_dash`'s own comment, `docs/
            // validation.md`'s entry-advance table). Modeled the same way,
            // at the source: `action_frame` is 1 (not 0) from this frame on.
            fighter.action_frame = 1;
            fighter.fox_up_special.gravity_delay = parameters.attributes.gravity_delay;
            return true;
        }
        if air {
            let fresh_b = input.buttons & !fighter.previous_input.buttons & BUTTON_B != 0;
            if fresh_b && input.stick[1] >= rules.vertical_threshold {
                fighter.velocity[0] /= parameters.attributes.entry_speed_div;
                fighter.velocity[1] = 0.0;
                simulation::enter(fighter, Action::SpecialHiHoldAir);
                // `ftFx_SpecialAirHiStart_Enter` makes the identical extra
                // advance `ftFx_SpecialHi_Enter` does above.
                fighter.action_frame = 1;
                fighter.fox_up_special.gravity_delay = parameters.attributes.gravity_delay;
                return true;
            }
        }
        false
    }

    fn update_animation(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        input: Controller,
        on_platform: bool,
    ) {
        let Some(p) = data.specials.as_ref().and_then(|s| s.fox_up()) else {
            return;
        };
        match fighter.action {
            Action::SpecialHiHold
                if fighter.action_frame as usize >= p.hold.ground.frames.len() =>
            {
                enter_from_ground_hold(fighter, data, p, input, on_platform);
            }
            Action::SpecialHiHoldAir
                if fighter.action_frame as usize >= p.hold.air.frames.len() =>
            {
                enter_aerial_launch(fighter, data, p, input);
            }
            Action::SpecialHi | Action::SpecialAirHi => {
                let len = if fighter.action == Action::SpecialHi {
                    p.travel.ground.frames.len()
                } else {
                    p.travel.air.frames.len()
                };
                // The Travel pose loops visually while `travel_frames`
                // (unrelated to `action_frame`) governs the actual duration.
                if len != 0 && fighter.action_frame as usize >= len {
                    fighter.action_frame %= len as u32;
                }
                fighter.fox_up_special.travel_frames -= 1.0;
                if fighter.fox_up_special.travel_frames <= 0.0 {
                    if fighter.grounded {
                        simulation::enter(fighter, Action::SpecialHiLanding);
                    } else {
                        simulation::enter(fighter, Action::SpecialHiFall);
                    }
                }
            }
            Action::SpecialHiLanding if fighter.action_frame as usize >= p.landing.frames.len() => {
                simulation::enter(fighter, Action::Wait);
            }
            Action::SpecialHiFall if fighter.action_frame as usize >= p.fall.frames.len() => {
                helpers::enter_fall_special(
                    fighter,
                    data,
                    p.attributes.freefall_mobility,
                    Some(p.attributes.landing_lag),
                );
            }
            Action::SpecialHiBound => {
                let frame = fighter.action_frame as usize;
                if p.bound.exit_flags.get(frame).copied().unwrap_or(false) && !fighter.grounded {
                    helpers::enter_fall_special(
                        fighter,
                        data,
                        p.attributes.freefall_mobility,
                        Some(p.attributes.landing_lag),
                    );
                } else if frame >= p.bound.pose.frames.len() {
                    if fighter.grounded {
                        simulation::enter(fighter, Action::Wait);
                    } else {
                        helpers::enter_fall_special(
                            fighter,
                            data,
                            p.attributes.freefall_mobility,
                            Some(p.attributes.landing_lag),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn ground_friction_override(&self, fighter: &Fighter, data: &FighterData) -> Option<f32> {
        let p = data.specials.as_ref()?.fox_up()?;
        match fighter.action {
            Action::SpecialHi => Some(if fighter.fox_up_special.unk >= p.attributes.duration_end {
                p.attributes.reverse_accel
            } else {
                0.0
            }),
            Action::SpecialHiLanding => Some(p.attributes.landing_friction),
            _ => None,
        }
    }

    fn air_physics(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        rules: &MatchRules,
        movement: &mut Movement,
    ) -> bool {
        let _ = rules;
        let Some(p) = data.specials.as_ref().and_then(|s| s.fox_up()) else {
            return false;
        };
        let terminal_velocity = movement.attributes.terminal_velocity;
        match fighter.action {
            Action::SpecialHiHoldAir => {
                helpers::gravity_delayed_fall(
                    &mut fighter.fox_up_special.gravity_delay,
                    movement,
                    p.attributes.hold_fall_accel,
                    terminal_velocity,
                    p.attributes.hold_air_friction,
                );
                true
            }
            Action::SpecialAirHi => {
                fighter.fox_up_special.unk += 1.0;
                if fighter.fox_up_special.unk >= p.attributes.duration_end {
                    let rotate = fighter.fox_up_special.rotate_model;
                    let accel = p.attributes.reverse_accel;
                    // `ftFx_SpecialAirHi_Phys`: `facing_dir * (x78 * cosf(rotateModel))`,
                    // kept bit-exact to the source's own grouping (see
                    // `enter_aerial_launch`'s identical note).
                    movement.animation_velocity[0] = -((fighter.facing
                        * (accel * crate::compat::math::trig::cosf(rotate)))
                        - movement.self_velocity[0]);
                    movement.animation_velocity[1] = -((accel
                        * crate::compat::math::trig::sinf(rotate))
                        - movement.self_velocity[1]);
                }
                true
            }
            Action::SpecialHiBound => {
                let frame = fighter.action_frame as usize;
                if let Some(&y) = p.bound.transn_y.get(frame) {
                    movement.self_velocity[1] = y;
                }
                movement.drift_clamp(
                    movement.attributes.air_drift_max,
                    p.attributes.air_drift_clamp_accel,
                );
                true
            }
            _ => false,
        }
    }

    fn tick_ground_timers(&self, fighter: &mut Fighter) {
        if fighter.action == Action::SpecialHi {
            // `ftFx_SpecialHi_Phys`'s `unk` and `ftFx_SpecialHi_Coll`'s
            // `unk2` are separate source callbacks, both once per grounded
            // Travel frame; this move ticks both together here since the
            // framework exposes a single grounded-frame hook.
            fighter.fox_up_special.unk += 1.0;
            fighter.fox_up_special.unk2 += 1.0;
        }
    }

    /// `ftFx_SpecialHi_Coll`'s own floor-contact branch: while ground
    /// Travel keeps touching the floor, `rotateModel` continuously tracks
    /// the current floor normal (the fighter's model visibly reorients as
    /// the floor's own slope changes underfoot), not just the launch
    /// angle from entry. Read by the air phase's own post-`duration_end`
    /// reverse acceleration if Travel later leaves the ground at an edge.
    fn update_ground_contact(&self, fighter: &mut Fighter) {
        if fighter.action == Action::SpecialHi && fighter.grounded {
            let floor_normal = fighter.floor_normal;
            fighter.fox_up_special.rotate_model = crate::compat::math::trig::atan2f(
                -floor_normal[0] * fighter.facing,
                floor_normal[1],
            );
        }
    }

    fn transfer_ground_air(&self, fighter: &mut Fighter, grounded: bool) -> bool {
        let destination = match (fighter.action, grounded) {
            (Action::SpecialHiHold, false) => Action::SpecialHiHoldAir,
            (Action::SpecialHiHoldAir, true) => Action::SpecialHiHold,
            (Action::SpecialHi, false) => Action::SpecialAirHi,
            // Travel's air-to-ground direction is not a plain conversion:
            // it is the bound/continue decision below, via `land`.
            _ => return false,
        };
        let state = fighter.fox_up_special.clone();
        helpers::transfer_frame(fighter, destination);
        fighter.fox_up_special = state;
        true
    }

    /// Travel air's own landing (`ftFx_SpecialAirHi_Coll`'s bound branch),
    /// Fall's own ordinary ground touch (`ftFx_SpecialHiFall_Coll` ->
    /// `ftFx_SpecialHiFall_Enter`), and Bound's landing while already
    /// grounded-eligible (`ftCommon_8007D7FC`, stay in Bound).
    fn land(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        on_platform: bool,
        pre_landing: &Fighter,
    ) -> Result<bool, Error> {
        let Some(p) = data.specials.as_ref().and_then(|s| s.fox_up()) else {
            return Ok(false);
        };
        match fighter.action {
            // The shared collision pipeline's own landing bookkeeping
            // (ground_velocity from self_vel.x, jumps reset, ecb unlock)
            // already ran before this hook; Bound has nothing more to do.
            Action::SpecialHiBound => Ok(true),
            // `ftFx_SpecialHiFall_Coll`'s own ground/ledge check succeeding
            // enters `SpecialHiLanding` directly at frame 13
            // (`ftFx_SpecialHiFall_Enter`: `Ft_MF_SkipColAnim |
            // Ft_MF_UpdateCmd`, start 13.0), distinct from Travel's own
            // duration-end landing (frame 0, `update_animation` above).
            // Without this arm the generic collision pipeline would instead
            // fall through to the ordinary `Action::Landing`. `ftFx_
            // SpecialHiFall_Enter` then makes the same extra, explicit
            // `ftAnim_8006EBA4(gobj)` call every other Start/Hold/Bound
            // entry in this table does (see `enter_bound`'s own comment),
            // one frame beyond the `13.0` `Fighter_ChangeMotionState`
            // start frame it just landed on: 14, not 13.
            Action::SpecialHiFall => {
                simulation::enter(fighter, Action::SpecialHiLanding);
                fighter.action_frame = 14;
                Ok(true)
            }
            Action::SpecialAirHi => {
                // ftFox_SpecialHi_IsBound.
                let bound_eligible = fighter.fox_up_special.unk2
                    >= p.attributes.bounce_frames as f32
                    || !on_platform;
                if bound_eligible {
                    // The source's own approach-angle check against the
                    // floor normal, using the velocity the fighter actually
                    // landed with -- the shared pipeline has already
                    // zeroed the vertical component of `fighter.velocity`
                    // itself by the time this hook runs, so this reads
                    // `pre_landing`'s own copy instead. `fighter.floor_normal`
                    // is already this frame's fresh contact (set by the
                    // caller just before this call), unaffected by the
                    // later `pre_landing` restore in the graze branch below.
                    let gate = bound_angle_gate(p.attributes.bound_angle_degrees);
                    #[allow(clippy::neg_cmp_op_on_partial_ord)]
                    let angle_ok = !(angle_xy(fighter.floor_normal, pre_landing.velocity) < gate);
                    if angle_ok {
                        enter_bound(fighter, p);
                    } else {
                        // The shallow-floor-angle graze (`goto facingDir`):
                        // the fighter stays airborne, so this declines the
                        // landing wholesale (restoring the pre-landing
                        // snapshot undoes grounded/velocity/knockback/jumps/
                        // wall-jump state together, rather than
                        // hand-reverting each), then applies only the
                        // redirect on top, matching the source's own
                        // "recompute facing/rotateModel, nothing else".
                        let (facing, rotate_model) = redirect_from_velocity(pre_landing.velocity);
                        *fighter = pre_landing.clone();
                        fighter.facing = facing;
                        fighter.fox_up_special.rotate_model = rotate_model;
                    }
                } else {
                    let state = fighter.fox_up_special.clone();
                    helpers::transfer_frame(fighter, Action::SpecialHi);
                    fighter.fox_up_special = state;
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn collision_mode(&self, action: Action) -> Option<Mode> {
        // Hold/Travel/Landing are plain (the unmatched default in
        // `edge::mode_for_action` already gives them that, matching Travel
        // ground naturally leaving the floor at an edge); only Bound clamps.
        matches!(action, Action::SpecialHiBound).then_some(Mode::Clamp)
    }

    fn ledge_catchable(&self, action: Action) -> bool {
        matches!(
            action,
            Action::SpecialHiHoldAir
                | Action::SpecialAirHi
                | Action::SpecialHiFall
                | Action::SpecialHiBound
        )
    }

    fn wants_redirect(&self, action: Action) -> bool {
        action == Action::SpecialAirHi
    }

    /// `ftFx_SpecialAirHi_Coll`'s own non-ground branch (past the ledge
    /// catch, handled by the separate ledge system): ceiling takes
    /// priority over either wall (the source's own `if/else if` chain, not
    /// a fallback -- if the ceiling candidate exists at all, only its own
    /// angle is tested, even when that angle fails the gate and a wall
    /// contact also exists this same step).
    fn air_contact(
        &self,
        fighter: &mut Fighter,
        data: &FighterData,
        ceiling: Option<([f32; 3], usize)>,
        wall: Option<([f32; 3], usize)>,
    ) -> bool {
        if fighter.action != Action::SpecialAirHi {
            return false;
        }
        let Some(p) = data.specials.as_ref().and_then(|s| s.fox_up()) else {
            return false;
        };
        let Some((normal, _line)) = ceiling.or(wall) else {
            return false;
        };
        let gate = bound_angle_gate(p.attributes.bound_angle_degrees);
        // `if (var < gate) { goto facingDir; } else { continue; }`. Written
        // as the negation (`!(var < gate)` bails out) rather than `>=`,
        // since a NaN angle (`angle_xy` can legitimately return one, see
        // its own doc) must still take the source's own "nothing happens"
        // path: `NaN < gate` is false either way, but `!(NaN < gate)` is
        // true (bail out, matching the source falling to its `continue`)
        // while `NaN >= gate` is *also* false (would wrongly redirect).
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        let clears_gate = !(angle_xy(normal, fighter.velocity) < gate);
        if clears_gate {
            return false;
        }
        let (facing, rotate_model) = redirect_from_velocity(fighter.velocity);
        fighter.facing = facing;
        fighter.fox_up_special.rotate_model = rotate_model;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_xy_zero_vectors_return_zero() {
        assert_eq!(angle_xy([0.0, 0.0, 0.0], [0.0, 0.0]), 0.0);
        assert_eq!(angle_xy([1.0, 0.0, 0.0], [0.0, 0.0]), 0.0);
        assert_eq!(angle_xy([0.0, 0.0, 0.0], [1.0, 0.0]), 0.0);
    }

    #[test]
    fn angle_xy_parallel_and_perpendicular() {
        assert_eq!(angle_xy([1.0, 0.0, 0.0], [1.0, 0.0]), 0.0);
        // `angle_xy` now calls `crate::compat::math::trig::acosf`, seeded from a real
        // reciprocal-sqrt estimate rather than this decompilation project's
        // own placeholder-derived (non-convergent) one -- see
        // `frsqrte_newton3`'s doc comment and `docs/math.md`. Unlike the
        // `libm` crate this replaced (each of the following one ULP short of
        // the idealized constant -- see `docs/fox-up-special.md`'s Oracle
        // section), it lands exactly on `FRAC_PI_2`/`PI`.
        assert_eq!(
            angle_xy([1.0, 0.0, 0.0], [0.0, 1.0]),
            std::f32::consts::FRAC_PI_2
        );
        assert_eq!(
            angle_xy([0.0, 1.0, 0.0], [1.0, 0.0]),
            std::f32::consts::FRAC_PI_2
        );
        assert_eq!(angle_xy([1.0, 0.0, 0.0], [-1.0, 0.0]), std::f32::consts::PI);
    }

    /// `lbVector_AngleXY`'s own `if (lena_lenb)` is a non-zero check, not a
    /// positive one: a product that overflows to NaN (a near-zero vector
    /// paired with one large enough that its own squared length overflows
    /// `f32`) must still reach `acosf`, propagating NaN, rather than
    /// silently substituting 0.0 the way a `> 0.0` guard would.
    #[test]
    fn angle_xy_nan_product_propagates_nan_not_zero() {
        let huge = 2.0e19_f32;
        let angle = angle_xy([0.0, 0.0, 0.0], [huge, 0.0]);
        assert!(
            (huge * huge).is_infinite(),
            "test premise: this must overflow"
        );
        assert!(angle.is_nan(), "expected NaN, got {angle}");
    }

    #[test]
    fn angle_xy_is_nan_safe_for_nan_inputs() {
        assert!(angle_xy([f32::NAN, 0.0, 0.0], [1.0, 0.0]).is_nan());
        assert!(angle_xy([1.0, 0.0, 0.0], [f32::NAN, 0.0]).is_nan());
    }

    #[test]
    fn face_stick_sign_and_zero() {
        assert_eq!(face_stick(0.5), 1.0);
        assert_eq!(face_stick(-0.5), -1.0);
        // `>= 0.0`: exactly zero (and negative zero) face right, matching
        // `ftCommon_UpdateFacing`'s own `>= 0` branch.
        assert_eq!(face_stick(0.0), 1.0);
        assert_eq!(face_stick(-0.0), 1.0);
    }
}
