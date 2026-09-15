#![cfg(feature = "experimental-continuations")]

use pon_ir::lower_source;
use skirmish_pon_runtime::continuation::AwaitToken;
use skirmish_pon_runtime::sequential::compile_sequential;
use skirmish_pon_runtime::sequential_move::{MoveStep, SequentialMoveExecutor};
use skirmish_pon_runtime::{Program, Value};

#[test]
fn immediate_scalar_return_has_zero_suspensions() {
    let source = r#"
class Move:
    async def run(self, action):
        return 23
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
    assert_eq!(image.segments.len(), 1);
    assert!(image.segments[0].suspensions.is_empty());
    let mut program = Program::new(source, "sequential-immediate.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let factory = program.callback_index("factory").unwrap();
    let result = program
        .invoke_native_entry_with_result_adapter(&mut image.entries[0], factory, &[], &[], None)
        .unwrap();
    assert_eq!(result, Value::List(vec![Value::Int(0), Value::Int(23)]));
}

#[test]
fn executor_completes_immediate_scalar_without_pending_state() {
    let source = r#"
class Move:
    async def run(self, action):
        return 23
def factory():
    return [None, None]
"#;
    let mut executor = SequentialMoveExecutor::new(source, "Move.run", "immediate:scalar").unwrap();
    let mut program = Program::new(source, "sequential-immediate-executor.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let factory = program.callback_index("factory").unwrap();
    let result = executor
        .start_scoped(
            AwaitToken {
                owner: 7,
                generation: 1,
                event_kind: "none".into(),
                deadline_frame: 0,
            },
            |entry, args| {
                program.invoke_native_entry_with_result_adapter(entry, factory, &[], args, None)
            },
        )
        .unwrap();
    assert!(matches!(result, MoveStep::Complete(Value::Int(23))));
    assert!(executor.checkpoint().is_none());
}

#[test]
fn immediate_branch_returns_preserve_each_result() {
    let source = r#"
class Move:
    async def run(self, action):
        if action.flag:
            return 31
        return 47
class Context:
    def __init__(self):
        self.flag = True
def factory():
    return [None, Context()]
"#;
    for (flag, expected) in [("True", 31), ("False", 47)] {
        let source = source.replace("self.flag = True", &format!("self.flag = {flag}"));
        let module = lower_source(&source).unwrap();
        let run = module
            .functions
            .iter()
            .position(|f| f.name == "run")
            .unwrap();
        let mut image = compile_sequential(&module, pon_ir::FunctionId(run as u32)).unwrap();
        assert_eq!(image.segments.len(), 1);
        assert!(image.segments[0].suspensions.is_empty());
        let mut program = Program::new(
            source.as_str(),
            "sequential-immediate-branch.py",
            ["factory"],
        )
        .prepare_for_thread()
        .unwrap();
        let factory = program.callback_index("factory").unwrap();
        let result = program
            .invoke_native_entry_with_result_adapter(&mut image.entries[0], factory, &[], &[], None)
            .unwrap();
        assert_eq!(
            result,
            Value::List(vec![Value::Int(0), Value::Int(expected)])
        );
    }
}
