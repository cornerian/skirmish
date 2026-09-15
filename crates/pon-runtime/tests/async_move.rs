#![cfg(feature = "experimental-continuations")]

use skirmish_pon_runtime::Value;
use skirmish_pon_runtime::async_move::{AsyncMoveExecutor, MoveStep};
use skirmish_pon_runtime::continuation::{AwaitToken, lower_continuation};

#[test]
fn class_move_run_executes_before_and_after_once_from_checkpoint() {
    let source = r#"
class Move:
    async def run(self, action):
        await action
        return 2
"#;
    let plan = lower_continuation(source, "run", "test:move.run").unwrap();
    let mut executor = AsyncMoveExecutor::new(plan, source).unwrap();
    let waiting = executor
        .start(
            &[Value::None, Value::None],
            AwaitToken {
                owner: 4,
                generation: 9,
                event_kind: "scheduled_deadline".into(),
                deadline_frame: 20,
            },
        )
        .unwrap();
    assert!(matches!(waiting, MoveStep::Waiting { .. }));
    let checkpoint = serde_json::to_vec(executor.checkpoint().unwrap()).unwrap();
    drop(executor);
    let pending: skirmish_pon_runtime::async_move::PendingMove =
        serde_json::from_slice(&checkpoint).unwrap();
    let mut executor = AsyncMoveExecutor::restore(
        lower_continuation(source, "run", "test:move.run").unwrap(),
        source,
        pending,
    )
    .unwrap();
    assert!(
        executor
            .resume_with_args(4, 9, "scheduled_deadline", 19, &[Value::None, Value::None])
            .unwrap()
            .is_none()
    );
    let resumed = executor
        .resume_with_args(4, 9, "scheduled_deadline", 20, &[Value::None, Value::None])
        .unwrap();
    assert!(matches!(resumed, Some(MoveStep::Complete(Value::Int(2)))));
    assert!(executor.checkpoint().is_none());
    assert!(
        executor
            .resume_with_args(4, 9, "scheduled_deadline", 20, &[Value::None, Value::None])
            .unwrap()
            .is_none()
    );
}

#[test]
fn interruption_invalidates_pending_move_and_late_event_is_ignored() {
    let source =
        "class Move:\n    async def run(self, action):\n        await action\n        return 2\n";
    let plan = lower_continuation(source, "run", "test:interrupt").unwrap();
    let mut executor = AsyncMoveExecutor::new(plan, source).unwrap();
    executor
        .start(
            &[Value::None, Value::None],
            AwaitToken {
                owner: 4,
                generation: 9,
                event_kind: "scheduled_deadline".into(),
                deadline_frame: 20,
            },
        )
        .unwrap();
    assert!(executor.cancel().unwrap());
    assert!(
        executor
            .resume_with_args(4, 9, "scheduled_deadline", 20, &[Value::None, Value::None])
            .unwrap()
            .is_none()
    );
}

#[test]
fn failed_post_step_keeps_checkpoint_for_transaction_rollback() {
    let source = "class Move:\n    async def run(self, action):\n        await action\n        return 1 / 0\n";
    let plan = lower_continuation(source, "run", "test:failed-post").unwrap();
    let mut executor = AsyncMoveExecutor::new(plan, source).unwrap();
    executor
        .start(
            &[Value::None, Value::None],
            AwaitToken {
                owner: 4,
                generation: 9,
                event_kind: "scheduled_deadline".into(),
                deadline_frame: 20,
            },
        )
        .unwrap();
    assert!(
        executor
            .resume_with_args(4, 9, "scheduled_deadline", 20, &[Value::None, Value::None])
            .is_err()
    );
    assert!(executor.checkpoint().is_some());
}

#[test]
fn scoped_resume_validates_before_invoking_and_restores_without_preexecution() {
    let source = "async def run(action):\n    await action\n    return 3\n";
    let plan = lower_continuation(source, "run", "test:scoped").unwrap();
    let mut executor = AsyncMoveExecutor::new(plan.clone(), source).unwrap();
    let waiting = executor
        .start_scoped(
            AwaitToken {
                owner: 1,
                generation: 2,
                event_kind: "scheduled_deadline".into(),
                deadline_frame: 8,
            },
            |_, _, _| Ok(Value::List(vec![Value::None])),
        )
        .unwrap();
    let pending = match waiting {
        MoveStep::Waiting { pending, .. } => pending,
        MoveStep::Complete(_) => panic!("pre step completed unexpectedly"),
    };
    let mut called = false;
    assert!(
        executor
            .resume_scoped(1, 99, "scheduled_deadline", 8, |_, _, _| {
                called = true;
                Ok(Value::Int(0))
            })
            .unwrap()
            .is_none()
    );
    assert!(!called);
    executor.restore_pending(None).unwrap();
    assert!(executor.checkpoint().is_none());
    executor.restore_pending(Some(pending)).unwrap();
    assert!(executor.checkpoint().is_some());
}
