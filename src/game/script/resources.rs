//! Generic, dotted-path lookup for character resources.

use crate::game::data::Attack;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AttackId(u32);

#[derive(Clone, Debug, Default, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    #[serde(flatten)]
    pub values: BTreeMap<String, Value>,
    #[serde(skip)]
    attacks: Vec<Attack>,
    #[serde(skip)]
    attack_ids: BTreeMap<String, AttackId>,
}

impl Resources {
    pub fn new(values: BTreeMap<String, Value>) -> Result<Self, String> {
        let mut result = Self {
            values,
            attacks: Vec::new(),
            attack_ids: BTreeMap::new(),
        };
        result.index_attacks()?;
        Ok(result)
    }

    pub fn lookup(&self, path: &str) -> Option<&Value> {
        let mut parts = path.split('.');
        let mut value = self.values.get(parts.next()?)?;
        for part in parts {
            value = value.get(part)?;
        }
        Some(value)
    }

    pub fn attack(&self, path: &str) -> Option<&Attack> {
        self.attack_id(path).and_then(|id| self.attack_by_id(id))
    }

    pub(crate) fn attack_id(&self, path: &str) -> Option<AttackId> {
        self.attack_ids.get(path).copied()
    }

    pub(crate) fn attack_by_id(&self, id: AttackId) -> Option<&Attack> {
        self.attacks.get(id.0 as usize)
    }

    /// Populate the typed cache once at resource load.  Simulation code never
    /// serializes or reparses the large pose/hitbox arrays per frame.
    pub fn index_attacks(&mut self) -> Result<(), String> {
        self.attacks.clear();
        self.attack_ids.clear();
        let keys: Vec<String> = self.values.keys().cloned().collect();
        for key in keys {
            self.index_value(&key)?;
        }
        Ok(())
    }

    fn index_value(&mut self, path: &str) -> Result<(), String> {
        let Some(value) = self.lookup(path) else {
            return Ok(());
        };
        if value.get("frames").is_some() {
            // The typed cache owns one copy of each attack. Keep traversal
            // borrowed so nested resource trees are not cloned repeatedly.
            let attack = serde_json::from_value::<Attack>(value.clone())
                .map_err(|error| format!("invalid attack resource {path}: {error}"))?;
            let id = AttackId(self.attacks.len() as u32);
            self.attacks.push(attack);
            self.attack_ids.insert(path.to_owned(), id);
            return Ok(());
        }
        if let Some(object) = value.as_object() {
            let children: Vec<String> = object.keys().map(|k| format!("{path}.{k}")).collect();
            for child in children {
                self.index_value(&child)?;
            }
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Resources {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let values = BTreeMap::<String, Value>::deserialize(deserializer)?;
        Self::new(values).map_err(serde::de::Error::custom)
    }
}

impl PartialEq for Resources {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Specials {
    pub character: String,
    #[serde(flatten)]
    pub resources: Resources,
}

impl<'de> Deserialize<'de> for Specials {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut fields = BTreeMap::<String, Value>::deserialize(deserializer)?;
        let character = match fields.remove("character") {
            Some(Value::String(value)) => value,
            Some(_) => {
                return Err(serde::de::Error::custom(
                    "specials.character must be a string",
                ));
            }
            None => return Err(serde::de::Error::missing_field("character")),
        };
        let resources = Resources::new(fields).map_err(serde::de::Error::custom)?;
        Ok(Self {
            character,
            resources,
        })
    }
}

impl Specials {
    pub fn lookup(&self, path: &str) -> Option<&Value> {
        self.resources.lookup(path)
    }
    pub fn attack(&self, path: &str) -> Option<&Attack> {
        self.resources.attack(path)
    }
    pub fn character_key(&self) -> String {
        self.character.to_ascii_lowercase()
    }
}
