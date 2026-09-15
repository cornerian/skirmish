//! Immutable callback binding selectors.
//!
//! Callback handles are linked by [`Program`](super::Program) while a
//! definition is loaded.  This module only answers the other half of the
//! question: whether a linked binding applies to one delivered event.  It
//! deliberately reads the small event records emitted by the native event
//! producers instead of reconstructing event state or parsing callback names
//! during dispatch.

use super::{Hook, definition::EventBinding};
use crate::game::Action;
use serde_json::Value;

#[derive(Clone, Debug)]
pub(crate) struct ResolvedCallback {
    pub(crate) callback: super::starlark::CallbackHandle,
    pub(crate) selector: CallbackSelector,
}

/// The statically resolved filters on one callback binding.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CallbackSelector {
    pub action: Option<Action>,
    pub actions: Vec<Action>,
    pub marker: Option<String>,
    pub countdown: Option<String>,
    pub buttons: Option<u16>,
    pub command_index: Option<u8>,
    pub deadline: Option<u32>,
    pub gate: Option<String>,
}

impl CallbackSelector {
    /// Resolve authoring action names once, at definition load.  Callers
    /// should reject the returned error rather than silently broadening an
    /// invalid declaration into a wildcard.
    pub(crate) fn from_binding(binding: &EventBinding) -> Result<Self, String> {
        let action = binding.action.as_deref().and_then(parse_action_name);
        if binding.action.is_some() && action.is_none() {
            return Err(format!("unknown callback action {:?}", binding.action));
        }
        let mut actions = Vec::with_capacity(binding.actions.len());
        for name in &binding.actions {
            let parsed = parse_action_name(name)
                .ok_or_else(|| format!("unknown callback action {name:?}"))?;
            if !actions.contains(&parsed) {
                actions.push(parsed);
            }
        }
        Ok(Self {
            action,
            actions,
            marker: binding.marker.clone(),
            countdown: binding.countdown.clone(),
            buttons: binding.buttons,
            command_index: binding.command_index,
            deadline: binding.deadline,
            gate: binding.gate.clone(),
        })
    }
}

/// Match one pre-resolved selector against the delivered event context.
/// `current_action` is the native fighter snapshot and is used for events
/// whose JSON intentionally carries no action (for example animation end).
pub(crate) fn matches(
    hook: Hook,
    selector: &CallbackSelector,
    context: &Value,
    current_action: Option<Action>,
    resources: Option<&super::lifecycle_resources::ResourceCache>,
) -> bool {
    if !action_matches(hook, selector, context, current_action) {
        return false;
    }
    let event = context.get("event").unwrap_or(context);
    if let Some(marker) = selector.marker.as_deref() {
        let direct = event.get("marker").and_then(Value::as_str) == Some(marker);
        let token_match = event
            .get("token")
            .and_then(Value::as_u64)
            .is_some_and(|token| {
                resources.is_some_and(|cache| {
                    cache
                        .action_events()
                        .markers()
                        .iter()
                        .any(|candidate| candidate.name == marker && candidate.token == token)
                })
            });
        if !direct && !token_match {
            return false;
        }
    }
    if let Some(field) = selector.countdown.as_deref() {
        let token_match = event
            .get("token")
            .and_then(Value::as_u64)
            .is_some_and(|token| {
                resources.is_some_and(|cache| {
                    cache
                        .action_events()
                        .countdowns()
                        .iter()
                        .any(|candidate| candidate.field == field && candidate.token == token)
                })
            });
        if !token_match && event.get("countdown").and_then(Value::as_str) != Some(field) {
            return false;
        }
    }
    if let Some(gate) = selector.gate.as_deref()
        && event.get("gate").and_then(Value::as_str) != Some(gate)
    {
        return false;
    }
    if let Some(mask) = selector.buttons {
        let field = match hook {
            Hook::InputReleased => "released",
            _ => "pressed",
        };
        let Some(buttons) = event.get(field).and_then(Value::as_u64) else {
            return false;
        };
        if buttons & u64::from(mask) == 0 {
            return false;
        }
    }
    if let Some(index) = selector.command_index
        && event.get("command_index").and_then(Value::as_u64) != Some(u64::from(index))
    {
        return false;
    }
    if let Some(deadline) = selector.deadline {
        let direct = event.get("deadline").and_then(Value::as_u64) == Some(u64::from(deadline))
            || event.get("frame").and_then(Value::as_u64) == Some(u64::from(deadline));
        let token_match =
            event
                .get("token")
                .and_then(Value::as_u64)
                .is_some_and(|token| {
                    resources.is_some_and(|cache| {
                        cache.action_events().frames().iter().any(|candidate| {
                            candidate.frame == deadline && candidate.token == token
                        })
                    })
                });
        if !direct && !token_match {
            return false;
        }
    }
    true
}

fn action_matches(
    hook: Hook,
    selector: &CallbackSelector,
    context: &Value,
    current_action: Option<Action>,
) -> bool {
    if selector.action.is_none() && selector.actions.is_empty() {
        return true;
    }
    let event = context.get("event").unwrap_or(context);
    let mut candidates = Vec::with_capacity(3);
    for key in action_keys(hook) {
        if let Some(action) = event.get(key).and_then(action_value) {
            candidates.push(action);
        }
    }
    if candidates.is_empty()
        && let Some(current_action) = current_action
    {
        candidates.push(current_action);
    }
    selector
        .action
        .is_some_and(|action| candidates.contains(&action))
        || selector
            .actions
            .iter()
            .any(|action| candidates.contains(action))
}

fn action_keys(hook: Hook) -> &'static [&'static str] {
    match hook {
        Hook::ActionEntered => &["to", "action"],
        Hook::ActionExited => &["from", "action"],
        _ => &["action", "to"],
    }
}

fn action_value(value: &Value) -> Option<Action> {
    value.as_str().and_then(parse_action_name)
}

fn parse_action_name(name: &str) -> Option<Action> {
    let name = name.strip_prefix("Action.").unwrap_or(name);
    super::parse_action(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn binding(hook: Hook) -> EventBinding {
        EventBinding {
            hook,
            callback: "cb".into(),
            action: None,
            marker: None,
            track: None,
            countdown: None,
            countdown_phase: crate::game::script::action_events::CountdownPhase::Physics,
            actions: Vec::new(),
            buttons: None,
            command_index: None,
            deadline: None,
            gate: None,
        }
    }

    #[test]
    fn action_entry_distinguishes_start_and_loop() {
        let mut start = binding(Hook::ActionEntered);
        start.action = Some("Action.SpecialNStart".into());
        let selector = CallbackSelector::from_binding(&start).unwrap();
        assert!(matches(
            Hook::ActionEntered,
            &selector,
            &json!({"event":{"kind":"action_entered","to":"SpecialNStart"}}),
            Some(Action::Wait),
            None,
        ));
        assert!(!matches(
            Hook::ActionEntered,
            &selector,
            &json!({"event":{"kind":"action_entered","to":"SpecialNLoop"}}),
            Some(Action::Wait),
            None,
        ));
    }

    #[test]
    fn marker_command_and_button_filters_use_event_fields() {
        let mut input_binding = binding(Hook::InputPressed);
        input_binding.buttons = Some(crate::game::BUTTON_B);
        let selector = CallbackSelector::from_binding(&input_binding).unwrap();
        assert!(matches(
            Hook::InputPressed,
            &selector,
            &json!({"event":{"pressed":crate::game::BUTTON_B}}),
            Some(Action::Wait),
            None,
        ));
        assert!(!matches(
            Hook::InputPressed,
            &selector,
            &json!({"event":{"pressed":crate::game::BUTTON_A}}),
            Some(Action::Wait),
            None,
        ));

        let mut command = binding(Hook::CommandTraceChanged);
        command.command_index = Some(2);
        let selector = CallbackSelector::from_binding(&command).unwrap();
        assert!(matches(
            Hook::CommandTraceChanged,
            &selector,
            &json!({"event":{"command_index":2}}),
            Some(Action::Wait),
            None,
        ));
    }
}
