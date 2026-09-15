//! Native coroutine conformance probes for the pinned Pon runtime.
//!
//! These tests deliberately drive the boxed ABI from Rust.  There is no
//! asyncio loop or wall-clock polling: the yielded value is the deterministic
//! event token, and the second native call supplies the resume value.

use std::{
    ptr,
    sync::{Mutex, Once},
};

use pon_runtime::{PyObject, abi, format_object_for_print, intern};
use skirmish_pon_runtime::{Program, Value, register_native_module};

unsafe extern "C" fn drive(argv: *mut *mut PyObject, argc: usize) -> *mut PyObject {
    if argc != 1 {
        return abi::return_null_with_error("drive expects one mode");
    }
    let mode = match format_object_for_print(unsafe { *argv }) {
        Ok(text) => text,
        Err(error) => return abi::return_null_with_error(error),
    };
    let (name, slot) = match mode.as_str() {
        "0" => ("coro", ptr::addr_of_mut!(FIRST)),
        "1" => ("coro", ptr::addr_of_mut!(FIRST)),
        "2" => ("cancel_coro", ptr::addr_of_mut!(SECOND)),
        "3" => ("cancel_coro", ptr::addr_of_mut!(SECOND)),
        "4" | "5" => ("coro_a", ptr::addr_of_mut!(THIRD)),
        "6" | "7" => ("coro_b", ptr::addr_of_mut!(FOURTH)),
        "8" | "9" => ("error_coro", ptr::addr_of_mut!(FIFTH)),
        _ => return abi::return_null_with_error("unknown drive mode"),
    };
    unsafe {
        if (*slot).is_null() {
            *slot = abi::r#gen::pon_await(
                abi::pon_load_global(intern(name), ptr::null_mut()),
                ptr::null_mut(),
            );
            if (*slot).is_null() {
                return ptr::null_mut();
            }
        }
        if mode == "3" {
            let result = abi::r#gen::pon_gen_close(*slot);
            if result.is_null() {
                return ptr::null_mut();
            }
            return abi::pon_load_global(intern("cleanup"), ptr::null_mut());
        }
        if mode == "9" {
            let result = abi::r#gen::pon_gen_close(*slot);
            if !result.is_null() {
                return abi::return_null_with_error("expected cancellation cleanup to fail");
            }
            return ptr::null_mut();
        }
        let value = match mode.as_str() {
            "0" | "2" | "4" | "6" | "8" => abi::pon_none(),
            "1" | "5" | "7" => abi::pon_const_str(b"event-ok".as_ptr(), b"event-ok".len()),
            _ => unreachable!(),
        };
        let result = abi::r#gen::pon_gen_send(*slot, value);
        if !result.is_null() {
            return result;
        }
        if mode == "1" {
            return abi::r#gen::pon_gen_stop_value();
        }
        if mode == "5" || mode == "7" {
            return abi::r#gen::pon_gen_stop_value();
        }
        result
    }
}

static mut FIRST: *mut PyObject = ptr::null_mut();
static mut SECOND: *mut PyObject = ptr::null_mut();
static mut THIRD: *mut PyObject = ptr::null_mut();
static mut FOURTH: *mut PyObject = ptr::null_mut();
static mut FIFTH: *mut PyObject = ptr::null_mut();
static REGISTER: Once = Once::new();
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn ensure_registered() {
    REGISTER.call_once(|| {
        register_native_module("host_coroutine_probe", [("drive", drive, 1)]).unwrap()
    });
}

#[test]
fn async_def_is_callable_and_completes_after_native_resume() {
    let _guard = TEST_LOCK.lock().unwrap();
    unsafe {
        FIRST = ptr::null_mut();
        SECOND = ptr::null_mut();
        THIRD = ptr::null_mut();
        FOURTH = ptr::null_mut();
        FIFTH = ptr::null_mut();
    }
    ensure_registered();
    let source = r#"
from host_coroutine_probe import drive
class Event:
    def __await__(self):
        value = yield "pending"
        return value
async def task():
    return await Event()
coro = task()
step = drive
"#;
    let mut program = Program::new(source, "coroutines.py", ["step"])
        .prepare_for_thread()
        .unwrap();

    assert!(
        matches!(program.invoke("step", &[Value::Int(0)]), Ok(Value::String(value)) if value == "pending")
    );
    assert_eq!(
        program.invoke("step", &[Value::Int(1)]).unwrap(),
        Value::String("event-ok".into())
    );
}

#[test]
fn cancellation_runs_finally_cleanup() {
    let _guard = TEST_LOCK.lock().unwrap();
    ensure_registered();
    unsafe {
        FIRST = ptr::null_mut();
        SECOND = ptr::null_mut();
        THIRD = ptr::null_mut();
        FOURTH = ptr::null_mut();
        FIFTH = ptr::null_mut();
    }
    let source = r#"
from host_coroutine_probe import drive
cleanup = []
class Event:
    def __await__(self):
        yield "pending"
async def task():
    try:
        await Event()
    finally:
        cleanup.append("closed")
cancel_coro = task()
cancel = drive
"#;
    let mut program = Program::new(source, "coroutines_cancel.py", ["cancel"])
        .prepare_for_thread()
        .unwrap();
    assert!(matches!(
        program.invoke("cancel", &[Value::Int(2)]),
        Ok(Value::String(value)) if value == "pending"
    ));
    let cleanup = program.invoke("cancel", &[Value::Int(3)]).unwrap();
    assert!(
        matches!(cleanup, Value::List(ref values) if values == &[Value::String("closed".into())]),
        "cleanup = {cleanup:?}"
    );
}

#[test]
fn one_compiled_callback_drives_independent_event_subscriptions() {
    let _guard = TEST_LOCK.lock().unwrap();
    ensure_registered();
    unsafe {
        FIRST = ptr::null_mut();
        SECOND = ptr::null_mut();
        THIRD = ptr::null_mut();
        FOURTH = ptr::null_mut();
        FIFTH = ptr::null_mut();
    }
    let source = r#"
from host_coroutine_probe import drive
class Event:
    def __await__(self):
        value = yield "event:hit-confirmed"
        return value
async def task():
    return await Event()
coro_a = task()
coro_b = task()
step = drive
"#;
    let mut program = Program::new(source, "coroutines_instances.py", ["step"])
        .prepare_for_thread()
        .unwrap();
    assert_eq!(
        program.invoke("step", &[Value::Int(4)]).unwrap(),
        Value::String("event:hit-confirmed".into())
    );
    assert_eq!(
        program.invoke("step", &[Value::Int(6)]).unwrap(),
        Value::String("event:hit-confirmed".into())
    );
    assert_eq!(
        program.invoke("step", &[Value::Int(5)]).unwrap(),
        Value::String("event-ok".into())
    );
    assert_eq!(
        program.invoke("step", &[Value::Int(7)]).unwrap(),
        Value::String("event-ok".into())
    );
}

#[test]
fn cancellation_propagates_exception_raised_by_finally() {
    let _guard = TEST_LOCK.lock().unwrap();
    ensure_registered();
    unsafe {
        FIRST = ptr::null_mut();
        SECOND = ptr::null_mut();
        THIRD = ptr::null_mut();
        FOURTH = ptr::null_mut();
        FIFTH = ptr::null_mut();
    }
    let source = r#"
from host_coroutine_probe import drive
class Event:
    def __await__(self):
        yield "pending"
async def task():
    try:
        await Event()
    finally:
        raise ValueError("cleanup-failed")
error_coro = task()
cancel = drive
"#;
    let mut program = Program::new(source, "coroutines_error.py", ["cancel"])
        .prepare_for_thread()
        .unwrap();
    assert_eq!(
        program.invoke("cancel", &[Value::Int(8)]).unwrap(),
        Value::String("pending".into())
    );
    let error = program.invoke("cancel", &[Value::Int(9)]).unwrap_err();
    assert!(
        error.to_string().contains("cleanup-failed"),
        "error = {error}"
    );
}
