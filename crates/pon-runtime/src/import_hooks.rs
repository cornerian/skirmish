//! Pon frontend hooks used by the embedded runtime.
//!
//! The runtime owns import and dynamic-code namespace semantics.  These
//! callbacks only lower, compile, and enter source through `pon-jit`.

use std::path::Path;

use pon_runtime::{
    PyObject,
    dynexec::{DynCodeMode, DynCompileRequest, DynExecuteRequest, set_dynamic_code_hooks},
    import::{SourceModuleRequest, cached_module, set_source_module_loader},
    intern,
};

/// Install the source importer and dynamic-code bridge.
///
/// The Pon setters replace process-global function pointers and are
/// idempotent, so calling this before each runtime attachment is harmless.
pub(crate) fn install() {
    set_source_module_loader(load_source_module);
    pon_runtime::import::set_import_policy_hook(crate::import_policy::check_import);
    pon_runtime::import::set_native_module_provenance_hook(
        crate::import_policy::record_native_module,
    );
    set_dynamic_code_hooks(validate_dynamic_source, execute_dynamic_source);
}

fn load_source_module(request: SourceModuleRequest<'_>) -> Result<*mut PyObject, String> {
    crate::import_policy::check_source_path(request.name, request.path, request.source)?;
    crate::import_policy::record_source_module(request.name, request.module);
    let handle = compile(request.source, request.path, "exec")?;
    // JIT code can leave runtime objects borrowing constants from its engine.
    // Leak the handle deliberately: the runtime module cache can outlive this
    // callback and has no unload notification.  This also keeps the handle
    // thread-independent without claiming that JIT state is Send.
    let handle = Box::leak(Box::new(handle));
    let result = unsafe { pon_jit::execute(handle, std::ptr::null_mut(), std::ptr::null_mut()) };
    if result.is_null() {
        return Err(format!(
            "failed to execute source module '{}': {}",
            request.name,
            pon_runtime::pon_err_message().unwrap_or_else(|| "unknown Pon error".into())
        ));
    }
    cached_module(intern(request.name))
        .ok_or_else(|| format!("source module '{}' was not cached", request.name))
}

fn compile(source: &str, filename: &Path, mode: &str) -> Result<pon_jit::DynExecHandle, String> {
    pon_jit::compile_source_to_module(source, &filename.to_string_lossy(), mode)
        .map_err(|error| error.to_string())
}

fn dynamic_source(source: &str, mode: DynCodeMode) -> String {
    match mode {
        DynCodeMode::Eval => format!("__pon_dyn_eval_result = ({source})\n"),
        DynCodeMode::Exec => source.to_owned(),
        DynCodeMode::Single => {
            let wrapped = format!("__pon_dyn_single_result = ({source})\n");
            // Keep interactive input useful for expressions while accepting
            // statement input through the normal compiler.
            if pon_jit::compile_source_to_module(&wrapped, "<dynamic>", "single").is_ok() {
                wrapped
            } else {
                source.to_owned()
            }
        }
    }
}

fn validate_dynamic_source(request: DynCompileRequest<'_>) -> Result<(), String> {
    crate::import_policy::check_dynamic_code()?;
    let source = dynamic_source(request.source, request.mode);
    compile(
        &source,
        std::path::Path::new(request.filename),
        request.mode.as_str(),
    )
    .map(|_| ())
}

fn execute_dynamic_source(request: DynExecuteRequest<'_>) -> Result<*mut PyObject, String> {
    crate::import_policy::check_dynamic_code()?;
    let source = dynamic_source(request.source, request.mode);
    let handle = compile(
        &source,
        std::path::Path::new(request.filename),
        request.mode.as_str(),
    )?;
    let handle = Box::leak(Box::new(handle));
    let result = unsafe { pon_jit::execute(handle, request.globals, request.locals) };
    if result.is_null() {
        return Err(format!(
            "failed to execute dynamic source '{}': {}",
            request.filename,
            pon_runtime::pon_err_message().unwrap_or_else(|| "unknown Pon error".into())
        ));
    }
    match request.mode {
        DynCodeMode::Eval => {
            pon_runtime::import::active_module_attr(intern("__pon_dyn_eval_result"))
                .ok_or_else(|| "dynamic eval did not produce a result".to_owned())
        }
        DynCodeMode::Exec | DynCodeMode::Single => {
            let none = unsafe { pon_runtime::abi::pon_none() };
            if none.is_null() {
                Err("failed to allocate None for dynamic exec result".to_owned())
            } else {
                Ok(none)
            }
        }
    }
}
