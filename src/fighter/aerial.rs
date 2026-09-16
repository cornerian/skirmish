//! Ordinary aerial selection and landing arithmetic from ftCo_AttackAir.c,
//! ft_0DF1.c, ftcommon.c and ftCo_LandingAir.c. The caller supplies calibrated
//! sticks, common-data thresholds and the correct animation's end frame.
use super::combat::CombatError;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionRules {
    /// ftCommonData::xDC/xE0; also the C-stick excursion thresholds.
    pub thresholds: [f32; 2],
    /// ftCommonData::x20_radians, with strict up/down angle comparisons.
    pub vertical_angle: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Neutral,
    Forward,
    Back,
    Up,
    Down,
}

/// ftCo_800DF478. Either axis must newly cross its absolute threshold. A direct
/// sign reversal while still outside the threshold is not a fresh excursion.
pub fn fresh_cstick(previous: [f32; 2], current: [f32; 2], thresholds: [f32; 2]) -> bool {
    (crate::compat::source_ops::comparison_abs(previous[0]) < thresholds[0]
        && crate::compat::source_ops::comparison_abs(current[0]) >= thresholds[0])
        || (crate::compat::source_ops::comparison_abs(previous[1]) < thresholds[1]
            && crate::compat::source_ops::comparison_abs(current[1]) >= thresholds[1])
}

/// ftCo_GetLStickAngle / ftCo_GetCStickAngle use atan2(y, ABS(x)), independent
/// of facing. Runtime/platform.h's comparison-based ABS retains negative zero.
/// libm replaces the target transcendental routine; numerical parity is tested.
pub fn stick_angle([x, y]: [f32; 2]) -> f32 {
    crate::compat::math::trig::atan2f(y, crate::compat::source_ops::comparison_abs(x))
}

/// ftCo_AttackAir_GetMsidFromCStick. Only a fresh C-stick overrides the main
/// stick; neutral is an AND of strict axis thresholds, then vertical selection
/// precedes the facing-relative horizontal test (which includes equality).
pub fn select(
    main: [f32; 2],
    cstick: [f32; 2],
    previous_cstick: [f32; 2],
    facing: f32,
    rules: &SelectionRules,
) -> Direction {
    let [x, y] = if fresh_cstick(previous_cstick, cstick, rules.thresholds) {
        cstick
    } else {
        main
    };
    let angle = stick_angle([x, y]);
    if crate::compat::source_ops::comparison_abs(x) < rules.thresholds[0]
        && crate::compat::source_ops::comparison_abs(y) < rules.thresholds[1]
    {
        Direction::Neutral
    } else if angle > rules.vertical_angle {
        Direction::Up
    } else if angle < -rules.vertical_angle {
        Direction::Down
    } else if x * facing >= 0.0 {
        Direction::Forward
    } else {
        Direction::Back
    }
}

/// The lag branch in ftCo_LandingAir_EnterWithLag, with landing-lag script flag
/// enabled and a valid ordinary aerial selected. Auto-cancel/basic landing is
/// a separate caller decision. Invalid C float-to-int inputs return an error.
pub fn landing_lag(
    base: f32,
    shield_age: u8,
    window: i32,
    divisor: f32,
) -> Result<f32, CombatError> {
    if i32::from(shield_age) < window {
        let divided = base / divisor;
        if !(-2_147_483_648.0..2_147_483_648.0).contains(&divided) {
            return Err(CombatError::UndefinedIntegerConversion);
        }
        let frames = divided as i32;
        Ok(if frames == 0 { 1.0 } else { frames as f32 })
    } else {
        Ok(base)
    }
}

/// ftCo_LandingAir_EnterWithMsidLag's ftAnim_SetAnimRate argument. End frame
/// comes from ftAnim_8006F484's selected animation, not a count of sampled poses.
pub fn landing_animation_rate(end_frame: f32, lag: f32) -> f32 {
    (end_frame + 0.1) / lag
}

// Ordinary aerial callbacks composed with explicit sampled physics resources.
// Item throws, character overrides and the complete interrupt chain are separate.
use crate::game::{
    Action, BUTTON_A, Controller, Error, Fighter,
    data::{Attack, Bone, FighterData},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub selection: SelectionRules,
    pub l_cancel_window: i32,
    pub l_cancel_divisor: f32,
    /// Neutral, forward, back, up, down. No implicit fallback between actions.
    pub moves: [Move; 5],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Move {
    pub attack: Attack,
    /// One decoded command-state sample per attack pose, including recovery.
    pub flags: Vec<FrameFlags>,
    pub landing_lag: f32,
    pub landing_animation_end: f32,
    /// Integer animation-frame physics poses, covering the declared end frame.
    pub landing_poses: Vec<Vec<Bone>>,
    /// Motion-state entry blend metadata for the landing pose set.
    #[serde(default)]
    pub landing_poses_blend_frames: u8,
    #[serde(default)]
    pub landing_poses_dynamics_variant: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameFlags {
    pub landing_lag: bool,
    pub allow_interrupt: bool,
    /// A decoded throwB3 event, consumed once even when hitlag freezes this frame.
    pub reverse_facing: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct State {
    pub applied_frame: Option<u32>,
    pub landing_lag_enabled: bool,
    pub allow_interrupt: bool,
    pub landing_elapsed: f32,
    pub landing_rate: f32,
    /// `mv.co.fallspecial.mobility` is `ca->air_drift_max * mobility`
    /// (`ftCo_FallSpecial.c:55-59`); this field stores the multiplier, so
    /// the ordinary air-drift maximum is recovered unscaled by default.
    /// Every currently modeled `FallSpecial` entry but the Fox/Falco side
    /// special's own End phase passes the source's literal `mobility == 1`.
    pub mobility: f32,
    /// `mv.co.fallspecial.landing_lag`: `ftCo_80096900`/`ftCo_800969D8`'s own
    /// `landing_lag` argument, stored per `FallSpecial` instance and read
    /// back by its own eventual landing (`ftCo_FallSpecial_Coll` ->
    /// `ftCo_LandingFallSpecial_Enter(gobj, ..., fp->mv.co.fallspecial.
    /// landing_lag)`), exactly like `mobility` above -- not a single
    /// match-wide constant. `None` (the default, matching every entry that
    /// does not thread its own value through `helpers::enter_fall_special`)
    /// keeps this crate's prior behavior of reading `escape_air::Rules`'
    /// own `landing_lag` instead, at the landing site.
    pub landing_lag: Option<f32>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            applied_frame: None,
            landing_lag_enabled: false,
            allow_interrupt: false,
            landing_elapsed: 0.0,
            landing_rate: 0.0,
            mobility: 1.0,
            landing_lag: None,
        }
    }
}

const ATTACKS: [Action; 5] = [
    Action::AttackAirN,
    Action::AttackAirF,
    Action::AttackAirB,
    Action::AttackAirHi,
    Action::AttackAirLw,
];
const LANDINGS: [Action; 5] = [
    Action::LandingAirN,
    Action::LandingAirF,
    Action::LandingAirB,
    Action::LandingAirHi,
    Action::LandingAirLw,
];

pub(crate) fn attack_index(action: Action) -> Option<usize> {
    ATTACKS.iter().position(|&a| a == action)
}
pub(crate) fn landing_index(action: Action) -> Option<usize> {
    LANDINGS.iter().position(|&a| a == action)
}

pub(crate) fn interruptible(fighter: &Fighter) -> bool {
    attack_index(fighter.action).is_some() && fighter.aerial.allow_interrupt
}

fn commands(f: &mut Fighter, movement: &Move) {
    // AttackAir's entry calls ftAnim_8006EBA4 once after the motion change,
    // so the native action clock is one ahead of the zero-based resource
    // sample on the entry frame.  EscapeAir uses the same convention.
    let sample = f.action_frame.saturating_sub(1);
    if f.aerial.applied_frame == Some(sample) {
        return;
    }
    let flags = movement.flags[sample as usize];
    f.aerial.applied_frame = Some(sample);
    f.aerial.landing_lag_enabled = flags.landing_lag;
    f.aerial.allow_interrupt = flags.allow_interrupt;
    if flags.reverse_facing {
        f.facing = -f.facing;
    }
}

/// Install the destination action before any of its input callbacks run.
pub(crate) fn update_animation(f: &mut Fighter, data: &FighterData) {
    let Some(p) = &data.aerials else {
        return;
    };
    if let Some(index) = landing_index(f.action) {
        f.aerial.landing_elapsed += f.aerial.landing_rate;
        if f.aerial.landing_elapsed < p.moves[index].landing_animation_end {
            return;
        }
        crate::game::simulation::enter(f, Action::Wait);
        return;
    }
    if let Some(index) = attack_index(f.action) {
        let sample = f.action_frame.saturating_sub(1) as usize;
        if sample >= p.moves[index].attack.frames.len() {
            crate::game::simulation::enter(f, Action::Fall);
        } else {
            commands(f, &p.moves[index]);
        }
    }
}

/// Returns true while this callback owns action dispatch. Selection precedes
/// ordinary aerial jump input in the supported source interrupt branches.
pub(crate) fn update(f: &mut Fighter, data: &FighterData, input: Controller) -> bool {
    let Some(p) = &data.aerials else {
        return false;
    };
    if landing_index(f.action).is_some()
        || (attack_index(f.action).is_some() && !f.aerial.allow_interrupt)
    {
        return true;
    }
    let damage_air = crate::fighter::damage::damage_air_interruptible(f);
    let wall_tech = crate::fighter::damage::wall_tech_interruptible(f);
    if f.grounded
        || !(damage_air
            || wall_tech
            || matches!(
                f.action,
                Action::Jump | Action::JumpAerial | Action::Fall | Action::Pass
            )
            || attack_index(f.action).is_some())
    {
        return false;
    }
    if input.buttons & !f.previous_input.buttons & BUTTON_A != 0
        || fresh_cstick(
            f.previous_input.cstick,
            input.cstick,
            p.selection.thresholds,
        )
    {
        let direction = select(
            input.stick,
            input.cstick,
            f.previous_input.cstick,
            f.facing,
            &p.selection,
        );
        let index = match direction {
            Direction::Neutral => 0,
            Direction::Forward => 1,
            Direction::Back => 2,
            Direction::Up => 3,
            Direction::Down => 4,
        };
        match crate::game::script::move_selection::select_native_move(
            f,
            data,
            crate::game::script::move_registry::MoveGroup::Aerials,
            [
                crate::game::script::move_registry::MoveSlot::Neutral,
                crate::game::script::move_registry::MoveSlot::Forward,
                crate::game::script::move_registry::MoveSlot::Back,
                crate::game::script::move_registry::MoveSlot::Up,
                crate::game::script::move_registry::MoveSlot::Down,
            ][index]
                .clone(),
            ATTACKS[index],
        ) {
            crate::game::script::move_selection::NativeMoveSelection::Entered(action)
            | crate::game::script::move_selection::NativeMoveSelection::Unbound(action) => {
                commands(f, &p.moves[attack_index(action).unwrap_or(index)]);
            }
            crate::game::script::move_selection::NativeMoveSelection::CallbackDriven { .. } => {
                return true;
            }
        }
        return true;
    }
    if attack_index(f.action).is_some() || wall_tech || damage_air {
        crate::fighter::locomotion::try_aerial_jump(f, data, input);
        return true;
    }
    false
}

/// Called by the floor response before changing the aerial action or flags.
pub(crate) fn land(f: &mut Fighter, data: &FighterData) -> Result<bool, Error> {
    let Some(index) = attack_index(f.action) else {
        return Ok(false);
    };
    let p = data
        .aerials
        .as_ref()
        .ok_or_else(|| Error::Data("missing aerial resources".into()))?;
    if !f.aerial.landing_lag_enabled {
        return Ok(false);
    }
    let movement = &p.moves[index];
    let l_cancelled = i32::from(f.locomotion.trigger_age) < p.l_cancel_window;
    let lag = landing_lag(
        movement.landing_lag,
        f.locomotion.trigger_age,
        p.l_cancel_window,
        p.l_cancel_divisor,
    )
    .map_err(|e| Error::Physics(e.to_string()))?;
    crate::game::simulation::enter(f, LANDINGS[index]);
    f.l_cancel_status = if l_cancelled { 1 } else { 2 };
    f.aerial.landing_rate = landing_animation_rate(movement.landing_animation_end, lag);
    Ok(true)
}

pub(crate) fn landing_pose<'a>(f: &Fighter, data: &'a FighterData) -> Option<&'a Vec<Bone>> {
    let index = landing_index(f.action)?;
    data.aerials.as_ref()?.moves[index]
        .landing_poses
        .get(f.aerial.landing_elapsed as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undefined_lag_conversions_are_errors_only_on_the_cancel_path() {
        for (lag, divisor) in [
            (f32::NAN, 1.0),
            (f32::INFINITY, 1.0),
            (1.0, 0.0),
            (0.0, 0.0),
            (2_147_483_648.0, 1.0),
        ] {
            assert!(landing_lag(lag, 0, 3, divisor).is_err());
            let unchanged = landing_lag(lag, 3, 3, divisor).unwrap();
            if lag.is_nan() {
                assert!(unchanged.is_nan());
            } else {
                assert_eq!(unchanged.to_bits(), lag.to_bits());
            }
        }
        assert_eq!(
            landing_lag(-2_147_483_648.0, 0, 3, 1.0),
            Ok(-2_147_483_648.0)
        );
    }
}
