//! Generic fighter-script resource declarations.
//!
//! This module contains data only.  The simulation asks a definition for
//! action metadata and attack paths; it never dispatches on a character name.
//! The source program remains the sole owner of character policy.

use super::action_events::CountdownPhase;
use crate::game::{
    Action,
    script::{self, Error, Hook, Program},
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollisionMode {
    #[default]
    Normal,
    Clamp,
    Teeter,
}

impl CollisionMode {
    pub fn native(self) -> crate::fighter::edge::Mode {
        match self {
            Self::Normal => crate::fighter::edge::Mode::Plain,
            Self::Clamp => crate::fighter::edge::Mode::Clamp,
            Self::Teeter => crate::fighter::edge::Mode::Teeter,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionDefinition {
    /// Authoring class name for diagnostics and tooling. Runtime dispatch
    /// uses `action` and callback metadata; this field has no gameplay effect.
    #[serde(rename = "move", default)]
    pub move_name: Option<String>,
    /// Optional authoring resource label retained for validation tooling.
    #[serde(default)]
    pub resource: Option<String>,
    /// Canonical engine action reference emitted by the class contract (for
    /// example `Action.SPECIAL_N_START`). The map key remains the author's
    /// descriptive action name, so runtime lookup uses this stable reference.
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub id: Option<u16>,
    #[serde(default)]
    pub slippi_state: Option<u32>,
    #[serde(default)]
    pub animation: Option<u32>,
    #[serde(default)]
    pub collision_mode: CollisionMode,
    #[serde(default)]
    pub ledge_catchable: bool,
    #[serde(default)]
    pub wants_redirect: bool,
    #[serde(default)]
    pub hold_frame_var: Option<String>,
    /// Optional declared state field used as an action-specific delay gate.
    #[serde(default)]
    pub profile_delay_field: Option<String>,
    /// Whether finite animation samples cycle while the action remains active.
    /// Action frame and motion phase continue advancing independently.
    #[serde(default)]
    pub animation_loop: bool,
    #[serde(default)]
    pub attack: Option<String>,
    /// Optional native command trace resource for this action.  It is a
    /// generic descriptor field. The native bridge resolves this path from the
    /// cached action record and applies the declared command trace.
    #[serde(default)]
    pub command_trace: Option<String>,
    /// Stable behavior ID when this root descriptor was flattened from a
    /// behavior by the authoring exporter. Author-authored root descriptors
    /// leave this unset and are always linked strictly.
    #[serde(default)]
    pub source_behavior: Option<String>,
    /// Static action metadata retained for generic resource linkers. Motion
    /// constructors are decoded and linked at registration, never interpreted
    /// per frame.
    #[serde(default)]
    pub motion: Option<JsonValue>,
    #[serde(default)]
    pub metadata: BTreeMap<String, JsonValue>,
    #[serde(default)]
    pub callbacks: Vec<EventBinding>,
}

impl ActionDefinition {
    /// Resolve a callback binding through the typed hook enum.  Callers that
    /// already have an event kind avoid reconstructing or spelling callback
    /// names at runtime.
    pub fn callback(&self, hook: Hook) -> Option<&str> {
        self.callbacks_for_hook(hook)
            .next()
            .map(|binding| binding.callback.as_str())
    }

    pub fn callbacks_for_hook(&self, hook: Hook) -> impl Iterator<Item = &EventBinding> {
        self.callbacks
            .iter()
            .filter(move |binding| binding.hook == hook)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventBinding {
    pub hook: Hook,
    pub callback: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub marker: Option<String>,
    #[serde(default)]
    pub track: Option<String>,
    #[serde(default)]
    pub countdown: Option<String>,
    #[serde(default)]
    pub countdown_phase: CountdownPhase,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_button_mask")]
    pub buttons: Option<u16>,
    #[serde(default)]
    pub command_index: Option<u8>,
    #[serde(default)]
    pub deadline: Option<u32>,
    /// Optional native availability gate declared by a lifecycle callback.
    /// The bridge resolves this field against the registered fighter host.
    #[serde(default)]
    pub gate: Option<String>,
}

fn deserialize_button_mask<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };
    if let Some(mask) = value.as_u64() {
        return u16::try_from(mask)
            .map(Some)
            .map_err(|_| serde::de::Error::custom("button mask exceeds u16"));
    }
    let name = value
        .as_str()
        .and_then(|name| {
            if let Some((namespace, member)) = name.split_once('.') {
                (namespace == "Button").then_some(member)
            } else {
                Some(name)
            }
        })
        .ok_or_else(|| serde::de::Error::custom("buttons must be a mask or Button name"))?;
    let mask = match name {
        "A" => crate::game::BUTTON_A,
        "B" => crate::game::BUTTON_B,
        "X" => crate::game::BUTTON_X,
        "Y" => crate::game::BUTTON_Y,
        "Z" => crate::game::BUTTON_Z,
        "L" => crate::game::BUTTON_L,
        "R" => crate::game::BUTTON_R,
        "DPAD_LEFT" => crate::game::BUTTON_DPAD_LEFT,
        "DPAD_RIGHT" => crate::game::BUTTON_DPAD_RIGHT,
        "DPAD_DOWN" => crate::game::BUTTON_DPAD_DOWN,
        "DPAD_UP" => crate::game::BUTTON_DPAD_UP,
        _ => {
            return Err(serde::de::Error::custom(format!(
                "unknown Button name {name:?}"
            )));
        }
    };
    Ok(Some(mask))
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockBinding {
    pub field: String,
    #[serde(default)]
    pub actions: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorDefinition {
    /// Stable authoring identity for callback ownership and diagnostics.
    #[serde(default)]
    pub id: Option<String>,
    /// Optional native action used when an input selector enters this move.
    /// Missing means the move is callback-driven and has no canonical action.
    #[serde(default)]
    pub entry_action: Option<String>,
    /// Exported class move `run` callback identity, when present.
    #[serde(default)]
    pub run: Option<String>,
    /// Python module which defines the exported `run` method.  This is kept
    /// separately from its qualname because inherited methods may live in a
    /// different bundled module than the fighter root.
    #[serde(default)]
    pub run_module: Option<String>,
    /// Root resource which enables this behavior. A missing optional root
    /// disables its callbacks and validator for the registered fighter.
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub actions: BTreeMap<String, ActionDefinition>,
    #[serde(default)]
    pub callbacks: Vec<EventBinding>,
    #[serde(default)]
    pub clocks: Vec<ClockBinding>,
    #[serde(default)]
    pub validate: Option<String>,
}

impl BehaviorDefinition {
    pub fn callback(&self, hook: Hook) -> Option<&str> {
        self.callbacks_for_hook(hook)
            .next()
            .map(|binding| binding.callback.as_str())
    }

    pub fn callbacks_for_hook(&self, hook: Hook) -> impl Iterator<Item = &EventBinding> {
        self.callbacks
            .iter()
            .filter(move |binding| binding.hook == hook)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FighterDefinition {
    pub name: String,
    #[serde(default)]
    pub external_ids: Vec<u8>,
    #[serde(default)]
    pub parameters: BTreeMap<String, JsonValue>,
    #[serde(default)]
    pub state: script::StateSchema,
    #[serde(default)]
    pub action_state: script::StateSchema,
    #[serde(default)]
    pub behaviors: Vec<BehaviorDefinition>,
    #[serde(default)]
    pub actions: BTreeMap<String, ActionDefinition>,
    #[serde(default)]
    pub callbacks: Vec<EventBinding>,
    #[serde(default)]
    pub projectile_contact_resource: Option<String>,
    /// Class API grouping metadata. The engine does not dispatch through this
    /// map, but retaining it makes native metadata round trips lossless.
    #[serde(default)]
    pub movesets: BTreeMap<String, JsonValue>,
    /// Runtime-only action ownership/index data. It is built on first use so
    /// deserialized metadata remains lossless while dispatch avoids parsing
    /// authoring names on every frame.
    #[serde(skip, default = "default_definition_indexes")]
    pub(crate) indexes: Arc<DefinitionIndexes>,
}

fn default_definition_indexes() -> Arc<DefinitionIndexes> {
    Arc::new(DefinitionIndexes::default())
}

#[derive(Debug, Default)]
pub(crate) struct DefinitionIndexes {
    built: OnceLock<IndexedDefinition>,
}

impl PartialEq for DefinitionIndexes {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for DefinitionIndexes {}

#[derive(Debug)]
struct IndexedDefinition {
    top_actions: Vec<Option<String>>,
    behavior_actions: Vec<Vec<(String, usize)>>,
    behavior_owners: Vec<Vec<usize>>,
}

const ACTION_INDEX_SIZE: usize = Action::Eliminated as usize + 1;

impl DefinitionIndexes {
    fn get(&self, definition: &FighterDefinition) -> &IndexedDefinition {
        self.built.get_or_init(|| {
            let mut top_actions = vec![None; ACTION_INDEX_SIZE];
            for (key, action) in &definition.actions {
                if let Some(action) = parse_definition_action(key, action)
                    && (top_actions[action as usize].is_none() || key == &action_name(action))
                {
                    top_actions[action as usize] = Some(key.clone());
                }
            }
            let behavior_actions: Vec<Vec<(String, usize)>> = definition
                .behaviors
                .iter()
                .map(|behavior| {
                    let mut actions = Vec::new();
                    for (key, definition) in &behavior.actions {
                        if let Some(action) = parse_definition_action(key, definition) {
                            if let Some(existing) = actions
                                .iter_mut()
                                .find(|(_, indexed)| *indexed == action as usize)
                            {
                                if key == &action_name(action) {
                                    existing.0 = key.clone();
                                }
                            } else {
                                actions.push((key.clone(), action as usize));
                            }
                        }
                    }
                    actions
                })
                .collect();
            let mut behavior_owners = vec![Vec::new(); definition.behaviors.len()];
            for (index, behavior) in definition.behaviors.iter().enumerate() {
                if let Some(action) = behavior.entry_action.as_deref().and_then(parse_action_ref) {
                    behavior_owners[index].push(action as usize);
                }
                for (_, action) in &behavior_actions[index] {
                    if !behavior_owners[index].contains(action) {
                        behavior_owners[index].push(*action);
                    }
                }
            }
            IndexedDefinition {
                top_actions,
                behavior_actions,
                behavior_owners,
            }
        })
    }
}

fn parse_action_ref(name: &str) -> Option<Action> {
    script::parse_action(name.strip_prefix("Action.").unwrap_or(name))
}

fn parse_definition_action(key: &str, definition: &ActionDefinition) -> Option<Action> {
    definition
        .action
        .as_deref()
        .and_then(parse_action_ref)
        .or_else(|| parse_action_ref(key))
}

impl FighterDefinition {
    pub(crate) fn behavior_owns_action(&self, index: usize, action: Action) -> bool {
        self.indexes
            .get(self)
            .behavior_owners
            .get(index)
            .is_some_and(|actions| actions.contains(&(action as usize)))
    }

    /// Warm all immutable action metadata indexes at resource registration.
    /// This keeps authoring-name parsing out of `Match::step` entirely.
    pub(crate) fn prepare_runtime_indexes(&self) {
        self.indexes.get(self);
    }

    pub fn action(&self, action: Action) -> Option<&ActionDefinition> {
        let indexed = self.indexes.get(self);
        indexed.top_actions[action as usize]
            .as_deref()
            .and_then(|key| self.actions.get(key))
            .or_else(|| {
                indexed
                    .behavior_actions
                    .iter()
                    .enumerate()
                    .find_map(|(index, actions)| {
                        actions
                            .iter()
                            .find(|(_, indexed)| *indexed == action as usize)
                            .and_then(|(key, _)| {
                                self.behaviors
                                    .get(index)
                                    .and_then(|behavior| behavior.actions.get(key))
                            })
                    })
            })
    }

    pub fn action_for_owner(&self, owner: usize, action: Action) -> Option<&ActionDefinition> {
        let indexed = self.indexes.get(self);
        let key = indexed
            .behavior_actions
            .get(owner)?
            .iter()
            .find(|(_, indexed)| *indexed == action as usize)
            .map(|(key, _)| key)?;
        self.behaviors.get(owner)?.actions.get(key)
    }

    pub fn callback(&self, hook: Hook) -> Option<&str> {
        self.callbacks_for_hook(hook)
            .next()
            .map(|binding| binding.callback.as_str())
    }

    pub fn callbacks_for_hook(&self, hook: Hook) -> impl Iterator<Item = &EventBinding> {
        self.callbacks
            .iter()
            .filter(move |binding| binding.hook == hook)
    }

    #[allow(dead_code)] // Compatibility resolver retained for action-only callers.
    pub(crate) fn behavior_callbacks(
        &self,
        program: &Program,
        hook: Hook,
        action: Action,
        resources: Option<&crate::game::script::lifecycle_resources::ResourceCache>,
        context: &JsonValue,
    ) -> Vec<OwnedCallback> {
        self.behavior_callbacks_for_owner(program, hook, action, None, resources, context)
    }

    /// Resolve callbacks for a native action transition with an optional
    /// move owner.  A canonical action is shared by several authoring moves
    /// in composed fighters, so action alone cannot identify the callback
    /// owner.  `owner` is the immutable behavior index selected at entry;
    /// callers persist it with the action generation and pass it back for
    /// action lifecycle hooks.
    pub(crate) fn behavior_callbacks_for_owner(
        &self,
        program: &Program,
        hook: Hook,
        action: Action,
        owner: Option<usize>,
        resources: Option<&crate::game::script::lifecycle_resources::ResourceCache>,
        context: &JsonValue,
    ) -> Vec<OwnedCallback> {
        let action_enum = action;
        let mut result = Vec::new();
        let matches_binding =
            |binding: &crate::game::script::callback_routing::ResolvedCallback| {
                crate::game::script::callback_routing::matches(
                    hook,
                    &binding.selector,
                    context,
                    Some(action_enum),
                    resources,
                )
            };
        result.extend(
            program
                .callback_bindings(None, hook)
                .iter()
                .filter(|binding| matches_binding(binding))
                .map(|binding| OwnedCallback {
                    callback: binding.callback.clone(),
                    resource: None,
                    behavior_index: None,
                }),
        );
        let all_behaviors = matches!(
            hook.argument_contract(),
            script::HookArgumentContract::FighterHit
        ) || matches!(
            hook,
            Hook::InputPressed
                | Hook::InputReleased
                | Hook::StickChanged
                | Hook::ActionAvailabilityChanged
        );
        // An explicitly selected owner is authoritative for action hooks.
        // Do not scan behavior order in that case: two distinct moves may
        // intentionally share the same canonical Action.
        if let Some(owner) = owner.filter(|_| !all_behaviors) {
            let Some(behavior) = self.behaviors.get(owner) else {
                return result;
            };
            if self.behavior_enabled(owner, resources) {
                result.extend(
                    program
                        .callback_bindings(Some(owner), hook)
                        .iter()
                        .filter(|binding| matches_binding(binding))
                        .map(|binding| OwnedCallback {
                            callback: binding.callback.clone(),
                            resource: behavior.resource.clone(),
                            behavior_index: Some(owner),
                        }),
                );
            }
            return result;
        }

        let mut owns_action = false;
        for index in 0..self.behaviors.len() {
            let behavior = &self.behaviors[index];
            let owns = self
                .indexes
                .get(self)
                .behavior_owners
                .get(index)
                .is_some_and(|actions| actions.contains(&(action_enum as usize)));
            if self.behavior_enabled(index, resources) && (all_behaviors || (!owns_action && owns))
            {
                owns_action = true;
                result.extend(
                    program
                        .callback_bindings(Some(index), hook)
                        .iter()
                        .filter(|binding| matches_binding(binding))
                        .map(|binding| OwnedCallback {
                            callback: binding.callback.clone(),
                            resource: behavior.resource.clone(),
                            behavior_index: Some(index),
                        }),
                );
                if !all_behaviors {
                    break;
                }
            }
        }
        result
    }

    fn behavior_enabled(
        &self,
        index: usize,
        resources: Option<&crate::game::script::lifecycle_resources::ResourceCache>,
    ) -> bool {
        let Some(path) = self.behaviors[index].resource.as_deref() else {
            return true;
        };
        resources.is_some_and(|resources| resources.value(path).is_some())
    }
}

pub(crate) struct OwnedCallback {
    pub callback: crate::game::script::starlark::CallbackHandle,
    pub resource: Option<String>,
    pub behavior_index: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Definition {
    pub manifest: Arc<FighterDefinition>,
    pub program: Arc<Program>,
    pub dependency_sources: BTreeMap<String, String>,
    pub move_registry: Arc<super::move_registry::MoveRegistry>,
}

impl Definition {
    /// Load a gameplay definition. The Pon gameplay backend currently reports
    /// an explicit unavailable error at this boundary.
    pub fn load(source: impl Into<String>) -> Result<Self, Error> {
        Self::load_registered(source, &AssetStore::default())
    }

    pub fn load_registered(source: impl Into<String>, assets: &AssetStore) -> Result<Self, Error> {
        let source = source.into();
        let program = Arc::new(Program::new_registered(source.clone(), assets)?);
        let manifest = program.metadata_arc();
        Ok(Self {
            manifest,
            move_registry: program.moves_arc(),
            program,
            dependency_sources: assets.dependencies(&source)?,
        })
    }

    pub fn action(&self, action: Action) -> Option<&ActionDefinition> {
        self.manifest.action(action)
    }

    pub fn collision_mode(&self, action: Action) -> Option<crate::fighter::edge::Mode> {
        self.action(action).map(|d| d.collision_mode.native())
    }

    pub fn ledge_catchable(&self, action: Action) -> bool {
        self.action(action).is_some_and(|d| d.ledge_catchable)
    }

    pub fn slippi_ids(&self, action: Action) -> Option<(u32, u32)> {
        self.action(action)
            .and_then(|d| Some((d.slippi_state?, d.animation?)))
    }

    /// Resolve the authoring state's Slippi id without requiring an
    /// animation id.  Some source-authored action states are observable in
    /// replay even while their animation resource is intentionally absent.
    pub fn slippi_state(&self, action: Action) -> Option<u32> {
        self.action(action).and_then(|definition| definition.slippi_state)
    }
}

/// Sources available to `load`. No filesystem/module search is
/// performed; callers register the exact embedded resources they permit.
#[derive(Clone, Default)]
pub struct AssetStore {
    sources: BTreeMap<String, String>,
    shared_sources: BTreeMap<String, String>,
}

impl AssetStore {
    pub fn register(&mut self, name: impl Into<String>, source: impl Into<String>) {
        self.sources.insert(name.into(), source.into());
    }
    pub fn get(&self, name: &str) -> Option<&str> {
        self.sources.get(name).map(String::as_str)
    }

    /// Register a shared authoring library. Shared libraries are available to
    /// the loader, but are kept out of the fighter import namespace.
    pub fn register_shared(&mut self, name: impl Into<String>, source: impl Into<String>) {
        self.shared_sources.insert(name.into(), source.into());
    }

    pub fn shared(&self, name: &str) -> Option<&str> {
        self.shared_sources.get(name).map(String::as_str)
    }

    pub fn shared_sources(&self) -> &BTreeMap<String, String> {
        &self.shared_sources
    }

    pub fn dependencies(&self, _source: &str) -> Result<BTreeMap<String, String>, Error> {
        // Retain every registered source so resource identity remains
        // deterministic when a future Pon resource pack adds modules.
        let mut dependencies = self.sources.clone();
        // Prefix shared modules so a same-named private module cannot collide
        // with an engine resource in cache keys.
        for (name, source) in &self.shared_sources {
            dependencies.insert(format!("shared/{name}"), source.clone());
        }
        Ok(dependencies)
    }

    pub fn builtins() -> Self {
        let mut assets = Self::default();
        assets.register(
            "captain.py",
            include_str!("../../../scripts/fighters/captain.py"),
        );
        assets.register("fox.py", include_str!("../../../scripts/fighters/fox.py"));
        assets.register(
            "falco.py",
            include_str!("../../../scripts/fighters/falco.py"),
        );
        assets.register_shared(
            "common.py",
            include_str!("../../../scripts/fighters/common.py"),
        );
        assets
    }
}

/// `Debug` spelling is the stable bridge until the engine's Action enum gains
/// an explicit wire-name method.  It intentionally does not depend on Fox.
fn action_name(action: Action) -> String {
    format!("{action:?}")
}

fn cached_source(source: &str) -> Result<Arc<Definition>, Error> {
    type CacheKey = (
        &'static str,
        String,
        Vec<(String, String)>,
        Vec<(String, String)>,
    );
    static CACHE: OnceLock<Mutex<BTreeMap<CacheKey, Arc<Definition>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let assets = AssetStore::builtins();
    let key = (
        // Bump this whenever the frontend, engine namespace, or descriptor
        // lowering changes.  A source-only cache would otherwise reuse a
        // definition produced under an older ABI.
        "pon-definition-v2",
        source.to_owned(),
        assets
            .shared_sources()
            .iter()
            .map(|(name, source)| (name.clone(), source.clone()))
            .collect(),
        assets.dependencies(source)?.into_iter().collect(),
    );
    let mut cache = cache
        .lock()
        .map_err(|_| Error::Runtime("definition cache poisoned".into()))?;
    if let Some(definition) = cache.get(&key) {
        return Ok(Arc::clone(definition));
    }
    let definition = Definition::load_registered(source, &assets)?;
    // A match resource can contain arbitrary embedded scripts. Bound the
    // metadata cache so repeated resource loads cannot grow process memory.
    const MAX_CACHED_DEFINITIONS: usize = 128;
    if cache.len() >= MAX_CACHED_DEFINITIONS
        && let Some(oldest) = cache.keys().next().cloned()
    {
        cache.remove(&oldest);
    }
    let definition = Arc::new(definition);
    cache.insert(key, Arc::clone(&definition));
    Ok(definition)
}

/// Resolve a bundled program at registration time. Every source failure
/// is returned to the match constructor; malformed bundled resources must not
/// silently turn into a fighter with scripting disabled.
pub(crate) fn program_for_registration(
    data: &crate::game::data::FighterData,
) -> Result<Option<Arc<Program>>, Error> {
    let program = if let Some(program) = data.script.as_ref() {
        Arc::new(program.clone())
    } else {
        let Some(source) = script::bundled_source(data.specials.as_ref()) else {
            return Ok(None);
        };
        Arc::clone(&cached_source(source)?.program)
    };
    // The definition cache is process-global, but Pon preparation is
    // thread-local. Re-prepare a cached immutable program when registration
    // occurs on a new loading/session thread; runtime cached_program callers
    // never take this path.
    program.prepare_for_current_thread()?;
    Ok(Some(program))
}

/// Return the immutable linked program retained by match registration.
/// Runtime callers intentionally do not resolve source assets or compile a
/// fallback; an absent cache means this fighter has no registered program.
pub(crate) fn cached_program(data: &crate::game::data::FighterData) -> Option<Arc<Program>> {
    // Match construction retains the selected immutable program beside its
    // resource projection. Reuse that Arc before considering source assets;
    // this keeps callback dispatch independent of source text and program
    // cloning once a fighter has been registered.
    data.script_resources
        .get()
        .and_then(|cache| cache.program())
}

fn with_action_definition<T>(
    action: Action,
    data: &crate::game::data::FighterData,
    f: impl FnOnce(&ActionDefinition) -> Option<T>,
) -> Option<T> {
    let program = cached_program(data)?;
    f(program.metadata().action(action)?)
}

pub(crate) fn collision_mode(
    action: Action,
    data: &crate::game::data::FighterData,
) -> Option<crate::fighter::edge::Mode> {
    with_action_definition(action, data, |definition| {
        Some(definition.collision_mode.native())
    })
}

pub(crate) fn ledge_catchable(action: Action, data: &crate::game::data::FighterData) -> bool {
    with_action_definition(action, data, |definition| Some(definition.ledge_catchable))
        .unwrap_or(false)
}

pub(crate) fn wants_redirect(action: Action, data: &crate::game::data::FighterData) -> bool {
    with_action_definition(action, data, |definition| Some(definition.wants_redirect))
        .unwrap_or(false)
}

pub(crate) fn attack(
    action: Action,
    data: &crate::game::data::FighterData,
) -> Option<&crate::game::data::Attack> {
    let path = with_action_definition(action, data, |definition| definition.attack.clone())?;
    data.specials.as_ref()?.attack(path.as_str())
}

#[allow(dead_code)]
pub(crate) fn command_trace_for_owner(
    action: Action,
    owner: Option<usize>,
    data: &crate::game::data::FighterData,
) -> Option<String> {
    let program = cached_program(data)?;
    match owner {
        Some(owner) => program
            .metadata()
            .action_for_owner(owner, action)
            .and_then(|definition| definition.command_trace.clone()),
        None => program
            .metadata()
            .action(action)
            .and_then(|definition| definition.command_trace.clone()),
    }
}

pub(crate) fn hold_frame_var(
    action: Action,
    data: &crate::game::data::FighterData,
) -> Option<String> {
    with_action_definition(action, data, |definition| definition.hold_frame_var.clone())
}

pub(crate) fn projectile_contact_resource(data: &crate::game::data::FighterData) -> Option<String> {
    cached_program(data)?
        .metadata()
        .projectile_contact_resource
        .clone()
}

#[derive(Default)]
pub struct Registry {
    by_name: BTreeMap<String, Definition>,
    by_id: BTreeMap<u8, String>,
}

impl Registry {
    pub fn builtins() -> Result<Self, Error> {
        let assets = AssetStore::builtins();
        let mut registry = Self::default();
        for name in ["captain.py", "fox.py", "falco.py"] {
            let source = assets
                .get(name)
                .ok_or_else(|| Error::Invalid(format!("missing builtin {name}")))?;
            registry.insert(Definition::load_registered(source, &assets)?)?;
        }
        Ok(registry)
    }

    pub fn insert(&mut self, definition: Definition) -> Result<(), Error> {
        let name = definition.manifest.name.clone();
        if self.by_name.contains_key(&name) {
            return Err(Error::Invalid(format!("duplicate fighter name {name:?}")));
        }
        // Validate the complete insertion before mutating either index. This
        // keeps a rejected definition from leaving stale external-id routes.
        let mut ids = std::collections::BTreeSet::new();
        for id in &definition.manifest.external_ids {
            if !ids.insert(*id) || self.by_id.contains_key(id) {
                return Err(Error::Invalid(format!(
                    "duplicate fighter external id {id}"
                )));
            }
        }
        for id in &definition.manifest.external_ids {
            self.by_id.insert(*id, name.clone());
        }
        self.by_name.insert(name, definition);
        Ok(())
    }
    pub fn get(&self, name: &str) -> Option<&Definition> {
        self.by_name.get(name)
    }
    pub fn by_external_id(&self, id: u8) -> Option<&Definition> {
        self.by_id.get(&id).and_then(|name| self.get(name))
    }

    pub fn resolve(&self, name: &str) -> Result<Definition, Error> {
        // Composition is expressed by explicit module metadata. Registry
        // resolution therefore only looks up the
        // already materialized descriptor; implicit manifest inheritance
        // would make callback ownership and cache keys ambiguous.
        self.get(name)
            .cloned()
            .ok_or_else(|| Error::Invalid(format!("unknown fighter {name}")))
    }
}

/// Resolve a Slippi state/animation pair through the bundled fighter
/// definitions. The registry is initialized once so observation remains a
/// cheap read during frame streaming.
pub fn builtin_slippi_ids(character: Option<u8>, action: Action) -> Option<(u32, u32)> {
    let id = character?;
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY
        .get_or_init(|| Registry::builtins().expect("bundled fighter definitions must load"))
        .by_external_id(id)
        .and_then(|definition| definition.slippi_ids(action))
}

/// Resolve a bundled fighter's Slippi action-state id without requiring an
/// animation id.  Keep this separate from [`builtin_slippi_ids`], whose pair
/// contract remains useful to callers that need both replay identifiers.
pub fn builtin_slippi_state(character: Option<u8>, action: Action) -> Option<u32> {
    let id = character?;
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY
        .get_or_init(|| Registry::builtins().expect("bundled fighter definitions must load"))
        .by_external_id(id)
        .and_then(|definition| definition.slippi_state(action))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_definition(action: Option<&str>, resource: &str) -> ActionDefinition {
        ActionDefinition {
            action: action.map(str::to_owned),
            resource: Some(resource.to_owned()),
            ..ActionDefinition::default()
        }
    }

    #[test]
    fn aliased_behavior_descriptor_survives_entry_action_indexing() {
        let mut definition = FighterDefinition::default();
        definition.behaviors.push(BehaviorDefinition {
            entry_action: Some("Action.SpecialNStart".into()),
            actions: BTreeMap::from([(
                "startup".into(),
                action_definition(Some("Action.SpecialNStart"), "startup-resource"),
            )]),
            ..BehaviorDefinition::default()
        });
        definition.prepare_runtime_indexes();
        assert_eq!(
            definition
                .action(Action::SpecialNStart)
                .and_then(|action| action.resource.as_deref()),
            Some("startup-resource")
        );
        assert_eq!(
            definition.indexes.get(&definition).behavior_owners[0],
            vec![Action::SpecialNStart as usize]
        );
    }

    #[test]
    fn canonical_action_key_wins_over_alias_collision() {
        let definition = FighterDefinition {
            actions: BTreeMap::from([
                (
                    "alias".into(),
                    action_definition(Some("Action.SpecialNStart"), "alias-resource"),
                ),
                (
                    "SpecialNStart".into(),
                    action_definition(Some("Action.SpecialNStart"), "canonical-resource"),
                ),
            ]),
            ..FighterDefinition::default()
        };
        definition.prepare_runtime_indexes();
        assert_eq!(
            definition
                .action(Action::SpecialNStart)
                .and_then(|action| action.resource.as_deref()),
            Some("canonical-resource")
        );
    }

    #[test]
    fn same_action_behaviors_keep_declaration_order() {
        let mut definition = FighterDefinition::default();
        for owner in ["first", "second"] {
            definition.behaviors.push(BehaviorDefinition {
                entry_action: Some("Action.SpecialNStart".into()),
                id: Some(owner.into()),
                ..BehaviorDefinition::default()
            });
        }
        definition.prepare_runtime_indexes();
        assert_eq!(
            definition
                .indexes
                .get(&definition)
                .behavior_owners
                .iter()
                .map(|actions| actions[0])
                .collect::<Vec<_>>(),
            vec![
                Action::SpecialNStart as usize,
                Action::SpecialNStart as usize,
            ]
        );
    }

    #[test]
    fn slippi_state_does_not_require_animation_id() {
        let definition = FighterDefinition {
            actions: BTreeMap::from([(
                "SpecialNStart".into(),
                ActionDefinition {
                    action: Some("Action.SpecialNStart".into()),
                    slippi_state: Some(347),
                    ..ActionDefinition::default()
                }),
            ]),
            ..FighterDefinition::default()
        };
        assert_eq!(
            definition
                .action(Action::SpecialNStart)
                .and_then(|action| action.slippi_state),
            Some(347)
        );
        assert_eq!(
            definition
                .action(Action::SpecialNStart)
                .and_then(|action| Some((action.slippi_state?, action.animation?))),
            None
        );
    }
}
