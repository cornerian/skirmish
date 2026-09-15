//! Small end-to-end Pon embedding probe.
//!
//! The source is compiled once, then entered once. Its decorated event hook
//! is retained by the module and called repeatedly from Rust through Pon's
//! boxed runtime ABI, exercising class/decorator syntax without an
//! interpreter.

const FIGHTER_SOURCE: &str = r#"
def on_event(function):
    def registered(self, event):
        return function(self, event)
    return registered

class ProbeFighter:
    @on_event
    def hit(self, event):
        return "hit:" + event

fighter = ProbeFighter()
hook = fighter.hit
"#;

fn main() {
    let attached = unsafe { pon_runtime::pon_thread_attach() };
    assert!(!attached.is_null(), "Pon runtime thread attach failed");

    // Keep this marker alive around every generated-code entry. The collector
    // scans from this published boundary and pon_call roots its callee/argv
    // operands across nested dispatch and allocation-triggered collections.
    let mut stack_base_marker = 0usize;
    pon_runtime::aot_entry::capture_stack_base(
        std::ptr::addr_of_mut!(stack_base_marker).cast::<u8>(),
    );

    let mut handle = pon_jit::compile_source_to_module(FIGHTER_SOURCE, "probe_fighter.py", "exec")
        .expect("Pon failed to compile the fighter source");

    // SAFETY: the handle is live, owns the finalized JIT module, and Pon's
    // execution seam accepts null namespace pointers for module execution.
    let result =
        unsafe { pon_jit::execute(&mut handle, std::ptr::null_mut(), std::ptr::null_mut()) };
    assert!(
        !result.is_null(),
        "Pon returned its NULL exception sentinel"
    );

    let hook_name = pon_runtime::intern("hook");
    let hook = unsafe { pon_runtime::abi::pon_load_global(hook_name, std::ptr::null_mut()) };
    assert!(
        !hook.is_null(),
        "compiled module did not publish its decorated hook"
    );
    for event in ["jab", "tilt", "smash"] {
        let value = unsafe { pon_runtime::abi::pon_const_str(event.as_ptr(), event.len()) };
        assert!(!value.is_null(), "failed to allocate event argument");
        let mut argv = [value];
        let returned = unsafe { pon_runtime::abi::pon_call(hook, argv.as_mut_ptr(), argv.len()) };
        assert!(
            !returned.is_null(),
            "native callback failed: {:?}",
            pon_runtime::pon_err_message()
        );
        let returned_text = pon_runtime::format_object_for_print(returned)
            .expect("hook returned an unprintable value");
        assert_eq!(returned_text, format!("hit:{event}"));
    }

    // Exercise the NULL sentinel and thread-local diagnostic path. Clear the
    // error before leaving so this probe cannot leak an exception to callers.
    let error = unsafe { pon_runtime::abi::pon_call(hook, std::ptr::null_mut(), 0) };
    assert!(
        error.is_null(),
        "missing event argument unexpectedly succeeded"
    );
    let diagnostic = pon_runtime::pon_err_message().expect("missing Pon diagnostic");
    assert!(!diagnostic.is_empty(), "Pon diagnostic was empty");
    pon_runtime::pon_err_clear();

    println!("pon-native-event-hook-ok");
    println!("pon-probe-ok");

    let status = unsafe { pon_runtime::pon_thread_detach() };
    assert_eq!(status, 0, "Pon runtime thread detach failed");
}
