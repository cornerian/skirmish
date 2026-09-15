use std::{
    sync::{Arc, Barrier, Mutex, mpsc},
    thread,
    time::Duration,
};

use skirmish_pon_runtime::{Program, Value};

// Pon's importer and JIT state are process-global. Keep the two integration
// tests from queueing behind each other while retaining the worker-level
// concurrency each test is intended to exercise.
static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn independent_programs_can_prepare_and_invoke_concurrently() {
    let _test_lock = TEST_LOCK.lock().expect("test lock not poisoned");
    let barrier = Arc::new(Barrier::new(2));
    let (sender, receiver) = mpsc::channel();
    let mut handles = Vec::new();
    for offset in [10, 20] {
        let barrier = Arc::clone(&barrier);
        let sender = sender.clone();
        handles.push(thread::spawn(move || {
            barrier.wait();
            let result = (|| {
                let mut program = Program::new(
                    format!("offset = {offset}\ndef action():\n    return offset\n"),
                    format!("concurrent-{offset}.py"),
                    ["action"],
                )
                .prepare_for_thread()?;
                program.invoke("action", &[])
            })();
            sender.send(result).expect("receiver remains alive");
        }));
    }
    drop(sender);

    let mut results = Vec::new();
    for _ in 0..2 {
        results.push(
            receiver
                .recv_timeout(Duration::from_secs(30))
                .expect("concurrent runtime operation completed"),
        );
    }
    for handle in handles {
        handle.join().expect("worker completed");
    }
    results.sort_by_key(|result| match result {
        Ok(Value::Int(value)) => *value,
        _ => i64::MIN,
    });
    let values = results
        .into_iter()
        .map(|result| result.expect("callback succeeded"))
        .collect::<Vec<_>>();
    assert_eq!(values, [Value::Int(10), Value::Int(20)]);
}

#[test]
fn full_sdk_bundle_loads_concurrently_without_cross_talk() {
    let _test_lock = TEST_LOCK.lock().expect("test lock not poisoned");
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/api");
    let barrier = Arc::new(Barrier::new(2));
    let (sender, receiver) = mpsc::channel();
    let mut handles = Vec::new();
    for value in [31, 47] {
        let source_root = source_root.clone();
        let barrier = Arc::clone(&barrier);
        let sender = sender.clone();
        handles.push(thread::spawn(move || {
            let result = (|| {
                let cache = tempfile::tempdir().map_err(|error| error.to_string())?;
                let bundle = skirmish_pon_runtime::SourceBundle::from_directory(
                    "concurrent-sdk-v1",
                    source_root,
                )
                .map_err(|error| error.to_string())?
                .with_file("state.py", format!("values = [{value}]\n"))
                .map_err(|error| error.to_string())?
                .materialize(cache.path())
                .map_err(|error| error.to_string())?;
                let source = "from fighter.api import Fighter\ndef probe(value):\n    import state\n    import gc\n    gc.collect()\n    state.values[0] = state.values[0] + 1\n    return [state.values[0], value, value + 1, value + 2]\n"
                    .to_string();
                let mut program = Program::new(
                    source,
                    "concurrent-sdk.py",
                    ["probe"],
                )
                .prepare_for_thread_in_bundle(&bundle)
                .map_err(|error| error.to_string())?;
                barrier.wait();
                program
                    .invoke("probe", &[Value::Int(value)])
                    .map_err(|error| error.to_string())
            })();
            sender.send(result).expect("receiver remains alive");
        }));
    }
    drop(sender);
    let mut results = (0..2)
        .map(|_| {
            receiver
                .recv_timeout(Duration::from_secs(180))
                .expect("SDK operation completed")
                .expect("SDK callback succeeded")
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().expect("worker completed");
    }
    results.sort_by_key(|value| match value {
        Value::List(values) => match values.first() {
            Some(Value::Int(value)) => *value,
            _ => i64::MIN,
        },
        _ => i64::MIN,
    });
    assert_eq!(
        results,
        [
            Value::List(vec![
                Value::Int(32),
                Value::Int(31),
                Value::Int(32),
                Value::Int(33),
            ]),
            Value::List(vec![
                Value::Int(48),
                Value::Int(47),
                Value::Int(48),
                Value::Int(49),
            ]),
        ]
    );
}
