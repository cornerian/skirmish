//! Small native-runtime probes for Python attribute hooks.
//!
//! These tests intentionally use only source features and the public `Program`
//! facade.  They pin which hooks Pon invokes for ordinary access, `getattr`,
//! assignment, and bound methods before those semantics are used by the game
//! host proxy.

use std::sync::Mutex;

use skirmish_pon_runtime::{Program, Value};

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn run(source: &str, callback: &str) -> Value {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut program = Program::new(source, "attribute_hooks.py", [callback])
        .prepare_for_thread()
        .expect("attribute probe must compile");
    program
        .invoke(callback, &[])
        .expect("attribute probe must run")
}

fn run_result(source: &str, callback: &str) -> Result<Value, skirmish_pon_runtime::Error> {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut program = Program::new(source, "attribute_hooks.py", [callback])
        .prepare_for_thread()
        .expect("attribute probe must compile");
    program.invoke(callback, &[])
}

#[test]
fn custom_getattr_is_used_for_missing_direct_access_and_getattr() {
    let source = r#"
class Probe:
    def __getattr__(self, name):
        return "fallback:" + name
probe = Probe()
def check():
    return [probe.missing, getattr(probe, "other")]
"#;

    assert_eq!(
        run(source, "check"),
        Value::List(vec![
            Value::String("fallback:missing".into()),
            Value::String("fallback:other".into()),
        ])
    );
}

#[test]
fn custom_getattribute_is_not_invoked_by_pinned_pon() {
    let source = r#"
class Probe:
    present = "class-value"
    def __getattribute__(self, name):
        if name == "present":
            return "intercepted"
        if name == "missing":
            return "synthetic"
        return object.__getattribute__(self, name)
probe = Probe()
def check():
    return [probe.present, getattr(probe, "present")]
"#;

    assert_eq!(
        run(source, "check"),
        Value::List(vec![
            Value::String("class-value".into()),
            Value::String("class-value".into()),
        ])
    );
}

#[test]
fn custom_getattribute_cannot_supply_a_missing_attribute() {
    let source = r#"
class Probe:
    def __getattribute__(self, name):
        if name == "missing":
            return "synthetic"
        return object.__getattribute__(self, name)
probe = Probe()
def check():
    return probe.missing
"#;

    let error = run_result(source, "check").expect_err("missing lookup should fail");
    assert!(error.to_string().contains("AttributeError"), "{error}");
}

#[test]
fn property_getter_and_setter_round_trip() {
    let source = r#"
class Probe:
    def __init__(self):
        self._value = 1
    @property
    def value(self):
        return self._value + 1
    @value.setter
    def value(self, new_value):
        self._value = new_value
probe = Probe()
def check():
    before = probe.value
    probe.value = 40
    return [before, probe.value, probe._value]
"#;

    assert_eq!(
        run(source, "check"),
        Value::List(vec![Value::Int(2), Value::Int(41), Value::Int(40)])
    );
}

#[test]
fn custom_setattr_is_not_invoked_by_pinned_pon() {
    let source = r#"
class Probe:
    def __init__(self):
        self.events = []
    def __setattr__(self, name, value):
        if name == "value":
            self.events.append(name)
            self.__dict__["stored"] = value
        else:
            object.__setattr__(self, name, value)
probe = Probe()
def check():
    probe.value = 7
    return probe.value
"#;

    assert_eq!(run(source, "check"), Value::Int(7));
}

#[test]
fn bound_method_reads_dynamic_attribute() {
    let source = r#"
class Probe:
    def __getattr__(self, name):
        if name == "amount":
            return 41
        return object.__getattribute__(self, name)
    def read(self):
        return self.amount + 1
probe = Probe()
read = probe.read
def check():
    return read()
"#;

    assert_eq!(run(source, "check"), Value::Int(42));
}

#[test]
fn callback_tuple_and_generator_argument_expansion_match() {
    let source = r#"
def callback(first, second):
    return first + second
args = (20, 22)
def check():
    tuple_result = callback(*args)
    generator_result = callback(*(value for value in args))
    return [tuple_result, generator_result]
"#;

    assert_eq!(
        run(source, "check"),
        Value::List(vec![Value::Int(42), Value::Int(42)])
    );
}

#[test]
fn decorator_metadata_and_inherited_method_are_preserved() {
    let source = r#"
def mark(function):
    function.marker = "marked"
    return function
class Move:
    __slots__ = ()
    @mark
    def execute(self):
        return 41
class Child(Move):
    pass
move = Child()
def check():
    return [move.execute(), getattr(move.execute, "marker", None)]
"#;

    assert_eq!(
        run(source, "check"),
        Value::List(vec![Value::Int(41), Value::String("marked".into()),])
    );
}

#[test]
fn bound_method_retained_in_dict_survives_repeated_calls() {
    let source = r#"
class Probe:
    def __init__(self):
        self.value = 41
    def read(self):
        return self.value + 1
probe = Probe()
holder = {"callback": probe.read}
def check():
    return [holder["callback"](), holder["callback"]()]
"#;

    assert_eq!(
        run(source, "check"),
        Value::List(vec![Value::Int(42), Value::Int(42)])
    );
}
