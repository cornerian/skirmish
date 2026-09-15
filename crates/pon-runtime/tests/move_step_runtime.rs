#![cfg(feature = "experimental-continuations")]

use pon_ir::lower_source;
use skirmish_pon_runtime::continuation::{StepPhase, compile_module_entry, lower_continuation};
use skirmish_pon_runtime::{Program, Value};

#[test]
fn move_step_uses_retained_move_and_fresh_context_for_both_phases() {
    let source = r#"
class Context:
    def __init__(self, post):
        self.pre = None
        self.post = post

class Move:
    def __init__(self, value):
        self.value = value
    async def run(self, action):
        await action.pre
        return self.value + action.post

move = Move(5)
def factory():
    return [move, Context(7)]

def adapt(value):
    return 99
"#;
    let plan = lower_continuation(source, "Move.run", "test:move-step").unwrap();
    let mut program = Program::new(source, "move-step.py", ["factory", "adapt"])
        .prepare_for_thread()
        .unwrap();
    let slot = program.callback_index("factory").unwrap();
    let adapter_slot = program.callback_index("adapt").unwrap();
    let mut step = plan.compile_native(source).unwrap();

    assert_eq!(
        program
            .invoke_move_step(&mut step, StepPhase::Pre, slot, &[], None)
            .unwrap(),
        Value::List(vec![Value::None]),
    );
    assert_eq!(
        program
            .invoke_move_step(&mut step, StepPhase::Post, slot, &[], None)
            .unwrap(),
        Value::Int(12),
    );
    assert_eq!(
        program
            .invoke_move_step_with_result_adapter(
                &mut step,
                StepPhase::Post,
                slot,
                &[],
                None,
                Some(adapter_slot),
            )
            .unwrap(),
        Value::Int(99),
    );
}

#[test]
fn move_step_rejects_tagged_integer_factory_result_before_type_access() {
    let source = r#"
class Move:
    async def run(self, action):
        await action
        return 1

def factory():
    return 1
"#;
    let plan = lower_continuation(source, "Move.run", "test:tagged-factory").unwrap();
    let mut program = Program::new(source, "tagged-factory.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let slot = program.callback_index("factory").unwrap();
    let mut step = plan.compile_native(source).unwrap();
    let error = program
        .invoke_move_step(&mut step, StepPhase::Pre, slot, &[], None)
        .unwrap_err();
    assert!(
        error.to_string().contains("tuple/list"),
        "unexpected error: {error}"
    );
}

#[test]
fn cached_adapter_unboxes_raw_wait_object_while_rooted() {
    let source = r#"
class Wait:
    def __init__(self, deadline):
        self.deadline = deadline

class Context:
    def __init__(self, post):
        self.pre = Wait(23)
        self.post = post

class Move:
    def __init__(self, value):
        self.value = value
    async def run(self, action):
        await action.pre
        return self.value + action.post

move = Move(5)
def factory():
    return [move, Context(7)]

def adapt(value):
    return value[0].deadline
"#;
    let plan = lower_continuation(source, "Move.run", "test:raw-wait").unwrap();
    let mut program = Program::new(source, "raw-wait.py", ["factory", "adapt"])
        .prepare_for_thread()
        .unwrap();
    let factory_slot = program.callback_index("factory").unwrap();
    let adapter_slot = program.callback_index("adapt").unwrap();
    let mut step = plan.compile_native(source).unwrap();

    // The raw Wait object cannot be represented by Value directly; the
    // optional cached adapter consumes it while the native result remains
    // rooted in the invocation scope.
    assert!(
        program
            .invoke_move_step(&mut step, StepPhase::Pre, factory_slot, &[], None)
            .is_err()
    );
    assert_eq!(
        program
            .invoke_move_step_with_result_adapter(
                &mut step,
                StepPhase::Pre,
                factory_slot,
                &[],
                None,
                Some(adapter_slot),
            )
            .unwrap(),
        Value::Int(23)
    );
    assert_eq!(
        program
            .invoke_move_step(&mut step, StepPhase::Post, factory_slot, &[], None)
            .unwrap(),
        Value::Int(12)
    );
}

#[test]
fn compiles_selected_entry_from_pre_lowered_module() {
    let module =
        lower_source("class Box:\n    value = 9\ndef read(box):\n    return box.value\n").unwrap();
    let function = module
        .functions
        .iter()
        .position(|function| function.name == "read")
        .map(|index| pon_ir::FunctionId(index as u32))
        .unwrap();
    let entry = compile_module_entry(module, function).unwrap();
    assert_eq!(entry.arity(), 1);
}

#[test]
fn invokes_pre_lowered_native_entry_with_retained_factory_objects() {
    let source = r#"
class Context:
    def __init__(self, value):
        self.value = value

class Move:
    def __init__(self, value):
        self.value = value

move = Move(5)
def factory():
    import gc
    gc.collect()
    return [move, Context(7)]

def double(value):
    return value + value

def entry(move, context, spill):
    return double(move.value) + context.value + spill
"#;
    let module = lower_source(source).unwrap();
    let function = module
        .functions
        .iter()
        .position(|function| function.name == "entry")
        .map(|index| pon_ir::FunctionId(index as u32))
        .unwrap();
    let mut entry = compile_module_entry(module, function).unwrap();
    let mut program = Program::new(source, "native-entry.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let factory_slot = program.callback_index("factory").unwrap();
    let non_scalar_error = program
        .invoke_native_entry_with_result_adapter(
            &mut entry,
            factory_slot,
            &[],
            &[Value::List(vec![Value::Int(3)])],
            None,
        )
        .unwrap_err();
    assert!(!non_scalar_error.to_string().is_empty());
    assert_eq!(
        program
            .invoke_native_entry_with_result_adapter(
                &mut entry,
                factory_slot,
                &[],
                &[Value::Int(3)],
                None,
            )
            .unwrap(),
        Value::Int(20)
    );
}

#[test]
fn native_entry_preserves_exception_and_roots_factory_objects_across_gc() {
    let source = r#"
class Context:
    def __init__(self, value):
        self.value = value

class Move:
    def __init__(self, value):
        self.value = value

move = Move(5)
def factory():
    import gc
    gc.collect()
    return [move, Context(7)]

attempt = 0
def entry(move, context):
    global attempt
    attempt = attempt + 1
    if attempt == 1:
        raise ValueError("native entry boom")
    return move.value + context.value
"#;
    let module = lower_source(source).unwrap();
    let function = module
        .functions
        .iter()
        .position(|function| function.name == "entry")
        .map(|index| pon_ir::FunctionId(index as u32))
        .unwrap();
    let mut entry = compile_module_entry(module, function).unwrap();
    let mut program = Program::new(source, "native-entry-error.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let factory_slot = program.callback_index("factory").unwrap();
    let error = program
        .invoke_native_entry_with_result_adapter(&mut entry, factory_slot, &[], &[], None)
        .unwrap_err();
    assert!(
        error.to_string().contains("native entry boom"),
        "unexpected error: {error}"
    );
    assert_eq!(
        program
            .invoke_native_entry_with_result_adapter(&mut entry, factory_slot, &[], &[], None)
            .unwrap(),
        Value::Int(12)
    );
}
