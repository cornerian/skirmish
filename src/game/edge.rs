//! Floor-end collision modes and the edge teeter: `game`-layer wiring around
//! the pure arithmetic in `crate::fighter::edge`. See `docs/edges.md` for the
//! per-action mode table (every `_Coll` callback cited) and the Ottotto/
//! OttottoWait entry, IASA and exit-distance sources.
use super::{
    Action, Error, Fighter,
    data::{Bone, FighterData, HurtboxState},
};
use crate::{collision::stage, fighter::edge as math};
use serde::{Deserialize, Serialize};

pub use math::Side as EdgeSide;

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
        super::validation::validate_animation_pose(&frame.bones, fighter)?;
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
pub(crate) fn mode_for_action(action: Action, edge_rules_present: bool) -> math::Mode {
    use Action::*;
    match action {
        Jab | Attack12 | Attack13 | Attack100Start | Attack100Loop | Attack100End | AttackS3Hi
        | AttackS3HiS | AttackS3S | AttackS3LwS | AttackS3Lw | AttackHi3 | AttackLw3
        | AttackS4Hi | AttackS4HiS | AttackS4S | AttackS4LwS | AttackS4Lw | AttackHi4
        | AttackLw4 | AttackDash | EscapeF | EscapeB | EscapeN | CatchCut | Catch | CatchDash
        | DownAttack | PassiveStandF | PassiveStandB | CliffClimb | RunTurn | Ottotto
        | OttottoWait | AppealSR | AppealSL
        // ft_800827A0 (`ftFx_SpecialSEnd_Coll`): mode 2, clamp. The Start
        // phase's `ft_80082708` is plain (the unmatched default below).
        | SpecialSEnd => math::Mode::Clamp,
        Wait | Walk | Landing | RunBrake => {
            if edge_rules_present {
                math::Mode::Teeter
            } else {
                math::Mode::Plain
            }
        }
        _ => math::Mode::Plain,
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
    super::simulation::enter(fighter, Action::Ottotto);
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
        super::simulation::enter(fighter, Action::OttottoWait);
    }
    Ok(())
}

/// `ftCo_Ottotto_Coll`/`ftCo_OttottoWait_Coll`'s exit check, run once per
/// frame after collision settles: `ABS(fp->cur_pos.x - pos.x) > x478 + x47C`
/// against the current floor's end on the facing side (`mpFloorGetRight` for
/// `facing_dir > 0`, `mpFloorGetLeft` otherwise) enters Wait (`ft_8008A2BC`).
pub(crate) fn check_exit(fighter: &mut Fighter, rules: &Rules, floor_end_x: f32) {
    if math::exit_distance_exceeded(
        fighter.position[0],
        floor_end_x,
        rules.teeter_exit_distance,
        rules.teeter_exit_tolerance,
    ) {
        super::simulation::enter(fighter, Action::Wait);
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
