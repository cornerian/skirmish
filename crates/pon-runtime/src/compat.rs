//! Small compatibility adapters installed by the Skirmish Pon embedding.

use std::ptr;

use pon_runtime::{PyObject, abi, descr, intern, tag};

/// Resolve `abs` through the operand's class/MRO, with Pon's native numeric
/// fallback for tagged integers and types without a Python-level override.
unsafe extern "C" fn abs_dispatch(argv: *mut *mut PyObject, argc: usize) -> *mut PyObject {
    if argc != 1 || argv.is_null() {
        let message = b"abs() takes exactly one argument";
        return unsafe { abi::pon_raise_type_error(message.as_ptr(), message.len()) };
    }
    let operand = unsafe { *argv };
    if operand.is_null() {
        let message = b"bad operand type for abs()";
        return unsafe { abi::pon_raise_type_error(message.as_ptr(), message.len()) };
    }
    if tag::is_small_int(operand) {
        return abi::number::abs_object(operand);
    }
    let ty = unsafe { (*operand).ob_type.cast_mut() };
    let descriptor = unsafe { descr::lookup_in_type(ty, intern("__abs__")) };
    if descriptor.is_null() {
        return abi::number::abs_object(operand);
    }
    let method = unsafe { descr::descriptor_get(descriptor, operand, ty) };
    if method.is_null() {
        return ptr::null_mut();
    }
    unsafe { abi::pon_call(method, ptr::null_mut(), 0) }
}

/// Install the adapter after Pon's builtins module has been initialized.
pub(crate) fn install_abs_dispatch() -> Result<(), String> {
    let function = unsafe { abi::pon_make_function(abs_dispatch as *const u8, 1, intern("abs")) };
    if function.is_null() {
        return Err("failed to allocate the abs compatibility adapter".into());
    }
    let result = unsafe { abi::pon_store_global(intern("abs"), function) };
    if result.is_null() {
        Err("failed to install the abs compatibility adapter".into())
    } else {
        Ok(())
    }
}
