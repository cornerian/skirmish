//! Common special-action shell and Starlark special-policy boundary.
//!
//! The fighter definition owns action metadata and the lifecycle program owns
//! character policy. This module contains only shared eligibility and ABI
//! adapters used by the simulation.

use crate::{
    fighter::{Movement, aerial, damage, edge::Mode, tilt},
    game::{
        self, Action, Controller, Error, Fighter,
        data::{Attack, FighterData, Rules as MatchRules, StageGeometry},
        script::{
            Hook,
            lifecycle::{self, NativeContext},
        },
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub side_stick_threshold: f32,
    pub turn_threshold: f32,
    pub vertical_threshold: f32,
    pub air_drift_recovery_step: f32,
}

pub(crate) fn validate_rules(rules: &Rules) -> Result<(), Error> {
    let finite = |v: f32| v.is_finite() && v.abs() <= 1_000_000.0;
    if !finite(rules.side_stick_threshold)
        || rules.side_stick_threshold < 0.0
        || !finite(rules.turn_threshold)
        || rules.turn_threshold < 0.0
        || !finite(rules.vertical_threshold)
        || rules.vertical_threshold < 0.0
        || !finite(rules.air_drift_recovery_step)
        || rules.air_drift_recovery_step < 0.0
    {
        return Err(Error::Data("invalid special stick rules".into()));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // Context fields mirror the native callback ABI.
fn context(
    f: &Fighter,
    _data: &FighterData,
    rules: Option<&MatchRules>,
    input: Option<Controller>,
    ground_open: Option<bool>,
    air_open: Option<bool>,
    on_platform: Option<bool>,
    pre_landing: Option<&Fighter>,
    ceiling: Option<([f32; 3], usize)>,
    wall: Option<([f32; 3], usize)>,
) -> Value {
    json!({"input": input, "ground_open": ground_open, "air_open": air_open,
        "on_platform": on_platform, "grounded": f.grounded, "pre_landing": pre_landing,
        "ceiling": ceiling.map(|(normal, line)| json!({"normal": normal, "line": line})),
        "wall": wall.map(|(normal, line)| json!({"normal": normal, "line": line})),
        "rules": rules})
}

fn invoke(
    hook: Hook,
    f: &mut Fighter,
    data: &FighterData,
    rules: Option<&MatchRules>,
    context: Value,
    movement: Option<&mut Movement>,
) -> Result<Value, Error> {
    if data.script.is_none()
        && crate::game::script::bundled_source(data.specials.as_ref()).is_none()
    {
        return Ok(Value::Null);
    }
    lifecycle::invoke(hook, f, Some(data), rules, context, movement)
}

fn invoke_with_native(
    hook: Hook,
    f: &mut Fighter,
    data: &FighterData,
    rules: Option<&MatchRules>,
    context: Value,
    movement: Option<&mut Movement>,
    native: NativeContext<'_>,
) -> Result<Value, Error> {
    if data.script.is_none()
        && crate::game::script::bundled_source(data.specials.as_ref()).is_none()
    {
        return Ok(Value::Null);
    }
    lifecycle::invoke_with_native(hook, f, Some(data), rules, context, movement, native)
}

fn policy_enabled(data: &FighterData) -> bool {
    data.script.is_some() || crate::game::script::bundled_source(data.specials.as_ref()).is_some()
}

fn bool_result(v: Value) -> bool {
    v.as_bool().unwrap_or(false)
}

/// Apply the authored command-variable row before the action's animation
/// callback. Native animation callbacks observe command variables written by
/// the same animation sample, and may consume them immediately (Fox's
/// neutral special uses command 0 to arm its loop and command 2 to fire).
fn command_trace_delta(
    previous: [Option<u32>; 4],
    row: &[Option<u32>; 4],
) -> ([Option<u32>; 4], Vec<(usize, u32)>) {
    let mut next = [None; 4];
    let mut events = Vec::new();
    for (index, value) in row.iter().copied().enumerate() {
        // A nullable row means no SetCmdVar command at this frame. Native
        // command variables persist until another command writes the slot;
        // forward-fill the source cursor so callback consumption cannot make
        // an absent row look like a native clear.
        next[index] = value.or(previous[index]);
        if let Some(value) = value
            && previous[index] != Some(value)
        {
            events.push((index, value));
        }
    }
    (next, events)
}

/// Apply the authored command row for the current action sample. This is
/// reusable at the action-entry boundary because native animation advances
/// the destination frame before the next ordinary animation sample.
pub(crate) fn sample_command_trace(
    f: &mut Fighter,
    data: &FighterData,
    rules: &MatchRules,
) -> Result<(), Error> {
    let owner = f
        .script_events
        .active_move
        .filter(|owner| owner.matches(f.action, f.script_events.action_generation))
        .map(|owner| owner.behavior_index);
    let Some(cache) = data.script_resources.get() else {
        return Ok(());
    };
    let generation = f.script_events.action_generation;
    let Some(rows) = cache.command_trace(owner, f.action) else {
        return Ok(());
    };
    let sample = (generation, f.action, f.action_frame);
    if f.script_events.command_trace_sample == Some(sample) {
        return Ok(());
    }
    if f.script_events.command_trace_sample.is_none_or(
        |(previous_generation, previous_action, _)| {
            previous_generation != generation || previous_action != f.action
        },
    ) {
        f.script_events.command_trace_values = [None; 4];
    }
    f.script_events.command_trace_sample = Some(sample);
    let Some(row) = rows.get(f.action_frame as usize) else {
        return Ok(());
    };
    let (next_values, events) = command_trace_delta(f.script_events.command_trace_values, row);
    f.script_events.command_trace_values = next_values;
    let sampled_action = f.action;
    let sampled_generation = generation;
    for (index, value) in events {
        let mut command = match f.action_state.get("command") {
            Some(crate::game::script::LocalValue::Tuple(values)) if values.len() == 4 => values
                .iter()
                .map(|value| match value {
                    crate::game::script::LocalValue::Integer(value) => *value,
                    _ => 0,
                })
                .collect::<Vec<_>>(),
            _ => vec![0; 4],
        };
        command[index] = i64::from(value);
        f.action_state.insert(
            "command".into(),
            crate::game::script::LocalValue::Tuple(
                command
                    .into_iter()
                    .map(crate::game::script::LocalValue::Integer)
                    .collect(),
            ),
        );
        let context = json!({
            "event": {
                "kind": Hook::CommandTraceChanged.name(),
                "command_index": index,
                "value": value,
                "frame": f.action_frame,
            }
        });
        invoke(
            Hook::CommandTraceChanged,
            f,
            data,
            Some(rules),
            context,
            None,
        )?;
        // A command callback may enter a new action. Do not continue
        // delivering events from the old action's sampled row into the new
        // action's state or callback set.
        if f.action != sampled_action || f.script_events.action_generation != sampled_generation {
            break;
        }
    }
    Ok(())
}

/// Deliver rising bits from the native per-frame animation-event sidecar.
/// The exporter stores cumulative native flags, so a set bit is a source edge,
/// not a per-frame deadline. A generation/sample cursor makes repeated
/// snapshots idempotent and lets same-action re-entry fire again.
pub(crate) fn sample_animation_event_masks(
    f: &mut Fighter,
    data: &FighterData,
    rules: &MatchRules,
) -> Result<(), Error> {
    let owner = f
        .script_events
        .active_move
        .filter(|owner| owner.matches(f.action, f.script_events.action_generation))
        .map(|owner| owner.behavior_index);
    let Some(cache) = data.script_resources.get() else {
        return Ok(());
    };
    let generation = f.script_events.action_generation;
    let action = f.action;
    let mask = cache
        .animation_event_masks(owner, action)
        .and_then(|rows| rows.get(action_frame_index(f.action_frame)))
        .copied()
        .unwrap_or(0);
    let new_bits = animation_event_delta(
        &mut f.script_events,
        generation,
        action,
        f.action_frame,
        mask,
    );
    if new_bits == 0 {
        return Ok(());
    }
    for event_index in 0..u8::BITS as u8 {
        let bit = 1u8 << event_index;
        if new_bits & bit == 0 {
            continue;
        }
        let context = json!({
            "event": {
                "kind": Hook::AnimationEvent.name(),
                "event_id": event_index,
                "frame": f.action_frame,
            }
        });
        invoke(Hook::AnimationEvent, f, data, Some(rules), context, None)?;
        if f.action != action || f.script_events.action_generation != generation {
            break;
        }
    }
    Ok(())
}

fn animation_event_delta(
    state: &mut crate::game::script::events::NativeEventState,
    generation: crate::game::script::scheduler::ActionGeneration,
    action: crate::game::Action,
    frame: u32,
    mask: u8,
) -> u8 {
    let sample = (generation, action, frame);
    if state.animation_event_sample == Some(sample) {
        return 0;
    }
    if state.animation_event_generation != Some(generation) {
        state.animation_event_generation = Some(generation);
        state.animation_event_seen_mask = 0;
    }
    let new_bits = mask & !state.animation_event_seen_mask;
    state.animation_event_seen_mask = mask;
    state.animation_event_sample = Some(sample);
    new_bits
}

#[inline]
fn action_frame_index(frame: u32) -> usize {
    usize::try_from(frame).unwrap_or(usize::MAX)
}

fn motion_cache(
    f: &mut Fighter,
    data: &FighterData,
) -> Option<std::sync::Arc<crate::game::script::lifecycle_resources::ResourceCache>> {
    let cache = data.script_resources.get()?;
    let owner = f
        .script_events
        .active_move
        .filter(|owner| owner.matches(f.action, f.script_events.action_generation))
        .map(|owner| owner.behavior_index);
    let Some((profile_id, profile)) = cache.motion_profile_for_owner(owner, f.action) else {
        f.script_events.motion_profile_id = None;
        return None;
    };
    if f.script_events.motion_profile_id != Some(profile_id) {
        let preserve = f
            .script_events
            .pending_transitions
            .last()
            .is_some_and(|transition| {
                transition.to == f.action
                    && transition.generation == f.script_events.action_generation
                    && transition.preserve_state
            });
        f.script_events.motion_profile_id = Some(profile_id);
        if !preserve {
            f.script_events.motion_state = profile.initial_state();
        }
    }
    f.script_events.motion_state.phase_frame = f.action_frame as f32;
    if let Some(field) = cache.profile_delay_field_for_owner(owner, f.action)
        && let Some(crate::game::script::LocalValue::Number(value)) = f.action_state.get(field)
        && value.is_finite()
    {
        f.script_events.motion_state.gravity_delay = *value as f32;
    }
    Some(cache)
}

/// Read the command tuple at the instant aerial profile physics runs.  The
/// animation trace updates this action-state value before callbacks, so it
/// must not be copied into the immutable motion binding or sampled from a
/// stale event record.
fn current_action_command(f: &Fighter) -> [i64; crate::game::script::motion::COMMAND_SLOTS] {
    match f.action_state.get("command") {
        Some(crate::game::script::LocalValue::Tuple(values))
            if values.len() == crate::game::script::motion::COMMAND_SLOTS =>
        {
            let mut command = [0; crate::game::script::motion::COMMAND_SLOTS];
            for (slot, value) in command.iter_mut().zip(values) {
                *slot = command_value(value);
            }
            command
        }
        _ => [0; crate::game::script::motion::COMMAND_SLOTS],
    }
}

/// Command variables are native integers, but a host snapshot can carry an
/// integral value through the generic numeric representation. Preserve that
/// value instead of silently turning it into zero before command-sensitive
/// Fox/Falco aerial motion runs.
#[inline]
fn command_value(value: &crate::game::script::LocalValue) -> i64 {
    match value {
        crate::game::script::LocalValue::Integer(value) => *value,
        crate::game::script::LocalValue::Number(value)
            if value.is_finite()
                && value.fract() == 0.0
                && *value >= i64::MIN as f64
                && *value <= i64::MAX as f64 =>
        {
            *value as i64
        }
        _ => 0,
    }
}

fn sync_profile_delay(
    f: &mut Fighter,
    cache: &crate::game::script::lifecycle_resources::ResourceCache,
) {
    let owner = f
        .script_events
        .active_move
        .filter(|owner| owner.matches(f.action, f.script_events.action_generation))
        .map(|owner| owner.behavior_index);
    if let Some(field) = cache.profile_delay_field_for_owner(owner, f.action)
        && let Some(crate::game::script::LocalValue::Number(value)) = f.action_state.get_mut(field)
    {
        *value = f64::from(f.script_events.motion_state.gravity_delay);
    }
}
fn is_input_hook(hook: Hook) -> bool {
    matches!(
        hook,
        Hook::InputPressed
            | Hook::InputReleased
            | Hook::StickChanged
            | Hook::ActionAvailabilityChanged
    )
}

// These are deliberately separate queries: metadata is static and cached by
// the definition loader, while lifecycle callbacks are mutable policy.
pub(crate) fn attack(action: Action, data: &FighterData) -> Option<&Attack> {
    crate::game::script::definition::attack(action, data)
}
pub(crate) fn collision_mode(action: Action, data: &FighterData) -> Option<Mode> {
    crate::game::script::definition::collision_mode(action, data)
}
pub(crate) fn ledge_catchable(action: Action, data: &FighterData) -> bool {
    crate::game::script::definition::ledge_catchable(action, data)
}
pub(crate) fn wants_redirect(action: Action, data: &FighterData) -> bool {
    crate::game::script::definition::wants_redirect(action, data)
}
pub(crate) fn hold_frame_var(action: Action, data: &FighterData) -> Option<String> {
    crate::game::script::definition::hold_frame_var(action, data)
}

/// Return the immutable registration-time animation loop declaration for an
/// action. Native action-frame progression remains independent of this flag;
/// it only changes which finite animation sample is selected.
pub(crate) fn animation_loop(action: Action, data: &FighterData) -> bool {
    data.script_resources
        .get()
        .is_some_and(|cache| cache.animation_loop(action))
}

pub(crate) fn animation_loop_for_owner(fighter: &Fighter, data: &FighterData) -> bool {
    data.script_resources.get().is_some_and(|cache| {
        let owner = fighter
            .script_events
            .active_move
            .filter(|owner| owner.matches(fighter.action, fighter.script_events.action_generation))
            .map(|owner| owner.behavior_index);
        cache.animation_loop_for_owner(owner, fighter.action)
    })
}

fn grounded_chain_open(f: &Fighter, data: &FighterData) -> bool {
    f.grounded
        && (matches!(
            f.action,
            Action::Wait
                | Action::Walk
                | Action::Dash
                | Action::Run
                | Action::RunBrake
                | Action::Turn
                | Action::Squat
                | Action::SquatWait
                | Action::SquatRv
        ) || matches!(
            tilt::interrupt_chain(f, data),
            Some(tilt::Chain::Wait) | Some(tilt::Chain::Taunt)
        ) || game::flow::landing::interruptible(f, data))
}
fn aerial_chain_open(f: &Fighter) -> bool {
    !f.grounded
        && (damage::wall_tech_interruptible(f)
            || damage::damage_air_interruptible(f)
            || matches!(
                f.action,
                Action::Jump | Action::JumpAerial | Action::Fall | Action::Pass
            )
            || aerial::interruptible(f))
}

pub(crate) fn update_actions(
    f: &mut Fighter,
    data: &FighterData,
    rules: &MatchRules,
    input: Controller,
) -> Result<bool, Error> {
    if !policy_enabled(data) {
        return Ok(false);
    }
    let ground = grounded_chain_open(f, data);
    let air = aerial_chain_open(f);
    let jump_available = data
        .locomotion
        .as_ref()
        .is_none_or(|parameters| f.locomotion.jumps_used < parameters.max_jumps);
    let previous_mask = f.script_events.availability_mask;
    let mut availability_mask = 0;
    if ground {
        availability_mask |= crate::game::script::events::NativeEventState::GROUND_OPEN;
    }
    if air {
        availability_mask |= crate::game::script::events::NativeEventState::AIR_OPEN;
    }
    if jump_available {
        availability_mask |= crate::game::script::events::NativeEventState::JUMP_AVAILABLE;
    }
    let pressed = input.buttons & !f.previous_input.buttons;
    let released = f.previous_input.buttons & !input.buttons;
    let stick_changed = input.stick != f.previous_input.stick
        || input.cstick != f.previous_input.cstick
        || input.trigger != f.previous_input.trigger;
    let availability_changed = previous_mask != availability_mask;
    // Input policy is event driven. Held neutral input does not re-enter VM;
    // every concrete edge that occurred in this sample is delivered in the
    // fixed native order below.
    if pressed == 0 && released == 0 && !stick_changed && !availability_changed {
        return Ok(false);
    }
    let mut consumed = false;
    let event_specs = [
        (pressed != 0, Hook::InputPressed),
        (released != 0, Hook::InputReleased),
        (stick_changed, Hook::StickChanged),
        (availability_changed, Hook::ActionAvailabilityChanged),
    ];
    for (occurred, hook) in event_specs {
        if !occurred {
            continue;
        }
        let mut event_context = context(
            f,
            data,
            Some(rules),
            Some(input),
            Some(grounded_chain_open(f, data)),
            Some(aerial_chain_open(f)),
            None,
            None,
            None,
            None,
        );
        if let Some(object) = event_context.as_object_mut() {
            object.insert(
                "event".into(),
                json!({
                    "kind": hook.name(),
                    "pressed": pressed,
                    "released": released,
                    "availability_changed": availability_changed,
                }),
            );
        }
        let result = invoke(hook, f, data, Some(rules), event_context, None)?;
        if is_input_hook(hook) && bool_result(result) {
            consumed = true;
        }
    }
    // Availability is rollback state owned by this native event boundary.
    // Stage it until callback dispatch has succeeded so an invalid return or
    // host failure cannot leave a partial event transition behind.
    f.script_events.availability_mask = availability_mask;
    Ok(consumed)
}
pub(crate) fn update_animation(
    f: &mut Fighter,
    data: &FighterData,
    rules: &MatchRules,
    input: Controller,
    on_platform: bool,
) -> Result<(), Error> {
    if !policy_enabled(data) {
        return Ok(());
    }
    sample_command_trace(f, data, rules)?;
    sample_animation_event_masks(f, data, rules)?;
    // Animation policy is an edge event.  Special action descriptors carry
    // the immutable attack frame table used by the native animation owner;
    // dispatch exactly when that phase reaches its terminal frame.  Using
    // equality also prevents a callback that deliberately leaves its action
    // unchanged from becoming a hidden every-frame VM hook after completion.
    let Some(animation) = attack(f.action, data) else {
        return Ok(());
    };
    if !animation_end_due(
        f.action_frame,
        animation.frames.len(),
        animation_loop(f.action, data),
        f.script_events.animation_due(),
    ) {
        return Ok(());
    }
    let generation = f.script_events.action_generation;
    let mut context = context(
        f,
        data,
        Some(rules),
        Some(input),
        None,
        None,
        Some(on_platform),
        None,
        None,
        None,
    );
    if let Some(object) = context.as_object_mut() {
        object.insert(
            "event".into(),
            json!({"kind": "animation_ended", "action_frame": f.action_frame}),
        );
    }
    invoke(Hook::AnimationEnded, f, data, Some(rules), context, None).map(|_| {
        // Mark the generation that reached the terminal frame. A callback may
        // enter a new action; that transition has already staged its own
        // generation, so retain the captured generation as the delivered one.
        f.script_events.animation_delivered_generation = Some(generation);
    })
}

/// A terminal animation edge exists only for a non-looping, non-empty
/// executable animation.  The `>=` comparison deliberately tolerates a
/// retained action frame on entry: a custom phase can inherit a frame from
/// its predecessor without losing its one terminal notification.
#[inline]
fn animation_end_due(frame: u32, length: usize, looping: bool, generation_due: bool) -> bool {
    !looping
        && length != 0
        && usize::try_from(frame).is_ok_and(|frame| frame >= length)
        && generation_due
}
pub(crate) fn ground_target_velocity(
    f: &mut Fighter,
    data: &FighterData,
    _rules: &MatchRules,
) -> Result<Option<f32>, Error> {
    let Some(cache) = motion_cache(f, data) else {
        return Ok(None);
    };
    let Some(profile_id) = f.script_events.motion_profile_id else {
        return Ok(None);
    };
    let Some(profile) = cache.motion_profile_by_id(profile_id) else {
        return Ok(None);
    };
    Ok(profile.ground_target_velocity(
        f.script_events.motion_state.phase_frame,
        &f.script_events.motion_binding,
    ))
}
pub(crate) fn ground_friction_override(
    f: &mut Fighter,
    data: &FighterData,
    _rules: &MatchRules,
) -> Result<Option<f32>, Error> {
    let Some(cache) = motion_cache(f, data) else {
        return Ok(None);
    };
    let Some(profile_id) = f.script_events.motion_profile_id else {
        return Ok(None);
    };
    let Some(profile) = cache.motion_profile_by_id(profile_id) else {
        return Ok(None);
    };
    Ok(profile.ground_friction(f.script_events.motion_state.phase_frame))
}

pub(crate) fn apply_ground_profile(
    f: &mut Fighter,
    data: &FighterData,
    movement: &mut Movement,
) -> Result<bool, Error> {
    let Some(cache) = motion_cache(f, data) else {
        return Ok(false);
    };
    let Some(profile_id) = f.script_events.motion_profile_id else {
        return Ok(false);
    };
    let Some(profile) = cache.motion_profile_by_id(profile_id) else {
        return Ok(false);
    };
    if profile.ground.is_empty() {
        return Ok(false);
    }
    let applied = profile.apply_with_binding(
        &mut f.script_events.motion_state,
        movement,
        true,
        &f.script_events.motion_binding,
    );
    sync_profile_delay(f, &cache);
    Ok(applied)
}

pub(crate) fn air_physics(
    f: &mut Fighter,
    data: &FighterData,
    _rules: &MatchRules,
    movement: &mut Movement,
) -> Result<bool, Error> {
    let Some(cache) = motion_cache(f, data) else {
        return Ok(false);
    };
    let Some(profile_id) = f.script_events.motion_profile_id else {
        return Ok(false);
    };
    let Some(profile) = cache.motion_profile_by_id(profile_id) else {
        return Ok(false);
    };
    if profile.air.is_empty() {
        return Ok(false);
    }
    let command = current_action_command(f);
    let applied = profile.apply_with_binding_and_command(
        &mut f.script_events.motion_state,
        movement,
        false,
        &f.script_events.motion_binding,
        &command,
    );
    sync_profile_delay(f, &cache);
    Ok(applied)
}
pub(crate) fn tick_ground_timers(
    f: &mut Fighter,
    data: &FighterData,
    _rules: &MatchRules,
) -> Result<(), Error> {
    if data.script_resources.get().is_none() {
        return Ok(());
    }
    let _ = motion_cache(f, data);
    tick_countdowns_at_phase(
        f,
        data,
        crate::game::script::action_events::CountdownPhase::Physics,
    )?;
    let Some(cache) = data.script_resources.get() else {
        return Ok(());
    };
    cache
        .action_events()
        .advance_clocks(f.action, &mut f.action_state)
        .map_err(|error| Error::Data(error.to_string()))?;
    // Fox's grounded side phases tick the shared gravity delay even though
    // they do not apply aerial gravity. Native ground arithmetic clamps the
    // delay at zero; the aerial profile keeps its strict-positive subtraction
    // semantics in `MotionState::tick_gravity_delay`.
    let owner = f
        .script_events
        .active_move
        .filter(|owner| owner.matches(f.action, f.script_events.action_generation))
        .map(|owner| owner.behavior_index);
    if f.grounded
        && cache
            .profile_delay_field_for_owner(owner, f.action)
            .is_some()
        && let Some((profile_id, _)) = cache.motion_profile_for_owner(owner, f.action)
        && f.script_events.motion_profile_id == Some(profile_id)
        && f.script_events.motion_state.gravity_delay > 0.0
    {
        f.script_events.motion_state.gravity_delay =
            (f.script_events.motion_state.gravity_delay - 1.0).max(0.0);
        sync_profile_delay(f, &cache);
    }
    Ok(())
}

pub(crate) fn tick_countdowns_at_phase(
    f: &mut Fighter,
    data: &FighterData,
    phase: crate::game::script::action_events::CountdownPhase,
) -> Result<(), Error> {
    let Some(cache) = data.script_resources.get() else {
        return Ok(());
    };
    let tokens = cache
        .action_events()
        .advance_countdowns_at_phase(f.action, &mut f.action_state, phase)
        .map_err(|error| Error::Data(error.to_string()))?;
    for token in tokens {
        f.script_events
            .push_deadline_token(token)
            .map_err(|error| Error::Data(error.to_string()))?;
    }
    Ok(())
}
pub(crate) fn update_ground_contact(
    f: &mut Fighter,
    data: &FighterData,
    rules: &MatchRules,
) -> Result<(), Error> {
    if !policy_enabled(data) {
        return Ok(());
    }
    let mut context = context(
        f,
        data,
        Some(rules),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    if let Some(object) = context.as_object_mut() {
        object.insert(
            "event".into(),
            json!({
                "kind": "surface_contact",
                "surface": "floor",
                "line": f.ground_line,
                "normal": f.floor_normal,
            }),
        );
    }
    invoke(Hook::SurfaceContact, f, data, Some(rules), context, None).map(|_| ())
}
pub(crate) fn transfer_ground_air(
    f: &mut Fighter,
    data: &FighterData,
    rules: &MatchRules,
) -> Result<bool, Error> {
    if !policy_enabled(data) {
        return Ok(false);
    }
    let previous_action = f.action;
    let previous_generation = f.script_events.action_generation;
    let result = invoke(
        Hook::GroundAirChanged,
        f,
        data,
        Some(rules),
        context(
            f,
            data,
            Some(rules),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        None,
    )?;
    Ok(bool_result(result)
        || f.action != previous_action
        || f.script_events.action_generation != previous_generation)
}
pub(crate) fn land(
    f: &mut Fighter,
    data: &FighterData,
    rules: &MatchRules,
    on_platform: bool,
    pre_landing: &Fighter,
) -> Result<bool, Error> {
    if !policy_enabled(data) {
        return Ok(false);
    }
    Ok(bool_result(invoke_with_native(
        Hook::Landed,
        f,
        data,
        Some(rules),
        context(
            f,
            data,
            Some(rules),
            None,
            None,
            None,
            Some(on_platform),
            Some(pre_landing),
            None,
            None,
        ),
        None,
        NativeContext {
            pre_landing: Some(pre_landing),
            geometry: None,
        },
    )?))
}
pub(crate) fn air_contact(
    f: &mut Fighter,
    data: &FighterData,
    rules: &MatchRules,
    ceiling: Option<([f32; 3], usize)>,
    wall: Option<([f32; 3], usize)>,
) -> Result<bool, Error> {
    if !policy_enabled(data) {
        return Ok(false);
    }
    Ok(bool_result(invoke(
        Hook::SurfaceContact,
        f,
        data,
        Some(rules),
        context(
            f,
            data,
            Some(rules),
            None,
            None,
            None,
            None,
            None,
            ceiling,
            wall,
        ),
        None,
    )?))
}
pub(crate) fn platform_drop(
    f: &mut Fighter,
    data: &FighterData,
    rules: &MatchRules,
    input: Controller,
    geometry: &StageGeometry,
) -> Result<bool, Error> {
    if !policy_enabled(data) {
        return Ok(false);
    }
    // A platform decision is emitted on the native downward threshold edge.
    // The simulation asks this adapter after every action dispatch, so a
    // held stick must not turn the adapter into a polling VM hook.
    let Some(locomotion) = data.locomotion.as_ref() else {
        return Ok(false);
    };
    let crossed_down = input.stick[1] <= -locomotion.pass_stick_threshold
        && f.previous_input.stick[1] > -locomotion.pass_stick_threshold;
    if !crossed_down {
        return Ok(false);
    }
    let on_platform = game::collision::on_platform(f, geometry);
    Ok(bool_result(invoke_with_native(
        Hook::PlatformDropDecision,
        f,
        data,
        Some(rules),
        context(
            f,
            data,
            Some(rules),
            Some(input),
            None,
            None,
            Some(on_platform),
            None,
            None,
            None,
        ),
        None,
        NativeContext {
            pre_landing: None,
            geometry: Some(geometry),
        },
    )?))
}
/// Drain the fighter-owned typed emission queue after lifecycle callbacks
/// have committed, preserving native player ordering.
pub(crate) fn drain_pending(
    f: &mut Fighter,
    _data: &FighterData,
    _rules: &MatchRules,
) -> Result<Vec<game::projectile::PendingProjectile>, Error> {
    Ok(std::mem::take(&mut f.pending_projectiles))
}

/// Run deferred emissions after fighter simulation, in player order. Native
/// projectile construction remains here so scripts cannot manufacture state.
pub(crate) fn emit_projectiles(
    data: &game::data::MatchData,
    state: &mut game::State,
) -> Result<(), Error> {
    for player in 0..state.fighters.len() {
        let article_spawns = std::mem::take(&mut state.fighters[player].pending_article_spawns);
        if !article_spawns.is_empty() {
            let cache = data.fighters[player]
                .script_resources
                .get()
                .ok_or_else(|| Error::Data("article resource cache is unavailable".into()))?;
            for item in article_spawns {
                let article = cache.article(item.article_id).ok_or_else(|| {
                    Error::Data(format!("article {:?} is unavailable", item.article_id))
                })?;
                let (kind, behavior, angle, speed, lifetime, hitboxes, move_id, launch_facing) =
                    match &article.behavior {
                        crate::game::script::lifecycle_resources::ArticleBehavior::Ray {
                            kind,
                            lifetime,
                            hitboxes,
                            move_id,
                        } => {
                            let crate::game::projectile::ArticleLaunch::Explicit { angle, speed } =
                                item.launch
                            else {
                                return Err(Error::Data(
                                    "ray article requires an explicit launch".into(),
                                ));
                            };
                            (
                                *kind,
                                crate::game::projectile::ProjectileBehavior::Ray,
                                angle,
                                speed,
                                *lifetime,
                                hitboxes.to_vec(),
                                *move_id,
                                None,
                            )
                        }
                        crate::game::script::lifecycle_resources::ArticleBehavior::Gravity {
                        speed,
                        angle,
                        lifetime,
                        half_life,
                        gravity,
                        terminal_velocity,
                        surface_multiplier,
                        terrain_stop_speed,
                        hitboxes,
                        move_id,
                        contact,
                    } => {
                        let crate::game::projectile::ArticleLaunch::Facing(facing) = item.launch
                        else {
                            return Err(Error::Data(
                                "gravity article requires a facing launch".into(),
                            ));
                        };
                        (
                            crate::game::projectile::ProjectileKind::Gravity(item.article_id),
                            crate::game::projectile::ProjectileBehavior::Gravity(
                                crate::game::projectile::GravityProjectileState {
                                    gravity: *gravity,
                                    terminal_velocity: *terminal_velocity,
                                    surface_multiplier: *surface_multiplier,
                                    terrain_stop_speed: *terrain_stop_speed,
                                    half_life: *half_life,
                                    contact: *contact,
                                },
                            ),
                            *angle,
                            *speed,
                            *lifetime,
                            hitboxes.to_vec(),
                            *move_id,
                            Some(if facing >= 0.0 { 1.0 } else { -1.0 }),
                        )
                    }
                    crate::game::script::lifecycle_resources::ArticleBehavior::MarioFireball {
                        speed,
                        angle,
                        lifetime,
                        half_life,
                        gravity,
                        terminal_velocity,
                        surface_multiplier,
                        terrain_stop_speed,
                        hitboxes,
                        move_id,
                        contact,
                    } => {
                        let crate::game::projectile::ArticleLaunch::Facing(facing) = item.launch
                        else {
                            return Err(Error::Data("Mario fireball requires facing launch".into()));
                        };
                        (
                            crate::game::projectile::ProjectileKind::Gravity(item.article_id),
                            crate::game::projectile::ProjectileBehavior::MarioFireball(
                                crate::game::projectile::GravityProjectileState {
                                    gravity: *gravity,
                                    terminal_velocity: *terminal_velocity,
                                    surface_multiplier: *surface_multiplier,
                                    terrain_stop_speed: *terrain_stop_speed,
                                    half_life: *half_life,
                                    contact: *contact,
                                },
                            ),
                            *angle,
                            *speed,
                            *lifetime,
                            hitboxes.to_vec(),
                            *move_id,
                            Some(if facing >= 0.0 { 1.0 } else { -1.0 }),
                        )
                    }
                };
                let mut projectile = game::projectile::spawn(
                    state.allocate_article_handle()?,
                    kind,
                    behavior,
                    player,
                    item.position,
                    angle,
                    speed,
                    lifetime,
                    hitboxes,
                    move_id,
                    &mut state.attack_instances,
                );
                if let Some(facing) = launch_facing {
                    // Native Mario fireballs keep the authored elevation and
                    // mirror only their horizontal velocity component.
                    projectile.velocity[0] *= facing;
                    projectile.facing = if projectile.velocity[0] >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                }
                let projectile_kind = projectile.kind;
                state.projectiles.push(projectile);
                state.events.push(game::Event::ProjectileSpawned {
                    owner: player,
                    projectile_kind,
                });
            }
        }
        let pending = drain_pending(
            &mut state.fighters[player],
            &data.fighters[player],
            &data.rules,
        )?;
        for item in pending {
            let projectile = game::projectile::spawn(
                state.allocate_article_handle()?,
                item.kind,
                game::projectile::ProjectileBehavior::Ray,
                player,
                item.position,
                item.angle,
                item.speed,
                item.lifetime,
                item.hitboxes,
                item.move_id,
                &mut state.attack_instances,
            );
            let projectile_kind = projectile.kind;
            state.projectiles.push(projectile);
            state.events.push(game::Event::ProjectileSpawned {
                owner: player,
                projectile_kind,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod command_trace_tests {
    use super::{animation_end_due, animation_event_delta, command_trace_delta, command_value};
    use crate::game::script::scheduler::ActionGeneration;
    use crate::game::{Action, script::LocalValue, script::events::NativeEventState};

    #[test]
    fn integral_numeric_command_values_reach_native_motion() {
        assert_eq!(command_value(&LocalValue::Integer(4)), 4);
        assert_eq!(command_value(&LocalValue::Number(4.0)), 4);
        assert_eq!(command_value(&LocalValue::Number(4.5)), 0);
        assert_eq!(command_value(&LocalValue::Number(f64::NAN)), 0);
    }

    #[test]
    fn animation_end_is_one_shot_at_or_after_terminal_frame() {
        assert!(!animation_end_due(2, 3, false, true));
        assert!(animation_end_due(3, 3, false, true));
        assert!(animation_end_due(4, 3, false, true));
        assert!(!animation_end_due(3, 3, true, true));
        assert!(!animation_end_due(3, 0, false, true));
        assert!(!animation_end_due(3, 3, false, false));
    }

    #[test]
    fn animation_event_mask_is_once_per_sample_and_rising_edge() {
        let mut state = NativeEventState::default();
        let generation = ActionGeneration::default().next().unwrap();
        assert_eq!(
            animation_event_delta(&mut state, generation, Action::Wait, 3, 1),
            1
        );
        assert_eq!(
            animation_event_delta(&mut state, generation, Action::Wait, 3, 1),
            0
        );
        assert_eq!(
            animation_event_delta(&mut state, generation, Action::Wait, 4, 1),
            0
        );
        assert_eq!(
            animation_event_delta(&mut state, generation, Action::Wait, 5, 0),
            0
        );
        assert_eq!(
            animation_event_delta(&mut state, generation, Action::Wait, 6, 1),
            1
        );
    }

    #[test]
    fn animation_event_mask_reentry_resets_delivered_bits() {
        let mut state = NativeEventState::default();
        let first = ActionGeneration::default().next().unwrap();
        let second = first.next().unwrap();
        assert_eq!(
            animation_event_delta(&mut state, first, Action::Wait, 3, 1),
            1
        );
        assert_eq!(
            animation_event_delta(&mut state, second, Action::Wait, 0, 1),
            1
        );
    }

    #[test]
    fn emits_source_writes_without_rearming_from_consumed_state() {
        let row = [Some(1), None, None, None];
        let (source, events) = command_trace_delta([None; 4], &row);
        assert_eq!(events, vec![(0, 1)]);

        // The callback may consume command 0 in fighter state; source history
        // remains the authored value, so an unchanged snapshot emits nothing.
        let (source, events) = command_trace_delta(source, &row);
        assert_eq!(source, [Some(1), None, None, None]);
        assert!(events.is_empty());

        let row = [None; 4];
        let (source, events) = command_trace_delta(source, &row);
        assert_eq!(source, [Some(1), None, None, None]);
        assert!(events.is_empty());

        let row = [Some(1), None, None, None];
        let (_, events) = command_trace_delta(source, &row);
        assert!(events.is_empty());
    }

    #[test]
    fn repeated_same_value_rows_are_snapshots_not_four_callbacks() {
        let values = [None, None, Some(1), None];
        let (source, first) = command_trace_delta([None; 4], &values);
        let (source, second) = command_trace_delta(source, &values);
        let (_, third) = command_trace_delta(source, &values);
        assert_eq!(first, vec![(2, 1)]);
        assert!(second.is_empty());
        assert!(third.is_empty());
    }
}
