#![cfg(feature = "experimental-continuations")]

use pon_ir::lower_source;
use skirmish_pon_runtime::continuation::{AwaitToken, Scalar};
use skirmish_pon_runtime::sequential::compile_sequential;
use skirmish_pon_runtime::sequential_move::{MoveStep, SequentialMoveExecutor};
use skirmish_pon_runtime::{Program, Value};

fn token(generation: u64, event: &str) -> AwaitToken {
    AwaitToken {
        owner: 7,
        generation,
        event_kind: event.into(),
        deadline_frame: 0,
    }
}

fn native_first(
    source: &str,
    function: &str,
) -> (
    skirmish_pon_runtime::PreparedProgram,
    skirmish_pon_runtime::sequential::SequentialNativeImage,
    usize,
) {
    let module = lower_source(source).expect("source lowers");
    let function_id = module
        .functions
        .iter()
        .position(|candidate| candidate.name == function)
        .expect("function exists");
    let image = compile_sequential(&module, pon_ir::FunctionId(function_id as u32))
        .expect("sequential image");
    let program = Program::new(source, "sequential-cfg-review.py", ["factory"])
        .prepare_for_thread()
        .expect("native program");
    let factory = program.callback_index("factory").expect("factory callback");
    (program, image, factory)
}

#[test]
fn helper_float_and_bool_results_survive_wait_as_exact_scalars() {
    let source = r#"
def float_helper():
    return 2.5
def bool_helper():
    return True
class Move:
    async def run(self, action):
        amount = float_helper()
        enabled = bool_helper()
        await action.first
        return [amount, enabled]
class Context:
    def __init__(self):
        self.first = None
def factory():
    return [None, Context()]
"#;
    let (mut program, mut image, factory) = native_first(source, "run");
    assert_eq!(image.segments[0].suspensions[0].output_locals.len(), 2);
    let waiting = program
        .invoke_native_entry_with_result_adapter(&mut image.entries[0], factory, &[], &[], None)
        .expect("entry executes");
    assert_eq!(
        waiting,
        Value::List(vec![
            Value::Int(1),
            Value::Int(1),
            Value::None,
            Value::F32(2.5),
            Value::Bool(true),
        ])
    );
    let complete = program
        .invoke_native_entry_with_result_adapter(
            &mut image.entries[1],
            factory,
            &[],
            &[Value::F32(2.5), Value::Bool(true), Value::None],
            None,
        )
        .expect("resume entry executes");
    assert_eq!(
        complete,
        Value::List(vec![
            Value::Int(0),
            Value::List(vec![Value::F32(2.5), Value::Bool(true)])
        ])
    );
}

#[test]
fn scalar_context_attribute_loaded_before_wait_is_preserved() {
    let source = r#"
class Context:
    def __init__(self):
        self.first = None
        self.count = 17
class Move:
    async def run(self, action):
        count = action.count
        await action.first
        return count
def factory():
    return [None, Context()]
"#;
    let (mut program, mut image, factory) = native_first(source, "run");
    let waiting = program
        .invoke_native_entry_with_result_adapter(&mut image.entries[0], factory, &[], &[], None)
        .expect("entry executes");
    assert_eq!(
        waiting,
        Value::List(vec![
            Value::Int(1),
            Value::Int(1),
            Value::None,
            Value::Int(17)
        ])
    );
    let complete = program
        .invoke_native_entry_with_result_adapter(
            &mut image.entries[1],
            factory,
            &[],
            &[Value::Int(17), Value::None],
            None,
        )
        .expect("resume entry executes");
    assert_eq!(complete, Value::List(vec![Value::Int(0), Value::Int(17)]));
}

#[test]
fn branch_local_assignment_before_wait_has_explicit_result() {
    let source = r#"
class Move:
    async def run(self, action):
        if action.flag:
            branch_value = 11
        else:
            branch_value = 22
        await action.first
        return branch_value
class Context:
    def __init__(self):
        self.first = None
        self.flag = True
def factory():
    return [None, Context()]
"#;
    for flag in [true, false] {
        let py_flag = if flag { "True" } else { "False" };
        let source = source.replace("self.flag = True", &format!("self.flag = {py_flag}"));
        let (mut program, mut image, factory) = native_first(&source, "run");
        let branch_value = if flag { 11 } else { 22 };
        let waiting = program
            .invoke_native_entry_with_result_adapter(&mut image.entries[0], factory, &[], &[], None)
            .expect("branch entry executes");
        assert_eq!(
            waiting,
            Value::List(vec![
                Value::Int(1),
                Value::Int(1),
                Value::None,
                Value::Int(branch_value)
            ])
        );
        let complete = program
            .invoke_native_entry_with_result_adapter(
                &mut image.entries[1],
                factory,
                &[],
                &[Value::Int(branch_value), Value::None],
                None,
            )
            .expect("branch resume executes");
        assert_eq!(
            complete,
            Value::List(vec![Value::Int(0), Value::Int(branch_value)])
        );
    }
}

#[test]
fn distinct_if_else_awaits_join_without_silent_stage_mismatch() {
    let source = r#"
class Move:
    async def run(self, action):
        if action.flag:
            await action.first
        else:
            await action.second
        return 9
class Context:
    def __init__(self):
        self.first = None
        self.second = None
        self.flag = True
def factory():
    return [None, Context()]
"#;
    for (flag, tag) in [(true, 1), (false, 2)] {
        let awaitable = if flag { 101 } else { 202 };
        let py_flag = if flag { "True" } else { "False" };
        let source = source
            .replace("self.first = None", &format!("self.first = {awaitable}"))
            .replace("self.second = None", "self.second = 202")
            .replace("self.flag = True", &format!("self.flag = {py_flag}"));
        let (mut program, mut image, factory) = native_first(&source, "run");
        let waiting = program
            .invoke_native_entry_with_result_adapter(&mut image.entries[0], factory, &[], &[], None)
            .expect("branch await entry executes");
        assert_eq!(
            waiting,
            Value::List(vec![Value::Int(1), Value::Int(tag), Value::Int(awaitable)])
        );
        let complete = program
            .invoke_native_entry_with_result_adapter(
                &mut image.entries[tag as usize],
                factory,
                &[],
                &[Value::None],
                None,
            )
            .expect("branch await join executes");
        assert_eq!(complete, Value::List(vec![Value::Int(0), Value::Int(9)]));
    }
}

#[test]
fn early_return_before_await_is_tagged_or_rejected_explicitly() {
    let source = r#"
class Move:
    async def run(self, action):
        if action.flag:
            return 3
        await action.first
        return 4
class Context:
    def __init__(self):
        self.first = None
        self.flag = True
def factory():
    return [None, Context()]
"#;
    for flag in [true, false] {
        let py_flag = if flag { "True" } else { "False" };
        let source = source.replace("self.flag = True", &format!("self.flag = {py_flag}"));
        let (mut program, mut image, factory) = native_first(&source, "run");
        if flag {
            let result = program
                .invoke_native_entry_with_result_adapter(
                    &mut image.entries[0],
                    factory,
                    &[],
                    &[],
                    None,
                )
                .expect("entry executes");
            assert_eq!(result, Value::List(vec![Value::Int(0), Value::Int(3)]));
        } else {
            let waiting = program
                .invoke_native_entry_with_result_adapter(
                    &mut image.entries[0],
                    factory,
                    &[],
                    &[],
                    None,
                )
                .expect("entry executes");
            assert_eq!(
                waiting,
                Value::List(vec![Value::Int(1), Value::Int(1), Value::None])
            );
            let result = program
                .invoke_native_entry_with_result_adapter(
                    &mut image.entries[1],
                    factory,
                    &[],
                    &[Value::None],
                    None,
                )
                .expect("resume executes");
            assert_eq!(result, Value::List(vec![Value::Int(0), Value::Int(4)]));
        }
    }
}

#[test]
fn object_spill_is_rejected_by_sequential_move_constructor() {
    let source = r#"
class Move:
    async def run(self, action):
        counter = action.payload
        await action.first
        await action.second
        return counter
class Context:
    def __init__(self):
        self.first = None
        self.second = None
        self.payload = [1]
def factory():
    return [None, Context()]
"#;
    let mut executor = SequentialMoveExecutor::new(source, "Move.run", "cfg-review:object-spill")
        .expect("image compiles; object spill is checked at native boundary");
    let mut program = Program::new(source, "sequential-cfg-object-spill.py", ["factory"])
        .prepare_for_thread()
        .expect("native program");
    let factory = program.callback_index("factory").expect("factory callback");
    let error = executor
        .start_scoped(token(1, "first"), |entry, spills| {
            program.invoke_native_entry_with_result_adapter(entry, factory, &[], spills, None)
        })
        .expect_err("heap spill must be rejected explicitly");
    assert!(error.to_string().contains("scalar"), "{error}");
    assert!(executor.checkpoint().is_none());
}

#[test]
fn explicit_non_none_await_result_is_read_after_resume() {
    let source = r#"
class Move:
    async def run(self, action):
        answer = await action.first
        return answer + 1
class Context:
    def __init__(self):
        self.first = None
def factory():
    return [None, Context()]
"#;
    let mut executor = SequentialMoveExecutor::new(source, "Move.run", "cfg-review:await-result")
        .expect("await result image");
    let mut program = Program::new(source, "sequential-cfg-await-result.py", ["factory"])
        .prepare_for_thread()
        .expect("native program");
    let factory = program.callback_index("factory").unwrap();
    let waiting = executor
        .start_scoped(token(1, "first"), |entry, spills| {
            program.invoke_native_entry_with_result_adapter(entry, factory, &[], spills, None)
        })
        .expect("start");
    assert!(matches!(waiting, MoveStep::Waiting { .. }));
    let complete = executor
        .resume_scoped(7, 1, "first", 0, Scalar::Int(41), |entry, args| {
            program.invoke_native_entry_with_result_adapter(entry, factory, &[], args, None)
        })
        .expect("resume")
        .expect("pending move");
    assert!(matches!(complete, MoveStep::Complete(Value::Int(42))));
}

#[test]
fn first_await_result_remains_live_across_second_suspension() {
    let source = r#"
class Move:
    async def run(self, action):
        first = await action.first
        await action.second
        return first
class Context:
    def __init__(self):
        self.first = None
        self.second = None
def factory():
    return [None, Context()]
"#;
    let mut executor = SequentialMoveExecutor::new(source, "Move.run", "cfg-review:two-results")
        .expect("two-await image");
    let mut program = Program::new(source, "sequential-cfg-two-results.py", ["factory"])
        .prepare_for_thread()
        .expect("native program");
    let factory = program.callback_index("factory").unwrap();
    executor
        .start_scoped(token(1, "first"), |entry, spills| {
            program.invoke_native_entry_with_result_adapter(entry, factory, &[], spills, None)
        })
        .expect("start");
    let waiting = executor
        .resume_scoped_with_token(
            7,
            1,
            "first",
            0,
            token(2, "second"),
            Scalar::Int(10),
            |entry, args| {
                program.invoke_native_entry_with_result_adapter(entry, factory, &[], args, None)
            },
        )
        .expect("first resume")
        .expect("second wait");
    let second_pending = match waiting {
        MoveStep::Waiting { pending, .. } => pending,
        MoveStep::Complete(_) => panic!("first resume completed unexpectedly"),
    };
    executor
        .restore_pending(Some(second_pending.clone()))
        .expect("restore second checkpoint");
    assert_eq!(executor.checkpoint(), Some(&second_pending));
    let complete = executor
        .resume_scoped(7, 2, "second", 0, Scalar::Int(32), |entry, args| {
            program.invoke_native_entry_with_result_adapter(entry, factory, &[], args, None)
        })
        .expect("second resume")
        .expect("pending move");
    assert!(matches!(complete, MoveStep::Complete(Value::Int(10))));
}

#[test]
fn pre_first_local_and_first_completion_cross_distinct_suspensions() {
    let source = r#"
class Move:
    async def run(self, action):
        prefix = action.prefix
        first = await action.first
        await action.second
        return [prefix, first]
class Context:
    def __init__(self):
        self.first = None
        self.second = None
        self.prefix = 17
def factory():
    return [None, Context()]
"#;
    let mut executor = SequentialMoveExecutor::new(source, "Move.run", "cfg-review:mixed-liveness")
        .expect("mixed two-await image");
    let mut program = Program::new(source, "sequential-cfg-mixed-liveness.py", ["factory"])
        .prepare_for_thread()
        .expect("native program");
    let factory = program.callback_index("factory").unwrap();
    let waiting = executor
        .start_scoped(token(1, "first"), |entry, spills| {
            program.invoke_native_entry_with_result_adapter(entry, factory, &[], spills, None)
        })
        .expect("start");
    assert!(matches!(waiting, MoveStep::Waiting { .. }));

    let waiting = executor
        .resume_scoped_with_token(
            7,
            1,
            "first",
            0,
            token(2, "second"),
            Scalar::Int(10),
            |entry, args| {
                program.invoke_native_entry_with_result_adapter(entry, factory, &[], args, None)
            },
        )
        .expect("first resume")
        .expect("second wait");
    let second_pending = match waiting {
        MoveStep::Waiting { pending, .. } => pending,
        MoveStep::Complete(_) => panic!("first resume completed unexpectedly"),
    };
    executor
        .restore_pending(Some(second_pending))
        .expect("restore second checkpoint");

    let complete = executor
        .resume_scoped(7, 2, "second", 0, Scalar::Int(32), |entry, args| {
            program.invoke_native_entry_with_result_adapter(entry, factory, &[], args, None)
        })
        .expect("second resume")
        .expect("pending move");
    assert!(matches!(
        complete,
        MoveStep::Complete(Value::List(values))
            if values == vec![Value::Int(17), Value::Int(10)]
    ));
}

#[test]
fn unrelated_generator_activity_cannot_replace_explicit_completion_value() {
    let source = r#"
def unrelated():
    yield 999
    return 12345
def delegated():
    result = yield from unrelated()
    return result
def driver():
    iterator = delegated()
    next(iterator)
    try:
        next(iterator)
    except StopIteration as error:
        return error.value
class Move:
    async def run(self, action):
        answer = await action.first
        return answer + 1
class Context:
    def __init__(self):
        self.first = None
def factory():
    return [None, Context()]
"#;
    let mut executor =
        SequentialMoveExecutor::new(source, "Move.run", "cfg-review:foreign-generator")
            .expect("generator isolation image");
    let mut program = Program::new(
        source,
        "sequential-cfg-foreign-generator.py",
        ["factory", "driver"],
    )
    .prepare_for_thread()
    .expect("native program");
    let factory = program.callback_index("factory").unwrap();
    executor
        .start_scoped(token(1, "first"), |entry, spills| {
            program.invoke_native_entry_with_result_adapter(entry, factory, &[], spills, None)
        })
        .expect("start");
    assert_eq!(
        program.invoke("driver", &[]).expect("exhaust generator"),
        Value::Int(12345)
    );
    let complete = executor
        .resume_scoped(7, 1, "first", 0, Scalar::Int(41), |entry, args| {
            program.invoke_native_entry_with_result_adapter(entry, factory, &[], args, None)
        })
        .expect("resume")
        .expect("pending move");
    assert!(matches!(complete, MoveStep::Complete(Value::Int(42))));
}
