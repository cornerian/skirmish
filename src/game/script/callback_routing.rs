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
    pub event_id: Option<u8>,
    pub gate: Option<String>,
}

impl CallbackSelector {
    /// Resolve authoring action names once, at definition load.  Callers
    /// should reject the returned error rather than silently broadening an
    /// invalid declaration into a wildcard.
    pub(crate) fn from_binding(binding: &EventBinding) -> Result<Self, String> {
        if binding.hook == Hook::AnimationEvent {
            if binding.event_id.is_none_or(|event_id| event_id >= 8) {
                return Err("animation_event requires a numeric event_id in 0..7".into());
            }
        } else if binding.event_id.is_some() {
            return Err("event_id is only valid for animation_event callbacks".into());
        }
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
            event_id: binding.event_id,
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
    if let Some(event_id) = selector.event_id
        && event.get("event_id").and_then(Value::as_u64) != Some(u64::from(event_id))
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
    let mut found_candidate = false;
    for key in action_keys(hook) {
        if let Some(action) = event.get(key).and_then(action_value) {
            found_candidate = true;
            if selector.action.is_some_and(|expected| expected == action)
                || selector.actions.contains(&action)
            {
                return true;
            }
        }
    }
    // Preserve the source event contract: the fighter snapshot is only a
    // fallback when the event carried no recognized action field.  In
    // particular, an event with an explicit custom action must not be
    // rebound to the current canonical action.  The old implementation
    // allocated a temporary candidate vector for every selector match;
    // callback routing runs once per binding on frame hot paths.
    !found_candidate
        && current_action.is_some_and(|action| {
            selector.action.is_some_and(|expected| expected == action)
                || selector.actions.contains(&action)
        })
}

fn action_keys(hook: Hook) -> &'static [&'static str] {
    match hook {
        Hook::ActionEntered => &["to", "action"],
        Hook::ActionExited => &["from", "action"],
        _ => &["action", "to"],
    }
}

fn action_value(value: &Value) -> Option<Action> {
    if let Some(name) = value.as_str() {
        return parse_action_name(name);
    }
    value
        .get("custom")
        .and_then(Value::as_u64)
        .map(crate::game::CustomActionId::new)
        .map(Action::Custom)
}

fn parse_action_name(name: &str) -> Option<Action> {
    let name = name.strip_prefix("Action.").unwrap_or(name);
    // Projectile dispatch snapshots the native action with `Debug`, so
    // custom/source states arrive as `Custom(CustomActionId(<u64>))` instead
    // of their canonical Source.* or Custom.* authoring spelling. Preserve
    // action filters for that route without accepting broader debug output.
    if let Some(value) = name
        .strip_prefix("Custom(CustomActionId(")
        .and_then(|value| value.strip_suffix("))"))
        .and_then(|value| value.parse::<u64>().ok())
    {
        return Some(Action::Custom(crate::game::CustomActionId::new(value)));
    }
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
            event_id: None,
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

    #[test]
    fn animation_event_selector_requires_and_matches_numeric_id() {
        let mut binding = binding(Hook::AnimationEvent);
        binding.event_id = Some(0);
        let selector = CallbackSelector::from_binding(&binding).unwrap();
        assert!(matches(
            Hook::AnimationEvent,
            &selector,
            &json!({"event":{"event_id":0}}),
            Some(Action::Wait),
            None,
        ));
        assert!(!matches(
            Hook::AnimationEvent,
            &selector,
            &json!({"event":{"event_id":1}}),
            Some(Action::Wait),
            None,
        ));

        binding.event_id = None;
        assert!(CallbackSelector::from_binding(&binding).is_err());
    }

    #[test]
    fn action_filter_uses_snapshot_only_when_event_has_no_action() {
        let mut binding = binding(Hook::AnimationEnded);
        binding.action = Some("Wait".into());
        let selector = CallbackSelector::from_binding(&binding).unwrap();

        assert!(matches(
            Hook::AnimationEnded,
            &selector,
            &json!({"event": {}}),
            Some(Action::Wait),
            None,
        ));
        assert!(!matches(
            Hook::AnimationEnded,
            &selector,
            &json!({"event": {"action": "SpecialNStart"}}),
            Some(Action::Wait),
            None,
        ));
    }

    #[test]
    fn action_filter_matches_custom_action_objects_without_snapshot_fallback() {
        let mut binding = binding(Hook::ActionEntered);
        binding.action = Some("Custom.test:phase".into());
        let selector = CallbackSelector::from_binding(&binding).unwrap();
        let custom_id = crate::game::script::custom_action_id("test", "phase").get();
        let other_id = crate::game::script::custom_action_id("test", "other").get();

        assert!(matches(
            Hook::ActionEntered,
            &selector,
            &json!({"event": {"to": {"custom": custom_id}}}),
            Some(Action::Wait),
            None,
        ));
        assert!(!matches(
            Hook::ActionEntered,
            &selector,
            &json!({"event": {"to": {"custom": other_id}}}),
            Some(Action::Custom(crate::game::CustomActionId::new(custom_id))),
            None,
        ));
    }

    #[test]
    fn projectile_debug_action_spelling_matches_custom_filter() {
        let mut binding = binding(Hook::ProjectileContact);
        binding.action = Some("Custom.test:phase".into());
        let selector = CallbackSelector::from_binding(&binding).unwrap();
        let custom_id = crate::game::script::custom_action_id("test", "phase").get();

        assert!(matches(
            Hook::ProjectileContact,
            &selector,
            &json!({
                "event": {
                    "action": format!("Custom(CustomActionId({custom_id}))")
                }
            }),
            None,
            None,
        ));
        assert!(!matches(
            Hook::ProjectileContact,
            &selector,
            &json!({
                "event": {
                    "action": "Custom(CustomActionId(7))"
                }
            }),
            Some(Action::Wait),
            None,
        ));
    }
}
