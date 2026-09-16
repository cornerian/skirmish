//! Deterministic native character-script boundary.
//!
//! Scripts communicate through plain tables and a small validated result. A
//! VM is retained only in the immutable compiled program; checkpointing never
//! has to capture a foreign interpreter heap. The simulation owns applying the returned
//! commands and hit patch transactionally.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub mod action_events;
#[cfg(feature = "experimental-continuations")]
pub mod async_move;
pub(crate) mod attributes;
pub(crate) mod builtins;
pub(crate) mod callback_routing;
pub mod events;
pub mod lifecycle;
pub(crate) mod lifecycle_host;
pub(crate) mod lifecycle_movement;
pub mod lifecycle_resources;
pub(crate) mod lifecycle_state;
pub mod motion;
pub mod motion_resources;
pub(crate) mod native_math;
pub mod scheduler;

pub mod definition;
pub(crate) mod identity;
pub mod move_registry;
pub mod move_selection;
pub(crate) mod move_validation;
pub mod resources;
pub use skirmish_script_runtime as starlark;
pub use starlark::{HookArgumentContract, HookKind as Hook};

/// Hard limits for one script dispatch. These are deliberately fixed until
/// resource validation grows a script-resource section.
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;
pub const MAX_INSTRUCTIONS: u32 = 100_000;
pub const MAX_MEMORY_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_LOCALS: usize = 128;
pub const MAX_LOCAL_KEY_BYTES: usize = 64;
pub const MAX_LOCAL_STRING_BYTES: usize = 256;
pub const MAX_COMMANDS: usize = 16;

/// Selects the bundled source owned by each fighter when its reflector exists.
pub fn bundled_source(specials: Option<&resources::Specials>) -> Option<&'static str> {
    match specials?.character_key().as_str() {
        "captain-falcon" => Some(include_str!("../../scripts/fighters/captain.py")),
        "fox" => Some(include_str!("../../scripts/fighters/fox.py")),
        "falco" => Some(include_str!("../../scripts/fighters/falco.py")),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct Program {
    source: Arc<str>,
    dependency_sources: Arc<BTreeMap<String, String>>,
    /// Native callback state, when a backend is available, is retained so
    /// match registration can share immutable resource metadata.
    pub(crate) compiled: Option<std::sync::Arc<starlark::CompiledProgram>>,
    hook_indices: [Option<usize>; Hook::COUNT],
    callback_bindings: [Vec<callback_routing::ResolvedCallback>; Hook::COUNT],
    /// Callback handles nested below `fighter.behaviors`.  These are indexed
    /// once at resource load; lifecycle dispatch can then select an owner by
    /// action without looking up a Starlark name.
    behavior_bindings: Vec<[Vec<callback_routing::ResolvedCallback>; Hook::COUNT]>,
    /// Parsed descriptor retained beside the frozen module so metadata users
    /// never rebuild it from source during gameplay.
    metadata: Arc<crate::game::script::definition::FighterDefinition>,
    /// Immutable moveset links compiled beside metadata at resource load.
    move_registry: Arc<move_registry::MoveRegistry>,
}

impl PartialEq for Program {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.dependency_sources == other.dependency_sources
    }
}

impl Eq for Program {}

impl Serialize for Program {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct Wire<'a> {
            version: &'static str,
            source: &'a str,
            dependencies: &'a BTreeMap<String, String>,
        }
        Wire {
            version: "pon-v2",
            source: &self.source,
            dependencies: &self.dependency_sources,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Program {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Deserialization is a resource-load boundary.  Compile here so a
        // malformed embedded program cannot reach match construction and so
        // every clone of a loaded program can share its immutable compiled
        // representation once the Starlark backend is attached.
        let value = serde_json::Value::deserialize(deserializer)?;
        let (source, dependencies) = if let Some(source) = value.as_str() {
            (source.to_owned(), BTreeMap::new())
        } else {
            #[derive(Deserialize)]
            struct Wire {
                version: String,
                source: String,
                dependencies: BTreeMap<String, String>,
            }
            let wire: Wire = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
            if wire.version != "pon-v2" {
                return Err(serde::de::Error::custom("unsupported Pon program version"));
            }
            (wire.source, wire.dependencies)
        };
        let mut assets = crate::game::script::definition::AssetStore::default();
        for (name, source) in dependencies {
            assets.register(name, source);
        }
        Self::new_registered(source, &assets).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum LocalValue {
    Bool(bool),
    Integer(i64),
    Number(f64),
    String(String),
    Tuple(Vec<LocalValue>),
}

impl<'de> Deserialize<'de> for LocalValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Starlark enables serde_json's arbitrary-precision feature. Direct
        // untagged deserialization then attempts each scalar visitor against
        // the buffered number representation and can reject valid fractions.
        // Buffer the scalar as JSON first so integer-vs-number selection uses
        // Number's lossless classification while preserving the wire format.
        let value = serde_json::Value::deserialize(deserializer)?;
        local_value_from_json(&value).ok_or_else(|| {
            serde::de::Error::custom("local state value must be a scalar or fixed tuple")
        })
    }
}

pub type LocalState = BTreeMap<String, LocalValue>;

/// The scalar types allowed in rollback-persistent character state.  A
/// A gameplay module declares these fields once; callback writes are checked
/// against the declaration before the staged fighter is committed.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateType {
    Bool,
    Integer,
    Number,
    String,
    FixedTuple {
        element: Box<StateType>,
        length: usize,
    },
}

impl StateType {
    pub fn of(value: &LocalValue) -> Self {
        match value {
            LocalValue::Bool(_) => Self::Bool,
            LocalValue::Integer(_) => Self::Integer,
            LocalValue::Number(_) => Self::Number,
            LocalValue::String(_) => Self::String,
            LocalValue::Tuple(values) => {
                let element = values.first().map(Self::of).unwrap_or(Self::Integer);
                Self::FixedTuple {
                    element: Box::new(element),
                    length: values.len(),
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateField {
    #[serde(rename = "type")]
    pub value_type: StateType,
    pub default: LocalValue,
}

impl StateField {
    pub fn from_default(default: LocalValue) -> Self {
        Self {
            value_type: StateType::of(&default),
            default,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StateSchema {
    pub fields: BTreeMap<String, StateField>,
}

impl<'de> Deserialize<'de> for StateSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("state schema must be an object"))?;
        let fields = if let Some(fields) = object.get("fields") {
            serde_json::from_value::<BTreeMap<String, StateField>>(fields.clone())
                .map_err(serde::de::Error::custom)?
        } else {
            object
                .iter()
                .map(|(name, value)| {
                    let value = local_value_from_json(value).ok_or_else(|| {
                        serde::de::Error::custom(format!(
                            "state field {name:?} must be a scalar or fixed tuple"
                        ))
                    })?;
                    Ok((name.clone(), StateField::from_default(value)))
                })
                .collect::<Result<BTreeMap<_, _>, D::Error>>()?
        };
        Ok(Self { fields })
    }
}

fn local_value_from_json(value: &serde_json::Value) -> Option<LocalValue> {
    match value {
        serde_json::Value::Bool(value) => Some(LocalValue::Bool(*value)),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(LocalValue::Integer)
            .or_else(|| value.as_f64().map(LocalValue::Number)),
        serde_json::Value::String(value) => Some(LocalValue::String(value.clone())),
        serde_json::Value::Array(values) => values
            .iter()
            .map(local_value_from_json)
            .collect::<Option<Vec<_>>>()
            .map(LocalValue::Tuple),
        _ => None,
    }
}

fn native_to_json(value: &starlark::NativeValue) -> Result<serde_json::Value, Error> {
    Ok(match value {
        starlark::NativeValue::None => serde_json::Value::Null,
        starlark::NativeValue::Bool(value) => serde_json::Value::Bool(*value),
        starlark::NativeValue::Int(value) => serde_json::json!(*value),
        starlark::NativeValue::F32(value) => serde_json::json!(*value),
        starlark::NativeValue::String(value) => serde_json::Value::String(value.clone()),
        starlark::NativeValue::Vec2(value) => serde_json::json!([value[0], value[1]]),
        starlark::NativeValue::Object(_) => {
            return Err(Error::Invalid("export contains a native object".into()));
        }
        starlark::NativeValue::List(values) => serde_json::Value::Array(
            values
                .iter()
                .map(native_to_json)
                .collect::<Result<_, _>>()?,
        ),
        starlark::NativeValue::Tuple(values) => serde_json::Value::Array(
            values
                .iter()
                .map(native_to_json)
                .collect::<Result<_, _>>()?,
        ),
        starlark::NativeValue::Dict(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| Ok((key.clone(), native_to_json(value)?)))
                .collect::<Result<_, Error>>()?,
        ),
    })
}

#[cfg(test)]
mod program_wire_tests {
    use super::Program;

    #[test]
    fn unsupported_program_wire_version_is_rejected_before_compile() {
        let value = serde_json::json!({
            "version": "pon-v0",
            "source": "class Broken: pass",
            "dependencies": {}
        });
        let error = serde_json::from_value::<Program>(value).expect_err("version must fail closed");
        assert!(
            error
                .to_string()
                .contains("unsupported Pon program version")
        );
    }
}

impl StateSchema {
    pub fn validate_declaration(&self) -> Result<(), Error> {
        for (key, field) in &self.fields {
            if key.is_empty() {
                return Err(Error::Invalid("state field name cannot be empty".into()));
            }
            if key.len() > MAX_LOCAL_KEY_BYTES {
                return Err(Error::Invalid(format!("state field {key:?} is too long")));
            }
            if !state_value_matches(field.value_type.clone(), &field.default) {
                return Err(Error::Invalid(format!(
                    "state field {key:?} default has the wrong type"
                )));
            }
        }
        self.validate(&self.defaults())
    }

    pub fn validate(&self, state: &LocalState) -> Result<(), Error> {
        validate_state(state)?;
        for (key, value) in state {
            let field = self.fields.get(key).ok_or_else(|| {
                Error::Invalid(format!("undeclared persistent state field {key:?}"))
            })?;
            if !state_value_matches(field.value_type.clone(), value) {
                return Err(Error::Invalid(format!(
                    "persistent state field {key:?} has the wrong type"
                )));
            }
        }
        Ok(())
    }

    pub fn defaults(&self) -> LocalState {
        self.fields
            .iter()
            .map(|(key, field)| (key.clone(), field.default.clone()))
            .collect()
    }
}

pub(crate) fn state_value_matches(expected: StateType, value: &LocalValue) -> bool {
    match (expected, value) {
        (StateType::Bool, LocalValue::Bool(_))
        | (StateType::Integer, LocalValue::Integer(_))
        | (StateType::Number, LocalValue::Number(_))
        | (StateType::String, LocalValue::String(_)) => true,
        (StateType::FixedTuple { element, length }, LocalValue::Tuple(values)) => {
            values.len() == length
                && values
                    .iter()
                    .all(|v| state_value_matches(*element.clone(), v))
        }
        _ => false,
    }
}

/// Input behavior precedence is part of the authoring ABI.  A declaration is
/// visited in this order and each active owner runs at most once for the
/// input event, which keeps composed fighters deterministic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputBehavior {
    Side,
    Up,
    Neutral,
    Down,
}

impl InputBehavior {
    pub const ORDER: &'static [Self] = &[Self::Side, Self::Up, Self::Neutral, Self::Down];
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FighterView {
    pub id: u8,
    pub action: String,
    pub action_frame: u32,
    pub velocity: [f32; 2],
    pub ground_velocity: f32,
    pub grounded: bool,
    pub percent: f32,
    pub hitlag: f32,
    pub hitstun: u32,
    pub flags: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HitView {
    pub frame: u32,
    pub attacker: u8,
    pub defender: u8,
    pub damage: f32,
    pub angle: f32,
    pub base_knockback: u32,
    pub knockback_growth: u32,
    pub knockback: f32,
    pub hitbox_group: u8,
    pub projectile: bool,
    pub max_damage: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitPatch {
    pub cancelled: bool,
    pub damage: f32,
    pub angle: f32,
    pub knockback: f32,
    pub apply_damage: bool,
    pub apply_knockback: bool,
    pub apply_hitlag: bool,
    pub apply_hitstun: bool,
    #[serde(default)]
    pub reflect: bool,
}

impl Default for HitPatch {
    fn default() -> Self {
        Self {
            cancelled: false,
            damage: 0.0,
            angle: 0.0,
            knockback: 0.0,
            apply_damage: true,
            apply_knockback: true,
            apply_hitlag: true,
            apply_hitstun: true,
            reflect: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    SetAction(String),
    SetVelocity([f32; 2]),
    SetGroundVelocity(f32),
    ApplyHitlag { fighter: u8, frames: u32 },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScriptResult {
    pub locals: LocalState,
    /// Staged action scoped state. `locals` remains the persistent state for
    /// compatibility with callers and serialized result adapters.
    #[serde(default)]
    pub action_state: LocalState,
    pub commands: Vec<Command>,
    pub hit: Option<HitPatch>,
}

/// Borrowed state and the match-owned immutable resource cache for a combat
/// callback. Both state domains are staged and returned as one result.
pub(crate) struct CombatContext<'a> {
    pub persistent: &'a LocalState,
    pub action_state: &'a LocalState,
    pub resources: Option<Arc<lifecycle_resources::ResourceCache>>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Pon gameplay backend is not integrated")]
    BackendUnavailable,
    #[error("script source exceeds {MAX_SOURCE_BYTES} bytes")]
    SourceTooLarge,
    #[error("script has no valid chunk: {0}")]
    Compile(String),
    #[error("script hook failed: {0}")]
    Runtime(String),
    #[error("script returned invalid data: {0}")]
    Invalid(String),
}

#[allow(dead_code)]
struct CombatHost {
    fighter: FighterView,
    hit: Option<HitView>,
    resources: Option<Arc<lifecycle_resources::ResourceCache>>,
    patch: HitPatch,
    persistent: LocalState,
    action_state: LocalState,
    commands: Vec<Command>,
    state_schema: StateSchema,
    action_schema: StateSchema,
}

#[allow(dead_code)]
impl CombatHost {
    fn new(
        fighter: &FighterView,
        hit: Option<&HitView>,
        persistent: &LocalState,
        action_state: &LocalState,
        baseline: Option<&HitPatch>,
        resources: Option<Arc<lifecycle_resources::ResourceCache>>,
        state_schema: &StateSchema,
        action_schema: &StateSchema,
    ) -> Self {
        let mut patch = baseline.cloned().unwrap_or_default();
        // A fresh patch is also the callback's mutable view of the incoming
        // hit.  Seed it from the immutable view so reads observe the actual
        // damage/angle/knockback before the script changes any field.
        if baseline.is_none()
            && let Some(hit) = hit
        {
            patch.damage = hit.damage;
            patch.angle = hit.angle;
            patch.knockback = hit.knockback;
        }
        let mut persistent = persistent.clone();
        for (key, value) in state_schema.defaults() {
            persistent.entry(key).or_insert(value);
        }
        let mut action_state = action_state.clone();
        for (key, value) in action_schema.defaults() {
            action_state.entry(key).or_insert(value);
        }
        Self {
            fighter: fighter.clone(),
            hit: hit.cloned(),
            resources,
            patch,
            persistent,
            action_state,
            commands: Vec::new(),
            state_schema: state_schema.clone(),
            action_schema: action_schema.clone(),
        }
    }

    fn push_command(&mut self, command: Command) -> Result<(), starlark::Error> {
        if self.commands.len() >= MAX_COMMANDS {
            return Err(starlark::Error::Host("too many script commands".into()));
        }
        self.commands.push(command);
        Ok(())
    }

    fn fighter_value(&self, field: &str) -> Result<starlark::NativeValue, starlark::Error> {
        Ok(match field {
            "id" => starlark::NativeValue::Int(i64::from(self.fighter.id)),
            "action" => starlark::NativeValue::String(self.fighter.action.clone()),
            "action_frame" => starlark::NativeValue::Int(i64::from(self.fighter.action_frame)),
            "velocity" => starlark::NativeValue::Vec2(self.fighter.velocity),
            "ground_velocity" => starlark::NativeValue::F32(self.fighter.ground_velocity),
            "grounded" => starlark::NativeValue::Bool(self.fighter.grounded),
            "percent" => starlark::NativeValue::F32(self.fighter.percent),
            "hitlag" => starlark::NativeValue::F32(self.fighter.hitlag),
            "hitstun" => starlark::NativeValue::Int(i64::from(self.fighter.hitstun)),
            "resource" => starlark::NativeValue::Object(starlark::NativeObject {
                kind: starlark::NativeKind::Value,
                path: "fighter.resource".into(),
            }),
            "flags" => starlark::NativeValue::Object(starlark::NativeObject {
                kind: starlark::NativeKind::Value,
                path: "fighter.flags".into(),
            }),
            "flags.reflecting" => starlark::NativeValue::Bool(
                self.fighter
                    .flags
                    .get("reflecting")
                    .copied()
                    .unwrap_or(false),
            ),
            field if field.starts_with("flags.") => {
                let name = &field["flags.".len()..];
                if name.is_empty() {
                    return Err(starlark::Error::Host(
                        "fighter flag name cannot be empty".into(),
                    ));
                }
                starlark::NativeValue::Bool(self.fighter.flags.get(name).copied().unwrap_or(false))
            }
            "state" => starlark::NativeValue::Object(starlark::NativeObject {
                kind: starlark::NativeKind::State,
                path: "fighter.state".into(),
            }),
            "action_state" => starlark::NativeValue::Object(starlark::NativeObject {
                kind: starlark::NativeKind::State,
                path: "fighter.action_state".into(),
            }),
            field if field.starts_with("state.") => self.state_value(field, false)?,
            field if field.starts_with("action_state.") => self.state_value(field, true)?,
            _ => {
                return Err(starlark::Error::Host(format!(
                    "unknown fighter field {field:?}"
                )));
            }
        })
    }

    fn resource_value(&self, path: &str) -> Result<starlark::NativeValue, starlark::Error> {
        let Some(resources) = self.resources.as_deref() else {
            return Ok(starlark::NativeValue::None);
        };
        let Some(value) = resources.value_path(path) else {
            return Ok(starlark::NativeValue::None);
        };
        let native_path = format!("fighter.resource.{path}");
        match value {
            serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                Ok(starlark::NativeValue::Object(starlark::NativeObject {
                    kind: starlark::NativeKind::Value,
                    path: native_path,
                }))
            }
            _ => lifecycle_host::json_to_native(value, &native_path),
        }
        .map_err(|error| starlark::Error::Host(error.to_string()))
    }

    fn hit_value(&self, field: &str) -> Result<starlark::NativeValue, starlark::Error> {
        let hit = self
            .hit
            .as_ref()
            .ok_or_else(|| starlark::Error::Host("hit is unavailable".into()))?;
        Ok(match field {
            "frame" => starlark::NativeValue::Int(i64::from(hit.frame)),
            "attacker" => starlark::NativeValue::Int(i64::from(hit.attacker)),
            "defender" => starlark::NativeValue::Int(i64::from(hit.defender)),
            "damage" => starlark::NativeValue::F32(self.patch.damage),
            "angle" => starlark::NativeValue::F32(self.patch.angle),
            "base_knockback" => starlark::NativeValue::Int(i64::from(hit.base_knockback)),
            "knockback_growth" => starlark::NativeValue::Int(i64::from(hit.knockback_growth)),
            "knockback" => starlark::NativeValue::F32(self.patch.knockback),
            "hitbox_group" => starlark::NativeValue::Int(i64::from(hit.hitbox_group)),
            "projectile" => starlark::NativeValue::Bool(hit.projectile),
            "max_damage" => starlark::NativeValue::Int(i64::from(hit.max_damage)),
            "cancelled" => starlark::NativeValue::Bool(self.patch.cancelled),
            "apply_damage" => starlark::NativeValue::Bool(self.patch.apply_damage),
            "apply_knockback" => starlark::NativeValue::Bool(self.patch.apply_knockback),
            "apply_hitlag" => starlark::NativeValue::Bool(self.patch.apply_hitlag),
            "apply_hitstun" => starlark::NativeValue::Bool(self.patch.apply_hitstun),
            "reflect" => starlark::NativeValue::Bool(self.patch.reflect),
            _ => {
                return Err(starlark::Error::Host(format!(
                    "unknown hit field {field:?}"
                )));
            }
        })
    }

    fn state_value(
        &self,
        path: &str,
        action: bool,
    ) -> Result<starlark::NativeValue, starlark::Error> {
        let key = path
            .split_once('.')
            .map(|(_, key)| key)
            .ok_or_else(|| starlark::Error::Host("state path is incomplete".into()))?;
        let schema = if action {
            &self.action_schema
        } else {
            &self.state_schema
        };
        if !schema.fields.contains_key(key) {
            return Err(starlark::Error::Host(format!(
                "undeclared persistent state field {key:?}"
            )));
        }
        let values = if action {
            &self.action_state
        } else {
            &self.persistent
        };
        let value = values.get(key).ok_or_else(|| {
            starlark::Error::Host(format!("state field {key:?} has no staged value"))
        })?;
        Ok(match value {
            LocalValue::Bool(value) => starlark::NativeValue::Bool(*value),
            LocalValue::Integer(value) => starlark::NativeValue::Int(*value),
            LocalValue::Number(value) => starlark::NativeValue::F32(*value as f32),
            LocalValue::String(value) => starlark::NativeValue::String(value.clone()),
            LocalValue::Tuple(values) => {
                starlark::NativeValue::Tuple(values.iter().map(local_to_native_value).collect())
            }
        })
    }

    fn set_state_value(
        &mut self,
        path: &str,
        value: starlark::NativeValue,
        action: bool,
    ) -> Result<(), starlark::Error> {
        let key = path
            .split_once('.')
            .map(|(_, key)| key)
            .ok_or_else(|| starlark::Error::Host("state path is incomplete".into()))?;
        let schema = if action {
            &self.action_schema
        } else {
            &self.state_schema
        };
        let field = schema.fields.get(key).ok_or_else(|| {
            starlark::Error::Host(format!("undeclared persistent state field {key:?}"))
        })?;
        let value = match value {
            starlark::NativeValue::Bool(value) if field.value_type == StateType::Bool => {
                LocalValue::Bool(value)
            }
            starlark::NativeValue::Int(value) if field.value_type == StateType::Integer => {
                LocalValue::Integer(value)
            }
            starlark::NativeValue::F32(value) if field.value_type == StateType::Number => {
                LocalValue::Number(f64::from(value))
            }
            starlark::NativeValue::String(value) if field.value_type == StateType::String => {
                LocalValue::String(value)
            }
            starlark::NativeValue::Tuple(values)
                if state_value_matches(
                    field.value_type.clone(),
                    &LocalValue::Tuple(values.iter().map(native_to_local_value).collect()),
                ) =>
            {
                LocalValue::Tuple(values.iter().map(native_to_local_value).collect())
            }
            starlark::NativeValue::List(values)
                if matches!(&field.value_type, StateType::FixedTuple { .. })
                    && state_value_matches(
                        field.value_type.clone(),
                        &LocalValue::Tuple(values.iter().map(native_to_local_value).collect()),
                    ) =>
            {
                LocalValue::Tuple(values.iter().map(native_to_local_value).collect())
            }
            _ => {
                return Err(starlark::Error::Host(format!(
                    "state field {key:?} has the wrong type"
                )));
            }
        };
        if action {
            self.action_state.insert(key.to_owned(), value);
        } else {
            self.persistent.insert(key.to_owned(), value);
        }
        Ok(())
    }

    fn set_sticky_gate(gate: &mut bool, value: bool, name: &str) -> Result<(), starlark::Error> {
        let enabling_gate = matches!(name, "cancelled" | "reflect");
        // Cancellation/reflection can only transition false -> true. Effect
        // gates can only transition true -> false. Repeating the current
        // value is harmless, which permits composed callbacks to be
        // declarative about an already-resolved hit.
        if enabling_gate {
            if *gate && !value {
                return Err(starlark::Error::Host(format!(
                    "hit gate {name} cannot be cleared after resolution"
                )));
            }
            *gate |= value;
        } else {
            if !*gate && value {
                return Err(starlark::Error::Host(format!(
                    "hit gate {name} cannot be re-enabled after resolution"
                )));
            }
            *gate &= value;
        }
        Ok(())
    }
}

fn local_to_native_value(value: &LocalValue) -> starlark::NativeValue {
    match value {
        LocalValue::Bool(x) => starlark::NativeValue::Bool(*x),
        LocalValue::Integer(x) => starlark::NativeValue::Int(*x),
        LocalValue::Number(x) => starlark::NativeValue::F32(*x as f32),
        LocalValue::String(x) => starlark::NativeValue::String(x.clone()),
        LocalValue::Tuple(values) => {
            starlark::NativeValue::Tuple(values.iter().map(local_to_native_value).collect())
        }
    }
}

fn native_to_local_value(value: &starlark::NativeValue) -> LocalValue {
    match value {
        starlark::NativeValue::Bool(x) => LocalValue::Bool(*x),
        starlark::NativeValue::Int(x) => LocalValue::Integer(*x),
        starlark::NativeValue::F32(x) => LocalValue::Number(f64::from(*x)),
        starlark::NativeValue::String(x) => LocalValue::String(x.clone()),
        starlark::NativeValue::Tuple(values) => {
            LocalValue::Tuple(values.iter().map(native_to_local_value).collect())
        }
        _ => LocalValue::Tuple(Vec::new()),
    }
}

impl starlark::NativeHost for CombatHost {
    fn get(&mut self, path: &str) -> Result<starlark::NativeValue, starlark::Error> {
        let (root, field) = path
            .split_once('.')
            .ok_or_else(|| starlark::Error::Host("native root is not a field".into()))?;
        match root {
            "fighter" if field == "resource" || field.starts_with("resource.") => {
                if field == "resource" {
                    self.fighter_value(field)
                } else {
                    self.resource_value(&field["resource.".len()..])
                }
            }
            "fighter" => self.fighter_value(field),
            "hit" => self.hit_value(field),
            _ => Err(starlark::Error::Host(format!(
                "unknown native root {root:?}"
            ))),
        }
    }

    fn set(&mut self, path: &str, value: starlark::NativeValue) -> Result<(), starlark::Error> {
        let (root, field) = path
            .split_once('.')
            .ok_or_else(|| starlark::Error::Host("native root is not a field".into()))?;
        match (root, field, value) {
            ("fighter", field, value) if field.starts_with("state.") => {
                self.set_state_value(field, value, false)?;
            }
            ("fighter", field, value) if field.starts_with("action_state.") => {
                self.set_state_value(field, value, true)?;
            }
            ("fighter", "action", starlark::NativeValue::String(value)) => {
                self.fighter.action = value.clone();
                self.push_command(Command::SetAction(value))?;
            }
            ("fighter", "velocity", value) => {
                let value = lifecycle_host::vec2(value, "velocity")?;
                if !value.iter().all(|x| x.is_finite()) {
                    return Err(starlark::Error::Host("non-finite velocity".into()));
                }
                self.fighter.velocity = value;
                self.push_command(Command::SetVelocity(value))?;
            }
            ("fighter", "ground_velocity", starlark::NativeValue::F32(value)) => {
                let value = finite(value).map_err(|e| starlark::Error::Host(e.to_string()))?;
                self.fighter.ground_velocity = value;
                self.push_command(Command::SetGroundVelocity(value))?;
            }
            ("fighter", "flags.reflecting", starlark::NativeValue::Bool(value)) => {
                let _ = value;
                return Err(starlark::Error::Host(
                    "fighter.flags.reflecting is read-only during combat".into(),
                ));
            }
            ("fighter", field, starlark::NativeValue::Bool(value))
                if field.starts_with("flags.") =>
            {
                let name = &field["flags.".len()..];
                if name.is_empty() {
                    return Err(starlark::Error::Host(
                        "fighter flag name cannot be empty".into(),
                    ));
                }
                self.fighter.flags.insert(name.to_owned(), value);
            }
            ("hit", "cancelled", starlark::NativeValue::Bool(value)) => {
                Self::set_sticky_gate(&mut self.patch.cancelled, value, "cancelled")?
            }
            ("hit", "damage", starlark::NativeValue::F32(value)) => {
                self.patch.damage =
                    finite(value).map_err(|e| starlark::Error::Host(e.to_string()))?
            }
            ("hit", "angle", starlark::NativeValue::F32(value)) => {
                self.patch.angle =
                    finite(value).map_err(|e| starlark::Error::Host(e.to_string()))?
            }
            ("hit", "knockback", starlark::NativeValue::F32(value)) => {
                self.patch.knockback =
                    finite(value).map_err(|e| starlark::Error::Host(e.to_string()))?
            }
            ("hit", "apply_damage", starlark::NativeValue::Bool(value)) => {
                Self::set_sticky_gate(&mut self.patch.apply_damage, value, "apply_damage")?
            }
            ("hit", "apply_knockback", starlark::NativeValue::Bool(value)) => {
                Self::set_sticky_gate(&mut self.patch.apply_knockback, value, "apply_knockback")?
            }
            ("hit", "apply_hitlag", starlark::NativeValue::Bool(value)) => {
                Self::set_sticky_gate(&mut self.patch.apply_hitlag, value, "apply_hitlag")?
            }
            ("hit", "apply_hitstun", starlark::NativeValue::Bool(value)) => {
                Self::set_sticky_gate(&mut self.patch.apply_hitstun, value, "apply_hitstun")?
            }
            ("hit", "reflect", starlark::NativeValue::Bool(value)) => {
                Self::set_sticky_gate(&mut self.patch.reflect, value, "reflect")?
            }
            _ => {
                return Err(starlark::Error::Host(format!(
                    "invalid native assignment {path:?}"
                )));
            }
        }
        Ok(())
    }

    fn call(
        &mut self,
        path: &str,
        args: &[starlark::NativeValue],
    ) -> Result<starlark::NativeValue, starlark::Error> {
        match path {
            "fighter.resource" => {
                let Some(starlark::NativeValue::String(path)) = args.first() else {
                    return Err(starlark::Error::Host(
                        "fighter.resource requires a string path".into(),
                    ));
                };
                return self.resource_value(path);
            }
            "fighter.change_action" | "fighter.set_action" => {
                let Some(starlark::NativeValue::String(action)) = args.first() else {
                    return Err(starlark::Error::Host("action requires a string".into()));
                };
                self.set(
                    "fighter.action",
                    starlark::NativeValue::String(action.clone()),
                )?;
            }
            "fighter.set_velocity" => {
                let velocity = match (args.first(), args.get(1)) {
                    (Some(starlark::NativeValue::Vec2(value)), None) => *value,
                    (Some(starlark::NativeValue::F32(x)), Some(starlark::NativeValue::F32(y))) => {
                        [*x, *y]
                    }
                    _ => {
                        return Err(starlark::Error::Host(
                            "set_velocity requires a Vec2 or two numbers".into(),
                        ));
                    }
                };
                self.set("fighter.velocity", starlark::NativeValue::Vec2(velocity))?;
            }
            "fighter.apply_hitlag" => {
                let (
                    Some(starlark::NativeValue::Int(fighter)),
                    Some(starlark::NativeValue::Int(frames)),
                ) = (args.first(), args.get(1))
                else {
                    return Err(starlark::Error::Host(
                        "apply_hitlag requires fighter and frames integers".into(),
                    ));
                };
                let fighter = u8::try_from(*fighter)
                    .map_err(|_| starlark::Error::Host("hitlag fighter is out of range".into()))?;
                let frames = u32::try_from(*frames)
                    .map_err(|_| starlark::Error::Host("hitlag frames are out of range".into()))?;
                self.push_command(Command::ApplyHitlag { fighter, frames })?;
            }
            "hit.cancel" => Self::set_sticky_gate(&mut self.patch.cancelled, true, "cancelled")?,
            "hit.reflect" => Self::set_sticky_gate(&mut self.patch.reflect, true, "reflect")?,
            "hit.disable_knockback" => {
                Self::set_sticky_gate(&mut self.patch.apply_knockback, false, "apply_knockback")?
            }
            _ => {
                return Err(starlark::Error::Host(format!(
                    "unknown native method {path:?}"
                )));
            }
        }
        Ok(starlark::NativeValue::None)
    }

    fn call_named(
        &mut self,
        path: &str,
        args: &[starlark::NativeValue],
        named: &BTreeMap<String, starlark::NativeValue>,
    ) -> Result<starlark::NativeValue, starlark::Error> {
        if path == "fighter.apply_hitlag" {
            let fighter = named.get("fighter").or_else(|| args.first());
            let frames = named.get("frames").or_else(|| args.get(1));
            let (Some(fighter), Some(frames)) = (fighter, frames) else {
                return Err(starlark::Error::Host(
                    "apply_hitlag requires fighter and frames".into(),
                ));
            };
            self.call(path, &[fighter.clone(), frames.clone()])
        } else if named.is_empty() {
            self.call(path, args)
        } else {
            Err(starlark::Error::Host(format!(
                "native call `{path}` does not accept named arguments"
            )))
        }
    }
}

impl Program {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn has_hook(&self, hook: Hook) -> Result<bool, Error> {
        Ok(self.hook_indices[hook.index()].is_some())
    }

    pub fn new(source: impl Into<String>) -> Result<Self, Error> {
        Self::new_registered(
            source,
            &crate::game::script::definition::AssetStore::default(),
        )
    }

    pub(crate) fn new_registered(
        source: impl Into<String>,
        assets: &crate::game::script::definition::AssetStore,
    ) -> Result<Self, Error> {
        native_math::register().map_err(Error::Compile)?;
        let source = source.into();
        if source.len() > MAX_SOURCE_BYTES {
            return Err(Error::SourceTooLarge);
        }
        let mut bundle = starlark::SourceBundle::new("fighter-pack-v1");
        for (name, dependency) in assets.dependencies(&source)? {
            bundle = bundle
                .with_file(name, dependency.as_str())
                .map_err(|error| Error::Compile(error.to_string()))?;
        }
        let compiled = starlark::CompiledProgram::new_with_bundle(
            Arc::<str>::from(source.clone()),
            Arc::<str>::from("fighter.py"),
            std::iter::empty(),
            Some(bundle),
        )
        .map_err(|error| Error::Compile(error.to_string()))?;
        let compiled = Arc::new(compiled);
        // Export and callback discovery are resource-load operations. Pon's
        // evaluator is thread-affine, so prepare explicitly on this loading
        // thread before querying the module; gameplay dispatch only uses an
        // already prepared per-thread handle.
        compiled
            .prepare_for_current_thread()
            .map_err(|error| Error::Compile(error.to_string()))?;
        let exported = compiled
            .export_metadata()
            .map_err(|error| Error::Compile(error.to_string()))?;
        let manifest: crate::game::script::definition::FighterDefinition =
            serde_json::from_value(native_to_json(&exported)?)
                .map_err(|error| Error::Invalid(format!("invalid fighter export: {error}")))?;
        manifest.prepare_runtime_indexes();
        manifest.state.validate_declaration()?;
        manifest.action_state.validate_declaration()?;
        let move_registry = move_registry::MoveRegistry::compile(&manifest)
            .map_err(|error| Error::Invalid(format!("invalid moveset registry: {error}")))?;
        let callback_keys = compiled
            .callback_keys()
            .map_err(|error| Error::Compile(error.to_string()))?;
        let mut callback_bindings = std::array::from_fn(|_| Vec::new());
        for binding in &manifest.callbacks {
            if !callback_keys.iter().any(|key| key == &binding.callback) {
                return Err(Error::Invalid(format!(
                    "exported callback {:?} is missing",
                    binding.callback
                )));
            }
            callback_bindings[binding.hook.index()].push(callback_routing::ResolvedCallback {
                callback: compiled.bind_callback(&binding.callback),
                selector: callback_routing::CallbackSelector::from_binding(binding)
                    .map_err(Error::Invalid)?,
            });
        }
        let behavior_bindings = manifest
            .behaviors
            .iter()
            .map(|behavior| -> Result<_, Error> {
                let mut result = std::array::from_fn(|_| Vec::new());
                for binding in &behavior.callbacks {
                    if !callback_keys.iter().any(|key| key == &binding.callback) {
                        return Err(Error::Invalid(format!(
                            "exported callback {:?} is missing",
                            binding.callback
                        )));
                    }
                    result[binding.hook.index()].push(callback_routing::ResolvedCallback {
                        callback: compiled.bind_callback(&binding.callback),
                        selector: callback_routing::CallbackSelector::from_binding(binding)
                            .map_err(Error::Invalid)?,
                    });
                }
                Ok(result)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let hook_indices = std::array::from_fn(|index| {
            (!callback_bindings[index].is_empty()
                || behavior_bindings
                    .iter()
                    .any(|callbacks| !callbacks[index].is_empty()))
            .then_some(index)
        });
        let dependency_sources = assets.dependencies(&source)?;
        Ok(Self {
            source: Arc::from(source),
            dependency_sources: Arc::new(dependency_sources),
            compiled: Some(compiled),
            hook_indices,
            callback_bindings,
            behavior_bindings,
            metadata: Arc::new(manifest),
            move_registry: Arc::new(move_registry),
        })
    }

    pub fn compiled(&self) -> Option<&starlark::CompiledProgram> {
        self.compiled.as_deref()
    }

    /// Prepare the Pon evaluator for the calling thread before gameplay
    /// dispatch. This is a session/thread boundary operation; dispatch itself
    /// never compiles or discovers source.
    pub fn prepare_for_current_thread(&self) -> Result<(), Error> {
        self.compiled
            .as_deref()
            .ok_or_else(|| Error::Runtime("program is not linked to Pon".into()))?
            .prepare_for_current_thread()
            .map_err(|error| Error::Runtime(error.to_string()))?;
        #[cfg(feature = "experimental-continuations")]
        async_move::prepare_program(self).map_err(Error::Runtime)?;
        Ok(())
    }

    /// Linking is unavailable until the Pon gameplay backend is integrated.
    pub(crate) fn linked_with_environment<S>(&self, _environment: &S) -> Result<Self, Error> {
        Ok(self.clone())
    }

    pub(crate) fn metadata(&self) -> &crate::game::script::definition::FighterDefinition {
        &self.metadata
    }

    pub(crate) fn metadata_arc(&self) -> Arc<crate::game::script::definition::FighterDefinition> {
        Arc::clone(&self.metadata)
    }

    /// Return the immutable native move links retained by this program.
    pub fn moves(&self) -> &move_registry::MoveRegistry {
        &self.move_registry
    }

    pub(crate) fn moves_arc(&self) -> Arc<move_registry::MoveRegistry> {
        Arc::clone(&self.move_registry)
    }

    pub(crate) fn callback_bindings(
        &self,
        behavior: Option<usize>,
        hook: Hook,
    ) -> &[callback_routing::ResolvedCallback] {
        match behavior {
            Some(index) => self
                .behavior_bindings
                .get(index)
                .map(|callbacks| callbacks[hook.index()].as_slice())
                .unwrap_or(&[]),
            None => &self.callback_bindings[hook.index()],
        }
    }

    pub fn dispatch(
        &self,
        hook: Hook,
        fighter: &FighterView,
        hit: Option<&HitView>,
        locals: &LocalState,
    ) -> Result<ScriptResult, Error> {
        self.dispatch_with_context_and_patch(
            hook,
            fighter,
            hit,
            CombatContext {
                persistent: locals,
                action_state: &LocalState::default(),
                resources: None,
            },
            None,
        )
    }

    /// Dispatch with independently supplied persistent and action state for
    /// native callers that do not own a match resource cache.
    pub fn dispatch_with_states(
        &self,
        hook: Hook,
        fighter: &FighterView,
        hit: Option<&HitView>,
        persistent: &LocalState,
        action_state: &LocalState,
    ) -> Result<ScriptResult, Error> {
        self.dispatch_with_context(
            hook,
            fighter,
            hit,
            CombatContext {
                persistent,
                action_state,
                resources: None,
            },
        )
    }

    pub fn dispatch_with_states_with_patch(
        &self,
        hook: Hook,
        fighter: &FighterView,
        hit: &HitView,
        baseline: &HitPatch,
        persistent: &LocalState,
        action_state: &LocalState,
    ) -> Result<ScriptResult, Error> {
        self.dispatch_with_context_with_patch(
            hook,
            fighter,
            hit,
            baseline,
            CombatContext {
                persistent,
                action_state,
                resources: None,
            },
        )
    }

    pub(crate) fn dispatch_with_context(
        &self,
        hook: Hook,
        fighter: &FighterView,
        hit: Option<&HitView>,
        context_state: CombatContext<'_>,
    ) -> Result<ScriptResult, Error> {
        self.dispatch_with_context_and_patch(hook, fighter, hit, context_state, None)
    }

    pub(crate) fn dispatch_with_context_with_patch(
        &self,
        hook: Hook,
        fighter: &FighterView,
        hit: &HitView,
        baseline: &HitPatch,
        context_state: CombatContext<'_>,
    ) -> Result<ScriptResult, Error> {
        let mut result = self.dispatch_with_context_and_patch(
            hook,
            fighter,
            Some(hit),
            context_state,
            Some(baseline),
        )?;
        merge_hit_baseline(&mut result, baseline);
        Ok(result)
    }

    fn dispatch_with_context_and_patch(
        &self,
        hook: Hook,
        fighter: &FighterView,
        hit: Option<&HitView>,
        context_state: CombatContext<'_>,
        baseline: Option<&HitPatch>,
    ) -> Result<ScriptResult, Error> {
        let context = serde_json::json!({
            "event": {"kind": hook.name(), "action": fighter.action},
        });
        let current_action = parse_action(&fighter.action);
        let resource_ref = context_state.resources.as_deref();
        let selected_root = self.callback_bindings(None, hook).iter().filter(|binding| {
            callback_routing::matches(
                hook,
                &binding.selector,
                &context,
                current_action,
                resource_ref,
            )
        });
        let selected_behaviors = self
            .behavior_bindings
            .iter()
            .enumerate()
            .filter_map(|(index, bindings)| {
                let enabled = self.metadata.behaviors.get(index).is_none_or(|behavior| {
                    behavior.resource.as_deref().is_none_or(|path| {
                        context_state
                            .resources
                            .as_ref()
                            .is_some_and(|cache| cache.value_path(path).is_some())
                    })
                });
                enabled.then_some(&bindings[hook.index()])
            })
            .flatten()
            .filter(|binding| {
                callback_routing::matches(
                    hook,
                    &binding.selector,
                    &context,
                    current_action,
                    resource_ref,
                )
            });
        if self.callback_bindings(None, hook).is_empty()
            && self
                .behavior_bindings
                .iter()
                .all(|bindings| bindings[hook.index()].is_empty())
        {
            return Ok(ScriptResult {
                locals: context_state.persistent.clone(),
                action_state: context_state.action_state.clone(),
                hit: baseline.cloned().or_else(|| {
                    hit.map(|value| HitPatch {
                        damage: value.damage,
                        angle: value.angle,
                        knockback: value.knockback,
                        ..HitPatch::default()
                    })
                }),
                ..ScriptResult::default()
            });
        }
        let compiled = self
            .compiled
            .as_ref()
            .ok_or_else(|| Error::Runtime("program is not linked to Pon".into()))?;
        let state = Arc::new(std::sync::Mutex::new(CombatHost::new(
            fighter,
            hit,
            context_state.persistent,
            context_state.action_state,
            baseline,
            context_state.resources.clone(),
            &self.metadata.state,
            &self.metadata.action_state,
        )));
        let host: starlark::SharedNativeHost = state.clone();
        let primary = starlark::HostRef::fighter(Arc::clone(&host));
        let extra = hit
            .map(|_| {
                vec![starlark::host_object(&starlark::HostRef::hit(Arc::clone(
                    &host,
                )))]
            })
            .unwrap_or_default();
        for binding in selected_root.chain(selected_behaviors) {
            compiled
                .dispatch(&binding.callback, primary.clone(), &extra)
                .map_err(|error| Error::Runtime(error.to_string()))?;
        }
        // The evaluator receives cloned host references for the primary
        // fighter and optional hit argument. Release those scoped handles
        // before recovering the staged host below.
        drop(extra);
        drop(primary);
        drop(host);
        let host = Arc::try_unwrap(state)
            .map_err(|_| Error::Runtime("combat host retained by callback".into()))?
            .into_inner()
            .map_err(|_| Error::Runtime("combat host lock poisoned".into()))?;
        self.metadata.state.validate(&host.persistent)?;
        self.metadata.action_state.validate(&host.action_state)?;
        Ok(ScriptResult {
            locals: host.persistent,
            action_state: host.action_state,
            commands: host.commands,
            hit: hit.map(|_| host.patch),
        })
    }

    /// Dispatch with an already modified hit baseline. This is used when the
    /// attacker hook runs before the defender hook; defender scripts must see
    /// the attacker's cancellation and resolution gates rather than a stale
    /// copy of the original hit.
    pub fn dispatch_with_patch(
        &self,
        hook: Hook,
        fighter: &FighterView,
        hit: &HitView,
        baseline: &HitPatch,
        locals: &LocalState,
    ) -> Result<ScriptResult, Error> {
        let result = self.dispatch_with_context_and_patch(
            hook,
            fighter,
            Some(hit),
            CombatContext {
                persistent: locals,
                action_state: &LocalState::default(),
                resources: None,
            },
            Some(baseline),
        )?;
        let Some(mut patch) = result.hit else {
            return Ok(result);
        };
        // A defender may further restrict a hit, but cannot resurrect an
        // attacker cancellation or re-enable a gate the attacker disabled.
        patch.cancelled |= baseline.cancelled;
        patch.apply_damage &= baseline.apply_damage;
        patch.apply_knockback &= baseline.apply_knockback;
        patch.apply_hitlag &= baseline.apply_hitlag;
        patch.apply_hitstun &= baseline.apply_hitstun;
        Ok(ScriptResult {
            hit: Some(patch),
            ..result
        })
    }
}

fn merge_hit_baseline(result: &mut ScriptResult, baseline: &HitPatch) {
    if let Some(patch) = &mut result.hit {
        patch.cancelled |= baseline.cancelled;
        patch.apply_damage &= baseline.apply_damage;
        patch.apply_knockback &= baseline.apply_knockback;
        patch.apply_hitlag &= baseline.apply_hitlag;
        patch.apply_hitstun &= baseline.apply_hitstun;
    }
}

impl Program {
    /// Query the generic projectile-contact policy. Collision and reflection
    /// physics remain engine-owned; scripts return only this disposition bit.
    pub fn projectile_contact(
        &self,
        fighter: &FighterView,
        damage: f32,
        max_damage: i32,
    ) -> Result<bool, Error> {
        let hit = HitView {
            damage,
            max_damage,
            projectile: true,
            ..Default::default()
        };
        Ok(self
            .dispatch(
                Hook::ProjectileContact,
                fighter,
                Some(&hit),
                &LocalState::new(),
            )?
            .hit
            .is_some_and(|patch| patch.reflect))
    }
}

#[allow(dead_code)]
fn finite(value: f32) -> Result<f32, Error> {
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| Error::Invalid("non-finite number".into()))
}

pub fn validate_program(program: &Program) -> Result<(), Error> {
    if program.source.len() > MAX_SOURCE_BYTES {
        return Err(Error::SourceTooLarge);
    }
    Ok(())
}

pub fn validate_state(state: &LocalState) -> Result<(), Error> {
    if state.len() > MAX_LOCALS {
        return Err(Error::Invalid("too many local variables".into()));
    }
    for (key, value) in state {
        if key.is_empty() {
            return Err(Error::Invalid("local key cannot be empty".into()));
        }
        if key.len() > MAX_LOCAL_KEY_BYTES {
            return Err(Error::Invalid("local key is too long".into()));
        }
        if matches!(value, LocalValue::String(value) if value.len() > MAX_LOCAL_STRING_BYTES) {
            return Err(Error::Invalid("local string is too long".into()));
        }
        if matches!(value, LocalValue::Number(value) if !value.is_finite()) {
            return Err(Error::Invalid("non-finite local number".into()));
        }
    }
    Ok(())
}

/// Apply validated generic commands after a successful hook.
pub(crate) fn apply_commands(
    state: &mut super::State,
    actor: usize,
    commands: &[Command],
) -> Result<(), super::Error> {
    if actor >= state.fighters.len() {
        return Err(super::Error::Data("invalid script actor".into()));
    }
    for command in commands {
        match command {
            Command::SetAction(name) => {
                let action = parse_action(name).ok_or_else(|| {
                    super::Error::Data(format!("unsupported script action {name:?}"))
                })?;
                super::simulation::enter(&mut state.fighters[actor], action);
            }
            Command::SetVelocity(velocity) => {
                if !velocity.iter().all(|value| value.is_finite()) {
                    return Err(super::Error::NonFinite);
                }
                state.fighters[actor].velocity = *velocity;
            }
            Command::SetGroundVelocity(velocity) => {
                if !velocity.is_finite() {
                    return Err(super::Error::NonFinite);
                }
                state.fighters[actor].ground_velocity = *velocity;
            }
            Command::ApplyHitlag { fighter, frames } => {
                let target = state
                    .fighters
                    .get_mut(usize::from(*fighter))
                    .ok_or_else(|| super::Error::Data("invalid script hitlag target".into()))?;
                target.hitlag = target.hitlag.max(*frames as f32);
            }
        }
    }
    Ok(())
}

pub(crate) fn parse_action(name: &str) -> Option<super::Action> {
    // Authoring exports use enum-style references (`SPECIAL_HI`) while older
    // metadata uses Rust-style CamelCase (`SpecialHi`). Normalize the former
    // directly; inserting separators before every uppercase character would
    // otherwise produce `s_p_e_c_i_a_l__h_i`.
    let authored_name = name.to_ascii_lowercase();
    // The authoring API keeps the familiar two-letter appeal suffix while
    // serde's acronym-aware snake case spells it `appeal_s_r`/`appeal_s_l`.
    // Keep both spellings accepted at this single native boundary.
    let authored_name = match authored_name.as_str() {
        "appeal_sr" => "appeal_s_r",
        "appeal_sl" => "appeal_s_l",
        _ => authored_name.as_str(),
    };
    if name.chars().any(|character| character.is_ascii_uppercase()) && name.contains('_') {
        return serde_json::from_value(serde_json::Value::String(authored_name.to_owned())).ok();
    }
    let mut snake = String::with_capacity(name.len() + 4);
    for (index, character) in name.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index != 0 {
                snake.push('_');
            }
            snake.push(character.to_ascii_lowercase());
        } else {
            snake.push(character);
        }
    }
    let snake = match snake.as_str() {
        "appeal_sr" => "appeal_s_r",
        "appeal_sl" => "appeal_s_l",
        _ => snake.as_str(),
    };
    serde_json::from_value(serde_json::Value::String(snake.to_owned())).ok()
}

#[cfg(test)]
mod action_name_tests {
    use super::parse_action;
    use crate::game::Action;

    #[test]
    fn appeal_acronym_spellings_round_trip() {
        for spelling in ["appeal_sr", "APPEAL_SR", "AppealSR", "appeal_s_r"] {
            assert_eq!(parse_action(spelling), Some(Action::AppealSR), "{spelling}");
        }
        for spelling in ["appeal_sl", "APPEAL_SL", "AppealSL", "appeal_s_l"] {
            assert_eq!(parse_action(spelling), Some(Action::AppealSL), "{spelling}");
        }
    }
}

#[cfg(test)]
mod combat_resource_tests {
    use super::{CombatContext, FighterView, HitView, LocalState, Program};
    use crate::game::script::lifecycle_resources::ResourceCache;
    use crate::game::script::resources::{Resources, Specials};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    #[test]
    fn combat_callback_reads_fighter_resource_scalar_and_nested_attribute() {
        let mut data: crate::game::MatchData = serde_json::from_str(include_str!(
            "../../tests/fixtures/game/integration-match.json"
        ))
        .expect("integration fixture decodes");
        data.fighters[0].specials = Some(Specials {
            character: "captain-falcon".into(),
            resources: Resources::new(BTreeMap::from([(
                "side".into(),
                serde_json::json!({"attributes": {"specials_gr_vel_x": 0.75}}),
            )]))
            .expect("resource fixture indexes"),
        });
        let resources = Arc::new(
            ResourceCache::build(Some(&data.fighters[0]), None, false)
                .expect("resource fixture builds"),
        );
        let program = Program::new(include_str!("../../scripts/fighters/captain.py"))
            .expect("Captain source compiles");
        let persistent = LocalState::new();
        let action_state = LocalState::new();
        let fighter = FighterView {
            action: "Action.SPECIAL_S_START".into(),
            velocity: [2.0, 3.0],
            ground_velocity: 4.0,
            ..FighterView::default()
        };
        let result = program
            .dispatch_with_context(
                super::Hook::BeforeHit,
                &fighter,
                Some(&HitView::default()),
                CombatContext {
                    persistent: &persistent,
                    action_state: &action_state,
                    resources: Some(resources),
                },
            )
            .expect("combat callback reads resource through fighter host");
        assert!(result.commands.iter().any(|command| matches!(
            command,
            super::Command::SetAction(action) if action == "special_s"
        )));
        assert!(result.commands.iter().any(|command| matches!(
            command,
            super::Command::SetVelocity([x, y]) if *x == 2.0 && *y == 0.0
        )));
        assert!(result.commands.iter().any(|command| matches!(
            command,
            super::Command::SetGroundVelocity(value) if *value == 3.0
        )));
    }
}

#[cfg(test)]
mod projectile_tests;
