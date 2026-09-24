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
#[path = "script/lifecycle/host.rs"]
pub(crate) mod lifecycle_host;
#[path = "script/lifecycle/movement.rs"]
pub(crate) mod lifecycle_movement;
#[path = "script/lifecycle/resources.rs"]
pub mod lifecycle_resources;
#[path = "script/lifecycle/state.rs"]
pub(crate) mod lifecycle_state;
pub mod motion;
#[path = "script/motion/resources.rs"]
pub mod motion_resources;
pub(crate) mod native_math;
pub mod scheduler;

pub mod definition;
pub(crate) mod identity;
#[path = "script/move/registry.rs"]
pub mod move_registry;
#[path = "script/move/selection.rs"]
pub mod move_selection;
#[path = "script/move/validation.rs"]
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

/// Every built-in fighter source and its resource identity.
///
/// Keep the compile-time source bundle in one place so source lookup, asset
/// registration, and definition discovery cannot drift apart as the roster
/// grows. The character key is the CSS resource key; the filename is the
/// private import name exposed to the bundled source loader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BuiltinScript {
    pub(crate) character_key: &'static str,
    pub(crate) filename: &'static str,
    pub(crate) source: &'static str,
}

pub(crate) static BUILTIN_SCRIPTS: [BuiltinScript; 26] = [
    BuiltinScript {
        character_key: "captain-falcon",
        filename: "captain.py",
        source: include_str!("../../scripts/fighters/captain.py"),
    },
    BuiltinScript {
        character_key: "donkey-kong",
        filename: "donkey_kong.py",
        source: include_str!("../../scripts/fighters/donkey_kong.py"),
    },
    BuiltinScript {
        character_key: "fox",
        filename: "fox.py",
        source: include_str!("../../scripts/fighters/fox.py"),
    },
    BuiltinScript {
        character_key: "game-and-watch",
        filename: "game_and_watch.py",
        source: include_str!("../../scripts/fighters/game_and_watch.py"),
    },
    BuiltinScript {
        character_key: "kirby",
        filename: "kirby.py",
        source: include_str!("../../scripts/fighters/kirby.py"),
    },
    BuiltinScript {
        character_key: "bowser",
        filename: "bowser.py",
        source: include_str!("../../scripts/fighters/bowser.py"),
    },
    BuiltinScript {
        character_key: "link",
        filename: "link.py",
        source: include_str!("../../scripts/fighters/link.py"),
    },
    BuiltinScript {
        character_key: "luigi",
        filename: "luigi.py",
        source: include_str!("../../scripts/fighters/luigi.py"),
    },
    BuiltinScript {
        character_key: "mario",
        filename: "mario.py",
        source: include_str!("../../scripts/fighters/mario.py"),
    },
    BuiltinScript {
        character_key: "marth",
        filename: "marth.py",
        source: include_str!("../../scripts/fighters/marth.py"),
    },
    BuiltinScript {
        character_key: "mewtwo",
        filename: "mewtwo.py",
        source: include_str!("../../scripts/fighters/mewtwo.py"),
    },
    BuiltinScript {
        character_key: "ness",
        filename: "ness.py",
        source: include_str!("../../scripts/fighters/ness.py"),
    },
    BuiltinScript {
        character_key: "peach",
        filename: "peach.py",
        source: include_str!("../../scripts/fighters/peach.py"),
    },
    BuiltinScript {
        character_key: "pikachu",
        filename: "pikachu.py",
        source: include_str!("../../scripts/fighters/pikachu.py"),
    },
    BuiltinScript {
        character_key: "ice-climbers",
        filename: "ice_climbers.py",
        source: include_str!("../../scripts/fighters/ice_climbers.py"),
    },
    BuiltinScript {
        character_key: "jigglypuff",
        filename: "jigglypuff.py",
        source: include_str!("../../scripts/fighters/jigglypuff.py"),
    },
    BuiltinScript {
        character_key: "samus",
        filename: "samus.py",
        source: include_str!("../../scripts/fighters/samus.py"),
    },
    BuiltinScript {
        character_key: "yoshi",
        filename: "yoshi.py",
        source: include_str!("../../scripts/fighters/yoshi.py"),
    },
    BuiltinScript {
        character_key: "zelda",
        filename: "zelda.py",
        source: include_str!("../../scripts/fighters/zelda.py"),
    },
    BuiltinScript {
        character_key: "sheik",
        filename: "sheik.py",
        source: include_str!("../../scripts/fighters/sheik.py"),
    },
    BuiltinScript {
        character_key: "falco",
        filename: "falco.py",
        source: include_str!("../../scripts/fighters/falco.py"),
    },
    BuiltinScript {
        character_key: "young-link",
        filename: "young_link.py",
        source: include_str!("../../scripts/fighters/young_link.py"),
    },
    BuiltinScript {
        character_key: "dr-mario",
        filename: "dr_mario.py",
        source: include_str!("../../scripts/fighters/dr_mario.py"),
    },
    BuiltinScript {
        character_key: "roy",
        filename: "roy.py",
        source: include_str!("../../scripts/fighters/roy.py"),
    },
    BuiltinScript {
        character_key: "pichu",
        filename: "pichu.py",
        source: include_str!("../../scripts/fighters/pichu.py"),
    },
    BuiltinScript {
        character_key: "ganondorf",
        filename: "ganondorf.py",
        source: include_str!("../../scripts/fighters/ganondorf.py"),
    },
];

/// Selects a bundled source by the canonical resource identity.
///
/// `FighterData.name` is the authoritative roster key.  The specials
/// reflector predates script-backed fighter resources and is retained only
/// for old resources that have no name yet; a non-empty, unknown name must
/// not silently select a different fighter's program.
pub fn bundled_source_for_name(
    name: &str,
    legacy_specials: Option<&resources::Specials>,
) -> Option<&'static str> {
    if !name.is_empty() {
        // Keep the canonical resource key as the fast path, then accept the
        // display names serialized by replay formats (for example `Fox` and
        // `Game & Watch`).  Normalization is deliberately only used to match
        // one of the bundled roster entries, so unknown names still fail
        // closed instead of selecting a different fighter.
        return BUILTIN_SCRIPTS
            .iter()
            .find(|builtin| builtin.character_key == name)
            .or_else(|| {
                let normalized_name = normalize_roster_name(name);
                BUILTIN_SCRIPTS
                    .iter()
                    .find(|builtin| normalize_roster_name(builtin.character_key) == normalized_name)
            })
            .map(|builtin| builtin.source);
    }
    bundled_source(legacy_specials)
}

/// Converts canonical keys and serialized display names to a comparable
/// roster spelling.  Ampersand is expanded because the display name for
/// `game-and-watch` is commonly serialized as `Game & Watch`.
fn normalize_roster_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for character in name.chars() {
        if character == '&' {
            normalized.push_str("and");
        } else if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        }
    }
    // Slippi's display name includes the honorific, while the canonical
    // resource key does not.
    if normalized == "mrgameandwatch" {
        "gameandwatch".into()
    } else {
        normalized
    }
}

/// Selects the bundled source owned by a fighter resource.
pub fn bundled_source_for_fighter(
    fighter: &crate::game::data::FighterData,
) -> Option<&'static str> {
    bundled_source_for_name(fighter.name.as_str(), fighter.specials.as_ref())
}

/// Legacy source selection for resources written before `FighterData.name`
/// became the canonical script identity.
pub fn bundled_source(specials: Option<&resources::Specials>) -> Option<&'static str> {
    let specials = specials?;
    BUILTIN_SCRIPTS
        .iter()
        .find(|builtin| specials.character_key_is(builtin.character_key))
        .map(|builtin| builtin.source)
}

#[cfg(test)]
mod builtin_source_tests {
    use super::{BUILTIN_SCRIPTS, bundled_source_for_fighter, bundled_source_for_name};
    use crate::game::script::resources::{Resources, Specials};

    #[test]
    fn roster_name_selects_script_without_special_resources() {
        let mut data: crate::game::MatchData = serde_json::from_str(include_str!(
            "../../tests/fixtures/game/integration-match.json"
        ))
        .expect("integration fixture decodes");
        let fighter = &mut data.fighters[0];
        fighter.name = "mario".into();
        fighter.specials = None;

        assert_eq!(
            bundled_source_for_fighter(fighter),
            BUILTIN_SCRIPTS
                .iter()
                .find(|builtin| builtin.character_key == "mario")
                .map(|builtin| builtin.source)
        );
    }

    #[test]
    fn serialized_display_name_selects_fox_script() {
        assert_eq!(
            bundled_source_for_name("Fox", None),
            BUILTIN_SCRIPTS
                .iter()
                .find(|builtin| builtin.character_key == "fox")
                .map(|builtin| builtin.source)
        );
    }

    #[test]
    fn multiword_punctuation_display_name_selects_script() {
        assert_eq!(
            bundled_source_for_name("Mr. Game & Watch", None),
            BUILTIN_SCRIPTS
                .iter()
                .find(|builtin| builtin.character_key == "game-and-watch")
                .map(|builtin| builtin.source)
        );
    }

    #[test]
    fn canonical_name_still_selects_script() {
        assert_eq!(
            bundled_source_for_name("fox", None),
            BUILTIN_SCRIPTS
                .iter()
                .find(|builtin| builtin.character_key == "fox")
                .map(|builtin| builtin.source)
        );
    }

    #[test]
    fn unknown_name_stays_unresolved() {
        assert_eq!(bundled_source_for_name("Not A Fighter", None), None);
    }

    #[test]
    fn legacy_specials_identity_is_used_only_without_a_name() {
        let mut data: crate::game::MatchData = serde_json::from_str(include_str!(
            "../../tests/fixtures/game/integration-match.json"
        ))
        .expect("integration fixture decodes");
        let fighter = &mut data.fighters[0];
        fighter.name.clear();
        fighter.specials = Some(Specials {
            character: "Fox".into(),
            special_attributes: None,
            animations: None,
            articles: None,
            resources: Resources::default(),
        });

        assert_eq!(
            bundled_source_for_fighter(fighter),
            BUILTIN_SCRIPTS
                .iter()
                .find(|builtin| builtin.character_key == "fox")
                .map(|builtin| builtin.source)
        );
    }

    #[test]
    fn unknown_name_does_not_fall_back_to_a_different_specials_identity() {
        let mut data: crate::game::MatchData = serde_json::from_str(include_str!(
            "../../tests/fixtures/game/integration-match.json"
        ))
        .expect("integration fixture decodes");
        let fighter = &mut data.fighters[0];
        fighter.name = "not-a-roster-fighter".into();
        fighter.specials = Some(Specials {
            character: "Fox".into(),
            special_attributes: None,
            animations: None,
            articles: None,
            resources: Resources::default(),
        });

        assert_eq!(bundled_source_for_fighter(fighter), None);
    }

    #[test]
    fn complete_roster_inventory_has_unique_keys_and_private_modules() {
        let expected = [
            "captain-falcon",
            "donkey-kong",
            "fox",
            "game-and-watch",
            "kirby",
            "bowser",
            "link",
            "luigi",
            "mario",
            "marth",
            "mewtwo",
            "ness",
            "peach",
            "pikachu",
            "ice-climbers",
            "jigglypuff",
            "samus",
            "yoshi",
            "zelda",
            "sheik",
            "falco",
            "young-link",
            "dr-mario",
            "roy",
            "pichu",
            "ganondorf",
        ];
        let keys = BUILTIN_SCRIPTS
            .iter()
            .map(|builtin| builtin.character_key)
            .collect::<std::collections::BTreeSet<_>>();
        let files = BUILTIN_SCRIPTS
            .iter()
            .map(|builtin| builtin.filename)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(BUILTIN_SCRIPTS.len(), expected.len());
        assert_eq!(keys.len(), expected.len());
        assert_eq!(files.len(), expected.len());
        for key in expected {
            assert!(
                BUILTIN_SCRIPTS
                    .iter()
                    .any(|builtin| builtin.character_key == key),
                "missing bundled fighter {key}"
            );
        }
        for builtin in BUILTIN_SCRIPTS {
            assert!(builtin.filename.ends_with(".py"));
            assert!(!builtin.source.is_empty());
        }
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
    /// Hooks whose only possible callback is an unconditional root callback.
    /// Dispatch can use this metadata to avoid building selector context on
    /// the frame hot path.
    fast_root_callbacks: [bool; Hook::COUNT],
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

fn is_unconditional_selector(selector: &callback_routing::CallbackSelector) -> bool {
    selector.action.is_none()
        && selector.actions.is_empty()
        && selector.marker.is_none()
        && selector.countdown.is_none()
        && selector.buttons.is_none()
        && !selector.buttons_all
        && selector.command_index.is_none()
        && selector.deadline.is_none()
        && selector.event_id.is_none()
        && selector.gate.is_none()
}

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
        // A serialized bundled fighter loses its original source filename at
        // the JSON boundary.  Restore only the matching authoritative builtin
        // asset so canonical bytes recover their private module name
        // (`yoshi.py`, `captain.py`, ...), without widening the import surface
        // of arbitrary external programs. Those programs still fall back to
        // `fighter.py` and must carry explicit identity.
        let mut assets = crate::game::script::definition::AssetStore::default();
        if let Some(builtin) = BUILTIN_SCRIPTS
            .iter()
            .find(|builtin| builtin.source == source)
        {
            assets.register(builtin.filename, builtin.source);
        }
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

#[cfg(test)]
mod builtin_identity_tests {
    use super::Program;
    use crate::game::script::definition::AssetStore;

    #[test]
    #[ignore = "requires the finalized verified Pon stdlib release artifact"]
    fn registered_yoshi_identity_is_trusted_and_external_spoof_is_rejected() {
        let yoshi = Program::new_registered(
            include_str!("../../scripts/fighters/yoshi.py"),
            &AssetStore::builtins(),
        )
        .expect("bundled Yoshi source must compile");
        assert_eq!(yoshi.metadata().name, "yoshi");
        assert_eq!(yoshi.metadata().external_ids, vec![17]);

        let spoof = r#"
from skirmish import Fighter
__skirmish_canonical_module__ = "yoshi"
class Spoof(Fighter):
    pass
"#;
        let error = Program::new_registered(spoof, &AssetStore::builtins())
            .expect_err("external source must not spoof builtin roster identity");
        assert!(
            error
                .to_string()
                .contains("needs an explicit name or external_ids"),
            "spoof should fail closed at identity resolution: {error}"
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
    #[allow(clippy::too_many_arguments)]
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
        // Canonical bundled sources must execute under their own private
        // module filename.  The authoring loader intentionally derives a
        // fighter's identity from that module boundary; using the generic
        // `fighter.py` root here makes an embedded roster script look like an
        // arbitrary external program and loses its canonical identity.
        let filename = assets
            .filename_for_source(&source)
            .or_else(|| {
                BUILTIN_SCRIPTS
                    .iter()
                    .find(|builtin| builtin.source == source)
                    .map(|builtin| builtin.filename)
            })
            .unwrap_or("fighter.py");
        // Pon's diagnostic filename is not the module's `__name__`/`__file__`.
        // Put trusted provenance in the root namespace after the fighter body
        // has run.  Always assign the marker: an external program may not
        // spoof a builtin roster module by declaring this private global.
        let canonical_module = BUILTIN_SCRIPTS
            .iter()
            .find(|builtin| builtin.source == source)
            .and_then(|builtin| builtin.filename.strip_suffix(".py"))
            .map_or_else(|| "None".to_owned(), |module| format!("{module:?}"));
        let runtime_source =
            format!("{source}\n__skirmish_canonical_module__ = {canonical_module}");
        let compiled = starlark::CompiledProgram::new_with_bundle(
            Arc::<str>::from(runtime_source),
            Arc::<str>::from(filename),
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
        // Program is also a standalone resource boundary: embedded programs
        // are deserialized and cached without passing through Definition.
        // Link custom action identities before warming any derived indexes.
        crate::game::script::definition::validate_custom_action_collisions(&manifest)?;
        crate::game::script::definition::validate_custom_slippi_states(&manifest)?;
        crate::game::script::definition::validate_action_animation_metadata(&manifest)?;
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
            let mut selector = callback_routing::CallbackSelector::from_binding(binding)
                .map_err(Error::Invalid)?;
            selector.buttons_all = binding.buttons_all;
            callback_bindings[binding.hook.index()].push(callback_routing::ResolvedCallback {
                callback: compiled.bind_callback(&binding.callback),
                selector,
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
                    let mut selector = callback_routing::CallbackSelector::from_binding(binding)
                        .map_err(Error::Invalid)?;
                    selector.buttons_all = binding.buttons_all;
                    result[binding.hook.index()].push(callback_routing::ResolvedCallback {
                        callback: compiled.bind_callback(&binding.callback),
                        selector,
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
        let fast_root_callbacks = std::array::from_fn(|index| {
            behavior_bindings.is_empty()
                && callback_bindings[index].len() == 1
                && is_unconditional_selector(&callback_bindings[index][0].selector)
        });
        let dependency_sources = assets.dependencies(&source)?;
        Ok(Self {
            source: Arc::from(source),
            dependency_sources: Arc::new(dependency_sources),
            compiled: Some(compiled),
            hook_indices,
            fast_root_callbacks,
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
        // Resource loading precomputes whether each hook has any bindings.
        // Most per-frame hooks are absent for a given fighter; skip context
        // construction and selector scans while preserving the same neutral
        // state and hit-baseline transaction.
        if self.hook_indices[hook.index()].is_none() {
            return Ok(neutral_script_result(
                hit,
                baseline,
                context_state.persistent,
                context_state.action_state,
            ));
        }
        let fast_root = self.fast_root_callbacks[hook.index()];
        let context = (!fast_root).then(|| {
            serde_json::json!({
                "event": {"kind": hook.name(), "action": fighter.action},
            })
        });
        let current_action = if fast_root {
            None
        } else {
            parse_action(&fighter.action)
        };
        let resource_ref = context_state.resources.as_deref();
        // Root bindings are immutable after resource load. Keep one slice for
        // both selection and the empty fast path; dispatch is a frame hot path
        // and should not repeatedly resolve the same hook index.
        let root_bindings = self.callback_bindings(None, hook);
        let selected_root = root_bindings.iter().filter(|binding| {
            fast_root
                || callback_routing::matches(
                    hook,
                    &binding.selector,
                    context
                        .as_ref()
                        .expect("selector context for normal dispatch"),
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
                    context
                        .as_ref()
                        .expect("selector context for normal dispatch"),
                    current_action,
                    resource_ref,
                )
            });
        let mut selected = selected_root.chain(selected_behaviors).peekable();
        if selected.peek().is_none() {
            return Ok(neutral_script_result(
                hit,
                baseline,
                context_state.persistent,
                context_state.action_state,
            ));
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
        // Keep one prepared evaluator invocation scope for the complete
        // callback batch. Each callback still receives its own native host
        // registration inside `dispatch_in_scope`, while the expensive
        // evaluator guards and thread-local preparation are shared.
        compiled
            .with_invocation_scope(|scope| {
                for binding in selected {
                    compiled.dispatch_in_scope(
                        scope,
                        &binding.callback,
                        primary.clone(),
                        &extra,
                    )?;
                }
                Ok(())
            })
            .map_err(|error| Error::Runtime(error.to_string()))?;
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

fn neutral_script_result(
    hit: Option<&HitView>,
    baseline: Option<&HitPatch>,
    persistent: &LocalState,
    action_state: &LocalState,
) -> ScriptResult {
    ScriptResult {
        locals: persistent.clone(),
        action_state: action_state.clone(),
        hit: baseline.cloned().or_else(|| {
            hit.map(|value| HitPatch {
                damage: value.damage,
                angle: value.angle,
                knockback: value.knockback,
                ..HitPatch::default()
            })
        }),
        ..ScriptResult::default()
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

/// Hash a qualified source action using a fixed algorithm whose output is
/// independent of process state, allocator state, or definition order.
pub fn custom_action_id(namespace: &str, key: &str) -> crate::game::CustomActionId {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in namespace.bytes().chain([0]).chain(key.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    crate::game::CustomActionId::new(hash)
}

/// Stable ID for a native source action.  The two numeric identities are
/// hashed directly, so equal Slippi states on different roster fighters never
/// alias merely because their authored names happen to match.
pub fn source_action_id(external_id: u8, slippi_state: u32) -> crate::game::CustomActionId {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in b"source"
        .iter()
        .copied()
        .chain([0])
        .chain(external_id.to_le_bytes())
        .chain(slippi_state.to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    crate::game::CustomActionId::new(hash)
}

/// Parse the canonical authoring reference `Custom.<namespace>:<key>`.
/// Namespace and key are intentionally retained in the definition linker for
/// collision diagnostics; the per-frame action only carries the stable ID.
pub(crate) fn parse_custom_action(name: &str) -> Option<super::Action> {
    let value = name.strip_prefix("Custom.")?;
    let (namespace, key) = value.split_once(':')?;
    if namespace.is_empty()
        || key.is_empty()
        || namespace.chars().any(char::is_whitespace)
        || key.chars().any(char::is_whitespace)
    {
        return None;
    }
    Some(super::Action::Custom(custom_action_id(namespace, key)))
}

fn parse_source_action(name: &str) -> Option<super::Action> {
    let value = name.strip_prefix("Source.")?;
    let (external, state) = value.split_once(':')?;
    let external = external.parse::<u8>().ok()?;
    let state = state.parse::<u32>().ok()?;
    Some(super::Action::Custom(source_action_id(external, state)))
}

pub(crate) fn parse_action(name: &str) -> Option<super::Action> {
    if let Some(action) = parse_source_action(name) {
        return Some(action);
    }
    if let Some(action) = parse_custom_action(name) {
        return Some(action);
    }
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
    use super::{custom_action_id, parse_action, source_action_id};
    use crate::game::{Action, CustomActionId};

    #[test]
    fn appeal_acronym_spellings_round_trip() {
        for spelling in ["appeal_sr", "APPEAL_SR", "AppealSR", "appeal_s_r"] {
            assert_eq!(parse_action(spelling), Some(Action::AppealSR), "{spelling}");
        }
        for spelling in ["appeal_sl", "APPEAL_SL", "AppealSL", "appeal_s_l"] {
            assert_eq!(parse_action(spelling), Some(Action::AppealSL), "{spelling}");
        }
    }

    #[test]
    fn custom_action_ids_are_stable_and_qualified() {
        let first = custom_action_id("fighter.training", "special.phase_a");
        assert_eq!(
            first,
            custom_action_id("fighter.training", "special.phase_a")
        );
        assert_ne!(first, custom_action_id("fighter.other", "special.phase_a"));
        assert_eq!(
            parse_action("Custom.fighter.training:special.phase_a"),
            Some(Action::Custom(first))
        );
        assert_eq!(parse_action("Custom.unqualified"), None);
    }

    #[test]
    fn custom_action_serde_round_trip_is_checkpoint_safe() {
        let action = Action::Custom(CustomActionId::new(0xfeed_beef));
        let bytes = serde_json::to_vec(&action).unwrap();
        assert_eq!(serde_json::from_slice::<Action>(&bytes).unwrap(), action);
    }

    #[test]
    fn source_action_ids_are_stable_and_roster_scoped() {
        let donkey = source_action_id(1, 381);
        assert_eq!(donkey, source_action_id(1, 381));
        assert_ne!(donkey, source_action_id(15, 381));
        assert_eq!(parse_action("Source.1:381"), Some(Action::Custom(donkey)));
        assert_eq!(
            parse_action("Source.15:381"),
            Some(Action::Custom(source_action_id(15, 381)))
        );
        assert_eq!(parse_action("Source.1"), None);
    }
}

#[cfg(test)]
mod dispatch_guard_tests {
    use super::{HitPatch, HitView, LocalState, is_unconditional_selector, neutral_script_result};
    use crate::game::script::Hook;
    use crate::game::script::callback_routing::{CallbackSelector, matches};

    #[test]
    fn nonmatching_selector_is_a_neutral_transaction_with_baseline() {
        let context = serde_json::json!({
            "event": {"kind": "before_hit", "action": "Action.Wait"}
        });
        let selector = CallbackSelector {
            action: Some(crate::game::Action::SpecialNStart),
            ..CallbackSelector::default()
        };
        assert!(!matches(Hook::BeforeHit, &selector, &context, None, None));

        let persistent =
            LocalState::from([(String::from("charge"), super::LocalValue::Integer(3))]);
        let action_state =
            LocalState::from([(String::from("armed"), super::LocalValue::Bool(true))]);
        let baseline = HitPatch {
            cancelled: true,
            damage: 7.0,
            ..HitPatch::default()
        };
        let result = neutral_script_result(
            Some(&HitView::default()),
            Some(&baseline),
            &persistent,
            &action_state,
        );
        assert_eq!(result.locals, persistent);
        assert_eq!(result.action_state, action_state);
        assert_eq!(result.hit, Some(baseline));
        assert!(result.commands.is_empty());
    }

    #[test]
    fn fast_path_accepts_only_the_empty_selector() {
        let default = CallbackSelector::default();
        assert!(is_unconditional_selector(&default));

        let mut action = default.clone();
        action.action = Some(crate::game::Action::SpecialNStart);
        assert!(!is_unconditional_selector(&action));

        let mut button = default.clone();
        button.buttons = Some(1);
        assert!(!is_unconditional_selector(&button));

        let mut chord = default;
        chord.buttons = Some(3);
        chord.buttons_all = true;
        assert!(!is_unconditional_selector(&chord));
    }
}

#[cfg(test)]
mod combat_resource_tests {
    use super::{CombatContext, FighterView, HitView, LocalState, Program};
    use crate::game::script::definition::AssetStore;
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
            special_attributes: None,
            animations: None,
            articles: None,
            resources: Resources::new(BTreeMap::from([
                (
                    "side".into(),
                    serde_json::json!({
                        "attributes": {
                            "specials_gr_vel_x": 0.75,
                            "specials_grav": 0.1,
                            "specials_terminal_vel": 2.0,
                            "specials_miss_landing_lag": 0.0,
                            "specials_hit_landing_lag": 0.0,
                        },
                    }),
                ),
                (
                    "up".into(),
                    serde_json::json!({
                        "capture": {
                            "attachment": {
                                "holder_bone": 0,
                                "holder_point": [0.0, 0.0, 0.0],
                                "victim_bone": 0,
                                "victim_point": [0.0, 0.0, 0.0]
                            },
                            "throw": {
                                "release_frame": 0,
                                "hit": {
                                    "damage": 12,
                                    "angle_raw": 361,
                                    "growth": 82,
                                    "fixed": 0,
                                    "base": 40,
                                    "element": 1
                                }
                            }
                        }
                    }),
                ),
            ]))
            .expect("resource fixture indexes"),
        });
        let resources = Arc::new(
            ResourceCache::build(Some(&data.fighters[0]), None, false)
                .expect("resource fixture builds"),
        );
        let assets = AssetStore::builtins();
        let source = include_str!("../../scripts/fighters/captain.py").replace(
            "class CaptainFalcon(Fighter):",
            "class CaptainFalcon(Fighter):\n    name = \"captain_falcon\"\n    external_ids = (0,)",
        );
        let program = Program::new_registered(source, &assets)
            .expect("Captain source and registered shared assets compile");
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
