//! Scoped native host proxies used by the Pon callback bridge.
//!
//! A Pon object must never contain a borrowed Rust reference.  `HostId` is
//! only a capability into the scope that created it; the generation and
//! owning thread are checked on every operation and the scope invalidates all
//! capabilities when it is dropped.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::ThreadId;

use crate::{Error, Value};

thread_local! {
    static ACTIVE_SCOPE: RefCell<Option<HostScope>> = const { RefCell::new(None) };
}

/// Opaque capability for one object in one callback scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HostId {
    generation: u64,
    slot: u32,
    thread: ThreadId,
}

impl HostId {
    pub fn generation(self) -> u64 {
        self.generation
    }
    pub fn slot(self) -> u32 {
        self.slot
    }

    pub fn token(self) -> Option<i64> {
        (self.generation <= i64::MAX as u64 >> 32)
            .then_some(((self.generation as i64) << 32) | i64::from(self.slot))
    }
}

/// Operations exposed by a game-owned native host. Implementations should
/// stage mutation and provide rollback through their surrounding transaction.
pub trait NativeHostProxy {
    fn get(&mut self, path: &str) -> Result<Value, Error>;
    fn set(&mut self, path: &str, value: Value) -> Result<(), Error>;
    fn call(&mut self, path: &str, args: &[Value]) -> Result<Value, Error>;
    fn call_named(
        &mut self,
        path: &str,
        args: &[Value],
        named: &BTreeMap<String, Value>,
    ) -> Result<Value, Error> {
        if named.is_empty() {
            self.call(path, args)
        } else {
            Err(Error::Value(format!(
                "native call `{path}` does not accept named arguments"
            )))
        }
    }
}

struct ScopeState {
    generation: u64,
    thread: ThreadId,
    active: bool,
    hosts: Vec<Rc<RefCell<dyn NativeHostProxy>>>,
}

/// Owns all host capabilities for one synchronous callback dispatch.
pub struct HostScope {
    state: Rc<RefCell<ScopeState>>,
    owner: bool,
}

impl Clone for HostScope {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
            owner: false,
        }
    }
}

impl HostScope {
    pub fn new(_generation: u64) -> Self {
        static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);
        Self {
            state: Rc::new(RefCell::new(ScopeState {
                generation: NEXT_SCOPE.fetch_add(1, Ordering::Relaxed),
                thread: std::thread::current().id(),
                active: true,
                hosts: Vec::new(),
            })),
            owner: true,
        }
    }

    pub fn register<H: NativeHostProxy + 'static>(&self, host: H) -> Result<HostId, Error> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| Error::Runtime("host scope borrowed".into()))?;
        self.validate_locked(&state)?;
        let slot = u32::try_from(state.hosts.len())
            .map_err(|_| Error::Value("too many host objects".into()))?;
        state.hosts.push(Rc::new(RefCell::new(host)));
        Ok(HostId {
            generation: state.generation,
            slot,
            thread: state.thread,
        })
    }

    pub fn handle(&self, id: HostId) -> HostHandle {
        HostHandle {
            state: Rc::clone(&self.state),
            id,
        }
    }

    pub fn handle_token(&self, token: i64) -> HostHandle {
        let generation = (token as u64) >> 32;
        let slot = token as u32;
        self.handle(HostId {
            generation,
            slot,
            thread: std::thread::current().id(),
        })
    }

    /// Make this scope visible to the private `_skirmish_native` functions
    /// for one synchronous Pon callback. The guard restores the prior scope.
    pub fn activate(&self) -> ScopeGuard {
        let previous = ACTIVE_SCOPE.with(|active| active.replace(Some(self.clone())));
        ScopeGuard { previous }
    }

    fn validate_locked(&self, state: &ScopeState) -> Result<(), Error> {
        if !state.active {
            return Err(Error::Runtime("native host scope is closed".into()));
        }
        if state.thread != std::thread::current().id() {
            return Err(Error::Runtime(
                "native host used from another thread".into(),
            ));
        }
        Ok(())
    }
}

pub struct ScopeGuard {
    previous: Option<HostScope>,
}

fn raise(error: Error) -> *mut pon_runtime::PyObject {
    let message = error.to_string();
    unsafe { pon_runtime::abi::exc::pon_raise_value_error(message.as_ptr(), message.len()) }
}

fn args_token_path(
    args: *mut *mut pon_runtime::PyObject,
    count: usize,
) -> Result<(i64, String), Error> {
    if count < 2 || args.is_null() {
        return Err(Error::Value(
            "native host operation expects token and path".into(),
        ));
    }
    let token_object = unsafe { *args };
    let token = match crate::unbox_value(token_object)? {
        Value::Int(token) => token,
        _ => return Err(Error::Value("host token must be an integer".into())),
    };
    let path_object = unsafe { *args.add(1) };
    let path = unsafe { pon_runtime::types::type_::unicode_text(path_object) }
        .map(str::to_owned)
        .ok_or_else(|| Error::Value("native host path must be a string".into()))?;
    Ok((token, path))
}

fn active_host(token: i64) -> Result<HostHandle, Error> {
    ACTIVE_SCOPE.with(|active| {
        let scope = active
            .try_borrow()
            .map_err(|_| Error::Runtime("native host scope borrowed".into()))?;
        let scope = scope
            .as_ref()
            .ok_or_else(|| Error::Runtime("no active native host scope".into()))?;
        Ok(scope.handle_token(token))
    })
}

/// Native callback for reading a host field.
///
/// # Safety
/// `args` must be null or point to an array of at least `count` valid Pon
/// object pointers supplied according to the native callback ABI.
pub unsafe extern "C" fn native_get(
    args: *mut *mut pon_runtime::PyObject,
    count: usize,
) -> *mut pon_runtime::PyObject {
    let result = std::panic::catch_unwind(|| {
        let (token, path) = args_token_path(args, count)?;
        let value = active_host(token)?.get(&path)?;
        crate::box_value(&value)
    });
    match result {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => raise(error),
        Err(_) => raise(Error::Runtime("native host callback panicked".into())),
    }
}

/// Native callback for writing a host field.
///
/// # Safety
/// `args` must be null or point to an array of at least `count` valid Pon
/// object pointers supplied according to the native callback ABI.
pub unsafe extern "C" fn native_set(
    args: *mut *mut pon_runtime::PyObject,
    count: usize,
) -> *mut pon_runtime::PyObject {
    let result = std::panic::catch_unwind(|| {
        let (token, path) = args_token_path(args, count)?;
        if count < 3 {
            return Err(Error::Value("native set expects a value".into()));
        }
        let value = crate::unbox_value(unsafe { *args.add(2) })?;
        active_host(token)?.set(&path, value)
    });
    match result {
        Ok(Ok(())) => unsafe { pon_runtime::abi::pon_none() },
        Ok(Err(error)) => raise(error),
        Err(_) => raise(Error::Runtime("native host callback panicked".into())),
    }
}

/// Native callback for calling a host method with positional arguments.
///
/// # Safety
/// `args` must be null or point to an array of at least `count` valid Pon
/// object pointers supplied according to the native callback ABI.
pub unsafe extern "C" fn native_call(
    args: *mut *mut pon_runtime::PyObject,
    count: usize,
) -> *mut pon_runtime::PyObject {
    let result = std::panic::catch_unwind(|| {
        let (token, path) = args_token_path(args, count)?;
        if count != 3 {
            return Err(Error::Value("native call expects an argument list".into()));
        }
        let values = match crate::unbox_value(unsafe { *args.add(2) })? {
            Value::List(values) => values,
            _ => return Err(Error::Value("native call arguments must be a list".into())),
        };
        let value = active_host(token)?.call(&path, &values)?;
        crate::box_value(&value)
    });
    match result {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => raise(error),
        Err(_) => raise(Error::Runtime("native host callback panicked".into())),
    }
}

/// Native callback for calling a host method with positional and named arguments.
///
/// # Safety
/// `args` must be null or point to an array of at least `count` valid Pon
/// object pointers supplied according to the native callback ABI.
pub unsafe extern "C" fn native_call_named(
    args: *mut *mut pon_runtime::PyObject,
    count: usize,
) -> *mut pon_runtime::PyObject {
    let result = std::panic::catch_unwind(|| {
        let (token, path) = args_token_path(args, count)?;
        if count != 4 {
            return Err(Error::Value(
                "native named call expects positional list and mapping".into(),
            ));
        }
        let values = match crate::unbox_value(unsafe { *args.add(2) })? {
            Value::List(values) => values,
            _ => {
                return Err(Error::Value(
                    "native named call arguments must be a list".into(),
                ));
            }
        };
        let named = match crate::unbox_value(unsafe { *args.add(3) })? {
            Value::Dict(values) => values,
            _ => {
                return Err(Error::Value(
                    "native named arguments must be a mapping".into(),
                ));
            }
        };
        let value = active_host(token)?.call_named(&path, &values, &named)?;
        crate::box_value(&value)
    });
    match result {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => raise(error),
        Err(_) => raise(Error::Runtime("native host callback panicked".into())),
    }
}
impl Drop for ScopeGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        ACTIVE_SCOPE.with(|active| {
            active.replace(previous);
        });
    }
}

impl Drop for HostScope {
    fn drop(&mut self) {
        if !self.owner {
            return;
        }
        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.active = false;
            state.hosts.clear();
        }
    }
}

/// A checked handle passed to the Pon value conversion layer.
#[derive(Clone)]
pub struct HostHandle {
    state: Rc<RefCell<ScopeState>>,
    id: HostId,
}

impl HostHandle {
    pub fn id(&self) -> HostId {
        self.id
    }

    fn host(&self) -> Result<Rc<RefCell<dyn NativeHostProxy>>, Error> {
        let state = self
            .state
            .try_borrow()
            .map_err(|_| Error::Runtime("host scope borrowed".into()))?;
        if !state.active
            || state.generation != self.id.generation
            || state.thread != self.id.thread
            || state.thread != std::thread::current().id()
        {
            return Err(Error::Runtime("native host capability expired".into()));
        }
        state
            .hosts
            .get(self.id.slot as usize)
            .cloned()
            .ok_or_else(|| Error::Runtime("invalid native host capability".into()))
    }

    pub fn get(&self, path: &str) -> Result<Value, Error> {
        self.host()?
            .try_borrow_mut()
            .map_err(|_| Error::Runtime("native host borrowed".into()))?
            .get(path)
    }
    pub fn set(&self, path: &str, value: Value) -> Result<(), Error> {
        self.host()?
            .try_borrow_mut()
            .map_err(|_| Error::Runtime("native host borrowed".into()))?
            .set(path, value)
    }
    pub fn call(&self, path: &str, args: &[Value]) -> Result<Value, Error> {
        self.host()?
            .try_borrow_mut()
            .map_err(|_| Error::Runtime("native host borrowed".into()))?
            .call(path, args)
    }
    pub fn call_named(
        &self,
        path: &str,
        args: &[Value],
        named: &BTreeMap<String, Value>,
    ) -> Result<Value, Error> {
        self.host()?
            .try_borrow_mut()
            .map_err(|_| Error::Runtime("native host borrowed".into()))?
            .call_named(path, args, named)
    }

    /// Run a native bridge operation while the capability is checked. The
    /// callback receives a short lived proxy and cannot retain the host
    /// borrow after this function returns.
    pub fn with_proxy<T>(
        &self,
        callback: impl FnOnce(&mut dyn NativeHostProxy) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let host = self.host()?;
        let mut host = host
            .try_borrow_mut()
            .map_err(|_| Error::Runtime("native host borrowed".into()))?;
        callback(&mut *host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Echo;
    impl NativeHostProxy for Echo {
        fn get(&mut self, path: &str) -> Result<Value, Error> {
            Ok(Value::String(path.into()))
        }
        fn set(&mut self, _path: &str, _value: Value) -> Result<(), Error> {
            Ok(())
        }
        fn call(&mut self, _path: &str, args: &[Value]) -> Result<Value, Error> {
            Ok(args.first().cloned().unwrap_or(Value::None))
        }
    }

    #[test]
    fn scope_capability_expires_on_drop() {
        let handle = {
            let scope = HostScope::new(7);
            let id = scope.register(Echo).unwrap();
            let handle = scope.handle(id);
            assert_eq!(
                handle.get("fighter.action").unwrap(),
                Value::String("fighter.action".into())
            );
            assert_eq!(
                handle
                    .with_proxy(|proxy| proxy.get("fighter.percent"))
                    .unwrap(),
                Value::String("fighter.percent".into())
            );
            handle
        };
        assert!(
            matches!(handle.get("fighter.action"), Err(Error::Runtime(message)) if message.contains("expired"))
        );
    }

    #[test]
    fn scope_nonce_is_unique_even_for_same_caller_generation() {
        let first = HostScope::new(3).register(Echo).unwrap();
        let second = HostScope::new(3).register(Echo).unwrap();
        assert_ne!(first.generation(), second.generation());
    }

    struct Store {
        value: Value,
    }
    impl NativeHostProxy for Store {
        fn get(&mut self, _path: &str) -> Result<Value, Error> {
            Ok(self.value.clone())
        }
        fn set(&mut self, _path: &str, value: Value) -> Result<(), Error> {
            self.value = value;
            Ok(())
        }
        fn call(&mut self, _path: &str, args: &[Value]) -> Result<Value, Error> {
            Ok(Value::List(args.to_vec()))
        }
        fn call_named(
            &mut self,
            _path: &str,
            args: &[Value],
            named: &BTreeMap<String, Value>,
        ) -> Result<Value, Error> {
            Ok(named
                .values()
                .next()
                .cloned()
                .or_else(|| args.first().cloned())
                .unwrap_or(Value::None))
        }
    }

    fn boxed(
        values: &[Value],
        roots: &mut crate::safety::RootedVecGuard<'_>,
    ) -> Vec<*mut pon_runtime::PyObject> {
        values
            .iter()
            .map(|value| {
                let object = crate::box_value(value).unwrap();
                roots.push(object);
                object
            })
            .collect()
    }

    #[test]
    fn native_functions_round_trip_through_pon_values() {
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = crate::safety::Attachment::acquire().unwrap();
        assert_eq!(unsafe { pon_runtime::abi::pon_runtime_init() }, 0);
        let mut marker = 0usize;
        let _stack =
            unsafe { crate::safety::StackBoundary::capture((&mut marker as *mut usize).cast()) };
        let storage = crate::safety::RootedVec::new();
        let mut roots = storage.guard();
        let scope = HostScope::new(1);
        let id = scope
            .register(Store {
                value: Value::Int(4),
            })
            .unwrap();
        let _guard = scope.activate();
        let token = id.token().unwrap();
        let mut get_args = boxed(
            &[Value::Int(token), Value::String("fighter.value".into())],
            &mut roots,
        );
        let got = unsafe { native_get(get_args.as_mut_ptr(), get_args.len()) };
        assert_eq!(crate::unbox_value(got).unwrap(), Value::Int(4));
        let mut set_args = boxed(
            &[
                Value::Int(token),
                Value::String("fighter.value".into()),
                Value::String("WAIT".into()),
            ],
            &mut roots,
        );
        assert!(!unsafe { native_set(set_args.as_mut_ptr(), set_args.len()) }.is_null());
        let mut call_args = boxed(
            &[
                Value::Int(token),
                Value::String("fighter.echo".into()),
                Value::List(vec![Value::Int(9)]),
            ],
            &mut roots,
        );
        let called = unsafe { native_call(call_args.as_mut_ptr(), call_args.len()) };
        assert_eq!(
            crate::unbox_value(called).unwrap(),
            Value::List(vec![Value::Int(9)])
        );
        let named = Value::Dict(BTreeMap::from([(String::from("frames"), Value::Int(3))]));
        let mut named_args = boxed(
            &[
                Value::Int(token),
                Value::String("fighter.named".into()),
                Value::List(vec![]),
                named,
            ],
            &mut roots,
        );
        let called_named = unsafe { native_call_named(named_args.as_mut_ptr(), named_args.len()) };
        assert_eq!(crate::unbox_value(called_named).unwrap(), Value::Int(3));
        drop(roots);
        drop(storage);
        drop(_stack);
        parked.restore();
    }

    #[test]
    fn registered_native_module_round_trips_through_python_proxy_calls() {
        crate::register_native_module(
            "_skirmish_native",
            [
                ("get", native_get, 2),
                ("set", native_set, 3),
                ("call", native_call, 3),
                ("call_named", native_call_named, 4),
            ],
        )
        .unwrap();
        let source = "import _skirmish_native as native\ndef run(token):\n    native.set(token, 'fighter.value', 'WAIT')\n    return (native.get(token, 'fighter.value'), native.call(token, 'echo', [9, 8, 7]), native.call_named(token, 'named', [1, 2], {'frames': 3}))\n";
        let mut program = crate::Program::new(source, "host_proxy.py", ["run"])
            .prepare_for_thread()
            .unwrap();
        let scope = HostScope::new(1);
        let id = scope
            .register(Store {
                value: Value::Int(4),
            })
            .unwrap();
        let _guard = scope.activate();
        let result = program
            .invoke("run", &[Value::Int(id.token().unwrap())])
            .unwrap();
        assert_eq!(
            result,
            Value::List(vec![
                Value::String("WAIT".into()),
                Value::List(vec![Value::Int(9), Value::Int(8), Value::Int(7)]),
                Value::Int(3)
            ])
        );
        drop(_guard);
        assert!(
            program
                .invoke("run", &[Value::Int(id.token().unwrap())])
                .is_err()
        );
    }

    #[test]
    fn python_wrap_probe_reports_tagged_host_type_and_field() {
        crate::register_native_module(
            "_skirmish_native",
            [
                ("get", native_get, 2),
                ("set", native_set, 3),
                ("call", native_call, 3),
                ("call_named", native_call_named, 4),
            ],
        )
        .unwrap();
        let source = "from skirmish._native import wrap\ndef inspect(values):\n    obj = wrap(values[0])\n    return (type(obj).__name__, repr(obj), float(obj.damage))\n";
        let bundle = crate::SourceBundle::from_directory(
            "host-probe",
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/api"),
        )
        .unwrap()
        .materialize("/tmp/skirmish-pon-bundles")
        .unwrap();
        let mut program = crate::Program::new(source, "wrap_probe.py", ["inspect"])
            .prepare_for_thread_in_bundle(&bundle)
            .unwrap();
        let scope = HostScope::new(1);
        let id = scope
            .register(Store {
                value: Value::F32(5.0),
            })
            .unwrap();
        let tagged = Value::Dict(BTreeMap::from([
            ("__skirmish_native__".into(), Value::Bool(true)),
            ("token".into(), Value::Int(id.token().unwrap())),
            ("kind".into(), Value::String("Hit".into())),
            ("path".into(), Value::String("hit".into())),
        ]));
        let _guard = scope.activate();
        let result = program
            .invoke("inspect", &[Value::List(vec![tagged])])
            .unwrap();
        assert_eq!(
            result,
            Value::List(vec![
                Value::String("NativeObject".into()),
                Value::String("<Hit host object at 'hit'>".into()),
                Value::F32(5.0)
            ])
        );
    }
}
