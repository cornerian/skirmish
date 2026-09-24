//! Immutable links between class move identities and native entry actions.
//!
//! The authoring exporter intentionally keeps movesets as plain JSON so the
//! wire format can carry future groups and slots.  This module validates that
//! shape once at the resource boundary and exposes cheap typed lookups during
//! gameplay.

use crate::game::Action;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MoveGroup {
    Specials,
    Aerials,
    Grounded,
    Tilts,
    Smashes,
    Grabs,
    Throws,
    Defense,
    Ledge,
    Getup,
    Taunt,
    Extension(String),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MoveSlot {
    Neutral,
    Forward,
    Side,
    Back,
    Up,
    Down,
    Jab,
    RapidJab,
    Dash,
    Standing,
    Pummel,
    Shield,
    SpotDodge,
    RollForward,
    RollBack,
    AirDodge,
    Wait,
    Getup,
    Roll,
    Attack,
    Jump,
    Taunt,
    Extension(String),
}

impl MoveGroup {
    fn parse(value: &str) -> Self {
        match value {
            "specials" => Self::Specials,
            "aerials" => Self::Aerials,
            "grounded" => Self::Grounded,
            "tilts" => Self::Tilts,
            "smashes" => Self::Smashes,
            "grabs" => Self::Grabs,
            "throws" => Self::Throws,
            "defense" => Self::Defense,
            "ledge" => Self::Ledge,
            "getup" => Self::Getup,
            "taunt" => Self::Taunt,
            value => Self::Extension(value.to_owned()),
        }
    }
}

impl MoveSlot {
    fn parse(value: &str) -> Self {
        match value {
            "neutral" => Self::Neutral,
            "forward" => Self::Forward,
            "side" => Self::Side,
            "back" => Self::Back,
            "up" => Self::Up,
            "down" => Self::Down,
            "jab" => Self::Jab,
            "rapid_jab" => Self::RapidJab,
            "dash" => Self::Dash,
            "standing" => Self::Standing,
            "pummel" => Self::Pummel,
            "shield" => Self::Shield,
            "spot_dodge" => Self::SpotDodge,
            "roll_forward" => Self::RollForward,
            "roll_back" => Self::RollBack,
            "air_dodge" => Self::AirDodge,
            "wait" => Self::Wait,
            "getup" => Self::Getup,
            "roll" => Self::Roll,
            "attack" => Self::Attack,
            "jump" => Self::Jump,
            "taunt" => Self::Taunt,
            value => Self::Extension(value.to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveEntry {
    pub behavior_index: usize,
    /// `None` means this move is entered by its callback policy.
    pub canonical: Option<Action>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveRegistry {
    /// Dense links for the finite native group/slot vocabulary.  Runtime
    /// selectors use this path and avoid allocating or hashing strings.
    known: [[Option<MoveEntry>; SLOT_COUNT]; GROUP_COUNT],
    /// Extension links remain available to tooling and subclass variants.
    entries: BTreeMap<(MoveGroup, MoveSlot), MoveEntry>,
    /// Dense reverse membership used when callback dispatch supplies a
    /// behavior owner. Unreferenced behavior records are not moves.
    registered_behaviors: Vec<bool>,
}

const GROUP_COUNT: usize = 11;
const SLOT_COUNT: usize = 22;

fn group_index(group: &MoveGroup) -> Option<usize> {
    match group {
        MoveGroup::Specials => Some(0),
        MoveGroup::Aerials => Some(1),
        MoveGroup::Grounded => Some(2),
        MoveGroup::Tilts => Some(3),
        MoveGroup::Smashes => Some(4),
        MoveGroup::Grabs => Some(5),
        MoveGroup::Throws => Some(6),
        MoveGroup::Defense => Some(7),
        MoveGroup::Ledge => Some(8),
        MoveGroup::Getup => Some(9),
        MoveGroup::Taunt => Some(10),
        MoveGroup::Extension(_) => None,
    }
}

fn slot_index(slot: &MoveSlot) -> Option<usize> {
    match slot {
        MoveSlot::Neutral => Some(0),
        MoveSlot::Forward => Some(1),
        MoveSlot::Side => Some(2),
        MoveSlot::Back => Some(3),
        MoveSlot::Up => Some(4),
        MoveSlot::Down => Some(5),
        MoveSlot::Jab => Some(6),
        MoveSlot::RapidJab => Some(7),
        MoveSlot::Dash => Some(8),
        MoveSlot::Standing => Some(9),
        MoveSlot::Pummel => Some(10),
        MoveSlot::Shield => Some(11),
        MoveSlot::SpotDodge => Some(12),
        MoveSlot::RollForward => Some(13),
        MoveSlot::RollBack => Some(14),
        MoveSlot::AirDodge => Some(15),
        MoveSlot::Wait => Some(16),
        MoveSlot::Getup => Some(17),
        MoveSlot::Roll => Some(18),
        MoveSlot::Attack => Some(19),
        MoveSlot::Jump => Some(20),
        MoveSlot::Taunt => Some(21),
        MoveSlot::Extension(_) => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveRegistryError {
    GroupMustBeObject(String),
    SlotMustBeString {
        group: String,
        slot: String,
    },
    EmptyMoveId {
        group: String,
        slot: String,
    },
    DuplicateBehaviorId(String),
    UnknownMoveId {
        group: String,
        slot: String,
        id: String,
    },
    UnknownEntryAction {
        id: String,
        action: String,
    },
    SourceActionNamespaceMismatch {
        id: String,
        action: String,
        external_id: u8,
    },
}

impl fmt::Display for MoveRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GroupMustBeObject(group) => {
                write!(formatter, "moveset group {group:?} must be an object")
            }
            Self::SlotMustBeString { group, slot } => write!(
                formatter,
                "moveset {group:?}.{slot:?} must be a move identity string"
            ),
            Self::EmptyMoveId { group, slot } => write!(
                formatter,
                "moveset {group:?}.{slot:?} has an empty move identity"
            ),
            Self::DuplicateBehaviorId(id) => write!(formatter, "duplicate behavior id {id:?}"),
            Self::UnknownMoveId { group, slot, id } => write!(
                formatter,
                "moveset {group:?}.{slot:?} references unknown behavior {id:?}"
            ),
            Self::UnknownEntryAction { id, action } => write!(
                formatter,
                "behavior {id:?} references unknown entry action {action:?}"
            ),
            Self::SourceActionNamespaceMismatch {
                id,
                action,
                external_id,
            } => write!(
                formatter,
                "behavior {id:?} references source action {action:?} for external id {external_id}, which is not owned by the fighter"
            ),
        }
    }
}

impl std::error::Error for MoveRegistryError {}

impl MoveRegistry {
    pub fn compile(
        definition: &super::definition::FighterDefinition,
    ) -> Result<Self, MoveRegistryError> {
        let mut behavior_by_id = BTreeMap::new();
        let mut canonical_by_index = Vec::with_capacity(definition.behaviors.len());
        for (index, behavior) in definition.behaviors.iter().enumerate() {
            let canonical = behavior
                .entry_action
                .as_deref()
                .map(|name| {
                    if let Some(external_id) = source_action_external_id(name)
                        && !definition.external_ids.contains(&external_id)
                    {
                        return Err(MoveRegistryError::SourceActionNamespaceMismatch {
                            id: behavior.id.as_deref().unwrap_or("<unnamed>").to_owned(),
                            action: name.to_owned(),
                            external_id,
                        });
                    }
                    super::parse_action(name.strip_prefix("Action.").unwrap_or(name)).ok_or_else(
                        || MoveRegistryError::UnknownEntryAction {
                            id: behavior.id.as_deref().unwrap_or("<unnamed>").to_owned(),
                            action: name.to_owned(),
                        },
                    )
                })
                .transpose()?;
            let Some(id) = behavior.id.as_deref() else {
                canonical_by_index.push(canonical);
                continue;
            };
            if behavior_by_id.insert(id.to_owned(), index).is_some() {
                return Err(MoveRegistryError::DuplicateBehaviorId(id.to_owned()));
            }
            canonical_by_index.push(canonical);
        }

        let mut entries = BTreeMap::new();
        let mut registered_behaviors = vec![false; definition.behaviors.len()];
        for (group_name, group_value) in &definition.movesets {
            let Some(group) = group_value.as_object() else {
                return Err(MoveRegistryError::GroupMustBeObject(group_name.clone()));
            };
            for (slot_name, move_value) in group {
                let Some(id) = move_value.as_str() else {
                    return Err(MoveRegistryError::SlotMustBeString {
                        group: group_name.clone(),
                        slot: slot_name.clone(),
                    });
                };
                if id.is_empty() {
                    return Err(MoveRegistryError::EmptyMoveId {
                        group: group_name.clone(),
                        slot: slot_name.clone(),
                    });
                }
                let Some(&behavior_index) = behavior_by_id.get(id) else {
                    return Err(MoveRegistryError::UnknownMoveId {
                        group: group_name.clone(),
                        slot: slot_name.clone(),
                        id: id.to_owned(),
                    });
                };
                entries.insert(
                    (MoveGroup::parse(group_name), MoveSlot::parse(slot_name)),
                    MoveEntry {
                        behavior_index,
                        canonical: canonical_by_index[behavior_index],
                    },
                );
                registered_behaviors[behavior_index] = true;
            }
        }
        let mut known = [[None; SLOT_COUNT]; GROUP_COUNT];
        for ((group, slot), entry) in &entries {
            if let (Some(group), Some(slot)) = (group_index(group), slot_index(slot)) {
                known[group][slot] = Some(*entry);
            }
        }
        Ok(Self {
            known,
            entries,
            registered_behaviors,
        })
    }

    /// Resolve a typed group and slot without parsing or allocating strings.
    pub fn resolve_typed(&self, group: &MoveGroup, slot: &MoveSlot) -> Option<MoveEntry> {
        match (group_index(group), slot_index(slot)) {
            (Some(group), Some(slot)) => self.known[group][slot],
            _ => self.entries.get(&(group.clone(), slot.clone())).copied(),
        }
    }

    pub fn resolve(&self, group: &str, slot: &str) -> Option<MoveEntry> {
        self.resolve_typed(&MoveGroup::parse(group), &MoveSlot::parse(slot))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_registered_behavior(&self, behavior_index: usize) -> bool {
        self.registered_behaviors
            .get(behavior_index)
            .copied()
            .unwrap_or(false)
    }
}

fn source_action_external_id(name: &str) -> Option<u8> {
    let name = name.strip_prefix("Action.").unwrap_or(name);
    let value = name.strip_prefix("Source.")?;
    let (external_id, _) = value.split_once(':')?;
    external_id.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::script::definition::{BehaviorDefinition, FighterDefinition};

    fn definition() -> FighterDefinition {
        let mut movesets = BTreeMap::new();
        movesets.insert(
            "specials".into(),
            serde_json::json!({"neutral": "move_0", "side": "move_0"}),
        );
        FighterDefinition {
            name: "test".into(),
            external_ids: vec![8],
            movesets,
            behaviors: vec![BehaviorDefinition {
                id: Some("move_0".into()),
                entry_action: Some("Action.SpecialNStart".into()),
                ..BehaviorDefinition::default()
            }],
            ..FighterDefinition::default()
        }
    }

    #[test]
    fn shared_identity_reuses_behavior_index() {
        let registry = MoveRegistry::compile(&definition()).unwrap();
        assert_eq!(
            registry.resolve("specials", "neutral"),
            registry.resolve("specials", "side")
        );
        assert_eq!(
            registry.resolve("specials", "neutral").unwrap().canonical,
            Some(Action::SpecialNStart)
        );
    }

    #[test]
    fn malformed_reference_is_rejected() {
        let mut definition = definition();
        definition
            .movesets
            .insert("specials".into(), serde_json::json!({"neutral": "missing"}));
        assert!(matches!(
            MoveRegistry::compile(&definition),
            Err(MoveRegistryError::UnknownMoveId { .. })
        ));
    }

    #[test]
    fn unknown_entry_action_is_rejected() {
        let mut definition = definition();
        definition.behaviors[0].entry_action = Some("Action.NoSuchAction".into());
        assert!(matches!(
            MoveRegistry::compile(&definition),
            Err(MoveRegistryError::UnknownEntryAction { .. })
        ));
    }

    #[test]
    fn source_entry_actions_resolve_to_custom_native_actions() {
        let mut definition = definition();
        definition.behaviors[0].entry_action = Some("Action.Source.8:341".into());
        let registry = MoveRegistry::compile(&definition).unwrap();
        let action = registry.resolve("specials", "neutral").unwrap().canonical;
        assert_eq!(
            action.and_then(Action::custom_id).map(|id| id.get()),
            Some(crate::game::script::source_action_id(8, 341).get())
        );
    }

    #[test]
    fn source_entry_action_must_use_the_definition_namespace() {
        let mut definition = definition();
        definition.external_ids = vec![21];
        definition.behaviors[0].entry_action = Some("Action.Source.6:344".into());
        assert!(matches!(
            MoveRegistry::compile(&definition),
            Err(MoveRegistryError::SourceActionNamespaceMismatch { external_id: 6, .. })
        ));

        definition.behaviors[0].entry_action = Some("Action.Source.21:344".into());
        assert!(MoveRegistry::compile(&definition).is_ok());
    }

    #[test]
    fn malformed_source_entry_actions_are_rejected() {
        let mut definition = definition();
        definition.behaviors[0].entry_action = Some("Action.Source.bad".into());
        assert!(matches!(
            MoveRegistry::compile(&definition),
            Err(MoveRegistryError::UnknownEntryAction { .. })
        ));
    }

    #[test]
    fn duplicate_behavior_ids_are_rejected() {
        let mut definition = definition();
        definition.behaviors.push(BehaviorDefinition {
            id: Some("move_0".into()),
            ..BehaviorDefinition::default()
        });
        assert!(matches!(
            MoveRegistry::compile(&definition),
            Err(MoveRegistryError::DuplicateBehaviorId(_))
        ));
    }

    #[test]
    fn non_object_group_is_rejected() {
        let mut definition = definition();
        definition
            .movesets
            .insert("specials".into(), serde_json::json!("move_0"));
        assert!(matches!(
            MoveRegistry::compile(&definition),
            Err(MoveRegistryError::GroupMustBeObject(_))
        ));
    }

    #[test]
    fn typed_resolution_uses_side_slot() {
        let registry = MoveRegistry::compile(&definition()).unwrap();
        assert_eq!(
            registry.resolve_typed(&MoveGroup::Specials, &MoveSlot::Side),
            registry.resolve("specials", "side")
        );
    }
}
