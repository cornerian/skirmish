//! Values and the deliberately small native ABI used by the typed script VM.
//!
//! Values in this module are owned Rust values.  They do not contain parser
//! nodes, interpreter objects, or references into a temporary VM heap.  A
//! native object is only a path into a host-owned object graph; reads and
//! writes are performed by [`NativeHost`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

/// Errors shared by compilation, VM execution, and the native bridge.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("Pon gameplay backend is not integrated")]
    BackendUnavailable,
    #[error("source `{0}` exceeds the source size limit")]
    SourceTooLarge(String),
    #[error("compile error: {0}")]
    Compile(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("invalid script value: {0}")]
    Invalid(String),
    #[error("native host error: {0}")]
    Host(String),
    #[error("script instruction budget exhausted")]
    FuelExhausted,
    #[error("script call stack limit exceeded")]
    CallStackOverflow,
    #[error("script memory limit exceeded")]
    MemoryLimit,
    #[error("script transaction failed: {0}")]
    Transaction(String),
    #[error("script type error: {0}")]
    Type(String),
    #[error("script integer arithmetic overflow")]
    IntegerOverflow,
    #[error("script integer division by zero")]
    DivisionByZero,
}

/// A path-spanned execution error.  VM APIs return [`Error`] for compatibility
/// with the host API; this wrapper is useful to callers that need source
/// location and call stack information without parsing an error string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionError {
    pub error: String,
    pub span: Option<Span>,
    pub call_stack: Vec<String>,
}

/// A half-open source range.  Frontends may use byte offsets or line/column
/// offsets, as long as both values use the same coordinate system.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

/// Values exchanged with the native host and held in VM registers.
///
/// `F32` is intentionally distinct from `Int`: arithmetic involving an f32
/// rounds to f32 after each operation.  Lists and dictionaries are bounded
/// temporary values used by helpers and record construction; they are not a
/// general purpose object heap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NativeValue {
    None,
    Bool(bool),
    Int(i64),
    F32(f32),
    String(String),
    Vec2([f32; 2]),
    Object(NativeObject),
    List(Vec<NativeValue>),
    /// Immutable Python tuple. This is kept distinct from List at the native
    /// boundary so persistent state cannot accidentally acquire mutable heap
    /// semantics.
    Tuple(Vec<NativeValue>),
    Dict(BTreeMap<String, NativeValue>),
}

pub type HostValue = NativeValue;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeObject {
    pub kind: NativeKind,
    pub path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NativeKind {
    Fighter,
    Context,
    Hit,
    Input,
    Vec2,
    F32,
    State,
    Value,
}

impl NativeKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fighter => "Fighter",
            Self::Context => "Context",
            Self::Hit => "Hit",
            Self::Input => "Input",
            Self::Vec2 => "Vec2",
            Self::F32 => "F32",
            Self::State => "State",
            Self::Value => "Value",
        }
    }
}

impl fmt::Display for NativeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Native calls receive owned snapshots and cannot retain VM registers.
pub trait NativeHost: Send {
    fn get(&mut self, path: &str) -> Result<NativeValue, Error>;
    fn set(&mut self, path: &str, value: NativeValue) -> Result<(), Error>;
    fn call(&mut self, path: &str, args: &[NativeValue]) -> Result<NativeValue, Error>;

    /// Hosts with durable rollback support override these hooks. The defaults
    /// preserve the bridge ABI and execute operations sequentially.
    fn begin_transaction(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn commit_transaction(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn rollback_transaction(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Preserve keyword arguments at the bridge boundary.  The default keeps
    /// old positional hosts source-compatible while rejecting accidental loss
    /// of a named argument.
    fn call_named(
        &mut self,
        path: &str,
        args: &[NativeValue],
        named: &BTreeMap<String, NativeValue>,
    ) -> Result<NativeValue, Error> {
        if named.is_empty() {
            self.call(path, args)
        } else {
            Err(Error::Host(format!(
                "native call `{path}` does not accept named arguments"
            )))
        }
    }
}

pub type SharedNativeHost = Arc<Mutex<dyn NativeHost + Send>>;

pub fn shared_host<H: NativeHost + 'static>(host: H) -> SharedNativeHost {
    Arc::new(Mutex::new(host))
}

/// Convert a host reference into the native value passed to Pon callbacks.
pub fn host_object(reference: &HostRef) -> NativeValue {
    NativeValue::Object(NativeObject {
        kind: reference.kind,
        path: reference.path.clone(),
    })
}

/// A host-owned object reference.  Cloning this value only clones the path and
/// shared host handle; no host state is copied into the VM.
#[derive(Clone)]
pub struct HostRef {
    host: SharedNativeHost,
    pub kind: NativeKind,
    pub path: String,
}

impl HostRef {
    pub fn new(host: SharedNativeHost, kind: NativeKind, path: impl Into<String>) -> Self {
        Self {
            host,
            kind,
            path: path.into(),
        }
    }

    pub fn fighter(host: SharedNativeHost) -> Self {
        Self::new(host, NativeKind::Fighter, "fighter")
    }
    pub fn context(host: SharedNativeHost) -> Self {
        Self::new(host, NativeKind::Context, "context")
    }
    pub fn hit(host: SharedNativeHost) -> Self {
        Self::new(host, NativeKind::Hit, "hit")
    }
    pub fn input(host: SharedNativeHost) -> Self {
        Self::new(host, NativeKind::Input, "input")
    }

    pub fn host(&self) -> &SharedNativeHost {
        &self.host
    }

    pub fn child(&self, kind: NativeKind, suffix: impl AsRef<str>) -> Self {
        let suffix = suffix.as_ref();
        let path = if suffix.starts_with('[') || suffix.starts_with('.') {
            format!("{}{}", self.path, suffix)
        } else {
            format!("{}.{}", self.path, suffix)
        };
        Self::new(Arc::clone(&self.host), kind, path)
    }

    pub fn get(&self) -> Result<NativeValue, Error> {
        self.host
            .lock()
            .map_err(|_| Error::Host("native host lock poisoned".into()))?
            .get(&self.path)
    }
}

impl fmt::Debug for HostRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostRef")
            .field("kind", &self.kind)
            .field("path", &self.path)
            .finish()
    }
}

/// A callback transaction over a host-owned state object. Operations execute in
/// source order so calls observe preceding writes. Hosts that implement the
/// transaction hooks can roll back calls and writes together; default hooks are
/// deliberately no-ops because arbitrary host state cannot be cloned safely.
pub struct HostTransaction {
    host: SharedNativeHost,
}

impl HostTransaction {
    pub fn new(host: SharedNativeHost) -> Result<Self, Error> {
        host.lock()
            .map_err(|_| Error::Host("native host lock poisoned".into()))?
            .begin_transaction()?;
        Ok(Self { host })
    }

    pub fn get(&mut self, path: &str) -> Result<NativeValue, Error> {
        self.host
            .lock()
            .map_err(|_| Error::Host("native host lock poisoned".into()))?
            .get(path)
    }

    pub fn set(&mut self, path: impl AsRef<str>, value: NativeValue) -> Result<(), Error> {
        self.host
            .lock()
            .map_err(|_| Error::Host("native host lock poisoned".into()))?
            .set(path.as_ref(), value)
    }

    pub fn call(&mut self, path: &str, args: &[NativeValue]) -> Result<NativeValue, Error> {
        self.host
            .lock()
            .map_err(|_| Error::Host("native host lock poisoned".into()))?
            .call(path, args)
    }

    pub fn call_named(
        &mut self,
        path: &str,
        args: &[NativeValue],
        named: &BTreeMap<String, NativeValue>,
    ) -> Result<NativeValue, Error> {
        self.host
            .lock()
            .map_err(|_| Error::Host("native host lock poisoned".into()))?
            .call_named(path, args, named)
    }

    pub fn commit(&mut self) -> Result<(), Error> {
        self.host
            .lock()
            .map_err(|_| Error::Host("native host lock poisoned".into()))?
            .commit_transaction()
    }

    pub fn rollback(self) -> Result<(), Error> {
        self.host
            .lock()
            .map_err(|_| Error::Host("native host lock poisoned".into()))?
            .rollback_transaction()
    }

    pub fn discard(self) {
        let _ = self.rollback();
    }
}

impl NativeValue {
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::None => false,
            Self::Bool(value) => *value,
            Self::Int(value) => *value != 0,
            Self::F32(value) => *value != 0.0,
            Self::String(value) => !value.is_empty(),
            Self::Vec2(value) => value[0] != 0.0 || value[1] != 0.0,
            Self::Object(_) | Self::List(_) | Self::Tuple(_) | Self::Dict(_) => true,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::F32(x) => Some(*x),
            Self::Int(x) => Some(*x as f32),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(x) => Some(*x),
            _ => None,
        }
    }

    pub fn estimated_size(&self) -> usize {
        match self {
            Self::None | Self::Bool(_) => 1,
            Self::Int(_) | Self::F32(_) => 8,
            Self::String(value) => 8 + value.len(),
            Self::Vec2(_) => 8,
            Self::Object(value) => 16 + value.path.len(),
            Self::List(values) => values.iter().fold(8usize, |size, value| {
                size.saturating_add(value.estimated_size())
            }),
            Self::Tuple(values) => values.iter().fold(8usize, |size, value| {
                size.saturating_add(value.estimated_size())
            }),
            Self::Dict(values) => values.iter().fold(8usize, |size, (key, value)| {
                size.saturating_add(key.len())
                    .saturating_add(value.estimated_size())
            }),
        }
    }
}
