#![cfg(feature = "experimental-continuations")]

use skirmish_pon_runtime::Value;
use skirmish_pon_runtime::continuation::{AwaitToken, Scalar, ScalarLocal, lower_continuation};

#[test]
fn executes_native_continuation_from_copied_state() {
    let source = r#"async def move(wait):
    charge = 3
    await wait
    return charge + 2
"#;
    let plan = lower_continuation(source, "move", "test:move").expect("one await is supported");
    assert_eq!(plan.await_point.resume_tag, 1);
    assert_eq!(plan.function_name, "move");
    let mut state = plan
        .initial(
            "test:move",
            AwaitToken {
                owner: 7,
                generation: 2,
                event_kind: "hit".to_owned(),
                deadline_frame: 30,
            },
        )
        .unwrap();
    plan.validate(&state).unwrap();

    let mut image = plan
        .compile_native(source)
        .expect("pinned JIT emits native image");
    let pre = image
        .call_values(&[Value::None])
        .expect("pre-await native step");
    let Value::List(values) = pre else {
        panic!("pre-await result must be a copied [awaitable, live-local] list");
    };
    assert_eq!(values, vec![Value::None, Value::Int(3)]);
    let saved_charge = values[1].clone();
    let Value::Int(saved_charge_int) = saved_charge else {
        panic!("copied live local must be an integer");
    };

    // The checkpoint contains only Rust-owned scalar state and survives a
    // serde round trip without retaining a Pon heap object or native image.
    let slot = plan
        .locals
        .first()
        .expect("charge is a serializable local")
        .0;
    state.locals = vec![ScalarLocal {
        slot,
        value: Scalar::Int(saved_charge_int),
    }];
    let encoded = serde_json::to_vec(&state).expect("serialize copied state");
    let restored = serde_json::from_slice(&encoded).expect("deserialize copied state");
    plan.validate(&restored).expect("restored state validates");
    let restored_charge = match restored.locals.as_slice() {
        [
            ScalarLocal {
                value: Scalar::Int(value),
                ..
            },
        ] => *value,
        _ => panic!("restored checkpoint must contain one integer live local"),
    };

    // Rebuilding from source proves resume does not depend on the dropped
    // pre-await image or on a saved Pon frame.
    drop(image);
    let mut rebuilt = plan.compile_native(source).expect("rebuild native image");
    assert_eq!(
        rebuilt
            .call_post_with_args(&[Value::None], Some(Value::Int(restored_charge)))
            .expect("resume with saved charge"),
        Value::Int(5)
    );
    assert_eq!(
        rebuilt
            .call_post_with_args(&[Value::None], Some(Value::Int(7)))
            .expect("resume with altered copied charge"),
        Value::Int(9)
    );
}

#[test]
fn rejects_multiple_scalar_locals_for_single_slot_resume_abi() {
    let source = r#"async def move(wait):
    charge = 3
    bonus = 4
    await wait
    return charge + bonus
"#;
    let error = lower_continuation(source, "move", "test:two-locals").unwrap_err();
    assert!(
        error.to_string().contains("scalar") && error.to_string().contains("local"),
        "unexpected rejection: {error}"
    );
}

#[test]
fn executes_native_continuation_without_live_locals() {
    let source = r#"async def move(wait):
    await wait
    return 11
"#;
    let plan = lower_continuation(source, "move", "test:no-local").unwrap();
    assert!(plan.locals.is_empty());
    let mut state = plan
        .initial(
            "test:no-local",
            AwaitToken {
                owner: 3,
                generation: 4,
                event_kind: "deadline".into(),
                deadline_frame: 12,
            },
        )
        .unwrap();
    let mut image = plan.compile_native(source).unwrap();
    let pre = image.call_values(&[Value::None]).unwrap();
    assert_eq!(pre, Value::List(vec![Value::None]));
    let encoded = serde_json::to_vec(&state).unwrap();
    state = serde_json::from_slice(&encoded).unwrap();
    plan.validate(&state).unwrap();
    drop(image);
    let mut rebuilt = plan.compile_native(source).unwrap();
    assert_eq!(
        rebuilt.call_post_with_args(&[Value::None], None).unwrap(),
        Value::Int(11)
    );
}

#[test]
fn rejects_multiple_await_boundaries() {
    let source = "async def move(action):\n    await action\n    await action\n";
    let error = lower_continuation(source, "move", "fox:bad").unwrap_err();
    assert!(error.to_string().contains("exactly one await"));
}

#[test]
fn checkpoint_identity_and_tag_are_validated() {
    let source = "async def move(action):\n    charge = 3\n    await action\n    return charge\n";
    let plan = lower_continuation(source, "move", "fox:test").unwrap();
    let mut state = plan
        .initial(
            "fox:test",
            AwaitToken {
                owner: 1,
                generation: 1,
                event_kind: "cue".to_owned(),
                deadline_frame: 9,
            },
        )
        .unwrap();
    state.source_identity[0] ^= 1;
    assert!(plan.validate(&state).is_err());
}

#[test]
fn event_matching_rejects_stale_generation_but_accepts_hit_before_deadline() {
    let source = "async def move(action):\n    await action\n    return 1\n";
    let plan = lower_continuation(source, "move", "fox:test").unwrap();
    let state = plan
        .initial(
            "fox:test",
            AwaitToken {
                owner: 2,
                generation: 8,
                event_kind: "hit_confirmed".into(),
                deadline_frame: 30,
            },
        )
        .unwrap();
    assert!(
        plan.event_matches(&state, 2, 8, "hit_confirmed", 12)
            .unwrap()
    );
    assert!(
        !plan
            .event_matches(&state, 2, 9, "hit_confirmed", 12)
            .unwrap()
    );
    assert!(
        !plan
            .event_matches(&state, 3, 8, "hit_confirmed", 12)
            .unwrap()
    );

    let deadline_plan = lower_continuation(source, "move", "fox:deadline").unwrap();
    let deadline_state = deadline_plan
        .initial(
            "fox:deadline",
            AwaitToken {
                owner: 2,
                generation: 8,
                event_kind: "scheduled_deadline".into(),
                deadline_frame: 30,
            },
        )
        .unwrap();
    assert!(
        !deadline_plan
            .event_matches(&deadline_state, 2, 8, "scheduled_deadline", 29)
            .unwrap()
    );
    assert!(
        deadline_plan
            .event_matches(&deadline_state, 2, 8, "scheduled_deadline", 30)
            .unwrap()
    );
}

#[test]
fn qualified_class_method_selects_declaring_move() {
    let source = "class First:\n    async def run(self, action):\n        await action\n        return 1\nclass Second:\n    async def run(self, action):\n        await action\n        return 2\n";
    let plan = lower_continuation(source, "Second.run", "test:second").unwrap();
    let mut image = plan.compile_native(source).unwrap();
    assert_eq!(
        image
            .call_post_with_args(&[Value::None, Value::None], None)
            .unwrap(),
        Value::Int(2)
    );
    let error = lower_continuation(source, "run", "test:ambiguous").unwrap_err();
    assert!(error.to_string().contains("ambiguous"));
}
