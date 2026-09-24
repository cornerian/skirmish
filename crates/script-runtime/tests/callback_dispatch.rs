//! Acceptance coverage for cached facade callback slots and native dispatch.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use skirmish_pon_runtime::{callback_phase_profile, reset_callback_phase_profile};
use skirmish_script_runtime::{
    CompiledProgram, Error, HostRef, NativeHost, NativeKind, NativeObject, NativeValue, shared_host,
};

const SOURCE: &str = r#"
from skirmish import (
    AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves, GrabMoves,
    GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves, TauntMoves,
    ThrowMoves, TiltMoves, hook, register,
)

class Empty(Move):
    def run(self, value):
        return value

empty = Empty()
saved = None

class State:
    counter: int = 0

@register
class CounterFighter(Fighter):
    name = "counter"
    attributes = Attributes
    state = State
    specials = SpecialMoves(empty, empty, empty, empty)
    aerials = AerialMoves(empty, empty, empty, empty, empty)
    grounded = GroundedMoves(empty, empty, empty)
    tilts = TiltMoves(empty, empty, empty)
    smashes = SmashMoves(empty, empty, empty)
    grabs = GrabMoves(empty, empty, empty)
    throws = ThrowMoves(empty, empty, empty, empty)
    defense = DefenseMoves(empty, empty, empty, empty, empty)
    ledge = LedgeMoves(empty, empty, empty, empty, empty)
    getup = GetupMoves(empty, empty, empty, empty)
    taunt = TauntMoves(empty)

    @hook.input_pressed("B")
    def pressed(self, fighter):
        fighter.state.counter = fighter.state.counter + 1
        return fighter.state.counter

    @hook.input_pressed("B")
    def second(self, fighter):
        fighter.state.counter = fighter.state.counter + 1
        return fighter.state.counter

    @hook.input_pressed("B")
    def save_proxy(self, fighter):
        global saved
        saved = fighter.state

    @hook.input_pressed("B")
    def use_old_proxy(self, fighter):
        return saved.counter

    @hook.input_pressed("B")
    def echo_extra(self, fighter, value):
        return value
"#;

struct CounterHost {
    counter: Arc<Mutex<i64>>,
}

#[test]
fn facade_batch_dispatch_reuses_runtime_scope_but_keeps_callback_state_ordered() {
    let program = CompiledProgram::new(SOURCE, "callback_batch.py", []).unwrap();
    let first = program.bind_callback("pressed");
    let second = program.bind_callback("second");
    let counter = Arc::new(Mutex::new(0));
    let host = shared_host(CounterHost {
        counter: Arc::clone(&counter),
    });
    let fighter = HostRef::new(Arc::clone(&host), NativeKind::Fighter, "fighter");

    program
        .with_invocation_scope(|scope| {
            assert_eq!(
                program.dispatch_in_scope(scope, &first, fighter.clone(), &[])?,
                NativeValue::Int(1)
            );
            assert_eq!(
                program.dispatch_in_scope(scope, &second, fighter.clone(), &[])?,
                NativeValue::Int(2)
            );
            Ok(())
        })
        .unwrap();
    assert_eq!(*counter.lock().unwrap(), 2);
}

#[test]
fn facade_batch_dispatch_reuses_argument_capacity_without_changing_extra_values() {
    let program = CompiledProgram::new(SOURCE, "callback_args.py", []).unwrap();
    let callback = program.bind_callback("echo_extra");
    let host = shared_host(CounterHost {
        counter: Arc::new(Mutex::new(0)),
    });
    let fighter = HostRef::new(host, NativeKind::Fighter, "fighter");

    program
        .with_invocation_scope(|scope| {
            for value in [7, 11, 19] {
                assert_eq!(
                    program.dispatch_in_scope(
                        scope,
                        &callback,
                        fighter.clone(),
                        &[NativeValue::Int(value)],
                    )?,
                    NativeValue::Int(value)
                );
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn facade_rejects_a_scope_owned_by_another_program() {
    let first = CompiledProgram::new(SOURCE, "scope_owner_a.py", []).unwrap();
    let second = CompiledProgram::new(SOURCE, "scope_owner_b.py", []).unwrap();
    let callback = second.bind_callback("pressed");
    let counter = Arc::new(Mutex::new(0));
    let host = shared_host(CounterHost { counter });
    let fighter = HostRef::new(host, NativeKind::Fighter, "fighter");

    first
        .with_invocation_scope(|scope| {
            let error = second
                .dispatch_in_scope(scope, &callback, fighter, &[])
                .unwrap_err();
            assert!(error.to_string().contains("invocation scope belongs"));
            Ok(())
        })
        .unwrap();
}

#[test]
fn facade_rejects_a_proxy_retained_from_the_previous_callback_scope() {
    let program = CompiledProgram::new(SOURCE, "scope_expiry.py", []).unwrap();
    let save = program.bind_callback("save_proxy");
    let stale = program.bind_callback("use_old_proxy");
    let counter = Arc::new(Mutex::new(0));
    let host = shared_host(CounterHost {
        counter: Arc::clone(&counter),
    });
    let fighter = HostRef::new(host, NativeKind::Fighter, "fighter");

    let error = program
        .with_invocation_scope(|scope| {
            program.dispatch_in_scope(scope, &save, fighter.clone(), &[])?;
            program.dispatch_in_scope(scope, &stale, fighter.clone(), &[])
        })
        .unwrap_err();
    assert!(error.to_string().contains("host") || error.to_string().contains("token"));
    assert_eq!(*counter.lock().unwrap(), 0);
}

#[test]
fn facade_scope_preserves_callback_error_variant_and_message() {
    let program = CompiledProgram::new(SOURCE, "scope_error.py", []).unwrap();
    let error = program
        .with_invocation_scope(|_scope| {
            Err::<(), Error>(Error::Invalid("callback ABI sentinel".into()))
        })
        .unwrap_err();
    assert_eq!(error, Error::Invalid("callback ABI sentinel".into()));
}

#[test]
fn facade_scope_keeps_runtime_first_error_over_later_outer_error() {
    let program = CompiledProgram::new(SOURCE, "scope_first_error.py", []).unwrap();
    let save = program.bind_callback("save_proxy");
    let stale = program.bind_callback("use_old_proxy");
    let host = shared_host(CounterHost {
        counter: Arc::new(Mutex::new(0)),
    });
    let fighter = HostRef::new(host, NativeKind::Fighter, "fighter");

    let error = program
        .with_invocation_scope(|scope| {
            program.dispatch_in_scope(scope, &save, fighter.clone(), &[])?;
            let _first = program
                .dispatch_in_scope(scope, &stale, fighter.clone(), &[])
                .unwrap_err();
            Err::<(), Error>(Error::Invalid("later outer error".into()))
        })
        .unwrap_err();
    assert!(!matches!(error, Error::Invalid(ref message) if message == "later outer error"));
    assert!(error.to_string().contains("host") || error.to_string().contains("token"));
}

#[test]
#[ignore = "manual scope amortization profile"]
fn facade_single_vs_multi_callback_scope_profile() {
    let program = CompiledProgram::new(SOURCE, "scope_profile.py", []).unwrap();
    let first = program.bind_callback("pressed");
    let second = program.bind_callback("second");
    let host = shared_host(CounterHost {
        counter: Arc::new(Mutex::new(0)),
    });
    let fighter = HostRef::new(host, NativeKind::Fighter, "fighter");
    let mut single = 0u128;
    reset_callback_phase_profile();
    for _ in 0..64 {
        let started = Instant::now();
        program.dispatch(&first, fighter.clone(), &[]).unwrap();
        single += started.elapsed().as_nanos();
    }
    let single_phases = callback_phase_profile();
    let mut multi = 0u128;
    reset_callback_phase_profile();
    for _ in 0..64 {
        let started = Instant::now();
        program
            .with_invocation_scope(|scope| {
                program.dispatch_in_scope(scope, &first, fighter.clone(), &[])?;
                program.dispatch_in_scope(scope, &second, fighter.clone(), &[])?;
                Ok(())
            })
            .unwrap();
        multi += started.elapsed().as_nanos();
    }
    println!(
        "facade scope profile: one={}ns two_in_scope={}ns",
        single / 64,
        multi / 64
    );
    for (phase, calls, nanos) in single_phases {
        println!("scope profile one: {phase} calls={calls} total={nanos}ns");
    }
    for (phase, calls, nanos) in callback_phase_profile() {
        println!("scope profile two: {phase} calls={calls} total={nanos}ns");
    }
}

impl NativeHost for CounterHost {
    fn get(&mut self, path: &str) -> Result<NativeValue, Error> {
        match path {
            "fighter.state" => Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::State,
                path: "fighter.state".into(),
            })),
            "fighter.state.counter" => Ok(NativeValue::Int(*self.counter.lock().unwrap())),
            other => Err(Error::Host(format!("unexpected get path {other}"))),
        }
    }

    fn set(&mut self, path: &str, value: NativeValue) -> Result<(), Error> {
        match (path, value) {
            ("fighter.state.counter", NativeValue::Int(value)) => {
                *self.counter.lock().unwrap() = value;
                Ok(())
            }
            (other, value) => Err(Error::Host(format!("unexpected set {other}: {value:?}"))),
        }
    }

    fn call(&mut self, path: &str, _args: &[NativeValue]) -> Result<NativeValue, Error> {
        Err(Error::Host(format!("unexpected call path {path}")))
    }
}

#[test]
fn facade_dispatch_uses_bound_hook_slot_and_native_host_state() {
    let program = CompiledProgram::new(SOURCE, "callback_dispatch.py", []).unwrap();
    let callback = program.bind_callback("pressed");
    let metadata = program.export_metadata().unwrap();
    assert!(matches!(metadata, NativeValue::Dict(_)));

    let error = program.invoke_values(&callback, &[]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("logical host callback; use dispatch"),
        "logical callback handles must not enter the raw export table: {error}"
    );

    let counter = Arc::new(Mutex::new(0));
    let host = shared_host(CounterHost {
        counter: Arc::clone(&counter),
    });
    let fighter = HostRef::new(Arc::clone(&host), NativeKind::Fighter, "fighter");

    assert_eq!(
        program.dispatch(&callback, fighter.clone(), &[]).unwrap(),
        NativeValue::Int(1)
    );
    assert_eq!(
        *counter.lock().unwrap(),
        1,
        "one dispatch must produce one native state effect"
    );

    let other = CompiledProgram::new(SOURCE, "other_callback_dispatch.py", []).unwrap();
    let foreign = other.bind_callback("pressed");
    let error = program.dispatch(&foreign, fighter, &[]).unwrap_err();
    assert!(
        error.to_string().contains("another Pon program"),
        "foreign callback handles must be rejected: {error}"
    );
    assert_eq!(*counter.lock().unwrap(), 1);
}
