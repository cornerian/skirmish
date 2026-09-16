// Grab input arithmetic and bone-driven catch, capture, pummel, and throw
// scheduling from `ftCo_Throw.c` and its related common callbacks.

use crate::game::script::move_registry::{MoveGroup, MoveSlot};
use crate::game::{
    Action, Controller, Error, Event, Fighter, State as MatchState,
    data::{Bone, Capsule, FighterData, Hitbox, MatchData, PlayerSettings},
    simulation,
};
use crate::{
    collision::{bones::BoneCapsule, shield as body_collision},
    fighter::combat::{self, Capsule as WorldCapsule},
};
use serde::{Deserialize, Serialize};

const MASH_BUTTONS: u32 = 0x8000_0F00;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MashState {
    pub axes: [i8; 2],
    pub shake_enabled: bool,
    pub shaking: bool,
    pub shake_frame: u8,
    pub shake_frames: u8,
}

/// Complete `ftCommon_GrabMash` timer, axis-latch, and shake-state mutation.
pub fn mash(
    timer: &mut f32,
    state: &mut MashState,
    pressed_buttons: u32,
    stick: [f32; 2],
    penalty: f32,
    threshold: f32,
) -> bool {
    let mut result = false;
    if pressed_buttons & MASH_BUTTONS != 0 {
        *timer -= penalty;
        result = true;
    }
    let previous = state.axes;
    for (value, axis) in stick.into_iter().zip(&mut state.axes) {
        if value < -threshold {
            *axis = -1;
        }
        if value > threshold {
            *axis = 1;
        }
    }
    if state.axes != previous {
        *timer -= penalty;
        result = true;
    }
    if result && state.shake_enabled {
        state.shaking = true;
        state.shake_frame = state.shake_frame.wrapping_add(1);
        if state.shake_frame >= state.shake_frames {
            state.shake_frame = 0;
        }
    } else {
        state.shaking = false;
    }
    result
}

/// `fn_800DA4C0`: a freshly pressed A button requests CatchAttack.
pub const fn pummel_pressed(pressed_buttons: u16) -> bool {
    pressed_buttons & 0x100 != 0
}

/// Victim-weight branch retained from `ftCo_800DD4B0`. Multiplication precedes
/// division; an independent direction does not inspect either coefficient.
pub fn throw_animation_rate(independent: bool, victim_weight: f32, scale: f32) -> f32 {
    if independent {
        1.0
    } else {
        1.0 / (victim_weight * scale)
    }
}

/// Matrix lookups excluded, this is complete `fn_800DAD18`: compute the three
/// bone-alignment deltas, test its strict scaled-height branch, then translate
/// the captured fighter in the source operation order.
pub fn capture_alignment(
    mut position: [f32; 3],
    holder_anchor: [f32; 3],
    victim_anchor: [f32; 3],
    height_threshold: f32,
    model_scale_y: f32,
) -> ([f32; 3], bool) {
    let delta = [
        holder_anchor[0] - victim_anchor[0],
        holder_anchor[1] - victim_anchor[1],
        holder_anchor[2] - victim_anchor[2],
    ];
    let lifted = delta[1] > height_threshold * model_scale_y;
    position[0] += delta[0];
    position[1] += delta[1];
    position[2] += delta[2];
    (position, lifted)
}

/// `ftCo_800DA824`: the real grab-escape capture timer, driven by a live
/// match's standing and handicap rather than the flattened
/// `EscapeRules::timer_base` path (`docs/grab-escape-timer.md`). `base`,
/// `handicap_scale`, `handicap_max`, `rank_scale`, `rank_max` and
/// `percent_scale` are the six `ftCommonData` constants (`x354`, `x358`,
/// `x35C`, `x360`, `x364`, `x368`); keep this exact f32 evaluation order:
/// `slot = standing + 1`; `value = rank_max - slot`; `value = rank_scale *
/// value`; `temp = handicap_max - handicap`; `temp = handicap_scale * temp +
/// base`; `temp += value`; `return percent * percent_scale + temp`.
/// The two `* +`-shaped steps (`handicap_scale * temp + base` and the final
/// `percent * percent_scale + temp`) are each a single Gekko `fmadds`
/// (`tools/ppc_fma_audit.py ftCo_800DA824`; `docs/math.md`), so both use
/// `f32::mul_add` for their one rounding instead of two.
#[allow(clippy::too_many_arguments)]
pub fn escape_timer(
    base: f32,
    handicap_scale: f32,
    handicap_max: f32,
    rank_scale: f32,
    rank_max: f32,
    percent_scale: f32,
    percent: f32,
    standing: u8,
    handicap: u8,
) -> f32 {
    let slot = standing as f32 + 1.0;
    let value = rank_max - slot;
    let value = rank_scale * value;
    let handicap = handicap as f32;
    let temp = handicap_max - handicap;
    let temp = handicap_scale.mul_add(temp, base);
    let temp = temp + value;
    percent.mul_add(percent_scale, temp)
}

/// A main-stick horizontal threshold crossing has priority over vertical throws.
pub fn fresh_horizontal(current: f32, previous: f32, threshold: f32) -> bool {
    (previous < threshold && current >= threshold)
        || (previous > -threshold && current <= -threshold)
}

/// Up throw uses a positive threshold crossing.
pub fn fresh_up(current: f32, previous: f32, threshold: f32) -> bool {
    previous < threshold && current >= threshold
}

/// Down throw's common-data threshold is signed and normally negative.
pub fn fresh_down(current: f32, previous: f32, threshold: f32) -> bool {
    previous > threshold && current <= threshold
}

/// Complete `ftCo_Catch_CheckInput` input branch after its item and tether
/// gates: a fresh logical A press while the logical shoulder is held.
/// `Fighter_procInput` folds physical Z into both of those logical bits.
pub fn shield_grab(shoulder_held: bool, a_pressed: bool) -> bool {
    shoulder_held && a_pressed
}

/// Complete `ftCo_800D8B9C`: a fresh logical A press inside the guard `x24`
/// dash-grab buffer starts CatchDash; otherwise the buffer counts down.
pub fn dash_shield_grab(a_pressed: bool, buffer: &mut f32) -> bool {
    if a_pressed && *buffer != 0.0 {
        return true;
    }
    if *buffer != 0.0 {
        *buffer -= 1.0;
    }
    false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrowDirection {
    Forward,
    Backward,
    Up,
    Down,
}

/// `ftCo_800DD1E4` direction priority for main and C-stick crossings.
pub fn direction(
    current: [f32; 2],
    previous: [f32; 2],
    ccurrent: [f32; 2],
    cprevious: [f32; 2],
    facing: f32,
    thresholds: [f32; 3],
) -> Option<ThrowDirection> {
    let [horizontal, up, down] = thresholds;
    if fresh_horizontal(current[0], previous[0], horizontal) {
        Some(if current[0] * facing > 0.0 {
            ThrowDirection::Forward
        } else {
            ThrowDirection::Backward
        })
    } else if fresh_horizontal(ccurrent[0], cprevious[0], horizontal) {
        Some(if ccurrent[0] * facing > 0.0 {
            ThrowDirection::Forward
        } else {
            ThrowDirection::Backward
        })
    } else if fresh_up(current[1], previous[1], up) || fresh_up(ccurrent[1], cprevious[1], up) {
        Some(ThrowDirection::Up)
    } else if fresh_down(current[1], previous[1], down)
        || fresh_down(ccurrent[1], cprevious[1], down)
    {
        Some(ThrowDirection::Down)
    } else {
        None
    }
}

// Bone-driven catch, paired capture, pummel, and throw scheduling.
//
// Resources supply every pose, collision volume, attachment, timer, and hit
// coefficient. The relationship is headless physics state and is serialized
// with match observations and checkpoints.

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub horizontal_threshold: f32,
    pub up_threshold: f32,
    /// Signed negative common-data threshold.
    pub down_threshold: f32,
    /// Common x37C multiplier for victim-weight-dependent throw animation.
    pub throw_weight_scale: f32,
    /// Common x3C4 rise threshold, scaled by the victim's root bone Y scale.
    pub capture_lift_threshold: f32,
    pub escape: EscapeRules,
    /// Grabs dispatched from a raised shield; absent means unsupported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield_grab: Option<ShieldGrabRules>,
}

/// Common data for `ftCo_Catch_CheckInput` and `ftCo_800D8B9C` from guard
/// states.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShieldGrabRules {
    /// `x68`: guard `x24` frames armed by `ftCo_80091B9C` when Run, or Dash
    /// past `dash_buffer_frame_limit`, raises the shield.
    pub dash_buffer_frames: f32,
    /// `x4C`: Dash animation frame that must be exceeded to arm the buffer.
    pub dash_buffer_frame_limit: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscapeRules {
    pub timer_base: f32,
    pub timer_percent_scale: f32,
    pub timer_decrement: f32,
    pub mash_penalty: f32,
    pub stick_threshold: f32,
    pub release_speed: f32,
    /// The real `ftCo_800DA824` common-data constants. When present, the
    /// capture timer is computed from standing and handicap instead of the
    /// flattened `timer_base`/`timer_percent_scale` path above (which stays
    /// for existing fixtures that do not carry a real match's standings).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<EscapeFormula>,
}

/// `ftCo_800DA824`'s six `ftCommonData` inputs (`ft/types.h:266-271`), named
/// by the field they scale rather than by offset. Shared by the Leadead
/// (`ftCo_800C7590.c:44-52`, `ftCo_800C78B0.c:46-54`) and DamageBind
/// (`ftCo_DamageBind.c:40-46`) readers, not modeled yet
/// (`docs/grab-escape-timer.md`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscapeFormula {
    /// `x354`: constant base term.
    pub base: f32,
    /// `x358`: multiplies `handicap_max - handicap`.
    pub handicap_scale: f32,
    /// `x35C`: handicap this player would need to contribute nothing.
    pub handicap_max: f32,
    /// `x360`: multiplies `rank_max - (standing + 1)`.
    pub rank_scale: f32,
    /// `x364`: standing this player would need to contribute nothing.
    pub rank_max: f32,
    /// `x368`: multiplies `percent`.
    pub percent_scale: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub catch: Catch,
    pub catch_dash: Catch,
    pub attachment: Attachment,
    pub pummel: Pummel,
    /// Complete victim physics poses for the two ordinary pummel reactions.
    pub capture_damage: CaptureDamage,
    pub escape: Escape,
    pub throws: Throws,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureDamage {
    pub high: Vec<Vec<Bone>>,
    pub low: Vec<Vec<Bone>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catch {
    /// One complete physics pose and its active catch volumes per frame.
    pub frames: Vec<CatchFrame>,
    pub pull_frames: u32,
    pub grounded_targets_only: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatchFrame {
    pub bones: Vec<Bone>,
    pub grabboxes: Vec<Capsule>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    pub holder_bone: usize,
    pub holder_point: [f32; 3],
    pub victim_bone: usize,
    pub victim_point: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pummel {
    /// Native move-table identity. Sentinel 1 is exempt from stale damage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_id: Option<u16>,
    /// One complete holder physics pose per frame.
    pub poses: Vec<Vec<Bone>>,
    /// The single captured-victim damage callback. Zero is not observable in
    /// this scheduler because input dispatch follows priority-1 callbacks.
    pub hit_frame: u32,
    pub damage: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Escape {
    /// This fighter's holder-side CatchCut physics poses.
    pub catch_cut_poses: Vec<Vec<Bone>>,
    /// This fighter's victim-side CaptureCut physics poses.
    pub capture_cut_poses: Vec<Vec<Bone>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Throw {
    /// Native move-table identity. Sentinel 1 is exempt from stale damage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_id: Option<u16>,
    /// The fighter's per-direction weight-independent throw mask.
    #[serde(default)]
    pub weight_independent: bool,
    /// One complete holder physics pose per frame.
    pub poses: Vec<Vec<Bone>>,
    /// Scripted release event. Zero is excluded so entry is observable.
    pub release_frame: u32,
    pub hit: ThrowHit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Throws {
    pub forward: Throw,
    pub backward: Throw,
    pub up: Throw,
    pub down: Throw,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThrowHit {
    pub damage: u32,
    pub angle_degrees: f32,
    pub growth: u32,
    pub fixed: u32,
    pub base: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct State {
    pub victim: Option<usize>,
    pub captor: Option<usize>,
    pub pummel_hit: bool,
    pub escape_timer: f32,
    pub mash: self::MashState,
    /// Paired HSD animation time and rate while a throw still owns its victim.
    pub throw_elapsed: f32,
    pub throw_rate: f32,
}

pub(crate) fn validate(
    rules: &Rules,
    parameters: &Parameters,
    fighter: &FighterData,
    staling: bool,
) -> Result<(), Error> {
    if let Some(shield_grab) = &rules.shield_grab
        && (!shield_grab.dash_buffer_frames.is_finite()
            || shield_grab.dash_buffer_frames <= 0.0
            || shield_grab.dash_buffer_frames > 1_000_000.0
            || !shield_grab.dash_buffer_frame_limit.is_finite()
            || !(0.0..=1_000_000.0).contains(&shield_grab.dash_buffer_frame_limit))
    {
        return Err(Error::Data("invalid shield-grab rules".into()));
    }
    if ![
        rules.horizontal_threshold,
        rules.up_threshold,
        rules.down_threshold,
        rules.throw_weight_scale,
        rules.capture_lift_threshold,
        rules.escape.timer_base,
        rules.escape.timer_percent_scale,
        rules.escape.timer_decrement,
        rules.escape.mash_penalty,
        rules.escape.stick_threshold,
        rules.escape.release_speed,
    ]
    .into_iter()
    .all(f32::is_finite)
        || !(0.0..=1.0).contains(&rules.horizontal_threshold)
        || rules.horizontal_threshold == 0.0
        || !(0.0..=1.0).contains(&rules.up_threshold)
        || rules.up_threshold == 0.0
        || !(-1.0..0.0).contains(&rules.down_threshold)
        || !(0.0..1_000_000.0).contains(&rules.throw_weight_scale)
        || !(0.0..1_000_000.0).contains(&rules.capture_lift_threshold)
        || rules.capture_lift_threshold == 0.0
        || !(0.0..1_000_000.0).contains(&rules.escape.timer_base)
        || !(0.0..1_000.0).contains(&rules.escape.timer_percent_scale)
        || !(0.0..1_000_000.0).contains(&rules.escape.timer_decrement)
        || !(0.0..1_000_000.0).contains(&rules.escape.mash_penalty)
        || !(0.0..=1.0).contains(&rules.escape.stick_threshold)
        || rules.escape.stick_threshold == 0.0
        || !(0.0..=1_000_000.0).contains(&rules.escape.release_speed)
        || rules.escape.timer_base + 999.0 * rules.escape.timer_percent_scale >= 1_000_000.0
        || parameters.attachment.holder_bone >= fighter.bones.len()
        || parameters.attachment.victim_bone >= fighter.bones.len()
        || parameters
            .attachment
            .holder_point
            .into_iter()
            .chain(parameters.attachment.victim_point)
            .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
    {
        return Err(Error::Data("invalid explicit grab parameters".into()));
    }
    if let Some(formula) = &rules.escape.formula {
        let worst_handicap_term =
            formula.base + formula.handicap_scale.abs() * formula.handicap_max.abs();
        let worst_rank_term = formula.rank_scale.abs() * formula.rank_max.abs();
        let worst_percent_term = 999.0 * formula.percent_scale.abs();
        if ![
            formula.base,
            formula.handicap_scale,
            formula.handicap_max,
            formula.rank_scale,
            formula.rank_max,
            formula.percent_scale,
        ]
        .into_iter()
        .all(f32::is_finite)
            || !(0.0..1_000_000.0).contains(&formula.base)
            || !(0.0..1_000.0).contains(&formula.percent_scale)
            || worst_handicap_term.abs() + worst_rank_term.abs() + worst_percent_term.abs()
                >= 1_000_000.0
        {
            return Err(Error::Data("invalid grab-escape formula".into()));
        }
    }
    for catch in [&parameters.catch, &parameters.catch_dash] {
        validate_catch(catch, fighter)?;
    }
    let pummel = &parameters.pummel;
    if pummel.poses.is_empty()
        || pummel.poses.len() > 4096
        || pummel.hit_frame == 0
        || pummel.hit_frame as usize >= pummel.poses.len()
        || pummel.damage > 999
        || (staling && pummel.move_id.is_none_or(|id| id == 0))
    {
        return Err(Error::Data("invalid explicit pummel parameters".into()));
    }
    for pose in &pummel.poses {
        crate::game::validation::validate_animation_pose(pose, fighter)?;
    }
    for poses in [
        &parameters.capture_damage.high,
        &parameters.capture_damage.low,
    ] {
        if poses.is_empty() || poses.len() > 4096 {
            return Err(Error::Data(
                "invalid explicit capture-damage animation".into(),
            ));
        }
        for pose in poses {
            crate::game::validation::validate_animation_pose(pose, fighter)?;
        }
    }
    for poses in [
        &parameters.escape.catch_cut_poses,
        &parameters.escape.capture_cut_poses,
    ] {
        if poses.is_empty() || poses.len() > 4096 {
            return Err(Error::Data("invalid explicit grab-escape animation".into()));
        }
        for pose in poses {
            crate::game::validation::validate_animation_pose(pose, fighter)?;
        }
    }
    for throw in [
        &parameters.throws.forward,
        &parameters.throws.backward,
        &parameters.throws.up,
        &parameters.throws.down,
    ] {
        if throw.poses.is_empty()
            || throw.poses.len() > 4096
            || throw.release_frame == 0
            || throw.release_frame as usize >= throw.poses.len()
            || throw.hit.damage > 999
            || throw.hit.growth > 1000
            || throw.hit.fixed > 1000
            || throw.hit.base > 1000
            || !(0.0..=361.0).contains(&throw.hit.angle_degrees)
            || throw.hit.angle_degrees.fract() != 0.0
            || (staling && throw.move_id.is_none_or(|id| id == 0))
        {
            return Err(Error::Data("invalid explicit throw parameters".into()));
        }
        for pose in &throw.poses {
            crate::game::validation::validate_animation_pose(pose, fighter)?;
        }
    }
    Ok(())
}

fn validate_catch(catch: &Catch, fighter: &FighterData) -> Result<(), Error> {
    if catch.frames.is_empty()
        || catch.frames.len() > 4096
        || catch.pull_frames == 0
        || catch.pull_frames >= 1_000_000
    {
        return Err(Error::Data("invalid explicit catch parameters".into()));
    }
    let mut active = false;
    for frame in &catch.frames {
        let pose = crate::game::validation::validate_animation_pose(&frame.bones, fighter)?;
        if frame.grabboxes.len() > 4 {
            return Err(Error::Data("at most four grabboxes per frame".into()));
        }
        active |= !frame.grabboxes.is_empty();
        for grabbox in &frame.grabboxes {
            if grabbox.bone >= frame.bones.len()
                || grabbox
                    .start
                    .into_iter()
                    .chain(grabbox.end)
                    .chain([grabbox.radius])
                    .any(|value| !value.is_finite() || !(0.0..=1_000_000.0).contains(&value.abs()))
                || grabbox.radius < 0.0
            {
                return Err(Error::Data("invalid grabbox".into()));
            }
            grabbox
                .physics()
                .transform(&pose, 1.0)
                .map_err(|error| Error::Data(error.to_string()))?;
        }
    }
    if !active {
        return Err(Error::Data("catch requires an active grabbox frame".into()));
    }
    Ok(())
}

pub(crate) fn valid_relationship(fighters: &[Fighter; 2], player: usize) -> bool {
    let other = 1 - player;
    let fighter = &fighters[player];
    let partner = &fighters[other];
    if !fighter.grab.escape_timer.is_finite()
        || fighter
            .grab
            .mash
            .axes
            .into_iter()
            .any(|axis| !(-1..=1).contains(&axis))
        || !fighter.grab.throw_elapsed.is_finite()
        || !fighter.grab.throw_rate.is_finite()
        || fighter.grab.throw_elapsed < 0.0
        || fighter.grab.throw_rate < 0.0
        || fighter.grab.pummel_hit
            && (fighter.grab.victim.is_none() || fighter.action != Action::CatchAttack)
    {
        return false;
    }
    match (fighter.grab.victim, fighter.grab.captor) {
        (None, None) => fighter.grab == State::default(),
        (Some(victim), None) => {
            victim == other
                && fighter.grab.escape_timer == 0.0
                && fighter.grab.mash == self::MashState::default()
                && partner.grab.victim.is_none()
                && partner.grab.captor == Some(player)
                && !partner.grab.pummel_hit
                && valid_throw_clock(fighter, partner)
                && pair_actions(fighter.action, partner.action)
        }
        (None, Some(holder)) => {
            holder == other
                && partner.grab.victim == Some(player)
                && partner.grab.captor.is_none()
                && valid_throw_clock(partner, fighter)
                && pair_actions(partner.action, fighter.action)
        }
        (Some(_), Some(_)) => false,
    }
}

fn valid_throw_clock(holder: &Fighter, victim: &Fighter) -> bool {
    if matches!(
        holder.action,
        Action::ThrowF | Action::ThrowB | Action::ThrowHi | Action::ThrowLw
    ) {
        holder.grab.throw_rate > 0.0
            && holder.grab.throw_rate.to_bits() == victim.grab.throw_rate.to_bits()
            && holder.grab.throw_elapsed.to_bits() == victim.grab.throw_elapsed.to_bits()
    } else {
        holder.grab.throw_rate == 0.0
            && holder.grab.throw_elapsed == 0.0
            && victim.grab.throw_rate == 0.0
            && victim.grab.throw_elapsed == 0.0
    }
}

fn pair_actions(holder: Action, victim: Action) -> bool {
    matches!(
        (holder, victim),
        (
            Action::CatchPull | Action::CatchDashPull,
            Action::CapturePulledHi | Action::CapturePulledLw
        ) | (
            Action::CatchWait | Action::CatchAttack,
            Action::CaptureWaitHi | Action::CaptureWaitLw
        ) | (
            Action::CatchWait | Action::CatchAttack,
            Action::CaptureDamageHi | Action::CaptureDamageLw
        ) | (Action::ThrowF, Action::ThrownF)
            | (Action::ThrowB, Action::ThrownB)
            | (Action::ThrowHi, Action::ThrownHi)
            | (Action::ThrowLw, Action::ThrownLw)
    )
}

fn capture_pulled_action(grounded: bool) -> Action {
    if grounded {
        Action::CapturePulledLw
    } else {
        Action::CapturePulledHi
    }
}

/// The `ftCo_800DA824` capture timer: the source's exact f32 evaluation
/// order (`slot = standing + 1`; `value = rank_max - slot`; `value =
/// rank_scale * value`; `temp = handicap_max - handicap`; `temp =
/// handicap_scale * temp + base`; `temp += value`; `return percent *
/// percent_scale + temp`) when `escape.formula` is present, otherwise the
/// legacy flattened `timer_base + percent * timer_percent_scale` path kept
/// for fixtures that predate the real formula.
fn capture_timer(data: &MatchData, state: &MatchState, victim: usize, escape: &EscapeRules) -> f32 {
    let percent = state.fighters[victim].percent;
    match &escape.formula {
        Some(formula) => {
            let stocks = [state.fighters[0].stocks, state.fighters[1].stocks];
            self::escape_timer(
                formula.base,
                formula.handicap_scale,
                formula.handicap_max,
                formula.rank_scale,
                formula.rank_max,
                formula.percent_scale,
                percent,
                standing(stocks, state.next_frame, victim),
                handicap(data.players.as_ref(), victim),
            )
        }
        None => escape.timer_base + percent * escape.timer_percent_scale,
    }
}

/// Live per-frame standing (`gm_8016C5C0`'s `x58[slot].x5`, refreshed by
/// `gm_80166378` and ranked by `fn_80165AC0`): the count of opponents whose
/// `fn_8016588C` score is *strictly* greater than `player`'s own, so 0 is
/// best and ties (equal score) share the same standing, since the ranking
/// loop only increments on a strictly-greater comparison. Two-player only
/// for now (`docs/grab-escape-timer.md`), but iterates `stocks` so a third
/// slot only needs the array size (and the loop bound below) to change.
///
/// Skirmish only models Stock-kind matches (`MatchData.rules.stocks`), so
/// this reproduces `fn_8016588C`'s `MatchKind_Stock` branch only: score is
/// the fighter's remaining stocks, or (once eliminated) a deeply negative
/// sentinel offset by survival time so a longer-lived loser still ranks
/// above an earlier one. `Player_GetFalls`/KOs feed the *other* match kinds
/// (Time, Coin, Bonus) and a match's own end-of-match `score` field, not the
/// Stock branch used here; contrary to a first reading of the source
/// comments, `percent` does not enter this ranking at all for stock
/// matches — only `stocks`, this simulator's match-ending condition, so the
/// elimination branch is unreachable from a live grab (noted in
/// `docs/grab-escape-timer.md`).
pub(crate) fn standing(stocks: [u8; 2], frame: u32, player: usize) -> u8 {
    let score = |index: usize| -> i32 {
        let stocks = stocks[index];
        if stocks != 0 {
            i32::from(stocks)
        } else {
            // `fn_8016588C`'s elimination fallback: `frame_count / 60 +
            // 0xFF000001` (sign-extended to -16_777_215), clamped by
            // `fn_8016588C_clamp` to +/-(2^24 - 1).
            const SENTINEL: i64 = -16_777_215;
            const LIMIT: i64 = (1 << 24) - 1;
            let v = SENTINEL + i64::from(frame) / 60;
            v.clamp(-LIMIT, LIMIT) as i32
        }
    };
    let own = score(player);
    stocks
        .iter()
        .enumerate()
        .filter(|&(index, _)| index != player && score(index) > own)
        .count() as u8
}

/// `Player_GetHandicap` (`pl/player.c:855-870`) via
/// `MatchData.players`; the handicap rule is off whenever `players` is
/// absent, which the source pins to 9 for every slot (`mn/mncharsel.c:4290`,
/// `gm/gm_1601.c:3502`, `gm/gmmain_lib.c:833`).
pub(crate) fn handicap(players: Option<&[PlayerSettings; 2]>, player: usize) -> u8 {
    players.map_or(9, |players| players[player].handicap)
}

fn capture_wait_action(action: Action) -> Option<Action> {
    match action {
        Action::CapturePulledHi | Action::CaptureWaitHi | Action::CaptureDamageHi => {
            Some(Action::CaptureWaitHi)
        }
        Action::CapturePulledLw | Action::CaptureWaitLw | Action::CaptureDamageLw => {
            Some(Action::CaptureWaitLw)
        }
        _ => None,
    }
}

fn capture_damage_action(action: Action) -> Option<Action> {
    match action {
        Action::CapturePulledHi | Action::CaptureWaitHi | Action::CaptureDamageHi => {
            Some(Action::CaptureDamageHi)
        }
        Action::CapturePulledLw | Action::CaptureWaitLw | Action::CaptureDamageLw => {
            Some(Action::CaptureDamageLw)
        }
        _ => None,
    }
}

fn capture_damage_poses(parameters: &Parameters, action: Action) -> Option<&[Vec<Bone>]> {
    match action {
        Action::CaptureDamageHi => Some(&parameters.capture_damage.high),
        Action::CaptureDamageLw => Some(&parameters.capture_damage.low),
        _ => None,
    }
}

fn capture_waiting(action: Action) -> bool {
    matches!(action, Action::CaptureWaitHi | Action::CaptureWaitLw)
}

fn captured(action: Action) -> bool {
    matches!(
        action,
        Action::CapturePulledHi
            | Action::CaptureWaitHi
            | Action::CaptureDamageHi
            | Action::CapturePulledLw
            | Action::CaptureWaitLw
            | Action::CaptureDamageLw
    )
}

pub(crate) fn transfer_capture_family(fighter: &mut Fighter, airborne: bool) -> bool {
    let action = match (fighter.action, airborne) {
        (Action::CapturePulledLw, true) => Action::CapturePulledHi,
        (Action::CaptureWaitLw, true) => Action::CaptureWaitHi,
        (Action::CaptureDamageLw, true) => Action::CaptureDamageHi,
        (Action::CapturePulledHi, false) => Action::CapturePulledLw,
        (Action::CaptureWaitHi, false) => Action::CaptureWaitLw,
        (Action::CaptureDamageHi, false) => Action::CaptureDamageLw,
        _ => return false,
    };
    let frame = fighter.action_frame;
    simulation::enter(fighter, action);
    fighter.action_frame = frame;
    if airborne {
        fighter.grounded = false;
        fighter.ground_line = None;
        fighter.ground_knockback = 0.0;
        fighter.ground_velocity = 0.0;
        fighter.fast_fall = false;
        fighter.locomotion.jumps_used = fighter.locomotion.jumps_used.max(1);
    }
    true
}

pub(crate) fn owns_action(action: Action) -> bool {
    matches!(
        action,
        Action::Catch
            | Action::CatchDash
            | Action::CatchPull
            | Action::CatchDashPull
            | Action::CatchWait
            | Action::CatchAttack
            | Action::CatchCut
            | Action::ThrowF
            | Action::ThrowB
            | Action::ThrowHi
            | Action::ThrowLw
            | Action::CapturePulledHi
            | Action::CaptureWaitHi
            | Action::CaptureDamageHi
            | Action::CapturePulledLw
            | Action::CaptureWaitLw
            | Action::CaptureDamageLw
            | Action::CaptureCut
            | Action::ThrownF
            | Action::ThrownB
            | Action::ThrownHi
            | Action::ThrownLw
    )
}

pub(crate) fn holder_action(action: Action) -> bool {
    matches!(
        action,
        Action::CatchPull
            | Action::CatchDashPull
            | Action::CatchWait
            | Action::CatchAttack
            | Action::ThrowF
            | Action::ThrowB
            | Action::ThrowHi
            | Action::ThrowLw
    )
}

pub(crate) fn update_fighter_animation(fighter: &mut Fighter, data: &FighterData) -> bool {
    let Some(parameters) = &data.grab else {
        return false;
    };
    if fighter.action == Action::CatchAttack
        && fighter.action_frame as usize >= parameters.pummel.poses.len()
    {
        fighter.grab.pummel_hit = false;
        simulation::enter(fighter, Action::CatchWait);
        return true;
    }
    if let Some(poses) = capture_damage_poses(parameters, fighter.action)
        && fighter.action_frame as usize >= poses.len()
    {
        simulation::enter(fighter, capture_wait_action(fighter.action).unwrap());
        return true;
    }
    let cut_complete = match fighter.action {
        Action::CatchCut => {
            fighter.action_frame as usize >= parameters.escape.catch_cut_poses.len()
        }
        Action::CaptureCut => {
            fighter.action_frame as usize >= parameters.escape.capture_cut_poses.len()
        }
        _ => false,
    };
    if cut_complete {
        simulation::enter(
            fighter,
            if fighter.grounded {
                Action::Wait
            } else {
                Action::Fall
            },
        );
        return true;
    }
    let complete = match fighter.action {
        Action::Catch | Action::CatchDash => catch_for_action(parameters, fighter.action)
            .is_some_and(|catch| fighter.action_frame as usize >= catch.frames.len()),
        action
            if throw_for_action(&parameters.throws, action)
                .is_some_and(|throw| fighter.action_frame as usize >= throw.poses.len()) =>
        {
            true
        }
        _ => false,
    };
    if complete {
        simulation::enter(
            fighter,
            if fighter.grounded {
                Action::Wait
            } else {
                Action::Fall
            },
        );
    }
    complete
}

fn enter_registered(
    fighter: &mut Fighter,
    data: &FighterData,
    group: MoveGroup,
    slot: MoveSlot,
    native: Action,
) -> bool {
    matches!(
        crate::game::script::move_selection::select_native_move(fighter, data, group, slot, native,),
        crate::game::script::move_selection::NativeMoveSelection::Entered(_)
            | crate::game::script::move_selection::NativeMoveSelection::Unbound(_)
            | crate::game::script::move_selection::NativeMoveSelection::CallbackDriven { .. }
    )
}

pub(crate) fn update_actions(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    controller: Controller,
) -> bool {
    let pressed = controller.buttons & !fighter.previous_input.buttons;
    if owns_action(fighter.action) {
        if fighter.action == Action::CatchWait {
            if self::pummel_pressed(pressed) {
                fighter.grab.pummel_hit = false;
                let _ = enter_registered(
                    fighter,
                    data,
                    MoveGroup::Grabs,
                    MoveSlot::Pummel,
                    Action::CatchAttack,
                );
                return true;
            }
            let Some(rules) = rules else { return true };
            let action = self::direction(
                controller.stick,
                fighter.previous_input.stick,
                controller.cstick,
                fighter.previous_input.cstick,
                fighter.facing,
                [
                    rules.horizontal_threshold,
                    rules.up_threshold,
                    rules.down_threshold,
                ],
            )
            .map(|direction| match direction {
                self::ThrowDirection::Forward => Action::ThrowF,
                self::ThrowDirection::Backward => Action::ThrowB,
                self::ThrowDirection::Up => Action::ThrowHi,
                self::ThrowDirection::Down => Action::ThrowLw,
            });
            if let Some(action) = action {
                let slot = match action {
                    Action::ThrowF => MoveSlot::Forward,
                    Action::ThrowB => MoveSlot::Back,
                    Action::ThrowHi => MoveSlot::Up,
                    Action::ThrowLw => MoveSlot::Down,
                    _ => unreachable!(),
                };
                let _ = enter_registered(fighter, data, MoveGroup::Throws, slot, action);
            }
        }
        return true;
    }
    // ftCo_Catch_CheckInput and ftCo_800D8A38 need a fresh logical A press
    // with the logical shoulder held; Fighter_procInput folds physical Z into
    // both bits.
    let a_pressed = logical_a(controller.buttons) && !logical_a(fighter.previous_input.buttons);
    let shoulder_held = controller.shield_held() || controller.buttons & crate::game::BUTTON_Z != 0;
    if fighter.grounded && a_pressed && shoulder_held && data.grab.is_some() && rules.is_some() {
        let action = match fighter.action {
            Action::Dash | Action::Run => Some(Action::CatchDash),
            Action::Turn => {
                // Turn_IASA temporarily applies facing_after before checking Catch.
                if !fighter.locomotion.turn_has_turned {
                    fighter.facing = -fighter.facing;
                }
                Some(Action::Catch)
            }
            // ftCo_Catch_CheckInput runs from Wait, Walk, Squat and KneeBend;
            // SquatWait and SquatRv chains never reach it.
            Action::Wait | Action::Walk | Action::Squat | Action::JumpSquat => Some(Action::Catch),
            // ftCo_AppealS_IASA reaches ftCo_Catch_CheckInput too.
            _ if matches!(
                crate::fighter::tilt::interrupt_chain(fighter, data),
                Some(crate::fighter::tilt::Chain::Wait) | Some(crate::fighter::tilt::Chain::Taunt)
            ) =>
            {
                Some(Action::Catch)
            }
            _ => None,
        };
        if let Some(action) = action {
            let (group, slot) = match action {
                Action::CatchDash => (MoveGroup::Grabs, MoveSlot::Dash),
                Action::Catch => (MoveGroup::Grabs, MoveSlot::Standing),
                _ => unreachable!(),
            };
            let _ = enter_registered(fighter, data, group, slot, action);
            return true;
        }
    }
    false
}

/// `ftCo_80091B9C` arms the guard `x24` buffer when Run, or Dash after the
/// `x4C` frame, raises a shield; `ftCo_800923B4` clears it for other entries.
/// Any stale union residue on a powershield raised elsewhere is not modeled.
pub(crate) fn shield_entry_buffer(fighter: &Fighter, rules: Option<&Rules>) -> f32 {
    let Some(shield_grab) = rules.and_then(|rules| rules.shield_grab.as_ref()) else {
        return 0.0;
    };
    match fighter.action {
        Action::Run => shield_grab.dash_buffer_frames,
        Action::Dash if fighter.action_frame as f32 > shield_grab.dash_buffer_frame_limit => {
            shield_grab.dash_buffer_frames
        }
        _ => 0.0,
    }
}

fn logical_a(buttons: u16) -> bool {
    buttons & (crate::game::BUTTON_A | crate::game::BUTTON_Z) != 0
}

/// `ftCo_800D8B9C` (GuardOn/GuardReflect only) followed by
/// `ftCo_Catch_CheckInput`, at their guard IASA positions after the escapes.
pub(crate) fn update_shield_actions(
    fighter: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    controller: Controller,
) -> bool {
    if data.grab.is_none() || rules.is_none_or(|rules| rules.shield_grab.is_none()) {
        return false;
    }
    // Fighter_procInput folds physical Z into logical A plus a held shoulder.
    let a_pressed = logical_a(controller.buttons) && !logical_a(fighter.previous_input.buttons);
    let shoulder_held = controller.shield_held() || controller.buttons & crate::game::BUTTON_Z != 0;
    if matches!(fighter.action, Action::GuardOn | Action::GuardReflect)
        && self::dash_shield_grab(a_pressed, &mut fighter.shield.dash_grab_buffer)
    {
        start_shield_catch(fighter, data, Action::CatchDash);
        return true;
    }
    if self::shield_grab(shoulder_held, a_pressed) {
        start_shield_catch(fighter, data, Action::Catch);
        return true;
    }
    false
}

/// `ftCo_800D8C54` through an ordinary Fighter_ChangeMotionState.
fn start_shield_catch(fighter: &mut Fighter, data: &FighterData, action: Action) {
    crate::fighter::shield::leave_guard(fighter);
    let slot = if action == Action::CatchDash {
        MoveSlot::Dash
    } else {
        MoveSlot::Standing
    };
    let _ = enter_registered(fighter, data, MoveGroup::Grabs, slot, action);
}

/// Paired priority-1 transitions and the scripted release event.
pub(crate) fn update_pairs(
    data: &MatchData,
    state: &mut MatchState,
    controllers: [Controller; 2],
    active: [bool; 2],
) -> Result<[bool; 2], Error> {
    let mut frozen = [false; 2];
    for holder in 0..2 {
        let Some(victim) = state.fighters[holder].grab.victim else {
            continue;
        };
        if victim != 1 - holder {
            return Err(Error::Physics("invalid capture relationship".into()));
        }
        let parameters = data.fighters[holder]
            .grab
            .as_ref()
            .ok_or_else(|| Error::Data("capture state requires grab resources".into()))?;
        if active[victim]
            && matches!(
                state.fighters[victim].action,
                Action::CaptureWaitHi
                    | Action::CaptureDamageHi
                    | Action::CaptureWaitLw
                    | Action::CaptureDamageLw
            )
        {
            update_escape(data, state, victim, controllers[victim]);
            if capture_waiting(state.fighters[victim].action)
                && state.fighters[victim].grab.escape_timer <= 0.0
            {
                escape_pair(data, state, holder, victim);
                continue;
            }
        }
        if !active[holder] {
            continue;
        }
        let holder_action = state.fighters[holder].action;
        match holder_action {
            Action::CatchPull
                if state.fighters[holder].action_frame >= parameters.catch.pull_frames =>
            {
                simulation::enter(&mut state.fighters[holder], Action::CatchWait);
                let wait = capture_wait_action(state.fighters[victim].action)
                    .ok_or_else(|| Error::Physics("invalid captured-victim action".into()))?;
                simulation::enter(&mut state.fighters[victim], wait);
            }
            Action::CatchDashPull
                if state.fighters[holder].action_frame >= parameters.catch_dash.pull_frames =>
            {
                simulation::enter(&mut state.fighters[holder], Action::CatchWait);
                let wait = capture_wait_action(state.fighters[victim].action)
                    .ok_or_else(|| Error::Physics("invalid captured-victim action".into()))?;
                simulation::enter(&mut state.fighters[victim], wait);
            }
            Action::CatchAttack
                if state.fighters[holder].action_frame == parameters.pummel.hit_frame
                    && !state.fighters[holder].grab.pummel_hit =>
            {
                state.fighters[holder].grab.pummel_hit = true;
                apply_pummel(data, state, holder, victim, parameters.pummel.damage)?;
                frozen[holder] = true;
                frozen[victim] = true;
            }
            action
                if throw_for_action(&parameters.throws, action).is_some_and(|throw| {
                    state.fighters[holder].action_frame == throw.release_frame
                }) =>
            {
                let throw = throw_for_action(&parameters.throws, action).unwrap();
                let hit = throw.hit;
                let staled = crate::game::staling::hit(
                    &state.fighters[holder].staling,
                    hit.damage,
                    data.rules.staling.as_ref(),
                )?;
                detach(state, holder, victim);
                let accepted = crate::game::hit_resolution::apply_hit(
                    data,
                    state,
                    holder,
                    &Hitbox {
                        clank: false,
                        rebound: false,
                        element: Default::default(),
                        group: 0,
                        bone: 0,
                        center: [0.0; 3],
                        radius: 0.0,
                        damage: hit.damage,
                        shield_damage: 0,
                        angle_degrees: hit.angle_degrees,
                        growth: hit.growth,
                        fixed: hit.fixed,
                        base: hit.base,
                    },
                    staled,
                    crate::fighter::damage::HurtHeight::Middle,
                    crate::game::hit_resolution::HitDirection::Throw,
                    false,
                )?;
                if accepted && data.rules.staling.is_some() {
                    state.fighters[holder]
                        .staling
                        .queue
                        .record(staled.identity, false);
                }
                frozen[holder] = true;
                frozen[victim] = true;
            }
            _ => {}
        }
    }
    Ok(frozen)
}

pub(crate) fn synchronize_actions(data: &MatchData, state: &mut MatchState) -> Result<(), Error> {
    for holder in 0..2 {
        let Some(victim) = state.fighters[holder].grab.victim else {
            continue;
        };
        let holder_action = state.fighters[holder].action;
        let victim_action = match holder_action {
            Action::ThrowF => Action::ThrownF,
            Action::ThrowB => Action::ThrownB,
            Action::ThrowHi => Action::ThrownHi,
            Action::ThrowLw => Action::ThrownLw,
            _ => continue,
        };
        if state.fighters[holder].grab.throw_rate == 0.0 {
            let throw = data.fighters[holder]
                .grab
                .as_ref()
                .and_then(|parameters| throw_for_action(&parameters.throws, holder_action))
                .ok_or_else(|| Error::Data("throw state requires grab resources".into()))?;
            let rate = self::throw_animation_rate(
                throw.weight_independent,
                data.fighters[victim].weight,
                data.rules.grab.as_ref().unwrap().throw_weight_scale,
            );
            if !rate.is_finite() || rate <= 0.0 {
                return Err(Error::Data("invalid weight-dependent throw rate".into()));
            }
            state.fighters[holder].grab.throw_rate = rate;
            state.fighters[victim].grab.throw_rate = rate;
        }
        if state.fighters[victim].action != victim_action {
            simulation::enter(&mut state.fighters[victim], victim_action);
        }
    }
    Ok(())
}

/// Advances paired HSD time. Before release, a fast rate stops at the scripted
/// event frame; the release callback restores ordinary rate-one advancement.
pub(crate) fn paired_throw_release(
    data: &MatchData,
    state: &MatchState,
    player: usize,
) -> Option<u32> {
    let holder = state.fighters[player].grab.captor.unwrap_or(player);
    state.fighters[holder].grab.victim?;
    let parameters = data.fighters[holder].grab.as_ref()?;
    throw_for_action(&parameters.throws, state.fighters[holder].action)
        .map(|throw_| throw_.release_frame)
}

pub(crate) fn advance_action_frame(fighter: &mut Fighter, release_frame: Option<u32>) -> bool {
    let Some(release_frame) = release_frame else {
        return false;
    };
    if fighter.grab.throw_rate == 0.0 {
        return false;
    }
    fighter.grab.throw_elapsed += fighter.grab.throw_rate;
    fighter.action_frame = (fighter.grab.throw_elapsed as u32).min(release_frame);
    true
}

pub(crate) fn release_broken_pairs(state: &mut MatchState) {
    for holder in 0..2 {
        let Some(victim) = state.fighters[holder].grab.victim else {
            continue;
        };
        if !holder_action(state.fighters[holder].action) {
            detach(state, holder, victim);
            if captured(state.fighters[victim].action)
                || matches!(
                    state.fighters[victim].action,
                    Action::ThrownF | Action::ThrownB | Action::ThrownHi | Action::ThrownLw
                )
            {
                let grounded = state.fighters[victim].grounded;
                simulation::enter(
                    &mut state.fighters[victim],
                    if grounded { Action::Wait } else { Action::Fall },
                );
            }
        }
    }
}

pub(crate) fn break_for_player(state: &mut MatchState, player: usize) {
    if let Some(victim) = state.fighters[player].grab.victim {
        detach(state, player, victim);
        let grounded = state.fighters[victim].grounded;
        simulation::enter(
            &mut state.fighters[victim],
            if grounded { Action::Wait } else { Action::Fall },
        );
    }
    if let Some(holder) = state.fighters[player].grab.captor {
        detach(state, holder, player);
        let grounded = state.fighters[holder].grounded;
        simulation::enter(
            &mut state.fighters[holder],
            if grounded { Action::Wait } else { Action::Fall },
        );
    }
}

pub(crate) fn attach_all(data: &MatchData, state: &mut MatchState) -> Result<(), Error> {
    for holder in 0..2 {
        if let Some(victim) = state.fighters[holder].grab.victim {
            attach(data, state, holder, victim, true)?;
        }
    }
    Ok(())
}

pub(crate) fn scan(
    data: &MatchData,
    state: &mut MatchState,
    frozen: [bool; 2],
) -> Result<(), Error> {
    for (holder, &holder_frozen) in frozen.iter().enumerate() {
        let victim = 1 - holder;
        let source = &state.fighters[holder];
        let source_action = source.action;
        let target = &state.fighters[victim];
        let Some(parameters) = &data.fighters[holder].grab else {
            continue;
        };
        let Some(catch) = catch_for_action(parameters, source_action) else {
            continue;
        };
        if holder_frozen
            || source.grab != State::default()
            || target.grab != State::default()
            || !source.special_capture.is_empty()
            || !target.special_capture.is_empty()
            || target.invincibility > 0
            || target.intangibility > 0
            || !target.body_state.accepts_contact()
            || matches!(
                target.action,
                Action::Respawn | Action::Eliminated | Action::Rebirth | Action::RebirthWait
            )
            || catch.grounded_targets_only && !target.grounded
        {
            continue;
        }
        let source_pose = simulation::pose(source, &data.fighters[holder])?;
        let target_pose = simulation::pose(target, &data.fighters[victim])?;
        let frame = catch
            .frames
            .get(source.action_frame as usize)
            .ok_or_else(|| Error::Physics("catch frame is outside supplied animation".into()))?;
        let mut collided = false;
        for grabbox in &frame.grabboxes {
            let grab = grabbox
                .physics()
                .transform(&source_pose, 1.0)
                .map_err(physics)?;
            let grab = WorldCapsule {
                start: grab.start,
                end: grab.end,
                radius: grab.radius,
            };
            for (index, hurtbox) in data.fighters[victim].hurtboxes.iter().enumerate() {
                if !hurtbox.grabbable
                    || !simulation::hurtbox_state(target, &data.fighters[victim], index)?
                        .accepts_contact()
                {
                    continue;
                }
                let hurt = hurtbox
                    .physics()
                    .transform(&target_pose, 1.0)
                    .map_err(physics)?;
                let hurt = WorldCapsule {
                    start: hurt.start,
                    end: hurt.end,
                    radius: hurt.radius,
                };
                let matrix = target_pose.world_matrix(hurtbox.bone).map_err(physics)?;
                let mut contact = body_collision::Contact::default();
                if body_collision::capsule_matrix(&grab, &hurt, matrix, 3.0, &mut contact)
                    .map_err(physics)?
                {
                    collided = true;
                    break;
                }
            }
            if collided {
                break;
            }
        }
        if collided {
            let escape = &data.rules.grab.as_ref().unwrap().escape;
            state.fighters[holder].grab.victim = Some(victim);
            state.fighters[victim].grab.captor = Some(holder);
            state.fighters[victim].grab.escape_timer = capture_timer(data, state, victim, escape);
            state.fighters[victim].grab.mash = self::MashState::default();
            state.fighters[victim].facing = state.fighters[holder].facing;
            state.fighters[victim].velocity = [0.0; 2];
            state.fighters[victim].knockback = [0.0; 2];
            state.fighters[victim].ground_knockback = 0.0;
            state.fighters[victim].ground_velocity = 0.0;
            let pull = if source_action == Action::CatchDash {
                Action::CatchDashPull
            } else {
                Action::CatchPull
            };
            let victim_action = capture_pulled_action(state.fighters[victim].grounded);
            simulation::enter(&mut state.fighters[holder], pull);
            simulation::enter(&mut state.fighters[victim], victim_action);
            state.events.push(Event::Grabbed { holder, victim });
            attach(data, state, holder, victim, true)?;
        }
    }
    Ok(())
}

pub(crate) fn pose<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a [Bone]> {
    let parameters = data.grab.as_ref()?;
    match fighter.action {
        Action::Catch | Action::CatchDash => catch_for_action(parameters, fighter.action)?
            .frames
            .get(fighter.action_frame as usize)
            .map(|frame| frame.bones.as_slice()),
        Action::CatchAttack => parameters
            .pummel
            .poses
            .get(fighter.action_frame as usize)
            .map(Vec::as_slice),
        Action::CaptureDamageHi => parameters
            .capture_damage
            .high
            .get(fighter.action_frame as usize)
            .map(Vec::as_slice),
        Action::CaptureDamageLw => parameters
            .capture_damage
            .low
            .get(fighter.action_frame as usize)
            .map(Vec::as_slice),
        Action::CatchCut => parameters
            .escape
            .catch_cut_poses
            .get(fighter.action_frame as usize)
            .map(Vec::as_slice),
        Action::CaptureCut => parameters
            .escape
            .capture_cut_poses
            .get(fighter.action_frame as usize)
            .map(Vec::as_slice),
        action if throw_for_action(&parameters.throws, action).is_some() => {
            throw_for_action(&parameters.throws, action)?
                .poses
                .get(fighter.action_frame as usize)
                .map(Vec::as_slice)
        }
        _ => None,
    }
}

fn catch_for_action(parameters: &Parameters, action: Action) -> Option<&Catch> {
    match action {
        Action::Catch => Some(&parameters.catch),
        Action::CatchDash => Some(&parameters.catch_dash),
        _ => None,
    }
}

fn attach(
    data: &MatchData,
    state: &mut MatchState,
    holder: usize,
    victim: usize,
    allow_lift: bool,
) -> Result<(), Error> {
    let attachment = data.fighters[holder]
        .grab
        .as_ref()
        .ok_or_else(|| Error::Data("capture state requires grab resources".into()))?
        .attachment;
    attach_points(data, state, holder, victim, attachment, allow_lift)
}

/// Apply a holder/victim bone attachment from any capture family.  Ordinary
/// grabs and source-specific captures share the transform arithmetic while
/// retaining ownership of their own relation and lifecycle state.
pub(crate) fn attach_points(
    data: &MatchData,
    state: &mut MatchState,
    holder: usize,
    victim: usize,
    attachment: Attachment,
    allow_lift: bool,
) -> Result<(), Error> {
    let holder_pose = simulation::pose(&state.fighters[holder], &data.fighters[holder])?;
    let holder_anchor = BoneCapsule::sphere(attachment.holder_bone, attachment.holder_point, 0.0)
        .transform(&holder_pose, 1.0)
        .map_err(physics)?
        .start;
    let victim_pose = simulation::pose(&state.fighters[victim], &data.fighters[victim])?;
    let victim_anchor = BoneCapsule::sphere(attachment.victim_bone, attachment.victim_point, 0.0)
        .transform(&victim_pose, 1.0)
        .map_err(physics)?
        .start;
    let position = [
        state.fighters[victim].position[0],
        state.fighters[victim].position[1],
        state.fighters[victim].depth,
    ];
    let capture_lift_threshold = if allow_lift {
        data.rules
            .grab
            .as_ref()
            .map_or(0.0, |rules| rules.capture_lift_threshold)
    } else {
        0.0
    };
    let (position, lifted) = self::capture_alignment(
        position,
        holder_anchor,
        victim_anchor,
        capture_lift_threshold,
        data.fighters[victim].bones[0].scale[1],
    );
    state.fighters[victim].position = [position[0], position[1]];
    state.fighters[victim].depth = position[2];
    if allow_lift && lifted && transfer_capture_family(&mut state.fighters[victim], true) {
        attach(data, state, holder, victim, false)?;
    }
    Ok(())
}

fn detach(state: &mut MatchState, holder: usize, victim: usize) {
    state.fighters[holder].grab = State::default();
    state.fighters[victim].grab = State::default();
}

fn update_escape(data: &MatchData, state: &mut MatchState, victim: usize, controller: Controller) {
    let rules = &data.rules.grab.as_ref().unwrap().escape;
    let target = &mut state.fighters[victim];
    target.grab.escape_timer -= rules.timer_decrement;
    let pressed = controller.buttons & !target.previous_input.buttons;
    let logical_pressed = u32::from(pressed)
        | if controller.shield_held() && !target.previous_input.shield_held() {
            1 << 31
        } else {
            0
        };
    self::mash(
        &mut target.grab.escape_timer,
        &mut target.grab.mash,
        logical_pressed,
        controller.stick,
        rules.mash_penalty,
        rules.stick_threshold,
    );
}

fn escape_pair(data: &MatchData, state: &mut MatchState, holder: usize, victim: usize) {
    let speed = data.rules.grab.as_ref().unwrap().escape.release_speed;
    let facing = state.fighters[holder].facing;
    detach(state, holder, victim);
    simulation::enter(&mut state.fighters[holder], Action::CatchCut);
    simulation::enter(&mut state.fighters[victim], Action::CaptureCut);
    release_velocity(&mut state.fighters[holder], -facing * speed);
    release_velocity(&mut state.fighters[victim], facing * speed);
    state.events.push(Event::GrabEscaped { holder, victim });
}

fn release_velocity(fighter: &mut Fighter, velocity: f32) {
    if fighter.grounded {
        fighter.ground_velocity = velocity;
    } else {
        fighter.velocity[0] = velocity;
    }
}

fn apply_pummel(
    data: &MatchData,
    state: &mut MatchState,
    holder: usize,
    victim: usize,
    damage: u32,
) -> Result<(), Error> {
    let staled = crate::game::staling::hit(
        &state.fighters[holder].staling,
        damage,
        data.rules.staling.as_ref(),
    )?;
    let hitlag = combat::hitlag(
        staled.damage as i32,
        false,
        1.0,
        &data.rules.hitlag.physics(),
    )
    .map_err(physics)?;
    if !hitlag.is_finite() || hitlag < 0.0 {
        return Err(Error::Physics("pummel produced invalid hitlag".into()));
    }
    state.fighters[holder].hitlag = state.fighters[holder].hitlag.max(hitlag);
    let target = &mut state.fighters[victim];
    let damage_action = capture_damage_action(target.action)
        .ok_or_else(|| Error::Physics("invalid captured-victim action".into()))?;
    target.percent = (target.percent + staled.damage).min(999.0);
    target.hitlag = target.hitlag.max(hitlag);
    target.di_pending = false;
    simulation::enter(target, damage_action);
    crate::game::combat_history::record_hit(
        state,
        holder,
        victim,
        staled.identity.move_id,
        &data.rules.damage.combo,
        true,
    );
    state.events.push(Event::Hit {
        attacker: holder,
        victim,
        damage: staled.damage,
        knockback: 0.0,
    });
    if data.rules.staling.is_some() {
        state.fighters[holder]
            .staling
            .queue
            .record(staled.identity, false);
    }
    Ok(())
}

pub(crate) fn move_id(
    parameters: Option<&Parameters>,
    action: Action,
) -> Result<Option<u16>, Error> {
    let move_id = match action {
        Action::CatchAttack => parameters.map(|parameters| parameters.pummel.move_id),
        Action::ThrowF => parameters.map(|parameters| parameters.throws.forward.move_id),
        Action::ThrowB => parameters.map(|parameters| parameters.throws.backward.move_id),
        Action::ThrowHi => parameters.map(|parameters| parameters.throws.up.move_id),
        Action::ThrowLw => parameters.map(|parameters| parameters.throws.down.move_id),
        _ => return Ok(None),
    };
    move_id
        .flatten()
        .map(Some)
        .ok_or_else(|| Error::Data("staling requires an explicit grab move_id".into()))
}

fn throw_for_action(throws: &Throws, action: Action) -> Option<&Throw> {
    match action {
        Action::ThrowF => Some(&throws.forward),
        Action::ThrowB => Some(&throws.backward),
        Action::ThrowHi => Some(&throws.up),
        Action::ThrowLw => Some(&throws.down),
        _ => None,
    }
}

fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shield_grab_needs_both_logical_inputs() {
        assert!(shield_grab(true, true));
        assert!(!shield_grab(false, true));
        assert!(!shield_grab(true, false));
    }

    #[test]
    fn dash_shield_grab_counts_down_only_while_armed() {
        let mut buffer = 2.0;
        assert!(!dash_shield_grab(false, &mut buffer));
        assert_eq!(buffer, 1.0);
        assert!(dash_shield_grab(true, &mut buffer));
        assert_eq!(buffer, 1.0);
        assert!(!dash_shield_grab(false, &mut buffer));
        assert_eq!(buffer, 0.0);
        assert!(!dash_shield_grab(true, &mut buffer));
        assert_eq!(buffer, 0.0);
        let mut negative = -0.5;
        assert!(dash_shield_grab(true, &mut negative));
        assert!(!dash_shield_grab(false, &mut negative));
        assert_eq!(negative, -1.5);
    }

    #[test]
    fn escape_timer_at_the_real_fox_constants_and_replay_settings_matches_the_flattened_value() {
        // Fox's real `ftCommonData` constants (`x354..x368`), extracted to
        // `/mnt/archive/datasets/melee/skirmish-gameplay/v2/rules.json`'s
        // `grab.escape_formula`: base 30.0, handicap_scale 8.0, handicap_max
        // 9.0, rank_scale 15.0, rank_max 4.0, percent_scale 1.6.
        let (base, handicap_scale, handicap_max, rank_scale, rank_max, percent_scale) =
            (30.0, 8.0, 9.0, 15.0, 4.0, 1.6);
        // The parity replay's settings: handicap rule off (9), best
        // standing (0). Derivation, in the source's own order: `slot =
        // standing + 1 = 1.0`; `value = rank_scale * (rank_max - slot) =
        // 15.0 * (4.0 - 1.0) = 45.0`; `temp = handicap_scale * (handicap_max
        // - handicap) + base = 8.0 * (9.0 - 9.0) + 30.0 = 30.0`; `temp +=
        // value = 75.0`; at percent 0, the timer is exactly `temp`.
        let at_zero_percent = escape_timer(
            base,
            handicap_scale,
            handicap_max,
            rank_scale,
            rank_max,
            percent_scale,
            0.0,
            0,
            9,
        );
        assert_eq!(at_zero_percent, 75.0);
        // This is exactly the flattened `timer_base` the same exporter
        // wrote for Fox (`docs/grab-escape-timer.md`), and `percent_scale`
        // is exactly the flattened `timer_percent_scale`, so the two paths
        // agree at these settings for every percent, not only zero.
        let at_forty_percent = escape_timer(
            base,
            handicap_scale,
            handicap_max,
            rank_scale,
            rank_max,
            percent_scale,
            40.0,
            0,
            9,
        );
        assert_eq!(at_forty_percent, 75.0 + 40.0 * 1.6);
    }

    /// `ftCo_800DA824`'s `handicap_scale * temp + base` is a single Gekko
    /// `fmadds` (`tools/ppc_fma_audit.py ftCo_800DA824`; `docs/math.md`);
    /// `escape_timer` computes it with `f32::mul_add` for that one
    /// rounding. At these (deliberately extreme, chosen to make the
    /// difference land outside the mantissa noise floor) inputs, a plain
    /// `handicap_scale * temp + base` and the fused form disagree, and this
    /// pins the fused, hardware-matching result -- hand-verified against
    /// `libm`'s `fmaf` outside Rust, independent of
    /// `tests/escape_formula_differential.rs`'s own C-oracle comparison
    /// (which, for this same function, needs a documented relative
    /// tolerance rather than an exact match; see that test's comment and
    /// `docs/math.md`).
    #[test]
    fn escape_timer_matches_the_hardware_fused_rounding_not_naive_two_rounding() {
        let base = f32::from_bits(0x0011_33c4); // 1.579773e-39
        let handicap_scale = f32::from_bits(0x7d11_a9fa); // 1.2101289e37
        let result = escape_timer(base, handicap_scale, 0.0, 0.0, 0.0, 0.0, 0.0, 0, 7);
        assert_eq!(result.to_bits(), 0xfe7e_e975);
        assert_ne!(
            result,
            handicap_scale * -7.0 + base,
            "the test inputs should straddle a rounding boundary; if this \
             assertion fails the inputs above no longer demonstrate anything"
        );
    }

    #[test]
    fn threshold_crossings_retain_source_strictness() {
        assert!(fresh_horizontal(0.7, 0.69, 0.7));
        assert!(!fresh_horizontal(0.7, 0.7, 0.7));
        assert!(fresh_horizontal(-0.7, -0.69, 0.7));
        assert!(fresh_up(0.6, 0.59, 0.6));
        assert!(!fresh_up(0.6, 0.6, 0.6));
        assert!(fresh_down(-0.6, -0.59, -0.6));
        assert!(!fresh_down(-0.6, -0.6, -0.6));
    }

    #[test]
    fn pummel_uses_only_the_fresh_a_bit() {
        assert!(!pummel_pressed(0));
        assert!(!pummel_pressed(0x200));
        assert!(pummel_pressed(0x100));
        assert!(pummel_pressed(0x110));
    }

    #[test]
    fn throw_rate_preserves_the_source_branch_and_operation_order() {
        assert_eq!(throw_animation_rate(true, f32::NAN, f32::NAN), 1.0);
        let weight = f32::from_bits(0x5104_7A75);
        let scale = f32::from_bits(0x2F2F_5F4B);
        assert_eq!(
            throw_animation_rate(false, weight, scale).to_bits(),
            0x3E34_8831
        );
        assert_eq!(((1.0 / weight) / scale).to_bits(), 0x3E34_8830);
    }

    #[test]
    fn capture_alignment_uses_the_source_strict_scaled_height_branch() {
        assert_eq!(
            capture_alignment([1.0, 2.0, 3.0], [5.0, 6.0, 7.0], [2.0, 4.0, 6.0], 1.0, 2.0,),
            ([4.0, 4.0, 4.0], false),
        );
        assert!(capture_alignment([0.0; 3], [0.0, 2.000_000_2, 0.0], [0.0; 3], 1.0, 2.0).1);
        assert!(!capture_alignment([0.0; 3], [0.0; 3], [0.0; 3], f32::NAN, 1.0).1);
    }

    #[test]
    fn grab_mash_preserves_strict_latches_double_penalty_and_shake_wrap() {
        let mut timer = 10.0;
        let mut state = MashState {
            shake_enabled: true,
            shake_frame: 1,
            shake_frames: 3,
            ..Default::default()
        };
        assert!(!mash(&mut timer, &mut state, 0, [0.5, -0.5], 2.0, 0.5));
        assert_eq!(timer, 10.0);
        assert!(mash(&mut timer, &mut state, 0x100, [0.6, 0.0], 2.0, 0.5));
        assert_eq!(timer, 6.0);
        assert_eq!(state.axes, [1, 0]);
        assert_eq!(state.shake_frame, 2);
        assert!(!mash(&mut timer, &mut state, 0, [0.0; 2], 2.0, 0.5));
        assert_eq!(state.axes, [1, 0]);
        assert!(!state.shaking);
        assert!(mash(&mut timer, &mut state, 1 << 31, [-0.6, 0.6], 2.0, 0.5));
        assert_eq!(timer, 2.0);
        assert_eq!(state.axes, [-1, 1]);
        assert_eq!(state.shake_frame, 0);
    }

    #[test]
    fn horizontal_main_and_cstick_precede_vertical_selection() {
        let thresholds = [0.7, 0.6, -0.6];
        assert_eq!(
            direction(
                [-0.7, 0.8],
                [0.0; 2],
                [0.8, 0.8],
                [0.0; 2],
                -1.0,
                thresholds,
            ),
            Some(ThrowDirection::Forward)
        );
        assert_eq!(
            direction([0.0, -0.7], [0.0; 2], [0.8, 0.8], [0.0; 2], 1.0, thresholds,),
            Some(ThrowDirection::Forward)
        );
    }

    use crate::game::*;

    #[test]
    fn equal_stocks_tie_at_standing_zero_regardless_of_percent() {
        // `fn_80165AC0` only increments a strictly-lesser score's opponent
        // count, so an equal score (here, equal stocks; the Stock-match
        // branch of `fn_8016588C` never reads percent) leaves both players
        // at standing 0.
        assert_eq!(standing([4, 4], 0, 0), 0);
        assert_eq!(standing([4, 4], 0, 1), 0);
        assert_eq!(standing([1, 1], 12_345, 0), 0);
    }

    #[test]
    fn fewer_stocks_ranks_strictly_worse() {
        assert_eq!(standing([4, 3], 0, 0), 0, "more stocks is standing 0");
        assert_eq!(standing([4, 3], 0, 1), 1, "fewer stocks is standing 1");
        assert_eq!(standing([1, 4], 0, 0), 1);
        assert_eq!(standing([1, 4], 0, 1), 0);
    }

    #[test]
    fn handicap_defaults_to_nine_when_the_rule_is_off() {
        assert_eq!(handicap(None, 0), 9);
        assert_eq!(handicap(None, 1), 9);
        let players = [
            PlayerSettings { handicap: 3 },
            PlayerSettings { handicap: 7 },
        ];
        assert_eq!(handicap(Some(&players), 0), 3);
        assert_eq!(handicap(Some(&players), 1), 7);
    }
}
