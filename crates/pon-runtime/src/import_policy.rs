//! Import roots for verified source bundles.
//!
//! Pon's resolver intentionally consults process environment, the current
//! directory, and installed packages.  A gameplay bundle needs a narrower
//! policy while its source is compiled and while callbacks execute (including
//! imports performed lazily by callback code).  This module checks the path
//! selected by Pon's resolver without changing Pon's pinned resolver.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    sync::Mutex,
};

use crate::Error;
use pon_runtime::{PyObject, abi, intern};
use sha2::{Digest, Sha256};
use std::ptr;

static ACTIVE: Mutex<Option<ImportPolicy>> = Mutex::new(None);
/// Native modules are trusted only when the runtime itself created them.  The
/// pointers are immortal through Pon's native module cache/factory lifetime.
#[derive(Clone, Debug)]
struct TrustedNative {
    module: usize,
}

static TRUSTED_NATIVE: Mutex<BTreeMap<String, TrustedNative>> = Mutex::new(BTreeMap::new());

#[derive(Clone, Debug)]
struct ImportPolicy {
    /// Every regular file that was present when the bundle was verified.
    /// Callback imports use this set and never consult the filesystem.
    files: Arc<BTreeMap<PathBuf, [u8; 32]>>,
    modules: Arc<BTreeMap<String, usize>>,
    strict: bool,
    frozen: bool,
}

/// Immutable import provenance prepared at bundle load time.  This is cheap
/// to clone and is safe to retain in a thread-affine prepared program.
#[derive(Clone, Debug)]
pub(crate) struct PreparedImportPolicy {
    roots: Arc<[PathBuf]>,
    files: Arc<BTreeMap<PathBuf, [u8; 32]>>,
    modules: Arc<BTreeMap<String, usize>>,
    strict: bool,
}

/// Installs the allowed materialized roots for the lifetime of the guard.
/// Guards are nested by the runtime's process-wide Pon lock, so restoring the
/// previous policy is sufficient for preparation and callback lifetimes.
pub(crate) fn install<I>(roots: I, strict: bool) -> Result<ImportPolicyGuard, Error>
where
    I: IntoIterator<Item = PathBuf>,
{
    let roots = roots
        .into_iter()
        .map(|root| {
            std::fs::canonicalize(&root).map_err(|error| {
                Error::Io(format!(
                    "cannot canonicalize import root `{}`: {error}",
                    root.display()
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if roots.is_empty() {
        return Err(Error::Runtime(
            "controlled import policy has no roots".into(),
        ));
    }
    let mut files = BTreeMap::new();
    for root in &roots {
        collect_files(root, &mut files)?;
    }
    let files = Arc::new(files);
    let path_guard = PathRootsGuard::install(&roots)?;
    let cached_modules = CachedModulesGuard::install(&roots, strict)?;
    let mut active = ACTIVE.lock().unwrap_or_else(|poison| poison.into_inner());
    let previous = active.replace(ImportPolicy {
        files: Arc::clone(&files),
        modules: Arc::new(BTreeMap::new()),
        strict,
        frozen: false,
    });
    Ok(ImportPolicyGuard {
        previous,
        _path_guard: path_guard,
        _cached_modules: cached_modules,
        prepared: PreparedImportPolicy {
            roots: roots.into(),
            files,
            modules: Arc::new(BTreeMap::new()),
            strict,
        },
    })
}

pub(crate) struct ImportPolicyGuard {
    previous: Option<ImportPolicy>,
    _path_guard: PathRootsGuard,
    _cached_modules: CachedModulesGuard,
    prepared: PreparedImportPolicy,
}

impl ImportPolicyGuard {
    pub(crate) fn prepared_policy(&self) -> &PreparedImportPolicy {
        &self.prepared
    }
}

pub(crate) fn loaded_modules(
    policy: &PreparedImportPolicy,
) -> impl Iterator<Item = *mut PyObject> + '_ {
    policy
        .modules
        .values()
        .copied()
        .map(|module| module as *mut PyObject)
}

/// Install a policy that was completely checked during preparation.  In this
/// mode the only per-call work is swapping `sys.path`; no paths are resolved,
/// files are read, or cached modules are inspected.
pub(crate) fn install_prepared(policy: &PreparedImportPolicy) -> Result<ImportPolicyGuard, Error> {
    let roots = policy.roots.to_vec();
    let path_guard = PathRootsGuard::install(&roots)?;
    let cached_modules = CachedModulesGuard::install_prepared(&policy.modules)?;
    let mut active = ACTIVE.lock().unwrap_or_else(|poison| poison.into_inner());
    let previous = active.replace(ImportPolicy {
        files: Arc::clone(&policy.files),
        modules: Arc::clone(&policy.modules),
        strict: policy.strict,
        frozen: true,
    });
    Ok(ImportPolicyGuard {
        previous,
        _path_guard: path_guard,
        _cached_modules: cached_modules,
        prepared: policy.clone(),
    })
}

impl Drop for ImportPolicyGuard {
    fn drop(&mut self) {
        let mut active = ACTIVE.lock().unwrap_or_else(|poison| poison.into_inner());
        *active = self.previous.take();
    }
}

fn collect_files(root: &Path, files: &mut BTreeMap<PathBuf, [u8; 32]>) -> Result<(), Error> {
    let metadata = std::fs::symlink_metadata(root).map_err(|error| {
        Error::Io(format!(
            "cannot inspect import root `{}`: {error}",
            root.display()
        ))
    })?;
    if !metadata.is_dir() {
        return Err(Error::Io(format!(
            "controlled import root is not a directory: {}",
            root.display()
        )));
    }
    for entry in std::fs::read_dir(root).map_err(|error| Error::Io(error.to_string()))? {
        let path = entry.map_err(|error| Error::Io(error.to_string()))?.path();
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|error| Error::Io(error.to_string()))?;
        if metadata.file_type().is_symlink() {
            return Err(Error::Io(format!(
                "controlled import root contains symlink: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            collect_files(&path, files)?;
        } else if metadata.is_file() {
            let bytes = std::fs::read(&path).map_err(|error| Error::Io(error.to_string()))?;
            files.insert(path, Sha256::digest(bytes).into());
        }
    }
    Ok(())
}

struct PathRootsGuard {
    path_list: *mut PyObject,
    saved: *mut PyObject,
    _roots: crate::safety::PersistentRoots,
}

impl PathRootsGuard {
    fn install(roots: &[PathBuf]) -> Result<Self, Error> {
        let mut root_keeper = crate::safety::PersistentRoots::new();
        let Some(path_list) = pon_runtime::import::module_attr(intern("sys"), intern("path"))
        else {
            return Err(Error::Runtime("Pon sys.path is unavailable".into()));
        };
        root_keeper.push(path_list);
        let copy = unsafe { abi::pon_get_attr(path_list, intern("copy"), ptr::null_mut()) };
        root_keeper.push(copy);
        let clear = unsafe { abi::pon_get_attr(path_list, intern("clear"), ptr::null_mut()) };
        root_keeper.push(clear);
        let append = unsafe { abi::pon_get_attr(path_list, intern("append"), ptr::null_mut()) };
        root_keeper.push(append);
        if copy.is_null() || clear.is_null() || append.is_null() {
            return Err(Error::Runtime(
                "Pon sys.path methods are unavailable".into(),
            ));
        }
        let mut no_args = [];
        let saved = unsafe { abi::pon_call(copy, no_args.as_mut_ptr(), 0) };
        if saved.is_null() {
            return Err(Error::Runtime("failed to save Pon sys.path".into()));
        }
        root_keeper.push(saved);
        if unsafe { abi::pon_call(clear, no_args.as_mut_ptr(), 0) }.is_null() {
            return Err(Error::Runtime("failed to clear Pon sys.path".into()));
        }
        for root in roots {
            let text = root.to_string_lossy();
            let entry = unsafe { abi::pon_const_str(text.as_ptr(), text.len()) };
            if entry.is_null() {
                Self::restore(path_list, saved, clear, &mut root_keeper);
                return Err(Error::Runtime("failed to allocate import root".into()));
            }
            root_keeper.push(entry);
            let mut argv = [entry];
            if unsafe { abi::pon_call(append, argv.as_mut_ptr(), 1) }.is_null() {
                Self::restore(path_list, saved, clear, &mut root_keeper);
                return Err(Error::Runtime("failed to install import root".into()));
            }
        }
        Ok(Self {
            path_list,
            saved,
            _roots: root_keeper,
        })
    }

    fn restore(
        path_list: *mut PyObject,
        saved: *mut PyObject,
        clear: *mut PyObject,
        roots: &mut crate::safety::PersistentRoots,
    ) {
        let mut no_args = [];
        let _ = unsafe { abi::pon_call(clear, no_args.as_mut_ptr(), 0) };
        let extend = unsafe { abi::pon_get_attr(path_list, intern("extend"), ptr::null_mut()) };
        if !extend.is_null() {
            roots.push(extend);
            let mut saved_argv = [saved];
            let _ = unsafe { abi::pon_call(extend, saved_argv.as_mut_ptr(), 1) };
        }
    }
}

impl Drop for PathRootsGuard {
    fn drop(&mut self) {
        let clear = unsafe { abi::pon_get_attr(self.path_list, intern("clear"), ptr::null_mut()) };
        self._roots.push(clear);
        let extend =
            unsafe { abi::pon_get_attr(self.path_list, intern("extend"), ptr::null_mut()) };
        self._roots.push(extend);
        if clear.is_null() || extend.is_null() {
            return;
        }
        let mut no_args = [];
        let _ = unsafe { abi::pon_call(clear, no_args.as_mut_ptr(), 0) };
        let mut argv = [self.saved];
        let _ = unsafe { abi::pon_call(extend, argv.as_mut_ptr(), 1) };
    }
}

/// Evict every cache entry whose exact identity was not created by the
/// trusted native registry while a strict policy is live.
/// Pon checks its cache before invoking the source loader, so a warm ambient
/// module must be removed for the policy to be effective. The exact bindings
/// are restored when the guard drops, including when preparation fails.
/// Looking at `__file__` is intentionally avoided: it is mutable user data and
/// cannot establish provenance. Verified source identities are recorded by the
/// source loader during this preparation and admitted only at capture time.
struct CachedModulesGuard {
    modules: *mut PyObject,
    saved: Vec<(String, Option<*mut PyObject>)>,
    roots: crate::safety::PersistentRoots,
}

fn root_module_values(roots: &mut crate::safety::PersistentRoots, module: *mut PyObject) {
    roots.push(module);
    if let Some(values) = pon_runtime::import::module_object_attr_values(module) {
        for value in values {
            roots.push(value);
        }
    }
}

impl CachedModulesGuard {
    fn install(_import_roots: &[PathBuf], strict: bool) -> Result<Self, Error> {
        let modules = pon_runtime::import::sys_modules_dict().map_err(Error::Runtime)?;
        let mut guard = Self {
            modules,
            saved: Vec::new(),
            roots: crate::safety::PersistentRoots::new(),
        };
        if !strict {
            return Ok(guard);
        }
        let entries = unsafe { pon_runtime::types::dict::dict_ref(modules) }
            .map_err(Error::Runtime)?
            .entries
            .iter()
            .map(|entry| (entry.key, entry.value))
            .collect::<Vec<_>>();
        let trusted_native = TRUSTED_NATIVE
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone();
        let mut foreign = Vec::new();
        for (key, module) in entries {
            let Some(name) =
                (unsafe { pon_runtime::types::type_::unicode_text(key) }).map(str::to_owned)
            else {
                continue;
            };
            let native_ok = trusted_native
                .get(&name)
                .is_some_and(|known| known.module == module as usize);
            if !native_ok {
                foreign.push((name, key, module));
            }
        }
        for (name, key, module) in foreign {
            // Root both pointers before dropping the dictionary's references:
            // removal and hashing may allocate and trigger collection.
            guard.roots.push(key);
            root_module_values(&mut guard.roots, module);
            guard.saved.push((name, Some(module)));
            unsafe { pon_runtime::types::dict::dict_remove(modules, key) }
                .map_err(Error::Runtime)?;
        }
        Ok(guard)
    }

    fn install_prepared(prepared: &BTreeMap<String, usize>) -> Result<Self, Error> {
        let modules = pon_runtime::import::sys_modules_dict().map_err(Error::Runtime)?;
        let mut guard = Self {
            modules,
            saved: Vec::new(),
            roots: crate::safety::PersistentRoots::new(),
        };
        for (name, pointer) in prepared {
            let key = unsafe { abi::pon_const_str(name.as_ptr(), name.len()) };
            if key.is_null() {
                return Err(Error::Runtime("failed to allocate module name".into()));
            }
            guard.roots.push(key);
            let existing = unsafe { pon_runtime::types::dict::dict_get(modules, key) }
                .map_err(Error::Runtime)?;
            if let Some(existing) = existing {
                root_module_values(&mut guard.roots, existing);
            }
            guard.saved.push((name.clone(), existing));
            let module = *pointer as *mut PyObject;
            if existing != Some(module) {
                unsafe { pon_runtime::types::dict::dict_remove(modules, key) }
                    .map_err(Error::Runtime)?;
                root_module_values(&mut guard.roots, module);
                unsafe { pon_runtime::types::dict::dict_insert(modules, key, module) }
                    .map_err(Error::Runtime)?;
            }
        }
        Ok(guard)
    }
}

impl Drop for CachedModulesGuard {
    fn drop(&mut self) {
        if self.modules.is_null() {
            return;
        }
        for (name, module) in &self.saved {
            let key = unsafe { abi::pon_const_str(name.as_ptr(), name.len()) };
            if key.is_null() {
                let _ = pon_runtime::pon_err_message();
                continue;
            }
            self.roots.push(key);
            let _ = unsafe { pon_runtime::types::dict::dict_remove(self.modules, key) };
            if let Some(module) = module {
                let _ =
                    unsafe { pon_runtime::types::dict::dict_insert(self.modules, key, *module) };
            }
        }
    }
}

/// Reject a source module selected outside the active controlled roots.
/// `None` means no controlled policy is active, preserving bare development
/// `Program` behavior.
pub(crate) fn check_source_path(name: &str, path: &Path, source: &str) -> Result<(), String> {
    let active = ACTIVE.lock().unwrap_or_else(|poison| poison.into_inner());
    let Some(policy) = active.as_ref() else {
        return Ok(());
    };
    if !policy.strict {
        return Ok(());
    }
    // A prepared callback may only use modules loaded during preparation.
    // The source loader is reached only after the resolver has established
    // that a module is absent from its cache, so rejecting here prevents any
    // new source read, parse, or JIT compilation during execution.
    if policy.frozen {
        return Err(format!(
            "controlled import rejected new source module '{name}' during callback execution"
        ));
    }
    if policy
        .files
        .get(path)
        .is_some_and(|digest| Sha256::digest(source.as_bytes()).as_slice() == digest)
    {
        return Ok(());
    }
    Err(format!(
        "controlled import rejected module '{name}' outside verified bundle files: {}",
        path.display()
    ))
}

pub(crate) fn record_source_module(name: &str, module: *mut PyObject) {
    let mut active = ACTIVE.lock().unwrap_or_else(|poison| poison.into_inner());
    if let Some(policy) = active
        .as_mut()
        .filter(|policy| policy.strict && !policy.frozen)
    {
        Arc::make_mut(&mut policy.modules).insert(name.to_owned(), module as usize);
    }
}

pub(crate) fn record_native_module(name: &str, module: *mut PyObject) {
    unsafe { pon_runtime::import::register_trusted_native_module(module) };
    TRUSTED_NATIVE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .insert(
            name.to_owned(),
            TrustedNative {
                module: module as usize,
            },
        );
    let mut active = ACTIVE.lock().unwrap_or_else(|poison| poison.into_inner());
    if let Some(policy) = active
        .as_mut()
        .filter(|policy| policy.strict && !policy.frozen)
    {
        Arc::make_mut(&mut policy.modules).insert(name.to_owned(), module as usize);
    }
}

pub(crate) fn check_dynamic_code() -> Result<(), String> {
    let active = ACTIVE.lock().unwrap_or_else(|poison| poison.into_inner());
    if active
        .as_ref()
        .is_some_and(|policy| policy.strict && policy.frozen)
    {
        Err("dynamic source compilation is disabled during prepared callback execution".into())
    } else {
        Ok(())
    }
}

/// Hooked into Pon before its cached-module fast path and source discovery.
/// Frozen execution therefore cannot read or compile a previously unseen
/// source module, while cached file-backed modules must remain in verified
/// roots. Native modules have no `__file__` and are admitted by the curated
/// native registry.
pub(crate) fn check_import(name: &str, cached: Option<*mut PyObject>) -> Result<(), String> {
    let active = ACTIVE.lock().unwrap_or_else(|poison| poison.into_inner());
    let Some(policy) = active.as_ref() else {
        return Ok(());
    };
    if !policy.strict || !policy.frozen {
        return Ok(());
    }
    let Some(module) = cached else {
        return Err(format!(
            "controlled import rejected new source module '{name}' during callback execution"
        ));
    };
    if policy
        .modules
        .get(name)
        .is_some_and(|known| *known == module as usize)
    {
        Ok(())
    } else {
        Err(format!(
            "No module named '{name}': controlled import rejected cached module with unverified module identity"
        ))
    }
}

pub(crate) fn check_loaded_module(
    policy: &PreparedImportPolicy,
    name: &str,
    module: *mut PyObject,
) -> Result<(), Error> {
    if policy
        .modules
        .get(name)
        .is_some_and(|known| *known == module as usize)
    {
        Ok(())
    } else {
        Err(Error::Runtime(format!(
            "controlled module '{name}' identity changed during callback"
        )))
    }
}

pub(crate) fn capture_loaded_modules(guard: &mut ImportPolicyGuard) -> Result<(), Error> {
    if !guard.prepared.strict {
        return Ok(());
    }
    let modules = pon_runtime::import::sys_modules_dict().map_err(Error::Runtime)?;
    let native = TRUSTED_NATIVE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .clone();
    let source = ACTIVE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .as_ref()
        .map(|policy| policy.modules.as_ref().clone())
        .unwrap_or_default();
    let entries = unsafe { pon_runtime::types::dict::dict_ref(modules) }
        .map_err(Error::Runtime)?
        .entries
        .iter()
        .filter_map(|entry| {
            let name = unsafe { pon_runtime::types::type_::unicode_text(entry.key) }?.to_owned();
            let pointer = entry.value as usize;
            (source
                .get(&name)
                .copied()
                .or_else(|| native.get(&name).map(|entry| entry.module))
                .filter(|known| *known == pointer))
            .map(|_| (name, pointer))
        })
        .collect::<BTreeMap<_, _>>();
    guard.prepared.modules = Arc::new(entries);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_module_guard_restores_evicted_binding_identity() {
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _attachment = crate::safety::Attachment::acquire().unwrap();
        assert_eq!(unsafe { abi::pon_runtime_init() }, 0);
        let name = "_import_policy_restore_probe";
        let module = pon_runtime::import::install_module(name, []).unwrap();
        let file = unsafe {
            abi::pon_const_str(
                b"/tmp/ambient-probe.py".as_ptr(),
                b"/tmp/ambient-probe.py".len(),
            )
        };
        assert!(pon_runtime::import::store_module_attr(
            intern(name),
            intern("__file__"),
            file
        ));
        let modules = pon_runtime::import::sys_modules_dict().unwrap();
        let root = tempfile::tempdir().unwrap();
        {
            let _guard = CachedModulesGuard::install(&[root.path().to_path_buf()], true).unwrap();
            let key = unsafe { abi::pon_const_str(name.as_ptr(), name.len()) };
            assert!(
                unsafe { pon_runtime::types::dict::dict_get(modules, key) }
                    .unwrap()
                    .is_none()
            );
            drop(_guard);
        }
        let key = unsafe { abi::pon_const_str(name.as_ptr(), name.len()) };
        assert_eq!(
            unsafe { pon_runtime::types::dict::dict_get(modules, key) }.unwrap(),
            Some(module)
        );
        fn fail_with_guard(root: &Path) -> Result<(), ()> {
            let _guard = CachedModulesGuard::install(&[root.to_path_buf()], true).unwrap();
            Err(())
        }
        let result = fail_with_guard(root.path());
        assert!(result.is_err());
        let key = unsafe { abi::pon_const_str(name.as_ptr(), name.len()) };
        assert_eq!(
            unsafe { pon_runtime::types::dict::dict_get(modules, key) }.unwrap(),
            Some(module)
        );
    }

    #[test]
    fn cached_module_guard_evicts_preparation_time_spoofs_without_using_file_metadata() {
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _attachment = crate::safety::Attachment::acquire().unwrap();
        crate::import_hooks::install();
        assert_eq!(unsafe { abi::pon_runtime_init() }, 0);
        // Force creation of the real native math module so its exact pointer
        // is present in the trusted registry, then replace it with a fake.
        let _real_math =
            unsafe { pon_runtime::import::pon_import_name(intern("math"), std::ptr::null(), 0, 0) };
        assert!(!_real_math.is_null());
        let fake_math = pon_runtime::import::install_module("math", []).unwrap();
        let fake_allowed = pon_runtime::import::install_module("_spoof_allowed", []).unwrap();
        let fake_malformed = pon_runtime::import::install_module("_spoof_malformed", []).unwrap();
        let allowed_file =
            unsafe { abi::pon_const_str(b"/verified/but-undeclared.py".as_ptr(), 26) };
        let malformed_file = unsafe { abi::pon_const_int(7) };
        assert!(pon_runtime::import::store_module_attr(
            intern("_spoof_allowed"),
            intern("__file__"),
            allowed_file
        ));
        assert!(pon_runtime::import::store_module_attr(
            intern("_spoof_malformed"),
            intern("__file__"),
            malformed_file
        ));
        let modules = pon_runtime::import::sys_modules_dict().unwrap();
        let root = tempfile::tempdir().unwrap();
        {
            let _guard = CachedModulesGuard::install(&[root.path().to_path_buf()], true).unwrap();
            for (name, fake) in [
                ("math", fake_math),
                ("_spoof_allowed", fake_allowed),
                ("_spoof_malformed", fake_malformed),
            ] {
                let key = unsafe { abi::pon_const_str(name.as_ptr(), name.len()) };
                assert_eq!(
                    unsafe { pon_runtime::types::dict::dict_get(modules, key) }.unwrap(),
                    None
                );
                assert!(!fake.is_null());
            }
        }
        // The guard restores the exact preexisting objects for its caller;
        // frozen capture subsequently admits only the trusted native pointer.
        let key = unsafe { abi::pon_const_str(b"math".as_ptr(), 4) };
        assert_eq!(
            unsafe { pon_runtime::types::dict::dict_get(modules, key) }.unwrap(),
            Some(fake_math)
        );
    }

    #[test]
    fn trusted_native_namespace_survives_cache_replacement_and_gc() {
        let name = "_trusted_native_gc_probe";
        let native = std::thread::spawn(move || {
            let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
            let _attachment = crate::safety::Attachment::acquire().unwrap();
            assert_eq!(unsafe { abi::pon_runtime_init() }, 0);
            let native = pon_runtime::import::install_module(name, []).unwrap();
            unsafe { pon_runtime::import::register_trusted_native_module(native) };
            let marker = unsafe { abi::pon_const_str(b"mutated-after-registration".as_ptr(), 26) };
            assert!(pon_runtime::import::store_module_attr(
                intern(name),
                intern("marker"),
                marker
            ));
            let fake = pon_runtime::import::install_module(name, []).unwrap();
            assert_ne!(native, fake);
            native as usize
        })
        .join()
        .unwrap() as *mut PyObject;
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _attachment = crate::safety::Attachment::acquire().unwrap();
        pon_runtime::abi::collect().unwrap();
        let marker = unsafe {
            (*native.cast::<pon_runtime::import::PyModuleObject>())
                .attrs
                .get(&intern("marker"))
                .copied()
                .unwrap()
        };
        assert_eq!(
            unsafe { pon_runtime::types::type_::unicode_text(marker) },
            Some("mutated-after-registration")
        );
    }
}
