//! Small, typed, synchronous facade over Pon's boxed runtime ABI.
//!
//! `Program` is source identity and is safe to cache/send. `PreparedProgram`
//! is deliberately thread-affine: Pon's runtime and JIT handles are not
//! `Send`, and must be prepared and invoked on the same attached OS thread.
#![forbid(unsafe_op_in_unsafe_fn)]

use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
    ptr,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

pub use pon_runtime::PyObject;
use pon_runtime::{abi, intern};
use sha2::{Digest, Sha256};

#[cfg(feature = "experimental-continuations")]
pub mod async_move;
mod compat;
pub mod compiler_identity;
#[cfg(feature = "experimental-continuations")]
pub mod continuation;
pub mod frozen_gameplay;
mod host;
mod import_hooks;
mod import_policy;
mod safety;
#[cfg(feature = "experimental-continuations")]
pub mod sequential;
#[cfg(feature = "experimental-continuations")]
pub mod sequential_move;
mod stdlib;
pub use frozen_gameplay::{FrozenGameplayGraph, FrozenGameplayScope};
pub use host::{
    HostHandle, HostId, HostScope, NativeHostProxy, native_call, native_call_named, native_get,
    native_set,
};
pub use stdlib::{MaterializedStandardLibrary, StandardLibrary};

#[derive(Clone, Debug)]
pub enum Error {
    Compile(String),
    MissingCallback(String),
    Runtime(String),
    Value(String),
    Io(String),
}

const PROFILE_PHASES: usize = 6;
const PROFILE_NAMES: [&str; PROFILE_PHASES] = [
    "guards",
    "boxing",
    "compiled_call",
    "recapture_rooting",
    "guard_teardown",
    "recovery_unbox",
];

static PROFILE_ENABLED: OnceLock<bool> = OnceLock::new();
static PROFILE_COUNTS: [AtomicU64; PROFILE_PHASES] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];
static PROFILE_NANOS: [AtomicU64; PROFILE_PHASES] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];
fn profile_enabled() -> bool {
    *PROFILE_ENABLED.get_or_init(|| std::env::var_os("SKIRMISH_PON_PROFILE_PHASES").is_some())
}

fn profile_record(index: usize, started: Option<Instant>) {
    if let Some(started) = started {
        PROFILE_COUNTS[index].fetch_add(1, Ordering::Relaxed);
        PROFILE_NANOS[index].fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
}

/// Reset the opt-in callback phase counters used by the manual benchmark.
pub fn reset_callback_phase_profile() {
    for counter in &PROFILE_COUNTS {
        counter.store(0, Ordering::Relaxed);
    }
    for total in &PROFILE_NANOS {
        total.store(0, Ordering::Relaxed);
    }
}

/// Return `(phase, calls, total_nanoseconds)` for the opt-in callback profile.
pub fn callback_phase_profile() -> Vec<(&'static str, u64, u64)> {
    PROFILE_NAMES
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            (
                name,
                PROFILE_COUNTS[index].load(Ordering::Relaxed),
                PROFILE_NANOS[index].load(Ordering::Relaxed),
            )
        })
        .collect()
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(e) => write!(f, "Pon compilation failed: {e}"),
            Self::MissingCallback(e) => write!(f, "Pon callback `{e}` is not published"),
            Self::Runtime(e) => write!(f, "Pon runtime error: {e}"),
            Self::Value(e) => write!(f, "unsupported value: {e}"),
            Self::Io(e) => write!(f, "source bundle I/O error: {e}"),
        }
    }
}

/// Ordinary Python source files loaded through Pon's filesystem importer.
#[derive(Clone, Debug)]
pub struct SourceBundle {
    version: Arc<str>,
    files: Arc<BTreeMap<String, Arc<str>>>,
}

impl SourceBundle {
    pub fn new(version: impl Into<Arc<str>>) -> Self {
        Self {
            version: version.into(),
            files: Arc::new(BTreeMap::new()),
        }
    }

    pub fn with_file(
        mut self,
        relative_path: impl Into<String>,
        source: impl Into<Arc<str>>,
    ) -> Result<Self, Error> {
        let path = relative_path.into();
        validate_bundle_path(&path)?;
        Arc::make_mut(&mut self.files).insert(path, source.into());
        Ok(self)
    }

    /// Look up an exact retained bundle entry by its normalized relative path.
    pub fn source(&self, relative_path: &str) -> Option<&str> {
        self.files.get(relative_path).map(Arc::as_ref)
    }

    /// Merge checked dependency files into this bundle, rejecting conflicting
    /// source identities so the embedded API cannot be shadowed by a mod.
    pub fn merge(mut self, other: &SourceBundle) -> Result<Self, Error> {
        for (path, source) in other.files.iter() {
            if let Some(existing) = self.files.get(path) {
                if existing.as_ref() != source.as_ref() {
                    return Err(Error::Value(format!(
                        "conflicting source bundle file `{path}`"
                    )));
                }
            } else {
                Arc::make_mut(&mut self.files).insert(path.clone(), Arc::clone(source));
            }
        }
        Ok(self)
    }

    /// Copies a checked-in Python package tree into the bundle without
    /// interpreting its contents. Only `.py` files are included.
    pub fn from_directory(
        version: impl Into<Arc<str>>,
        root: impl AsRef<Path>,
    ) -> Result<Self, Error> {
        fn visit(bundle: &mut SourceBundle, root: &Path, directory: &Path) -> Result<(), Error> {
            for entry in std::fs::read_dir(directory).map_err(io_error)? {
                let entry = entry.map_err(io_error)?;
                let path = entry.path();
                if path
                    .file_name()
                    .is_some_and(|name| name == "__pycache__" || name == ".venv")
                {
                    continue;
                }
                let metadata = std::fs::symlink_metadata(&path).map_err(io_error)?;
                if metadata.file_type().is_symlink() {
                    return Err(Error::Io(format!(
                        "source package contains symlink: {}",
                        path.display()
                    )));
                }
                if metadata.is_dir() {
                    visit(bundle, root, &path)?;
                    continue;
                }
                if path.extension().and_then(|ext| ext.to_str()) != Some("py") {
                    continue;
                }
                let relative = path
                    .strip_prefix(root)
                    .map_err(|error| Error::Io(error.to_string()))?;
                let source = std::fs::read_to_string(&path).map_err(io_error)?;
                *bundle = bundle.clone().with_file(
                    relative
                        .to_string_lossy()
                        .replace(std::path::MAIN_SEPARATOR, "/"),
                    source,
                )?;
            }
            Ok(())
        }
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(Error::Io(format!(
                "source package directory does not exist: {}",
                root.display()
            )));
        }
        let mut bundle = Self::new(version);
        visit(&mut bundle, root, root)?;
        Ok(bundle)
    }

    /// Materialize into a content-addressed, versioned directory below `root`.
    /// A completed directory is never overwritten, so equal versions with
    /// different source bytes cannot collide or reuse stale modules.
    pub fn materialize(&self, root: impl AsRef<Path>) -> Result<MaterializedBundle, Error> {
        validate_version(&self.version)?;
        let digest = self.identity_digest();
        let digest_hex = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = root
            .as_ref()
            .join(format!("skirmish-python-{}-{digest_hex}", self.version));
        let module_names = self
            .files
            .keys()
            .filter_map(|path| module_name_from_path(path))
            .collect::<Vec<_>>();
        std::fs::create_dir_all(root.as_ref()).map_err(io_error)?;
        if path.exists() {
            verify_materialized(&path, &self.files)?;
            return Ok(MaterializedBundle {
                root: path,
                module_names: module_names.into(),
            });
        }
        let staging = root.as_ref().join(format!(
            ".staging-{digest_hex}-{}-{}",
            std::process::id(),
            MODULE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&staging).map_err(io_error)?;
        let _staging_cleanup = StagingDir(&staging);
        for (relative, source) in self.files.iter() {
            let target = staging.join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(io_error)?;
            }
            std::fs::write(&target, source.as_bytes()).map_err(io_error)?;
        }
        if let Err(error) = std::fs::rename(&staging, &path) {
            if path.exists() {
                verify_materialized(&path, &self.files)?;
            } else {
                return Err(io_error(error));
            }
        }
        Ok(MaterializedBundle {
            root: path,
            module_names: module_names.into(),
        })
    }

    /// Return the content digest used for materialized bundle identity.
    pub fn identity_digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update((self.version.len() as u64).to_le_bytes());
        hash.update(self.version.as_bytes());
        for (name, source) in self.files.iter() {
            hash.update((name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash.update((source.len() as u64).to_le_bytes());
            hash.update(source.as_bytes());
        }
        hash.finalize().into()
    }
}

#[derive(Clone, Debug)]
pub struct MaterializedBundle {
    root: PathBuf,
    module_names: Arc<[String]>,
}
impl MaterializedBundle {
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn module_names(&self) -> &[String] {
        &self.module_names
    }
}

fn module_name_from_path(path: &str) -> Option<String> {
    let path = path
        .strip_suffix("/__init__.py")
        .or_else(|| path.strip_suffix(".py"))?;
    Some(path.replace('/', "."))
}

fn validate_bundle_path(path: &str) -> Result<(), Error> {
    let candidate = Path::new(path);
    let components = candidate.components().collect::<Vec<_>>();
    let canonical = components
        .iter()
        .filter_map(|component| match component {
            std::path::Component::Normal(name) => Some(name.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if path.is_empty()
        || path.contains('\\')
        || candidate.is_absolute()
        || components.len() != canonical.matches('/').count() + 1
        || components
            .iter()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        || canonical != path
        || path.as_bytes().contains(&0)
    {
        return Err(Error::Value(format!("invalid source bundle path `{path}`")));
    }
    Ok(())
}

fn verify_materialized(path: &Path, files: &BTreeMap<String, Arc<str>>) -> Result<(), Error> {
    if !path.is_dir()
        || std::fs::symlink_metadata(path)
            .map_err(io_error)?
            .file_type()
            .is_symlink()
    {
        return Err(Error::Io(format!(
            "materialized bundle is not a directory: {}",
            path.display()
        )));
    }
    for (relative, source) in files {
        let target = path.join(relative);
        let metadata = std::fs::symlink_metadata(&target).map_err(io_error)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(Error::Io(format!(
                "materialized bundle entry is unsafe: {}",
                target.display()
            )));
        }
        let bytes = std::fs::read(&target).map_err(io_error)?;
        if bytes != source.as_bytes() {
            return Err(Error::Io(format!(
                "materialized bundle entry is corrupt: {}",
                target.display()
            )));
        }
    }
    let mut seen = BTreeMap::new();
    collect_materialized(path, path, &mut seen)?;
    if seen.len() != files.len() || files.keys().any(|name| !seen.contains_key(name)) {
        return Err(Error::Io(format!(
            "materialized bundle contents do not match {}",
            path.display()
        )));
    }
    Ok(())
}

struct StagingDir<'a>(&'a Path);

impl Drop for StagingDir<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0);
    }
}

fn collect_materialized(
    root: &Path,
    directory: &Path,
    seen: &mut BTreeMap<String, ()>,
) -> Result<(), Error> {
    for entry in std::fs::read_dir(directory).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path).map_err(io_error)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::Io(format!(
                "materialized bundle contains symlink: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            collect_materialized(root, &path, seen)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| Error::Io(error.to_string()))?
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            seen.insert(relative, ());
        } else {
            return Err(Error::Io(format!(
                "materialized bundle contains special file: {}",
                path.display()
            )));
        }
    }
    Ok(())
}
fn validate_version(version: &str) -> Result<(), Error> {
    if version.is_empty()
        || version
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
    {
        return Err(Error::Value(format!(
            "invalid source bundle version `{version}`"
        )));
    }
    Ok(())
}
fn io_error(error: std::io::Error) -> Error {
    Error::Io(error.to_string())
}

/// Temporarily gives one bundle ownership of its source module names in
/// Pon's live `sys.modules`. Pon's importer treats that dictionary as the
/// public authority and evicts its internal cache when a binding is removed.
/// The process-wide path lock makes this swap safe across prepared programs.
struct BundleModuleGuard {
    modules: *mut PyObject,
    saved: Vec<(String, Option<*mut PyObject>)>,
    _roots: safety::PersistentRoots,
}

impl BundleModuleGuard {
    fn install(names: &[String], owned: Option<&[(String, *mut PyObject)]>) -> Result<Self, Error> {
        let modules = pon_runtime::import::sys_modules_dict().map_err(Error::Runtime)?;
        let mut guard = Self {
            modules,
            saved: Vec::new(),
            _roots: safety::PersistentRoots::new(),
        };
        for name in names {
            let key = unsafe { abi::pon_const_str(name.as_ptr(), name.len()) };
            if key.is_null() {
                return Err(Error::Runtime(diagnostic()));
            }
            let existing = unsafe { pon_runtime::types::dict::dict_get(guard.modules, key) }
                .map_err(Error::Runtime)?;
            if let Some(existing) = existing {
                root_module_values(&mut guard._roots, existing);
            }
            // Record before mutating the live dictionary so an allocation or
            // hashing failure later in this loop can unwind the partial swap.
            guard.saved.push((name.clone(), existing));
            unsafe { pon_runtime::types::dict::dict_remove(guard.modules, key) }
                .map_err(Error::Runtime)?;
            if let Some((_, owned)) =
                owned.and_then(|modules| modules.iter().find(|(module, _)| module == name))
            {
                unsafe { pon_runtime::types::dict::dict_insert(guard.modules, key, *owned) }
                    .map_err(Error::Runtime)?;
            }
        }
        Ok(guard)
    }

    fn capture(&self, names: &[String]) -> Result<Vec<(String, *mut PyObject)>, Error> {
        let mut captured = Vec::new();
        for name in names {
            let key = unsafe { abi::pon_const_str(name.as_ptr(), name.len()) };
            if key.is_null() {
                return Err(Error::Runtime(diagnostic()));
            }
            if let Some(module) = unsafe { pon_runtime::types::dict::dict_get(self.modules, key) }
                .map_err(Error::Runtime)?
            {
                captured.push((name.clone(), module));
            }
        }
        Ok(captured)
    }
}

impl Drop for BundleModuleGuard {
    fn drop(&mut self) {
        for (name, value) in self.saved.iter() {
            let key = unsafe { abi::pon_const_str(name.as_ptr(), name.len()) };
            if key.is_null() {
                let _ = diagnostic();
                continue;
            }
            let _ = unsafe { pon_runtime::types::dict::dict_remove(self.modules, key) };
            let result = match value {
                Some(value) => unsafe {
                    pon_runtime::types::dict::dict_insert(self.modules, key, *value)
                },
                None => Ok(()),
            };
            if result.is_err() {
                let _ = diagnostic();
            }
        }
    }
}

/// Pon's importer, active-module stack, JIT execution state, and attachment
/// registry are process-global; serialize all facade operations.
static PON_RUNTIME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static CONFIGURED_STDLIB_IDENTITY: std::sync::OnceLock<Option<[u8; 32]>> =
    std::sync::OnceLock::new();

fn controlled_import_policy(
    bundle_path: Option<&Path>,
    standard_library: Option<&Path>,
) -> Result<Option<import_policy::ImportPolicyGuard>, Error> {
    let roots = bundle_path
        .into_iter()
        .chain(standard_library)
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    if roots.is_empty() {
        Ok(None)
    } else {
        // Any materialized bundle is a verified execution boundary. The
        // policy is permissive while preparation compiles its graph and is
        // frozen for every later callback, including development bundles that
        // do not carry a separate stdlib root.
        import_policy::install(roots, true).map(Some)
    }
}

impl std::error::Error for Error {}

pub type NativeFunction = unsafe extern "C" fn(*mut *mut PyObject, usize) -> *mut PyObject;
pub type NativeValueFunction = fn(&[Value]) -> Result<Value, String>;

static VALUE_NATIVE_SLOTS: [std::sync::OnceLock<NativeValueFunction>; 5] = [
    std::sync::OnceLock::new(),
    std::sync::OnceLock::new(),
    std::sync::OnceLock::new(),
    std::sync::OnceLock::new(),
    std::sync::OnceLock::new(),
];

unsafe extern "C" fn value_native<const SLOT: usize>(
    args: *mut *mut PyObject,
    count: usize,
) -> *mut PyObject {
    let result = (|| {
        let values = (0..count)
            .map(|index| unsafe { native_unbox_value(*args.add(index)) })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        let function = VALUE_NATIVE_SLOTS[SLOT]
            .get()
            .ok_or_else(|| "native value callback slot is unregistered".to_owned())?;
        function(&values)
    })();
    match result {
        Ok(value) => {
            native_box_value(value).unwrap_or_else(|error| native_error(&error.to_string()))
        }
        Err(error) => native_error(&error),
    }
}

/// Register stateless callbacks implemented against the typed [`Value`] ABI.
/// Five slots are reserved for the gameplay math module, avoiding raw unsafe
/// callback declarations in the game crate.
pub fn register_native_value_module(
    name: &str,
    entries: [(&'static str, NativeValueFunction, usize); 5],
) -> Result<(), Error> {
    let callbacks: [NativeFunction; 5] = [
        value_native::<0>,
        value_native::<1>,
        value_native::<2>,
        value_native::<3>,
        value_native::<4>,
    ];
    for (slot, (_, function, _)) in entries.iter().enumerate() {
        VALUE_NATIVE_SLOTS[slot]
            .set(*function)
            .map_err(|_| Error::Runtime("native value callback slot already registered".into()))?;
    }
    register_native_module::<5>(
        name,
        std::array::from_fn(|index| (entries[index].0, callbacks[index], entries[index].2)),
    )
}

static MODULE_ID: AtomicU64 = AtomicU64::new(1);
static ABS_COMPAT: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();

fn install_compat_adapters() -> Result<(), Error> {
    match ABS_COMPAT.get_or_init(compat::install_abs_dispatch) {
        Ok(()) => Ok(()),
        Err(error) => Err(Error::Runtime(error.clone())),
    }
}

/// Installs a native Python module before compiling source that imports it.
/// The function objects are immortal through the runtime module registry.
pub fn register_native_module<const N: usize>(
    name: &str,
    functions: [(&'static str, NativeFunction, usize); N],
) -> Result<(), Error> {
    import_hooks::install();
    if unsafe { abi::pon_runtime_init() } != 0 {
        return Err(Error::Runtime(diagnostic()));
    }
    let storage = safety::RootedVec::new();
    let mut roots = storage.guard();
    let mut attrs = Vec::new();
    for (function_name, entry, arity) in functions {
        let object =
            unsafe { abi::pon_make_function(entry as *const u8, arity, intern(function_name)) };
        if object.is_null() {
            return Err(Error::Runtime(diagnostic()));
        }
        roots.push(object);
        attrs.push((intern(function_name), object));
    }
    pon_runtime::import::install_module(name, attrs)
        .map(|module| {
            import_policy::record_native_module(name, module);
        })
        .map(|_| ())
        .map_err(Error::Runtime)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    None,
    Bool(bool),
    Int(i64),
    F32(f32),
    String(String),
    List(Vec<Value>),
    Tuple(Vec<Value>),
    Dict(BTreeMap<String, Value>),
}

/// Send+Sync source identity. Compilation happens in `prepare_for_thread`.
#[derive(Clone, Debug)]
pub struct Program {
    source: Arc<str>,
    filename: Arc<str>,
    hooks: Arc<[String]>,
    standard_library: Option<(Arc<PathBuf>, [u8; 32])>,
}

impl Program {
    pub fn new(
        source: impl Into<Arc<str>>,
        filename: impl Into<Arc<str>>,
        hooks: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            source: source.into(),
            filename: filename.into(),
            hooks: hooks.into_iter().map(Into::into).collect::<Vec<_>>().into(),
            standard_library: None,
        }
    }

    /// Identity of the compiler/runtime/bridge used to prepare this program.
    /// Consumers should include this in persistent program-cache keys.
    pub fn compiler_identity(&self) -> [u8; 32] {
        compiler_identity::compiler_identity()
    }

    /// Stable cache identity for this source and the compiler/runtime that
    /// prepares it. Standard-library provenance is included when configured.
    pub fn identity_digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"skirmish-pon-program-v2\0");
        hash.update(self.compiler_identity());
        hash.update((self.source.len() as u64).to_le_bytes());
        hash.update(self.source.as_bytes());
        hash.update((self.filename.len() as u64).to_le_bytes());
        hash.update(self.filename.as_bytes());
        for hook in self.hooks.iter() {
            hash.update((hook.len() as u64).to_le_bytes());
            hash.update(hook.as_bytes());
        }
        if let Some((_, identity)) = &self.standard_library {
            hash.update([1]);
            hash.update(identity);
        } else {
            hash.update([0]);
        }
        hash.finalize().into()
    }

    /// Attach a verified stdlib root for compilation and lazy callback imports.
    pub fn with_standard_library(mut self, library: &MaterializedStandardLibrary) -> Self {
        self.standard_library = Some((
            Arc::new(library.root().to_path_buf()),
            library.identity_digest(),
        ));
        self
    }

    pub fn prepare_for_thread(&self) -> Result<PreparedProgram, Error> {
        self.prepare_for_thread_with_path(
            None,
            None,
            self.standard_library
                .as_ref()
                .map(|(path, _)| path.as_path()),
            self.standard_library
                .as_ref()
                .map(|(_, identity)| *identity),
        )
    }

    pub fn prepare_for_thread_in_bundle(
        &self,
        bundle: &MaterializedBundle,
    ) -> Result<PreparedProgram, Error> {
        self.prepare_for_thread_with_path(
            Some(bundle.root()),
            Some(bundle.module_names()),
            self.standard_library
                .as_ref()
                .map(|(path, _)| path.as_path()),
            self.standard_library
                .as_ref()
                .map(|(_, identity)| *identity),
        )
    }

    fn prepare_for_thread_with_path(
        &self,
        bundle_path: Option<&Path>,
        module_names: Option<&[String]>,
        standard_library: Option<&Path>,
        standard_library_identity: Option<[u8; 32]>,
    ) -> Result<PreparedProgram, Error> {
        let _pon_lock = PON_RUNTIME_LOCK.lock().expect("Pon runtime lock poisoned");
        // A different prepared handle may already be parked on this same
        // host thread.  Make the thread fully active before any preparation
        // allocations; the token restores those parked handles on every
        // error path and after the new handle is parked.
        let parked = safety::GcSafeRegion::suspend_all();
        let attachment = safety::Attachment::acquire().map_err(Error::Runtime)?;
        // Eager native modules are created by runtime init; install the hook
        // first so their identities enter the trusted registry at birth.
        import_hooks::install();
        if unsafe { abi::pon_runtime_init() } != 0 {
            return Err(Error::Runtime(diagnostic()));
        }
        install_compat_adapters()?;
        import_hooks::install();
        let configured = CONFIGURED_STDLIB_IDENTITY.get_or_init(|| standard_library_identity);
        if *configured != standard_library_identity {
            return Err(Error::Runtime(
                "Pon standard library identity is already fixed for this process".into(),
            ));
        }
        let mut marker = 0usize;
        let _stack = unsafe { safety::StackBoundary::capture(ptr::addr_of_mut!(marker).cast()) };
        let modules = module_names
            .map(|names| BundleModuleGuard::install(names, None))
            .transpose()?;
        let mut _import_policy = controlled_import_policy(bundle_path, standard_library)?;
        let mut prepared = self.prepare_for_thread_inner()?;
        // Capture provenance after the root module and all eager imports have
        // been prepared, so the frozen callback context covers the complete
        // loaded module graph.
        if let Some(guard) = _import_policy.as_mut() {
            import_policy::capture_loaded_modules(guard)?;
        }
        let prepared_import_policy = _import_policy
            .as_ref()
            .map(|guard| guard.prepared_policy().clone());
        prepared.bundle_path = bundle_path.map(Path::to_path_buf);
        prepared.standard_library_path = standard_library.map(Path::to_path_buf);
        prepared.module_names = module_names.map(|names| names.to_vec().into());
        prepared.owned_modules = match (modules.as_ref(), module_names) {
            (Some(modules), Some(names)) => Some(modules.capture(names)?.into()),
            _ => None,
        };
        prepared.import_policy = prepared_import_policy;
        let mut roots = safety::PersistentRoots::new();
        if let Some(owned) = prepared.owned_modules.as_deref() {
            for (_, module) in owned {
                root_module_values(&mut roots, *module);
            }
        }
        if let Some(policy) = prepared.import_policy.as_ref() {
            for module in import_policy::loaded_modules(policy) {
                root_module_values(&mut roots, module);
            }
        }
        for callback in prepared.callback_slots.iter().copied() {
            roots.push(callback);
        }
        // Publish the complete replacement before dropping the empty roots
        // created by prepare_for_thread_inner.
        prepared.roots = roots;
        // Keep the attachment (and its thread-local root registry) alive for
        // the prepared handle's entire lifetime.  Enter the idle region only
        // after all guards have been dropped; their destructors call Pon.
        drop(_import_policy);
        drop(modules);
        prepared._attachment = Some(attachment);
        prepared.idle_safe = Some(safety::GcSafeRegion::enter());
        parked.restore();
        Ok(prepared)
    }

    fn prepare_for_thread_inner(&self) -> Result<PreparedProgram, Error> {
        if unsafe { abi::pon_runtime_init() } != 0 {
            return Err(Error::Runtime(diagnostic()));
        }
        import_hooks::install();
        let module_name = format!(
            "_skirmish_program_{}",
            MODULE_ID.fetch_add(1, Ordering::Relaxed)
        );
        if let Err(error) = pon_runtime::import::install_module(&module_name, []) {
            return Err(Error::Runtime(error));
        }
        if let Err(error) = pon_runtime::import::begin_module_execution(&module_name) {
            return Err(Error::Runtime(error));
        }
        let module = pon_jit::compile_source_to_module(&self.source, &self.filename, "exec")
            .map_err(|e| {
                pon_runtime::import::end_module_execution(&module_name);
                Error::Compile(e.to_string())
            })?;
        // Pon's import registry is process-global and keeps module values
        // after this prepared handle is dropped. Keep the JIT image alive for
        // the same process lifetime; dropping it after execute could leave
        // globals and callbacks pointing at freed code or constants.
        let module = Box::leak(Box::new(module));
        let mut marker = 0usize;
        let _stack =
            unsafe { safety::StackBoundary::capture(ptr::addr_of_mut!(marker).cast::<u8>()) };
        let result = unsafe { pon_jit::execute(module, ptr::null_mut(), ptr::null_mut()) };
        pon_runtime::import::end_module_execution(&module_name);
        if result.is_null() {
            let error = Error::Runtime(diagnostic());
            return Err(error);
        }
        let mut callbacks = BTreeMap::new();
        let mut callback_slots = Vec::with_capacity(self.hooks.len());
        let mut callback_indices = BTreeMap::new();
        for name in self.hooks.iter() {
            let Some(callback) =
                pon_runtime::import::module_attr(intern(&module_name), intern(name))
            else {
                let error = Error::MissingCallback(name.clone());
                return Err(error);
            };
            callbacks.insert(name.clone(), callback);
            callback_indices.insert(name.clone(), callback_slots.len());
            callback_slots.push(callback);
        }
        Ok(PreparedProgram {
            filename: self.filename.to_string(),
            callbacks,
            callback_slots,
            callback_indices,
            module_name,
            bundle_path: None,
            standard_library_path: None,
            module_names: None,
            owned_modules: None,
            import_policy: None,
            roots: safety::PersistentRoots::new(),
            idle_safe: None,
            _attachment: None,
        })
    }
}

pub struct PreparedProgram {
    /// Keeps the executable code and its JIT data alive while callbacks run.
    filename: String,
    callbacks: BTreeMap<String, *mut PyObject>,
    callback_slots: Vec<*mut PyObject>,
    callback_indices: BTreeMap<String, usize>,
    module_name: String,
    bundle_path: Option<PathBuf>,
    standard_library_path: Option<PathBuf>,
    module_names: Option<Arc<[String]>>,
    owned_modules: Option<Arc<[(String, *mut PyObject)]>>,
    import_policy: Option<import_policy::PreparedImportPolicy>,
    roots: safety::PersistentRoots,
    idle_safe: Option<safety::GcSafeRegion>,
    // Declared after roots so roots unregister before the thread detaches.
    _attachment: Option<safety::Attachment>,
}

impl PreparedProgram {
    #[must_use]
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Resolve a published callback name to its stable slot before entering
    /// the gameplay hot path.
    pub fn callback_index(&self, callback: &str) -> Result<usize, Error> {
        self.callbacks
            .get(callback)
            .and_then(|_| self.callback_indices.get(callback).copied())
            .ok_or_else(|| Error::MissingCallback(callback.into()))
    }

    #[cfg(feature = "experimental-continuations")]
    pub fn invoke_move_step(
        &mut self,
        step: &mut continuation::NativeStep,
        phase: continuation::StepPhase,
        factory_slot: usize,
        factory_args: &[Value],
        scalar: Option<Value>,
    ) -> Result<Value, Error> {
        self.invoke_move_step_with_result_adapter(
            step,
            phase,
            factory_slot,
            factory_args,
            scalar,
            None,
        )
    }

    #[cfg(feature = "experimental-continuations")]
    pub fn invoke_move_step_with_result_adapter(
        &mut self,
        step: &mut continuation::NativeStep,
        phase: continuation::StepPhase,
        factory_slot: usize,
        factory_args: &[Value],
        scalar: Option<Value>,
        result_adapter_slot: Option<usize>,
    ) -> Result<Value, Error> {
        let _pon_lock = PON_RUNTIME_LOCK.lock().expect("Pon runtime lock poisoned");
        let parked = safety::GcSafeRegion::suspend_all();
        let result = self.invoke_move_step_active(
            step,
            phase,
            factory_slot,
            factory_args,
            scalar,
            result_adapter_slot,
        );
        parked.restore();
        result
    }

    #[cfg(feature = "experimental-continuations")]
    pub fn invoke_native_entry_with_result_adapter(
        &mut self,
        entry: &mut continuation::NativeEntry,
        factory_slot: usize,
        factory_args: &[Value],
        scalar_spills: &[Value],
        result_adapter_slot: Option<usize>,
    ) -> Result<Value, Error> {
        self.invoke_native_entry_with_result_adapter_in_module(
            "__root__",
            entry,
            factory_slot,
            factory_args,
            scalar_spills,
            result_adapter_slot,
        )
    }

    #[cfg(feature = "experimental-continuations")]
    pub fn invoke_native_entry_with_result_adapter_in_module(
        &mut self,
        module_name: &str,
        entry: &mut continuation::NativeEntry,
        factory_slot: usize,
        factory_args: &[Value],
        scalar_spills: &[Value],
        result_adapter_slot: Option<usize>,
    ) -> Result<Value, Error> {
        let entry_module = if module_name == "__root__" {
            self.module_name.clone()
        } else {
            let declared = self
                .module_names
                .as_deref()
                .is_some_and(|names| names.iter().any(|name| name == module_name));
            let owned = self
                .owned_modules
                .as_deref()
                .is_some_and(|modules| modules.iter().any(|(name, _)| name == module_name));
            if !declared || !owned {
                return Err(Error::Runtime(format!(
                    "async defining module `{module_name}` is not in the prepared source bundle"
                )));
            }
            module_name.to_owned()
        };
        let _pon_lock = PON_RUNTIME_LOCK.lock().expect("Pon runtime lock poisoned");
        let parked = safety::GcSafeRegion::suspend_all();
        let result = self.invoke_native_entry_active(
            &entry_module,
            factory_slot,
            factory_args,
            scalar_spills,
            result_adapter_slot,
            |argv, argc| unsafe { entry.call_active(argv, argc) },
        );
        parked.restore();
        result
    }

    #[cfg(feature = "experimental-continuations")]
    fn invoke_move_step_active(
        &mut self,
        step: &mut continuation::NativeStep,
        phase: continuation::StepPhase,
        factory_slot: usize,
        factory_args: &[Value],
        scalar: Option<Value>,
        result_adapter_slot: Option<usize>,
    ) -> Result<Value, Error> {
        self.invoke_native_entry_active(
            &self.module_name.clone(),
            factory_slot,
            factory_args,
            scalar.into_iter().collect::<Vec<_>>().as_slice(),
            result_adapter_slot,
            |argv, argc| unsafe { step.call_active(phase, argv, argc) },
        )
    }

    fn invoke_native_entry_active<F>(
        &mut self,
        entry_module: &str,
        factory_slot: usize,
        factory_args: &[Value],
        scalar_spills: &[Value],
        result_adapter_slot: Option<usize>,
        mut invoke: F,
    ) -> Result<Value, Error>
    where
        F: FnMut(*mut *mut PyObject, usize) -> *mut PyObject,
    {
        let mut marker = 0usize;
        let _stack = unsafe { safety::StackBoundary::capture(ptr::addr_of_mut!(marker).cast()) };
        let _modules = self
            .module_names
            .as_deref()
            .map(|names| BundleModuleGuard::install(names, self.owned_modules.as_deref()))
            .transpose()?;
        let _import_policy = match self.import_policy.as_ref() {
            Some(policy) => Some(import_policy::install_prepared(policy)?),
            None => controlled_import_policy(
                self.bundle_path.as_deref(),
                self.standard_library_path.as_deref(),
            )?,
        };
        let factory = *self
            .callback_slots
            .get(factory_slot)
            .ok_or_else(|| Error::MissingCallback(format!("slot {factory_slot}")))?;
        let result_adapter = result_adapter_slot
            .map(|slot| {
                self.callback_slots
                    .get(slot)
                    .copied()
                    .ok_or_else(|| Error::MissingCallback(format!("slot {slot}")))
            })
            .transpose()?;

        let factory_storage = safety::RootedVec::new();
        let mut factory_roots = factory_storage.guard();
        for value in factory_args {
            factory_roots.push(box_value(value)?);
        }
        let execution = (|| -> Result<Value, Error> {
            pon_runtime::import::begin_module_execution(&self.module_name)
                .map_err(Error::Runtime)?;
            let factory_result =
                unsafe { abi::pon_call(factory, factory_roots.as_mut_ptr(), factory_roots.len()) };
            pon_runtime::import::end_module_execution(&self.module_name);
            if factory_result.is_null() {
                return Err(Error::Runtime(diagnostic()));
            }
            if pon_runtime::tag::is_small_int(factory_result) {
                return Err(Error::Value(
                    "move argument factory must return a tuple/list".into(),
                ));
            }
            factory_roots.push(factory_result);

            let type_name = unsafe { (*(*factory_result).ob_type).name() };
            let items: &[*mut PyObject] = match type_name {
                "tuple" => {
                    let tuple =
                        unsafe { &*factory_result.cast::<pon_runtime::types::tuple::PyTuple>() };
                    unsafe { tuple.as_slice() }
                }
                "list" => {
                    let list =
                        unsafe { &*factory_result.cast::<pon_runtime::types::list::PyList>() };
                    unsafe { list.as_slice() }
                }
                _ => {
                    return Err(Error::Value(
                        "move argument factory must return a tuple/list".into(),
                    ));
                }
            };
            if items.len() != 2 {
                return Err(Error::Value(
                    "move argument factory must return (move, context) tuple/list".into(),
                ));
            }
            let step_storage = safety::RootedVec::new();
            let mut step_roots = step_storage.guard();
            let mut argv = Vec::with_capacity(items.len() + scalar_spills.len());
            for item in items.iter().copied() {
                if item.is_null() {
                    return Err(Error::Value("move argument factory returned NULL".into()));
                }
                step_roots.push(item);
                argv.push(item);
            }
            for value in scalar_spills {
                // Nested values may add several GC roots; only the returned
                // top-level object belongs in the native argv array.
                let boxed = box_value_into(value, &mut step_roots)?;
                argv.push(boxed);
            }
            pon_runtime::import::begin_module_execution(entry_module).map_err(Error::Runtime)?;
            let result = invoke(argv.as_mut_ptr(), argv.len());
            pon_runtime::import::end_module_execution(entry_module);
            if result.is_null() {
                return Err(Error::Runtime(diagnostic()));
            }
            step_roots.push(result);
            let result = if let Some(adapter) = result_adapter {
                let mut adapter_argv = [result];
                pon_runtime::import::begin_module_execution(&self.module_name)
                    .map_err(Error::Runtime)?;
                let adapted = unsafe { abi::pon_call(adapter, adapter_argv.as_mut_ptr(), 1) };
                pon_runtime::import::end_module_execution(&self.module_name);
                if adapted.is_null() {
                    return Err(Error::Runtime(diagnostic()));
                }
                step_roots.push(adapted);
                adapted
            } else {
                result
            };
            unbox_value(result)
        })();
        let capture =
            if let (Some(guard), Some(names)) = (_modules.as_ref(), self.module_names.as_deref()) {
                Some(guard.capture(names))
            } else {
                None
            };
        let capture_error = match capture {
            Some(Ok(captured)) => {
                if self.module_names.is_some() {
                    if let Some(policy) = self.import_policy.as_ref() {
                        for (name, module) in &captured {
                            import_policy::check_loaded_module(policy, name, *module)?;
                        }
                    }
                    let mut roots = safety::PersistentRoots::new();
                    for (_, module) in &captured {
                        root_module_values(&mut roots, *module);
                    }
                    if let Some(policy) = self.import_policy.as_ref() {
                        for module in import_policy::loaded_modules(policy) {
                            root_module_values(&mut roots, module);
                        }
                    }
                    for callback in self.callback_slots.iter().copied() {
                        roots.push(callback);
                    }
                    self.roots = roots;
                    self.owned_modules = Some(captured.into());
                }
                None
            }
            Some(Err(error)) => Some(error),
            None => None,
        };
        match execution {
            Err(error) => Err(error),
            Ok(result) => match capture_error {
                Some(error) => Err(error),
                None => Ok(result),
            },
        }
    }
}

impl PreparedProgram {
    pub fn invoke(&mut self, callback: &str, args: &[Value]) -> Result<Value, Error> {
        let callback = self.callback_index(callback)?;
        self.invoke_index(callback, args)
    }

    /// Invoke a callback resolved by [`Self::callback_index`].
    pub fn invoke_index(&mut self, callback: usize, args: &[Value]) -> Result<Value, Error> {
        self.with_invocation_scope(|scope| scope.invoke_index(callback, args))
    }

    /// Invoke several callbacks while keeping the runtime invocation guards
    /// installed for the duration of `body`.
    pub fn with_invocation_scope<R, F>(&mut self, body: F) -> Result<R, Error>
    where
        F: FnOnce(&mut InvocationScope<'_>) -> Result<R, Error>,
    {
        let _pon_lock = PON_RUNTIME_LOCK.lock().expect("Pon runtime lock poisoned");
        let parked = safety::GcSafeRegion::suspend_all();
        let result = (|| {
            let profiling = profile_enabled();
            let guards_started = profiling.then(Instant::now);
            let mut marker = 0usize;
            let _stack =
                unsafe { safety::StackBoundary::capture(ptr::addr_of_mut!(marker).cast()) };
            let modules = self
                .module_names
                .as_deref()
                .map(|names| BundleModuleGuard::install(names, self.owned_modules.as_deref()))
                .transpose()?;
            let import_policy = match self.import_policy.as_ref() {
                Some(policy) => Some(import_policy::install_prepared(policy)?),
                None => controlled_import_policy(
                    self.bundle_path.as_deref(),
                    self.standard_library_path.as_deref(),
                )?,
            };
            profile_record(0, guards_started);
            let mut scope = InvocationScope {
                program: self,
                modules: modules.as_ref(),
                import_policy: import_policy.as_ref(),
                first_error: None,
            };
            let result = body(&mut scope);
            let result = match (scope.first_error.take(), result) {
                (Some(error), _) => Err(error),
                (None, result) => result,
            };
            let teardown_started = profiling.then(Instant::now);
            drop(scope);
            drop(import_policy);
            drop(modules);
            profile_record(4, teardown_started);
            result
        })();
        parked.restore();
        result
    }

    fn invoke_active_index_with_guards(
        &mut self,
        callback: usize,
        args: &[Value],
        modules: Option<&BundleModuleGuard>,
        import_policy_guard: Option<&import_policy::ImportPolicyGuard>,
    ) -> Result<Value, Error> {
        let profiling = profile_enabled();
        let callee = *self
            .callback_slots
            .get(callback)
            .ok_or_else(|| Error::MissingCallback(format!("slot {callback}")))?;
        let boxing_started = profiling.then(Instant::now);
        let storage = safety::RootedVec::new();
        let mut boxed = storage.guard();
        for value in args {
            boxed.push(box_value(value)?);
        }
        pon_runtime::import::begin_module_execution(&self.module_name).map_err(Error::Runtime)?;
        profile_record(1, boxing_started);
        let compiled_started = profiling.then(Instant::now);
        let result = unsafe { abi::pon_call(callee, boxed.as_mut_ptr(), boxed.len()) };
        pon_runtime::import::end_module_execution(&self.module_name);
        profile_record(2, compiled_started);
        let callback_error = result.is_null().then(diagnostic);
        if !result.is_null() {
            boxed.push(result);
        }
        if let (Some(guard), Some(names)) = (modules, self.module_names.as_deref()) {
            let recapture_started = profiling.then(Instant::now);
            let captured = match guard.capture(names) {
                Ok(captured) => captured,
                Err(_) if callback_error.is_some() => {
                    return Err(Error::Runtime(callback_error.expect("callback error")));
                }
                Err(error) => return Err(error),
            };
            let policy = import_policy_guard
                .map(|guard| guard.prepared_policy())
                .or(self.import_policy.as_ref());
            if let Some(policy) = policy {
                for (name, module) in &captured {
                    if let Err(error) = import_policy::check_loaded_module(policy, name, *module) {
                        // Preserve the callback's original diagnostic when
                        // its failed execution also left an invalid module
                        // identity behind.
                        if let Some(callback_error) = callback_error.as_ref() {
                            return Err(Error::Runtime(callback_error.clone()));
                        }
                        return Err(error);
                    }
                }
            }
            let pointers = captured
                .iter()
                .map(|(_, module)| *module)
                .collect::<Vec<_>>();
            let mut roots = safety::PersistentRoots::new();
            for module in pointers {
                root_module_values(&mut roots, module);
            }
            if let Some(policy) = policy {
                for module in import_policy::loaded_modules(policy) {
                    root_module_values(&mut roots, module);
                }
            }
            for callback in self.callback_slots.iter().copied() {
                roots.push(callback);
            }
            // Keep the previous snapshot registered while this replacement
            // is assembled, then drop it only after the new snapshot exists.
            self.roots = roots;
            self.owned_modules = Some(captured.into());
            profile_record(3, recapture_started);
        }
        if let Some(error) = callback_error {
            return Err(Error::Runtime(error));
        }
        let recovery_started = profiling.then(Instant::now);
        let value = unbox_value(result);
        profile_record(5, recovery_started);
        value
    }
}

/// Active runtime context passed to a scoped invocation closure.
pub struct InvocationScope<'a> {
    program: &'a mut PreparedProgram,
    modules: Option<&'a BundleModuleGuard>,
    import_policy: Option<&'a import_policy::ImportPolicyGuard>,
    first_error: Option<Error>,
}

impl InvocationScope<'_> {
    /// Whether an invocation has already latched a runtime failure. Facades
    /// use this to preserve the first native failure over later adapter errors.
    pub fn has_failed(&self) -> bool {
        self.first_error.is_some()
    }

    /// Invoke a callback resolved by [`PreparedProgram::callback_index`].
    pub fn invoke_index(&mut self, callback: usize, args: &[Value]) -> Result<Value, Error> {
        if let Some(error) = &self.first_error {
            return Err(error.clone());
        }
        let result = self.program.invoke_active_index_with_guards(
            callback,
            args,
            self.modules,
            self.import_policy,
        );
        if let Err(error) = &result {
            self.first_error = Some(error.clone());
        }
        result
    }
}

/// Pon module objects are immortal descriptor boxes; their namespace values
/// therefore need explicit roots when a bundle is parked outside sys.modules.
fn root_module_values(roots: &mut safety::PersistentRoots, module: *mut PyObject) {
    // Duplicate roots are harmless and avoid an O(n²) linear membership scan
    // across large SDK namespaces. Visit the live namespace in place so the
    // callback path does not allocate a temporary Vec for every module.
    roots.push(module);
    // SAFETY: the visitor only appends already-owned pointers to `roots`; it
    // never calls back into Pon import or module mutation APIs.
    let _ = unsafe {
        pon_runtime::import::for_each_module_object_attr(module, |value| roots.push(value))
    };
}

fn diagnostic() -> String {
    pon_runtime::pon_err_message().unwrap_or_else(|| "unknown Pon error".into())
}

pub(crate) fn box_value(value: &Value) -> Result<*mut PyObject, Error> {
    let storage = safety::RootedVec::new();
    let mut roots = storage.guard();
    box_value_into(value, &mut roots)
}

/// Box a value for a stateless native callback. The returned object is rooted
/// only for the duration of this call, as required by the Pon callback ABI.
pub fn native_box_value(value: Value) -> Result<*mut PyObject, Error> {
    box_value(&value)
}

/// Decode one value at a stateless native module boundary.
/// # Safety
/// `value` must be a live Pon object pointer from the callback argument array.
pub unsafe fn native_unbox_value(value: *mut PyObject) -> Result<Value, Error> {
    if value.is_null() {
        return Err(Error::Value("native argument is NULL".into()));
    }
    unbox_value(value)
}

/// Raise a Pon type error from a stateless native module callback.
pub fn native_error(message: &str) -> *mut PyObject {
    unsafe { abi::exc::pon_raise_type_error(message.as_ptr(), message.len()) }
}

fn box_value_into(
    value: &Value,
    roots: &mut safety::RootedVecGuard<'_>,
) -> Result<*mut PyObject, Error> {
    let object = match value {
        Value::None => unsafe { abi::pon_none() },
        Value::Bool(v) => unsafe { abi::pon_const_bool(i32::from(*v)) },
        Value::Int(v) => unsafe { abi::pon_const_int(*v) },
        Value::F32(v) => unsafe { abi::number::pon_const_float(f64::from(*v)) },
        Value::String(v) => unsafe { abi::pon_const_str(v.as_ptr(), v.len()) },
        Value::List(values) => {
            let mut items = values
                .iter()
                .map(|value| box_value_into(value, roots))
                .collect::<Result<Vec<_>, _>>()?;
            unsafe { abi::seq::pon_build_list(items.as_mut_ptr(), items.len()) }
        }
        Value::Tuple(values) => {
            let mut items = values
                .iter()
                .map(|value| box_value_into(value, roots))
                .collect::<Result<Vec<_>, _>>()?;
            unsafe { abi::seq::pon_build_tuple(items.as_mut_ptr(), items.len()) }
        }
        Value::Dict(values) => box_dict_into(values, roots)?,
    };
    if object.is_null() {
        Err(Error::Runtime(diagnostic()))
    } else {
        roots.push(object);
        Ok(object)
    }
}

fn box_dict_into(
    values: &BTreeMap<String, Value>,
    roots: &mut safety::RootedVecGuard<'_>,
) -> Result<*mut PyObject, Error> {
    let constructor = unsafe { abi::pon_load_global(intern("dict"), ptr::null_mut()) };
    if constructor.is_null() {
        return Err(Error::Runtime(diagnostic()));
    }
    let dict = unsafe { abi::pon_call(constructor, ptr::null_mut(), 0) };
    if dict.is_null() {
        return Err(Error::Runtime(diagnostic()));
    }
    roots.push(dict);
    for (key, value) in values {
        let key_object = unsafe { abi::pon_const_str(key.as_ptr(), key.len()) };
        roots.push(key_object);
        let value_object = box_value_into(value, roots)?;
        if key_object.is_null() || value_object.is_null() {
            return Err(Error::Runtime(diagnostic()));
        }
        if unsafe { abi::map::pon_subscript_set(dict, key_object, value_object) }.is_null() {
            return Err(Error::Runtime(diagnostic()));
        }
    }
    Ok(dict)
}

pub(crate) fn unbox_value(value: *mut PyObject) -> Result<Value, Error> {
    if value.is_null() {
        return Err(Error::Value("cannot unbox null Pon object".into()));
    }
    if value == unsafe { abi::pon_none() } {
        return Ok(Value::None);
    }
    if pon_runtime::tag::is_small_int(value) {
        return Ok(Value::Int(pon_runtime::tag::untag_small_int(value)));
    }
    if let Some(value) = unsafe { pon_runtime::types::bool_::to_bool(value) } {
        return Ok(Value::Bool(value));
    }
    let type_name = unsafe { (*(*value).ob_type).name() };
    match type_name {
        "int" => Ok(Value::Int(unsafe {
            (*value.cast::<pon_runtime::PyLong>()).value
        })),
        "float" => Ok(Value::F32(unsafe {
            (*value.cast::<pon_runtime::types::float::PyFloat>()).value as f32
        })),
        "str" => unsafe { pon_runtime::types::type_::unicode_text(value) }
            .map(|text| Value::String(text.to_owned()))
            .ok_or_else(|| Error::Value("invalid Pon unicode result".into())),
        "list" => {
            let list = unsafe { &*value.cast::<pon_runtime::types::list::PyList>() };
            let values = unsafe { list.as_slice() }
                .iter()
                .copied()
                .map(unbox_value)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::List(values))
        }
        "tuple" => {
            let tuple = unsafe { &*value.cast::<pon_runtime::types::tuple::PyTuple>() };
            let values = unsafe { tuple.as_slice() }
                .iter()
                .copied()
                .map(unbox_value)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Tuple(values))
        }
        "dict" => {
            let dict =
                unsafe { pon_runtime::types::dict::dict_ref(value) }.map_err(Error::Value)?;
            let mut values = BTreeMap::new();
            for entry in &dict.entries {
                let key = unsafe { pon_runtime::types::type_::unicode_text(entry.key) }
                    .ok_or_else(|| Error::Value("dict result contains a non-string key".into()))?;
                values.insert(key.to_owned(), unbox_value(entry.value)?);
            }
            Ok(Value::Dict(values))
        }
        _ => Err(Error::Value(format!(
            "unsupported Pon result type `{type_name}`"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn class_method_repeated_callback() {
        let source = "class Fighter:\n    def action(self, value):\n        return value + 1\nfighter = Fighter()\naction = fighter.action\n";
        let mut program = Program::new(source, "fighter.py", ["action"])
            .prepare_for_thread()
            .expect("compile");
        assert_eq!(
            program.invoke("action", &[Value::Int(4)]).unwrap(),
            Value::Int(5)
        );
        assert_eq!(
            program.invoke("action", &[Value::Int(8)]).unwrap(),
            Value::Int(9)
        );
    }
}
