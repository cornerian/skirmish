//! Parser independent native resource schema types.

use crate::value::NativeKind;
use std::collections::BTreeMap;

/// Types used by the native host schema.
///
/// These describe values crossing the Pon/native boundary. They are kept
/// separate from Pon's compiler internals so schema inspection does not
/// depend on a custom bytecode representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueType {
    F32,
    I64,
    Bool,
    Unit,
    String,
    Handle,
    Option(Box<ValueType>),
    Sequence(Box<ValueType>),
    Tuple(Vec<ValueType>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostType {
    Value(ValueType),
    Object(NativeKind),
}

impl HostType {
    pub const fn bool() -> Self {
        Self::Value(ValueType::Bool)
    }
    pub const fn i64() -> Self {
        Self::Value(ValueType::I64)
    }
    pub const fn f32() -> Self {
        Self::Value(ValueType::F32)
    }
    pub const fn string() -> Self {
        Self::Value(ValueType::String)
    }
    pub fn as_value_type(&self) -> Option<ValueType> {
        match self {
            Self::Value(value) => Some(value.clone()),
            Self::Object(_) => Some(ValueType::Handle),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostField {
    pub ty: HostType,
    pub writable: bool,
}
impl HostField {
    pub fn read_only(ty: HostType) -> Self {
        Self {
            ty,
            writable: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostSchema {
    fields: BTreeMap<String, HostField>,
}
impl HostSchema {
    pub fn field(&self, path: &str) -> Option<&HostField> {
        self.fields.get(path)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Environment {
    schema: HostSchema,
}
impl Environment {
    pub fn host_schema(&self) -> &HostSchema {
        &self.schema
    }
    pub fn register_host_field(&mut self, path: String, field: HostField) -> Result<(), String> {
        use std::collections::btree_map::Entry;
        match self.schema.fields.entry(path.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(field);
                Ok(())
            }
            Entry::Occupied(_) => Err(format!("duplicate host field {path:?}")),
        }
    }
}
