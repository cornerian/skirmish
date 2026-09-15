#![cfg(feature = "experimental-continuations")]

use skirmish_pon_runtime::continuation::{AwaitToken, Scalar};
use skirmish_pon_runtime::sequential_move::{MoveStep, SequentialMoveExecutor};
use skirmish_pon_runtime::{Error, Program, Value};

const SOURCE: &str = r#"
class Context:
    def __init__(self):
        self.first = None
        self.second = None

class Move:
    async def run(self, action):
        counter = 1
        await action.first
        counter = counter + 1
        await action.second
        return counter + 1

def factory():
    return [None, Context()]
"#;

fn token(generation: u64, event: &str) -> AwaitToken {
    AwaitToken {
        owner: 7,
        generation,
        event_kind: event.into(),
        deadline_frame: 0,
    }
}

fn invoke<'a>(
    program: &'a mut skirmish_pon_runtime::PreparedProgram,
    slot: usize,
    calls: &'a mut usize,
) -> impl FnOnce(&mut skirmish_pon_runtime::continuation::NativeEntry, &[Value]) -> Result<Value, Error>
+ 'a {
    move |entry, spills| {
        *calls += 1;
        program.invoke_native_entry_with_result_adapter(entry, slot, &[], spills, None)
    }
}

#[test]
fn executes_two_awaits_with_native_entries_and_copyable_checkpoints() {
    let mut executor = SequentialMoveExecutor::new(SOURCE, "Move.run", "runtime:test").unwrap();
    let mut program = Program::new(SOURCE, "sequential-runtime.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let factory = program.callback_index("factory").unwrap();
    let mut calls = 0;

    let first = executor
        .start_scoped(token(1, "first"), invoke(&mut program, factory, &mut calls))
        .unwrap();
    let first_pending = match first {
        MoveStep::Waiting { pending, awaitable } => {
            assert_eq!(awaitable, Value::None);
            assert_eq!(pending.continuation.locals.len(), 1);
            pending
        }
        MoveStep::Complete(_) => panic!("first segment completed"),
    };
    assert_eq!(calls, 1);

    let second = executor
        .resume_scoped_with_token(
            7,
            1,
            "first",
            0,
            token(2, "second"),
            Scalar::None,
            invoke(&mut program, factory, &mut calls),
        )
        .unwrap()
        .unwrap();
    let second_pending = match second {
        MoveStep::Waiting { pending, .. } => {
            assert_eq!(pending.continuation.locals.len(), 1);
            assert_eq!(pending.continuation.await_token, token(2, "second"));
            pending
        }
        MoveStep::Complete(_) => panic!("second segment completed"),
    };
    assert_eq!(calls, 2);

    let copied = second_pending.clone();
    executor.restore_pending(Some(copied.clone())).unwrap();
    assert_eq!(executor.checkpoint(), Some(&copied));

    let complete = executor
        .resume_scoped(
            7,
            2,
            "second",
            0,
            Scalar::None,
            invoke(&mut program, factory, &mut calls),
        )
        .unwrap()
        .unwrap();
    assert!(matches!(complete, MoveStep::Complete(Value::Int(3))));
    assert_eq!(calls, 3);

    // The original first checkpoint was copied before any resume effects.
    assert_eq!(first_pending.continuation.locals.len(), 1);
    assert_eq!(
        first_pending.continuation.locals[0].value,
        skirmish_pon_runtime::continuation::Scalar::Int(1)
    );
}

#[test]
fn rejects_bad_restore_and_ignores_stale_or_cancelled_events() {
    let mut executor = SequentialMoveExecutor::new(SOURCE, "Move.run", "runtime:invalid").unwrap();
    let mut program = Program::new(SOURCE, "sequential-runtime-invalid.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let factory = program.callback_index("factory").unwrap();
    let mut calls = 0;
    executor
        .start_scoped(token(1, "first"), invoke(&mut program, factory, &mut calls))
        .unwrap();
    let original = executor.checkpoint().unwrap().clone();

    let mut wrong_identity = original.clone();
    wrong_identity.continuation.source_identity = [0; 32];
    assert!(executor.restore_pending(Some(wrong_identity)).is_err());
    let mut wrong_tag = original.clone();
    wrong_tag.continuation.resume_tag = 99;
    assert!(executor.restore_pending(Some(wrong_tag)).is_err());
    let mut wrong_slot = original.clone();
    wrong_slot.continuation.locals[0].slot = 999;
    assert!(executor.restore_pending(Some(wrong_slot)).is_err());
    assert_eq!(executor.checkpoint(), Some(&original));

    assert!(
        executor
            .resume_scoped(
                7,
                9,
                "first",
                0,
                Scalar::None,
                invoke(&mut program, factory, &mut calls)
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(calls, 1);
    assert!(executor.cancel());
    assert!(
        executor
            .resume_scoped(
                7,
                1,
                "first",
                0,
                Scalar::None,
                invoke(&mut program, factory, &mut calls)
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(calls, 1);
}

#[test]
fn native_error_preserves_pending_and_invalid_next_token_has_no_effect() {
    let mut executor = SequentialMoveExecutor::new(SOURCE, "Move.run", "runtime:error").unwrap();
    let mut program = Program::new(SOURCE, "sequential-runtime-error.py", ["factory"])
        .prepare_for_thread()
        .unwrap();
    let factory = program.callback_index("factory").unwrap();
    let mut calls = 0;
    executor
        .start_scoped(token(1, "first"), invoke(&mut program, factory, &mut calls))
        .unwrap();
    let before = executor.checkpoint().unwrap().clone();
    let bad_token = AwaitToken {
        owner: 7,
        generation: 2,
        event_kind: String::new(),
        deadline_frame: 0,
    };
    assert!(
        executor
            .resume_scoped_with_token(
                7,
                1,
                "first",
                0,
                bad_token,
                Scalar::None,
                |_entry, _args| panic!("native invoked"),
            )
            .is_err()
    );
    assert_eq!(executor.checkpoint(), Some(&before));
    assert!(
        executor
            .resume_scoped(7, 1, "first", 0, Scalar::None, |_entry, _args| Err(
                Error::Runtime("bridge failed".into())
            ))
            .is_err()
    );
    assert_eq!(executor.checkpoint(), Some(&before));
}
