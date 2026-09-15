#![cfg(feature = "experimental-continuations")]

use pon_ir::lower_source;
use skirmish_pon_runtime::continuation::AwaitToken;
use skirmish_pon_runtime::sequential::{compile_sequential, extract_segments};
use skirmish_pon_runtime::sequential_move::SequentialMoveExecutor;
use skirmish_pon_runtime::{Program, Value};

#[test]
fn sequential_segments_compile_with_spill_prologues() {
    let source = r#"
class Move:
    async def run(self, action):
        counter = 1
        await action.first
        counter = counter + 1
        await action.second
        return counter + 1
class Context:
    def __init__(self):
        self.first = None
        self.second = None
def factory():
    return [None, Context()]
"#;
    let module = lower_source(source).unwrap();
    let run = module
        .functions
        .iter()
        .position(|f| f.name == "run")
        .unwrap();
    let mut image = compile_sequential(&module, pon_ir::FunctionId(run as u32)).unwrap();
    assert_eq!(image.entries.len(), 3);
    assert_eq!(image.entries[0].arity(), 2);
    assert_eq!(image.entries[1].arity(), 4);
    assert_eq!(image.entries[2].arity(), 4);
    let mut program = Program::new(source, "sequential.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let factory = program.callback_index("factory").unwrap();
    let wait_a = program
        .invoke_native_entry_with_result_adapter(&mut image.entries[0], factory, &[], &[], None)
        .unwrap();
    assert_eq!(
        wait_a,
        Value::List(vec![
            Value::Int(1),
            Value::Int(1),
            Value::None,
            Value::Int(1)
        ])
    );
    let wait_b = program
        .invoke_native_entry_with_result_adapter(
            &mut image.entries[1],
            factory,
            &[],
            &[Value::Int(1), Value::None],
            None,
        )
        .unwrap();
    assert_eq!(
        wait_b,
        Value::List(vec![
            Value::Int(1),
            Value::Int(2),
            Value::None,
            Value::Int(2)
        ])
    );
    let complete = program
        .invoke_native_entry_with_result_adapter(
            &mut image.entries[2],
            factory,
            &[],
            &[Value::Int(2), Value::None],
            None,
        )
        .unwrap();
    assert_eq!(complete, Value::List(vec![Value::Int(0), Value::Int(3)]));
    assert_eq!(image.segments[0].suspensions[0].output_locals, vec![2]);
    assert_eq!(image.segments[1].input_locals, vec![2]);
}

#[test]
fn sequential_spills_are_boundary_live_and_initialized() {
    let source = r#"
class Move:
    async def run(self, action):
        counter = 1
        await action.first
        await action.second
        return counter
"#;
    let module = lower_source(source).unwrap();
    let run = module.functions.iter().find(|f| f.name == "run").unwrap();
    let segments = extract_segments(run).unwrap();
    assert_eq!(segments[0].suspensions[0].output_locals, vec![2]);
    assert_eq!(segments[1].input_locals, vec![2]);
    assert_eq!(segments[1].suspensions[0].output_locals, vec![2]);
    assert_eq!(segments[2].input_locals, vec![2]);
}

#[test]
fn sequential_rejects_scalar_to_object_overwrite() {
    let source = r#"
class Move:
    async def run(self, action):
        counter = [1]
        await action.first
        return counter
class Context:
    def __init__(self):
        self.first = None
def factory():
    return [None, Context()]
"#;
    let mut executor = SequentialMoveExecutor::new(source, "Move.run", "native:heap-spill")
        .expect("heap values are rejected at the ABI boundary, not during lowering");
    let mut program = Program::new(source, "sequential-heap-spill.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let factory = program.callback_index("factory").unwrap();
    let token = AwaitToken {
        owner: 7,
        generation: 1,
        event_kind: "first".into(),
        deadline_frame: 0,
    };
    let error = executor
        .start_scoped(token, |entry, spills| {
            program.invoke_native_entry_with_result_adapter(entry, factory, &[], spills, None)
        })
        .expect_err("heap spill must not enter PendingMove");
    assert!(error.to_string().contains("not scalar"), "{error}");
    assert!(executor.checkpoint().is_none());
}

#[test]
fn sequential_native_entry_keeps_function_references() {
    let source = r#"
def helper(value):
    return value + 1
class Move:
    async def run(self, action):
        counter = helper(1)
        await action.first
        return counter
class Context:
    def __init__(self):
        self.first = None
def factory():
    return [None, Context()]
"#;
    let module = lower_source(source).unwrap();
    let run = module
        .functions
        .iter()
        .position(|f| f.name == "run")
        .unwrap();
    let mut image = compile_sequential(&module, pon_ir::FunctionId(run as u32)).unwrap();
    let mut program = Program::new(source, "sequential-reference.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let factory = program.callback_index("factory").unwrap();
    let wait = program
        .invoke_native_entry_with_result_adapter(&mut image.entries[0], factory, &[], &[], None)
        .unwrap();
    assert_eq!(
        wait,
        Value::List(vec![
            Value::Int(1),
            Value::Int(1),
            Value::None,
            Value::Int(2)
        ])
    );
}

#[test]
fn zero_await_async_function_compiles_as_direct_native_entry() {
    let source = r#"
class Move:
    async def run(self, action):
        return 7
def factory():
    return [None, None]
"#;
    let module = lower_source(source).unwrap();
    let run = module
        .functions
        .iter()
        .position(|f| f.name == "run")
        .unwrap();
    let mut image = compile_sequential(&module, pon_ir::FunctionId(run as u32)).unwrap();
    assert_eq!(image.entries.len(), 1);
    let mut program = Program::new(source, "zero-await.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let factory = program.callback_index("factory").unwrap();
    let result = program
        .invoke_native_entry_with_result_adapter(&mut image.entries[0], factory, &[], &[], None)
        .unwrap();
    assert_eq!(result, Value::List(vec![Value::Int(0), Value::Int(7)]));
}
