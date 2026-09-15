//! Native script boundary and parser-independent execution data for Skirmish.
//!
#![forbid(unsafe_code)]

pub mod environment;
pub mod hooks;
pub mod value;

mod facade;
mod stdlib_config;
pub use environment::{Environment, ValueType};
pub use facade::{CallbackHandle, CompiledProgram, embedded_sdk_identity};
pub use hooks::{HookArgumentContract, HookKind, HookMetadata};
#[cfg(feature = "experimental-continuations")]
pub use skirmish_pon_runtime::Error as PonError;
pub use skirmish_pon_runtime::SourceBundle;
pub use skirmish_pon_runtime::compiler_identity;
#[cfg(feature = "experimental-continuations")]
pub use skirmish_pon_runtime::sequential_move;
pub use skirmish_pon_runtime::{MaterializedStandardLibrary, StandardLibrary};
pub use skirmish_pon_runtime::{
    NativeFunction, NativeValueFunction, PyObject, Value, native_box_value, native_error,
    native_unbox_value, register_native_module, register_native_value_module,
};
#[cfg(feature = "experimental-continuations")]
pub use skirmish_pon_runtime::{async_move, continuation};
pub use stdlib_config::{configure_standard_library, configured_standard_library_identity};
pub use value::host_object;
pub use value::{
    Error, HostRef, NativeHost, NativeKind, NativeObject, NativeValue, SharedNativeHost,
    shared_host,
};
