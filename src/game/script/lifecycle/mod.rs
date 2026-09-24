//! Lifecycle transaction boundary for the Starlark bridge.
//!
//! The evaluator and safe native values live in the scripting backend. This
//! module owns the stable game-facing ABI and the data needed by the backend
//! to stage a callback transaction. In particular, native state is never
//! borrowed by an evaluator: the backend clones it, applies pending writes,
//! runs native helpers, refreshes the visible values, and commits only after
//! callback return and validation have both succeeded.

use crate::game::data::StageGeometry;
use crate::{fighter::Movement, game};
use serde_json::Value as JsonValue;
use std::sync::Arc;

#[cfg(feature = "experimental-continuations")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn invoke_async_move_start(
    fighter: &mut game::Fighter,
    data: &game::data::FighterData,
    rules: &game::data::Rules,
    program: &Arc<super::Program>,
    behavior_index: usize,
    frame: u32,
    player: usize,
    entities: &game::entity::EntityStore,
) -> Result<(), game::Error> {
    let resources = data
        .script_resources
        .get()
        .ok_or_else(|| game::Error::Data("async move resources unavailable".into()))?;
    let context = serde_json::json!({"frame": frame, "player": player});
    let host = super::lifecycle_host::LifecycleHost::new_with_cache(
        fighter,
        None,
        context,
        NativeContext {
            pre_landing: None,
            geometry: None,
            entities: Some(entities),
            entity_owner_port: u8::try_from(player).ok(),
        },
        Some(data),
        Arc::clone(&resources),
    )
    .with_metadata(program.metadata_arc());
    let host = Arc::new(std::sync::Mutex::new(host));
    let shared: super::starlark::SharedNativeHost = host.clone();
    let fighter_ref = super::starlark::HostRef::fighter(Arc::clone(&shared));
    let action = super::starlark::NativeValue::Object(super::starlark::NativeObject {
        kind: super::starlark::NativeKind::Value,
        path: "context.action".into(),
    });
    let generation = fighter.script_events.action_generation.get();
    let token = super::starlark::continuation::AwaitToken {
        owner: player as u64,
        generation,
        event_kind: "scheduled_deadline".into(),
        deadline_frame: u64::from(frame.checked_add(1).ok_or(game::Error::FrameOverflow)?),
    };
    let result = super::async_move::with_prepared_move(program, behavior_index, |prepared| {
        prepared.restore_pending(fighter.script_events.pending_move.clone())?;
        prepared.invoke_scoped(
            super::starlark::continuation::StepPhase::Pre,
            fighter_ref,
            action,
            Some(token),
        )
    })
    .map_err(game::Error::Data)?;
    if let super::starlark::async_move::MoveStep::Waiting {
        mut pending,
        awaitable,
    } = result
    {
        let mut host = host
            .lock()
            .map_err(|_| game::Error::Data("async move host poisoned".into()))?;
        let frames = match awaitable {
            super::starlark::Value::Dict(values) => {
                values.get("frames").and_then(|value| match value {
                    super::starlark::Value::Int(value) if *value >= 0 => u32::try_from(*value).ok(),
                    _ => None,
                })
            }
            _ => None,
        }
        .ok_or_else(|| game::Error::Data("async move awaitable is not a finite deadline".into()))?;
        let deadline = frame
            .checked_add(frames)
            .ok_or(game::Error::FrameOverflow)?;
        pending.continuation.await_token.deadline_frame = u64::from(deadline);
        let action_deadline = host
            .fighter
            .action_frame
            .checked_add(frames)
            .ok_or(game::Error::FrameOverflow)?;
        let owner = super::scheduler::OwnerId::new(player as u32);
        let mut scheduler = host.fighter.script_events.scheduler.clone();
        let timer_id = scheduler
            .schedule(
                owner,
                super::scheduler::TimerSpec::AtActionFrame {
                    action: host.fighter.action,
                    frame: action_deadline,
                },
                pending.continuation.await_token.generation,
            )
            .map_err(|error| game::Error::Data(error.to_string()))?;
        host.fighter.script_events.pending_move_timer = Some(timer_id);
        host.fighter.script_events.scheduler = scheduler;
        host.fighter.script_events.pending_move = Some(pending);
        host.commit_into(fighter, None)
            .map_err(|error| game::Error::Data(error.to_string()))?;
    } else if matches!(
        result,
        super::starlark::sequential_move::MoveStep::Complete(_)
    ) {
        let mut host = host
            .lock()
            .map_err(|_| game::Error::Data("async move host poisoned".into()))?;
        host.fighter.script_events.pending_move = None;
        host.fighter.script_events.pending_move_timer = None;
        host.commit_into(fighter, None)
            .map_err(|error| game::Error::Data(error.to_string()))?;
    }
    let _ = rules;
    Ok(())
}

#[cfg(feature = "experimental-continuations")]
pub(crate) fn invoke_async_move_resume(
    fighter: &mut game::Fighter,
    data: &game::data::FighterData,
    frame: u32,
    player: usize,
    timer: super::scheduler::TimerId,
    token: u64,
    entities: &game::entity::EntityStore,
) -> Result<(), game::Error> {
    if fighter.script_events.pending_move_timer != Some(timer) {
        return Ok(());
    }
    let Some(selected) = fighter.script_events.active_move else {
        fighter.script_events.pending_move = None;
        fighter.script_events.pending_move_timer = None;
        return Ok(());
    };
    if !selected.matches(fighter.action, fighter.script_events.action_generation) {
        fighter.script_events.pending_move = None;
        fighter.script_events.pending_move_timer = None;
        return Ok(());
    }
    let Some(program) = data
        .script_resources
        .get()
        .and_then(|resources| resources.program())
    else {
        return Ok(());
    };
    let context = serde_json::json!({"frame": frame, "player": player, "event": {"kind": "scheduled_deadline", "token": token}});
    let resources = data
        .script_resources
        .get()
        .ok_or_else(|| game::Error::Data("async move resources unavailable".into()))?;
    let host = super::lifecycle_host::LifecycleHost::new_with_cache(
        fighter,
        None,
        context,
        NativeContext {
            pre_landing: None,
            geometry: None,
            entities: Some(entities),
            entity_owner_port: u8::try_from(player).ok(),
        },
        Some(data),
        Arc::clone(&resources),
    )
    .with_metadata(program.metadata_arc());
    let host = Arc::new(std::sync::Mutex::new(host));
    let shared: super::starlark::SharedNativeHost = host.clone();
    let fighter_ref = super::starlark::HostRef::fighter(Arc::clone(&shared));
    let action = super::starlark::NativeValue::Object(super::starlark::NativeObject {
        kind: super::starlark::NativeKind::Value,
        path: "context.action".into(),
    });
    let result =
        super::async_move::with_prepared_move(&program, selected.behavior_index, |prepared| {
            prepared.restore_pending(fighter.script_events.pending_move.clone())?;
            prepared.resume_scoped(
                player as u64,
                selected.generation.get(),
                "scheduled_deadline",
                frame as u64,
                fighter_ref,
                action,
            )
        })
        .map_err(game::Error::Data)?;
    match result {
        Some(super::starlark::sequential_move::MoveStep::Complete(_)) => {
            let mut host = host
                .lock()
                .map_err(|_| game::Error::Data("async move host poisoned".into()))?;
            host.fighter.script_events.pending_move = None;
            host.fighter.script_events.pending_move_timer = None;
            host.commit_into(fighter, None)
                .map_err(|error| game::Error::Data(error.to_string()))?;
        }
        Some(super::starlark::sequential_move::MoveStep::Waiting {
            mut pending,
            awaitable,
        }) => {
            let frames = match awaitable {
                super::starlark::Value::Dict(values) => {
                    values.get("frames").and_then(|value| match value {
                        super::starlark::Value::Int(value) if *value >= 0 => {
                            u32::try_from(*value).ok()
                        }
                        _ => None,
                    })
                }
                _ => None,
            }
            .ok_or_else(|| {
                game::Error::Data("async move awaitable is not a finite deadline".into())
            })?;
            let deadline = frame
                .checked_add(frames)
                .ok_or(game::Error::FrameOverflow)?;
            pending.continuation.await_token.deadline_frame = u64::from(deadline);
            let mut host = host
                .lock()
                .map_err(|_| game::Error::Data("async move host poisoned".into()))?;
            let action_deadline = host
                .fighter
                .action_frame
                .checked_add(frames)
                .ok_or(game::Error::FrameOverflow)?;
            let owner = super::scheduler::OwnerId::new(player as u32);
            let mut scheduler = host.fighter.script_events.scheduler.clone();
            let timer_id = scheduler
                .schedule(
                    owner,
                    super::scheduler::TimerSpec::AtActionFrame {
                        action: host.fighter.action,
                        frame: action_deadline,
                    },
                    pending.continuation.await_token.generation,
                )
                .map_err(|error| game::Error::Data(error.to_string()))?;
            host.fighter.script_events.pending_move_timer = Some(timer_id);
            host.fighter.script_events.scheduler = scheduler;
            host.fighter.script_events.pending_move = Some(pending);
            host.commit_into(fighter, None)
                .map_err(|error| game::Error::Data(error.to_string()))?;
        }
        None => {}
    }
    Ok(())
}

/// Native state which cannot be reconstructed from the lightweight lifecycle
/// context. The backend uses the exact landing snapshot and stage geometry
/// supplied by the caller for native helper operations.
pub(crate) struct NativeContext<'a> {
    pub pre_landing: Option<&'a game::Fighter>,
    pub geometry: Option<&'a StageGeometry>,
    pub entities: Option<&'a game::entity::EntityStore>,
    pub entity_owner_port: Option<u8>,
}

impl<'a> NativeContext<'a> {
    pub(crate) fn empty() -> Self {
        Self {
            pre_landing: None,
            geometry: None,
            entities: None,
            entity_owner_port: None,
        }
    }
}

/// Validate resource data once at match construction. Attack metadata and
/// frame summaries are retained in an immutable cache and are subsequently
/// exposed by path handles from the Starlark backend.
pub fn validate_resources(
    data: &game::data::FighterData,
    rules: &game::data::Rules,
) -> Result<(), super::Error> {
    super::lifecycle_resources::validate(data, rules)
}

/// Build the immutable resource view retained by a match for one fighter.
///
/// Resource validation has already run before this seam is called.  Keeping
/// construction here gives match integration a single ownership boundary and
/// lets every later lifecycle dispatch pass the same `Arc` to the host.
pub(crate) fn resource_cache(
    data: &game::data::FighterData,
    rules: &game::data::Rules,
) -> Result<Arc<super::lifecycle_resources::ResourceCache>, super::Error> {
    let cache = super::lifecycle_resources::ResourceCache::build(Some(data), Some(rules), true)?;
    Ok(Arc::new(cache))
}

/// Invoke a lifecycle callback through the installed Starlark backend.
///
/// `Program` and the backend are intentionally kept out of this module's
/// public API. `script.rs` supplies the dispatch implementation while this
/// function preserves the ABI used by native fighter integration.
pub fn invoke(
    hook: super::Hook,
    fighter: &mut game::Fighter,
    data: Option<&game::data::FighterData>,
    rules: Option<&game::data::Rules>,
    context: JsonValue,
    movement: Option<&mut Movement>,
) -> Result<JsonValue, game::Error> {
    invoke_with_native(
        hook,
        fighter,
        data,
        rules,
        context,
        movement,
        NativeContext::empty(),
    )
}

/// Backend dispatch hook. The Starlark integration layer provides the body
/// while retaining this signature so native callers do not need to know the
/// evaluator/cache implementation. A missing program is a no-op and returns
/// the original context unchanged.
pub(crate) fn invoke_with_native(
    hook: super::Hook,
    fighter: &mut game::Fighter,
    data: Option<&game::data::FighterData>,
    rules: Option<&game::data::Rules>,
    context: JsonValue,
    movement: Option<&mut Movement>,
    native: NativeContext<'_>,
) -> Result<JsonValue, game::Error> {
    invoke_with_native_callback(hook, fighter, data, rules, context, movement, native)
}

/// Dispatch through metadata-selected callback handles while retaining the
/// native transaction boundary of [`invoke_with_native`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn invoke_with_native_callback(
    hook: super::Hook,
    fighter: &mut game::Fighter,
    data: Option<&game::data::FighterData>,
    rules: Option<&game::data::Rules>,
    context: JsonValue,
    movement: Option<&mut Movement>,
    native: NativeContext<'_>,
) -> Result<JsonValue, game::Error> {
    let resources = data.and_then(|fighter| fighter.script_resources.get());
    invoke_with_native_callback_cached(
        hook, fighter, data, rules, context, movement, native, resources,
    )
}

/// Dispatch using a match-level immutable resource cache. The cache is passed
/// by the integration layer so per-frame hooks never rebuild or sanitize the
/// fighter resource tree.
#[allow(clippy::too_many_arguments)]
pub(crate) fn invoke_with_native_callback_cached(
    hook: super::Hook,
    fighter: &mut game::Fighter,
    data: Option<&game::data::FighterData>,
    rules: Option<&game::data::Rules>,
    context: JsonValue,
    movement: Option<&mut Movement>,
    native: NativeContext<'_>,
    resources: Option<Arc<super::lifecycle_resources::ResourceCache>>,
) -> Result<JsonValue, game::Error> {
    let behavior_index = fighter
        .script_events
        .active_move
        .filter(|owner| owner.matches(fighter.action, fighter.script_events.action_generation))
        .map(|owner| owner.behavior_index);
    invoke_with_native_callback_cached_for_action(
        hook,
        fighter,
        data,
        rules,
        context,
        movement,
        native,
        resources,
        fighter.action,
        behavior_index,
    )
}

/// Dispatch a lifecycle notification against an explicit action owner. This
/// is used for queued exit notifications, whose fighter has already entered
/// the destination action by the time the event is delivered.
#[allow(dead_code, clippy::too_many_arguments)] // Retained for queued action-owner integration.
pub(crate) fn invoke_with_native_callback_for_action(
    hook: super::Hook,
    fighter: &mut game::Fighter,
    data: Option<&game::data::FighterData>,
    rules: Option<&game::data::Rules>,
    context: JsonValue,
    movement: Option<&mut Movement>,
    native: NativeContext<'_>,
    action: game::Action,
) -> Result<JsonValue, game::Error> {
    invoke_with_native_callback_for_action_owner(
        hook, fighter, data, rules, context, movement, native, action, None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn invoke_with_native_callback_for_action_owner(
    hook: super::Hook,
    fighter: &mut game::Fighter,
    data: Option<&game::data::FighterData>,
    rules: Option<&game::data::Rules>,
    context: JsonValue,
    movement: Option<&mut Movement>,
    native: NativeContext<'_>,
    action: game::Action,
    behavior_index: Option<usize>,
) -> Result<JsonValue, game::Error> {
    let resources = data.and_then(|fighter| fighter.script_resources.get());
    invoke_with_native_callback_cached_for_action(
        hook,
        fighter,
        data,
        rules,
        context,
        movement,
        native,
        resources,
        action,
        behavior_index,
    )
}

#[allow(clippy::too_many_arguments)]
fn invoke_with_native_callback_cached_for_action(
    hook: super::Hook,
    fighter: &mut game::Fighter,
    data: Option<&game::data::FighterData>,
    rules: Option<&game::data::Rules>,
    context: JsonValue,
    movement: Option<&mut Movement>,
    native: NativeContext<'_>,
    resources: Option<Arc<super::lifecycle_resources::ResourceCache>>,
    action: game::Action,
    behavior_index: Option<usize>,
) -> Result<JsonValue, game::Error> {
    let Some(program) = data.and_then(super::definition::cached_program) else {
        return Ok(context);
    };

    // Callback ownership is resolved from the descriptor and the program's
    // immutable callback index. A composed fighter's lifecycle callback may
    // live below a behavior and its exported function name is independent of
    // the hook name.
    let callbacks = program.metadata().behavior_callbacks_for_owner(
        &program,
        hook,
        action,
        behavior_index,
        resources.as_deref(),
        &context,
    );
    if callbacks.is_empty() {
        return Ok(context);
    }

    dispatch_starlark(
        hook, &program, &callbacks, fighter, data, rules, context, movement, native, resources,
    )
    .map_err(|error| game::Error::Data(format!("lifecycle hook {hook:?}: {error}")))
}

/// Execute one frozen callback against a private native host and commit its
/// staged state atomically. Callback lookup is by the precomputed handle in
/// `Program::compiled`; this function never parses or evaluates a module.
#[allow(clippy::too_many_arguments)]
fn dispatch_starlark(
    hook: super::Hook,
    program: &Arc<super::Program>,
    callbacks: &[super::definition::OwnedCallback],
    fighter: &mut game::Fighter,
    data: Option<&game::data::FighterData>,
    rules: Option<&game::data::Rules>,
    context: JsonValue,
    movement: Option<&mut Movement>,
    native: NativeContext<'_>,
    resources: Option<Arc<super::lifecycle_resources::ResourceCache>>,
) -> Result<JsonValue, super::starlark::Error> {
    let host = if let Some(resources) = resources {
        super::lifecycle_host::LifecycleHost::new_with_cache(
            fighter,
            movement.as_deref(),
            context.clone(),
            native,
            data,
            resources,
        )
    } else {
        super::lifecycle_host::LifecycleHost::new(
            fighter,
            movement.as_deref(),
            context.clone(),
            native,
            data,
            rules,
        )?
    };
    let host = Arc::new(std::sync::Mutex::new(
        host.with_metadata(program.metadata_arc()),
    ));
    let shared: super::starlark::SharedNativeHost = host.clone();
    let compiled = program.compiled().ok_or_else(|| {
        super::starlark::Error::Host("program is not linked to a native environment".into())
    })?;
    let mut result = super::starlark::NativeValue::None;
    let mut input_result = None;
    compiled.with_invocation_scope(|scope| {
        for callback in callbacks {
            {
                // Drop the guard before dispatch: host access through the
                // Python bridge locks this same mutex.
                let mut host = host.lock().map_err(|_| {
                    super::starlark::Error::Host("lifecycle host lock poisoned".into())
                })?;
                host.owned_resource = callback.resource.clone();
                // Lifecycle callbacks retain their behavior owner just like
                // input callbacks.  ActionEntered/ActionExited and contact
                // callbacks may call `change_action`; dropping the owner for
                // those hooks loses move ownership at exactly the transition
                // boundary where the native dispatcher must preserve it.
                host.callback_owner = callback_owner_for_dispatch(callback);
            }
            let primary = super::starlark::HostRef::fighter(Arc::clone(&shared));
            let context_ref = super::starlark::HostRef::context(Arc::clone(&shared));
            let callback_result = compiled.dispatch_in_scope(
                scope,
                &callback.callback,
                primary,
                &[super::starlark::host_object(&context_ref)],
            )?;
            validate_return(hook, &callback_result)?;

            // Input events can have several active behavior owners. Visit them in
            // descriptor order and stop at the first affirmative result. All
            // handlers share the same staged host so earlier writes are visible
            // to the next handler.
            if is_input_hook(hook) {
                if let super::starlark::NativeValue::Bool(value) = callback_result {
                    let handled = input_result.unwrap_or(false) || value;
                    input_result = Some(handled);
                    result = super::starlark::NativeValue::Bool(handled);
                    if handled {
                        break;
                    }
                }
            } else {
                // Every selected binding for a notification runs in declaration
                // order. Decision hooks aggregate affirmative values while unit
                // notifications retain the neutral context result.
                if let super::starlark::NativeValue::Bool(value) = callback_result {
                    let handled = input_result.unwrap_or(false) || value;
                    input_result = Some(handled);
                    result = super::starlark::NativeValue::Bool(handled);
                }
            }
        }
        Ok(())
    })?;
    if is_input_hook(hook) && input_result.is_none() {
        result = super::starlark::NativeValue::None;
    }
    let mut host = host
        .lock()
        .map_err(|_| super::starlark::Error::Host("native host lock poisoned".into()))?;
    let output = if matches!(result, super::starlark::NativeValue::None) {
        context
    } else {
        super::lifecycle_host::native_to_json_with_resources(result, Some(&host.resources))?
    };
    host.commit_into(fighter, movement)?;
    Ok(output)
}

/// Check the callback ABI before any staged native state is committed.  A
/// A missing return is valid for side-effect-only hooks and is interpreted by
/// the public adapters as their neutral result. Typed policy hooks are kept
/// deliberately narrow so malformed policy cannot silently become a native
/// default.
fn validate_return(
    hook: super::Hook,
    value: &super::starlark::NativeValue,
) -> Result<(), super::starlark::Error> {
    use super::starlark::NativeValue;

    if matches!(value, NativeValue::None) {
        return Ok(());
    }
    // Keep this ABI check sourced from the runtime hook registry. The
    // compiler and native bridge therefore agree on optional decisions and
    // unit notifications without maintaining two match tables.
    let valid = hook.return_contract().allows_bool() && matches!(value, NativeValue::Bool(_));
    if valid {
        return Ok(());
    }
    Err(super::starlark::Error::Invalid(format!(
        "invalid return value for lifecycle hook `{}`",
        hook.name()
    )))
}

fn is_input_hook(hook: super::Hook) -> bool {
    matches!(
        hook,
        super::Hook::InputPressed
            | super::Hook::InputReleased
            | super::Hook::StickChanged
            | super::Hook::ActionAvailabilityChanged
    )
}

/// Preserve the linked behavior identity while a callback is running. Global
/// callbacks intentionally carry `None`; behavior callbacks carry the index
/// selected by metadata routing, regardless of hook kind.
fn callback_owner_for_dispatch(callback: &super::definition::OwnedCallback) -> Option<usize> {
    callback.behavior_index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_callbacks_keep_behavior_owner_for_action_changes() {
        let callback = crate::game::script::definition::OwnedCallback {
            callback: crate::game::script::starlark::CallbackHandle::new("on_action_entered"),
            resource: Some("special".into()),
            behavior_index: Some(7),
        };
        assert_eq!(callback_owner_for_dispatch(&callback), Some(7));

        let global = crate::game::script::definition::OwnedCallback {
            callback: crate::game::script::starlark::CallbackHandle::new("global_action_entered"),
            resource: None,
            behavior_index: None,
        };
        assert_eq!(callback_owner_for_dispatch(&global), None);
    }
}
