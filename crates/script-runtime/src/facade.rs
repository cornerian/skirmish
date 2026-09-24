//! Native Pon execution facade.

use crate::value::{Error, HostRef, NativeKind, NativeValue};
use skirmish_pon_runtime::{
    HostScope, NativeHostProxy, Program, Value, native_call, native_call_named, native_get,
    native_set, register_native_module,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, OnceLock},
};

/// A callback identity can only be produced by an integrated Pon backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallbackHandle {
    owner: u64,
    name: Arc<str>,
    slot: Option<usize>,
}

impl CallbackHandle {
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self {
            owner: 0,
            name: name.into(),
            slot: None,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A loaded gameplay backend program. Construction prepares the program on
/// the loading thread; additional gameplay threads must call
/// [`Self::prepare_for_current_thread`] before invoking callbacks.
#[derive(Clone, Debug)]
pub struct CompiledProgram {
    source: Arc<str>,
    root_source: Arc<str>,
    filename: Arc<str>,
    callbacks: Arc<[CallbackHandle]>,
    dispatch_index: usize,
    move_args_index: usize,
    move_result_index: usize,
    callback_indices: Arc<HashMap<String, usize>>,
    identity: u64,
    bundle: Option<Arc<skirmish_pon_runtime::SourceBundle>>,
}

/// Invocation state owned by one compiled program. The runtime scope is
/// intentionally wrapped so a scope cannot be paired with another program's
/// callback table.
pub struct InvocationScope<'scope, 'program> {
    owner: u64,
    inner: &'scope mut skirmish_pon_runtime::InvocationScope<'program>,
    /// Reused backing storage for the transient Pon callback argument list.
    /// The list is returned after each call; host scopes and tokens remain
    /// callback-local so proxies retained by Pon expire exactly as before.
    callback_args: Vec<Value>,
}

const BOOTSTRAP: &str = r#"
from skirmish._loader import export as __skirmish_export_impl, dispatch as __skirmish_dispatch_impl, move_args as __skirmish_move_args_impl, unwrap as __skirmish_unwrap_impl
__skirmish_bundle__ = __skirmish_export_impl(globals())
def __skirmish_export__():
    return __skirmish_bundle__["definition"]
def __skirmish_callbacks__():
    return list(__skirmish_bundle__["callback_names"])
def __skirmish_dispatch__(index, args):
    return __skirmish_dispatch_impl(__skirmish_bundle__, index, args)
def __skirmish_move_args__(behavior_index, fighter, action):
    return __skirmish_move_args_impl(__skirmish_bundle__, behavior_index, fighter, action)
def __skirmish_move_result__(value):
    return __skirmish_unwrap_impl(value)
"#;

include!(concat!(env!("OUT_DIR"), "/authoring_files.rs"));

fn authoring_bundle() -> Result<skirmish_pon_runtime::SourceBundle, Error> {
    let mut bundle = skirmish_pon_runtime::SourceBundle::new("fighter-api-embedded-v1");
    for &(path, source) in AUTHORING_FILES {
        bundle = bundle
            .with_file(path, source)
            .map_err(|error| Error::Compile(error.to_string()))?;
    }
    Ok(bundle)
}

/// Content identity of the embedded Pon authoring SDK.
pub fn embedded_sdk_identity() -> Result<[u8; 32], Error> {
    Ok(authoring_bundle()?.identity_digest())
}

static NEXT_IDENTITY: OnceLock<std::sync::atomic::AtomicU64> = OnceLock::new();

thread_local! {
    static PREPARED: RefCell<HashMap<u64, skirmish_pon_runtime::PreparedProgram>> =
        RefCell::new(HashMap::new());
}

static NATIVE_MODULE: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();

fn ensure_native_module() -> Result<(), Error> {
    crate::register_native_math().map_err(Error::Runtime)?;
    let result = NATIVE_MODULE.get_or_init(|| {
        register_native_module(
            "_skirmish_native",
            [
                ("get", native_get, 2),
                ("set", native_set, 3),
                ("call", native_call, 3),
                ("call_named", native_call_named, 4),
            ],
        )
        .map_err(|error| error.to_string())
    });
    result
        .as_ref()
        .map(|_| ())
        .map_err(|error| Error::Runtime(error.clone()))
}

impl CompiledProgram {
    /// Slot of the internal move-argument factory callback.
    #[allow(dead_code)] // Exposed for the pending native move dispatch integration.
    pub(crate) fn move_args_slot(&self) -> usize {
        self.move_args_index
    }

    #[cfg(feature = "experimental-continuations")]
    pub fn invoke_native_entry(
        &self,
        entry: &mut skirmish_pon_runtime::continuation::NativeEntry,
        behavior_index: usize,
        fighter: HostRef,
        action: NativeValue,
        scalar_spills: &[Value],
    ) -> Result<NativeValue, Error> {
        self.invoke_native_entry_in_module(
            "__root__",
            entry,
            behavior_index,
            fighter,
            action,
            scalar_spills,
        )
    }

    /// Invoke a lowered async entry while its defining Python module is the
    /// active global namespace. Inherited move methods can therefore retain
    /// globals from their helper module without relying on ambient imports.
    #[cfg(feature = "experimental-continuations")]
    pub fn invoke_native_entry_in_module(
        &self,
        module_name: &str,
        entry: &mut skirmish_pon_runtime::continuation::NativeEntry,
        behavior_index: usize,
        fighter: HostRef,
        action: NativeValue,
        scalar_spills: &[Value],
    ) -> Result<NativeValue, Error> {
        self.require_prepared_for_current_thread()?;
        let scope = HostScope::new(self.identity);
        let token_cell = std::rc::Rc::new(std::cell::Cell::new(0));
        let id = scope
            .register(PonHost {
                host: Arc::clone(fighter.host()),
                token: std::rc::Rc::clone(&token_cell),
            })
            .map_err(|error| Error::Runtime(error.to_string()))?;
        let token = id
            .token()
            .ok_or_else(|| Error::Runtime("native host token overflow".into()))?;
        token_cell.set(token);
        let _guard = scope.activate();
        let args = [
            Value::Int(behavior_index as i64),
            native_object_value(token, &fighter.kind, &fighter.path),
            native_to_pon(&action, token)?,
        ];
        PREPARED.with(|cache| {
            let mut cache = cache
                .try_borrow_mut()
                .map_err(|_| Error::Runtime("Pon prepared cache is borrowed".into()))?;
            let prepared = cache
                .get_mut(&self.identity)
                .ok_or_else(|| Error::Runtime("Pon prepared cache entry disappeared".into()))?;
            prepared
                .invoke_native_entry_with_result_adapter_in_module(
                    module_name,
                    entry,
                    self.move_args_index,
                    &args,
                    scalar_spills,
                    Some(self.move_result_index),
                )
                .map(native_to_native)
                .map_err(|error| Error::Runtime(error.to_string()))?
        })
    }

    #[cfg(feature = "experimental-continuations")]
    pub fn invoke_move_step(
        &self,
        step: &mut skirmish_pon_runtime::continuation::NativeStep,
        phase: skirmish_pon_runtime::continuation::StepPhase,
        behavior_index: usize,
        fighter: HostRef,
        action: NativeValue,
        scalar: Option<Value>,
    ) -> Result<NativeValue, Error> {
        self.require_prepared_for_current_thread()?;
        let scope = HostScope::new(self.identity);
        let token_cell = std::rc::Rc::new(std::cell::Cell::new(0));
        let id = scope
            .register(PonHost {
                host: Arc::clone(fighter.host()),
                token: std::rc::Rc::clone(&token_cell),
            })
            .map_err(|error| Error::Runtime(error.to_string()))?;
        let token = id
            .token()
            .ok_or_else(|| Error::Runtime("native host token overflow".into()))?;
        token_cell.set(token);
        let _guard = scope.activate();
        let args = [
            Value::Int(behavior_index as i64),
            native_object_value(token, &fighter.kind, &fighter.path),
            native_to_pon(&action, token)?,
        ];
        PREPARED.with(|cache| {
            let mut cache = cache
                .try_borrow_mut()
                .map_err(|_| Error::Runtime("Pon prepared cache is borrowed".into()))?;
            let prepared = cache
                .get_mut(&self.identity)
                .ok_or_else(|| Error::Runtime("Pon prepared cache entry disappeared".into()))?;
            prepared
                .invoke_move_step_with_result_adapter(
                    step,
                    phase,
                    self.move_args_index,
                    &args,
                    scalar,
                    Some(self.move_result_index),
                )
                .map(native_to_native)
                .map_err(|error| Error::Runtime(error.to_string()))?
        })
    }

    pub fn new(
        source: impl Into<Arc<str>>,
        filename: impl Into<Arc<str>>,
        callbacks: impl IntoIterator<Item = CallbackHandle>,
    ) -> Result<Self, Error> {
        Self::new_with_bundle(source, filename, callbacks, None)
    }

    pub fn new_with_bundle(
        source: impl Into<Arc<str>>,
        filename: impl Into<Arc<str>>,
        callbacks: impl IntoIterator<Item = CallbackHandle>,
        bundle: Option<skirmish_pon_runtime::SourceBundle>,
    ) -> Result<Self, Error> {
        let root_source: Arc<str> = source.into();
        let filename = filename.into();
        if filename.is_empty() {
            return Err(Error::Invalid("Pon filename cannot be empty".into()));
        }
        // Identity is loader provenance, never authored data.  Assign the
        // private marker after the root body so an arbitrary script cannot
        // spoof a roster entry by declaring the same global.  The trusted
        // table matches the exact bundled source bytes and private filename;
        // ordinary callers receive `None` and must declare explicit identity.
        let canonical_module = crate::builtin_roster::module_for(&root_source, &filename)
            .map_or_else(|| "None".to_owned(), |module| format!("{module:?}"));
        let source = format!(
            "{root_source}\n__skirmish_canonical_module__ = {canonical_module}\n{BOOTSTRAP}"
        );
        if source.len() > 256 * 1024 {
            return Err(Error::SourceTooLarge("Pon source".into()));
        }
        let identity = NEXT_IDENTITY
            .get_or_init(|| std::sync::atomic::AtomicU64::new(1))
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut callbacks = callbacks
            .into_iter()
            .map(|mut callback| {
                callback.owner = identity;
                callback
            })
            .collect::<Vec<_>>();
        callbacks.extend([
            CallbackHandle {
                owner: identity,
                name: Arc::from("__skirmish_export__"),
                slot: None,
            },
            CallbackHandle {
                owner: identity,
                name: Arc::from("__skirmish_dispatch__"),
                slot: None,
            },
            CallbackHandle {
                owner: identity,
                name: Arc::from("__skirmish_callbacks__"),
                slot: None,
            },
            CallbackHandle {
                owner: identity,
                name: Arc::from("__skirmish_move_args__"),
                slot: None,
            },
            CallbackHandle {
                owner: identity,
                name: Arc::from("__skirmish_move_result__"),
                slot: None,
            },
        ]);
        let dispatch_index = callbacks
            .iter()
            .position(|callback| callback.name() == "__skirmish_dispatch__")
            .expect("bootstrap dispatch callback is registered");
        let move_args_index = callbacks
            .iter()
            .position(|callback| callback.name() == "__skirmish_move_args__")
            .expect("bootstrap move args callback is registered");
        let move_result_index = callbacks
            .iter()
            .position(|callback| callback.name() == "__skirmish_move_result__")
            .expect("bootstrap move result callback is registered");
        let mut program = Self {
            source: source.into(),
            root_source,
            filename,
            callbacks: callbacks.into(),
            dispatch_index,
            move_args_index,
            move_result_index,
            callback_indices: Arc::new(HashMap::new()),
            identity,
            bundle: bundle.map(Arc::new),
        };
        // Construction is the resource-load boundary. Prepare once here so
        // existing loader/export callers receive an executable program, while
        // callers that migrate gameplay to another thread can explicitly
        // prepare that thread through `prepare_for_current_thread`.
        program.prepare_for_current_thread()?;
        let indices = program
            .callback_keys()?
            .into_iter()
            .enumerate()
            .map(|(index, name)| (name, index))
            .collect();
        program.callback_indices = Arc::new(indices);
        Ok(program)
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// Resolve a defining module from the exact immutable source bundle used
    /// to prepare this program. `__root__` denotes the authored root module.
    pub fn source_for_module(&self, module: &str) -> Option<&str> {
        if module == "__root__" {
            return Some(&self.root_source);
        }
        let path = format!("{}.py", module.replace('.', "/"));
        self.bundle.as_deref()?.source(&path).or_else(|| {
            self.bundle
                .as_deref()?
                .source(&format!("{}/__init__.py", module.replace('.', "/")))
        })
    }

    /// Stable process-local identity used by thread-local gameplay caches.
    pub fn identity(&self) -> u64 {
        self.identity
    }

    pub fn callbacks(&self) -> impl Iterator<Item = CallbackHandle> + '_ {
        self.callbacks.iter().cloned()
    }

    pub fn callback(&self, name: &str) -> Option<CallbackHandle> {
        self.callbacks
            .iter()
            .find(|callback| callback.name() == name)
            .cloned()
    }

    pub fn bind_callback(&self, name: &str) -> CallbackHandle {
        CallbackHandle {
            owner: self.identity,
            name: Arc::from(name),
            // Binding is a load-time operation. Resolve the logical callback
            // to the immutable exported slot once; dispatch carries only the
            // integer thereafter.
            slot: self.callback_indices.get(name).copied(),
        }
    }

    pub fn callback_keys(&self) -> Result<Vec<String>, Error> {
        let callback = CallbackHandle {
            owner: self.identity,
            name: Arc::from("__skirmish_callbacks__"),
            slot: None,
        };
        match self.invoke_values(&callback, &[])? {
            NativeValue::List(values) => values
                .into_iter()
                .map(|value| match value {
                    NativeValue::String(value) => Ok(value),
                    _ => Err(Error::Invalid("callback key is not a string".into())),
                })
                .collect(),
            _ => Err(Error::Invalid("callback export is not a list".into())),
        }
    }

    /// Prepare this program for execution on the calling thread.
    ///
    /// Pon keeps thread-affine evaluator state.  Callers must invoke this at
    /// the resource or session boundary before dispatching gameplay callbacks
    /// on a thread; gameplay invocation never performs compilation or source
    /// discovery implicitly.
    pub fn prepare_for_current_thread(&self) -> Result<(), Error> {
        ensure_native_module()?;
        crate::stdlib_config::configure_from_environment_if_needed().map_err(Error::Runtime)?;
        PREPARED.with(|cache| {
            let mut cache = cache
                .try_borrow_mut()
                .map_err(|_| Error::Runtime("Pon prepared cache is borrowed".into()))?;
            if let std::collections::hash_map::Entry::Vacant(entry) = cache.entry(self.identity) {
                if crate::stdlib_config::configured().is_none() {
                    crate::stdlib_config::freeze_unconfigured().map_err(Error::Runtime)?;
                }
                let mut program = Program::new(
                    Arc::clone(&self.source),
                    Arc::clone(&self.filename),
                    self.callbacks
                        .iter()
                        .map(|callback| callback.name.to_string()),
                );
                if let Some(library) = crate::stdlib_config::configured() {
                    program = program.with_standard_library(&library);
                }
                let embedded = authoring_bundle()?;
                let bundle = match self.bundle.as_deref() {
                    Some(dependencies) => embedded
                        .merge(dependencies)
                        .map_err(|error| Error::Compile(error.to_string()))?,
                    None => embedded,
                };
                let materialized = bundle
                    .materialize(std::path::Path::new("/tmp/skirmish-pon-bundles"))
                    .map_err(|error| Error::Compile(error.to_string()))?;
                let prepared = program
                    .prepare_for_thread_in_bundle(&materialized)
                    .map_err(|error| Error::Compile(error.to_string()))?;
                entry.insert(prepared);
            }
            Ok(())
        })
    }

    fn require_prepared_for_current_thread(&self) -> Result<(), Error> {
        PREPARED.with(|cache| {
            let cache = cache
                .try_borrow()
                .map_err(|_| Error::Runtime("Pon prepared cache is borrowed".into()))?;
            if cache.contains_key(&self.identity) {
                Ok(())
            } else {
                Err(Error::Runtime(
                    "Pon program is not prepared for current thread; call prepare_for_current_thread before gameplay dispatch"
                        .into(),
                ))
            }
        })
    }

    pub fn invoke_values(
        &self,
        callback: &CallbackHandle,
        args: &[Value],
    ) -> Result<NativeValue, Error> {
        if callback.owner != self.identity {
            return Err(Error::Runtime(format!(
                "callback `{}` is not exported",
                callback.name()
            )));
        }
        // A slot resolved by `bind_callback` addresses the logical callback
        // table consumed by `__skirmish_dispatch__`.  `invoke_values`, in
        // contrast, addresses the raw Pon export table.  Treating the former
        // as the latter can execute an unrelated bootstrap callback (or
        // return its metadata) before Pon reports any useful error.
        if callback.slot.is_some() {
            return Err(Error::Runtime(format!(
                "callback `{}` is a logical host callback; use dispatch",
                callback.name()
            )));
        }
        self.require_prepared_for_current_thread()?;
        PREPARED.with(|cache| {
            let mut cache = cache
                .try_borrow_mut()
                .map_err(|_| Error::Runtime("Pon prepared cache is borrowed".into()))?;
            let prepared = cache
                .get_mut(&self.identity)
                .ok_or_else(|| Error::Runtime("Pon prepared cache entry disappeared".into()))?;
            prepared
                .invoke(callback.name(), args)
                .map(native_to_native)
                .map_err(|error| Error::Runtime(error.to_string()))?
        })
    }

    pub fn export_metadata(&self) -> Result<NativeValue, Error> {
        let callback = CallbackHandle {
            owner: self.identity,
            name: Arc::from("__skirmish_export__"),
            slot: None,
        };
        self.invoke_values(&callback, &[])
    }

    pub fn dispatch_named(
        &self,
        callback: &str,
        primary: HostRef,
        extra: &[NativeValue],
    ) -> Result<NativeValue, Error> {
        let handle = self.bind_callback(callback);
        self.dispatch(&handle, primary, extra)
    }

    pub fn dispatch(
        &self,
        callback: &CallbackHandle,
        primary: HostRef,
        extra: &[NativeValue],
    ) -> Result<NativeValue, Error> {
        if callback.owner != self.identity {
            return Err(Error::Runtime(
                "callback belongs to another Pon program".into(),
            ));
        }
        let index = callback.slot.ok_or_else(|| {
            Error::Runtime(format!("callback `{}` is not exported", callback.name()))
        })?;
        self.dispatch_bound(index, primary, extra)
    }

    /// Run several dispatches while keeping the runtime's expensive guards
    /// installed. The callback closure must still create a fresh native host
    /// scope for every callback.
    pub fn with_invocation_scope<R, F>(&self, body: F) -> Result<R, Error>
    where
        F: for<'scope, 'program> FnOnce(&mut InvocationScope<'scope, 'program>) -> Result<R, Error>,
    {
        self.require_prepared_for_current_thread()?;
        PREPARED.with(|cache| {
            let mut callback_error = None;
            let mut cache = cache
                .try_borrow_mut()
                .map_err(|_| Error::Runtime("Pon prepared cache is borrowed".into()))?;
            let prepared = cache
                .get_mut(&self.identity)
                .ok_or_else(|| Error::Runtime("Pon prepared cache entry disappeared".into()))?;
            prepared
                .with_invocation_scope(|scope| {
                    let mut scope = InvocationScope {
                        owner: self.identity,
                        inner: scope,
                        callback_args: Vec::new(),
                    };
                    match body(&mut scope) {
                        Ok(value) => Ok(value),
                        Err(error) => {
                            let message = error.to_string();
                            if !scope.inner.has_failed() {
                                callback_error = Some(error);
                            }
                            Err(skirmish_pon_runtime::Error::Runtime(message))
                        }
                    }
                })
                .map_err(|error| {
                    callback_error.unwrap_or_else(|| Error::Runtime(error.to_string()))
                })
        })
    }

    /// Dispatch a logical callback using an already-open invocation scope.
    pub fn dispatch_in_scope(
        &self,
        scope: &mut InvocationScope<'_, '_>,
        callback: &CallbackHandle,
        primary: HostRef,
        extra: &[NativeValue],
    ) -> Result<NativeValue, Error> {
        if callback.owner != self.identity {
            return Err(Error::Runtime(
                "callback belongs to another Pon program".into(),
            ));
        }
        if scope.owner != self.identity {
            return Err(Error::Runtime(
                "invocation scope belongs to another Pon program".into(),
            ));
        }
        let index = callback.slot.ok_or_else(|| {
            Error::Runtime(format!("callback `{}` is not exported", callback.name()))
        })?;
        self.dispatch_bound_in_scope(scope, index, primary, extra)
    }

    fn dispatch_bound(
        &self,
        index: usize,
        primary: HostRef,
        extra: &[NativeValue],
    ) -> Result<NativeValue, Error> {
        self.with_invocation_scope(|scope| {
            self.dispatch_bound_in_scope(scope, index, primary, extra)
        })
    }

    fn dispatch_bound_in_scope(
        &self,
        scope: &mut InvocationScope<'_, '_>,
        index: usize,
        primary: HostRef,
        extra: &[NativeValue],
    ) -> Result<NativeValue, Error> {
        // Reject a scope from another program before allocating the host
        // proxy or converting arguments.  Dispatch is a frame hot path, and
        // this also keeps the ownership check side effect free on errors.
        if scope.owner != self.identity {
            return Err(Error::Runtime(
                "invocation scope belongs to another Pon program".into(),
            ));
        }
        let host_scope = HostScope::new(self.identity);
        let token_cell = std::rc::Rc::new(std::cell::Cell::new(0));
        let id = host_scope
            .register(PonHost {
                host: Arc::clone(primary.host()),
                token: std::rc::Rc::clone(&token_cell),
            })
            .map_err(|error| Error::Runtime(error.to_string()))?;
        let token = id
            .token()
            .ok_or_else(|| Error::Runtime("native host token overflow".into()))?;
        token_cell.set(token);
        let _guard = host_scope.activate();
        let args = &mut scope.callback_args;
        args.clear();
        args.reserve((extra.len() + 1).saturating_sub(args.capacity()));
        args.push(native_object_value(token, &primary.kind, &primary.path));
        for value in extra {
            args.push(native_to_pon(value, token)?);
        }
        let mut call_args = [Value::Int(index as i64), Value::List(std::mem::take(args))];
        let result = scope
            .inner
            .invoke_index(self.dispatch_index, &call_args)
            .map(native_to_native)
            .map_err(|error| Error::Runtime(error.to_string()))
            .and_then(|result| result);
        if let Value::List(reusable_args) = std::mem::replace(&mut call_args[1], Value::None) {
            *args = reusable_args;
        }
        result
    }
}

fn native_object_value(token: i64, kind: &crate::value::NativeKind, path: &str) -> Value {
    Value::Dict(
        [
            ("__skirmish_native__".into(), Value::Bool(true)),
            ("token".into(), Value::Int(token)),
            ("kind".into(), Value::String(kind.name().into())),
            ("path".into(), Value::String(path.into())),
        ]
        .into_iter()
        .collect(),
    )
}

fn native_to_pon(value: &NativeValue, token: i64) -> Result<Value, Error> {
    Ok(match value {
        NativeValue::None => Value::None,
        NativeValue::Bool(value) => Value::Bool(*value),
        NativeValue::Int(value) => Value::Int(*value),
        NativeValue::F32(value) => Value::F32(*value),
        NativeValue::String(value) => Value::String(value.clone()),
        NativeValue::Vec2(value) => Value::List(value.iter().map(|x| Value::F32(*x)).collect()),
        NativeValue::Object(value) => native_object_value(token, &value.kind, &value.path),
        NativeValue::List(values) => Value::List(
            values
                .iter()
                .map(|value| native_to_pon(value, token))
                .collect::<Result<_, _>>()?,
        ),
        NativeValue::Tuple(values) => Value::Tuple(
            values
                .iter()
                .map(|value| native_to_pon(value, token))
                .collect::<Result<_, _>>()?,
        ),
        NativeValue::Dict(values) => Value::Dict(
            values
                .iter()
                .map(|(key, value)| Ok((key.clone(), native_to_pon(value, token)?)))
                .collect::<Result<_, Error>>()?,
        ),
    })
}

struct PonHost {
    host: crate::value::SharedNativeHost,
    token: std::rc::Rc<std::cell::Cell<i64>>,
}

impl NativeHostProxy for PonHost {
    fn get(&mut self, path: &str) -> Result<Value, skirmish_pon_runtime::Error> {
        self.host
            .lock()
            .map_err(|_| skirmish_pon_runtime::Error::Runtime("native host lock poisoned".into()))?
            .get(path)
            .map(|value| native_to_pon(&value, self.token.get()))
            .and_then(|value| value)
            .map_err(|error| skirmish_pon_runtime::Error::Runtime(error.to_string()))
    }

    fn set(&mut self, path: &str, value: Value) -> Result<(), skirmish_pon_runtime::Error> {
        self.host
            .lock()
            .map_err(|_| skirmish_pon_runtime::Error::Runtime("native host lock poisoned".into()))?
            .set(
                path,
                native_to_native_host(value, self.token.get())
                    .map_err(|error| skirmish_pon_runtime::Error::Value(error.to_string()))?,
            )
            .map_err(|error| skirmish_pon_runtime::Error::Runtime(error.to_string()))
    }

    fn call(&mut self, path: &str, args: &[Value]) -> Result<Value, skirmish_pon_runtime::Error> {
        let args = args
            .iter()
            .cloned()
            .map(|value| native_to_native_host(value, self.token.get()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| skirmish_pon_runtime::Error::Value(error.to_string()))?;
        self.host
            .lock()
            .map_err(|_| skirmish_pon_runtime::Error::Runtime("native host lock poisoned".into()))?
            .call(path, &args)
            .map(|value| native_to_pon(&value, self.token.get()))
            .and_then(|value| value)
            .map_err(|error| skirmish_pon_runtime::Error::Runtime(error.to_string()))
    }

    fn call_named(
        &mut self,
        path: &str,
        args: &[Value],
        named: &std::collections::BTreeMap<String, Value>,
    ) -> Result<Value, skirmish_pon_runtime::Error> {
        let positional = args
            .iter()
            .cloned()
            .map(|value| native_to_native_host(value, self.token.get()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| skirmish_pon_runtime::Error::Value(error.to_string()))?;
        let named = named
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.clone(),
                    native_to_native_host(value.clone(), self.token.get())?,
                ))
            })
            .collect::<Result<std::collections::BTreeMap<_, _>, Error>>()
            .map_err(|error| skirmish_pon_runtime::Error::Value(error.to_string()))?;
        self.host
            .lock()
            .map_err(|_| skirmish_pon_runtime::Error::Runtime("native host lock poisoned".into()))?
            .call_named(path, &positional, &named)
            .map(|value| native_to_pon(&value, self.token.get()))
            .and_then(|value| value)
            .map_err(|error| skirmish_pon_runtime::Error::Runtime(error.to_string()))
    }
}

fn native_to_native(value: Value) -> Result<NativeValue, Error> {
    Ok(match value {
        Value::None => NativeValue::None,
        Value::Bool(value) => NativeValue::Bool(value),
        Value::Int(value) => NativeValue::Int(value),
        Value::F32(value) => NativeValue::F32(value),
        Value::String(value) => NativeValue::String(value),
        Value::List(values) => NativeValue::List(
            values
                .into_iter()
                .map(native_to_native)
                .collect::<Result<_, _>>()?,
        ),
        Value::Tuple(values) => NativeValue::Tuple(
            values
                .into_iter()
                .map(native_to_native)
                .collect::<Result<_, _>>()?,
        ),
        Value::Dict(values) => NativeValue::Dict(
            values
                .into_iter()
                .map(|(key, value)| Ok((key, native_to_native(value)?)))
                .collect::<Result<_, Error>>()?,
        ),
    })
}

fn native_to_native_host(value: Value, token: i64) -> Result<NativeValue, Error> {
    if let Value::Dict(values) = &value
        && matches!(values.get("__skirmish_native__"), Some(Value::Bool(true)))
    {
        let actual = values
            .get("token")
            .and_then(|value| match value {
                Value::Int(value) => Some(*value),
                _ => None,
            })
            .ok_or_else(|| Error::Invalid("native object token is missing".into()))?;
        if actual != token {
            return Err(Error::Invalid("native object token expired".into()));
        }
        let kind = match values.get("kind").and_then(|value| match value {
            Value::String(value) => Some(value.as_str()),
            _ => None,
        }) {
            Some("Fighter") => NativeKind::Fighter,
            Some("Context") => NativeKind::Context,
            Some("Hit") => NativeKind::Hit,
            Some("Input") => NativeKind::Input,
            Some("Vec2") => NativeKind::Vec2,
            Some("F32") => NativeKind::F32,
            Some("State") => NativeKind::State,
            Some("Value") => NativeKind::Value,
            _ => return Err(Error::Invalid("native object kind is invalid".into())),
        };
        let path = values
            .get("path")
            .and_then(|value| match value {
                Value::String(value) => Some(value.clone()),
                _ => None,
            })
            .ok_or_else(|| Error::Invalid("native object path is missing".into()))?;
        return Ok(NativeValue::Object(crate::value::NativeObject {
            kind,
            path,
        }));
    }
    native_to_native(value)
}
