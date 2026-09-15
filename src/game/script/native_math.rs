//! Stateless Pon ABI bridge for the game's source-compatible fighter math.

use crate::compat::math::kinematics;
use crate::compat::math::trig;
use skirmish_script_runtime::{Value, register_native_value_module};
use std::sync::OnceLock;

fn number(value: &Value, name: &str) -> Result<f32, String> {
    match value {
        Value::Int(value) => Ok(*value as f32),
        Value::F32(value) => Ok(*value),
        _ => Err(format!("fighter.math.{name} expects a number")),
    }
}

fn unary(args: &[Value], name: &str, operation: fn(f32) -> f32) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("fighter.math.{name} expects one argument"));
    }
    (|| {
        let value = args.first().expect("arity checked");
        let value = number(value, name)?;
        Ok(Value::F32(operation(value)))
    })()
}

pub fn sin(args: &[Value]) -> Result<Value, String> {
    unary(args, "sin", trig::sinf)
}

pub fn cos(args: &[Value]) -> Result<Value, String> {
    unary(args, "cos", trig::cosf)
}

pub fn atan2(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("fighter.math.atan2 expects two arguments".into());
    }
    Ok(Value::F32(trig::atan2f(
        number(&args[0], "atan2")?,
        number(&args[1], "atan2")?,
    )))
}

pub fn angle_xy(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("fighter.math.angle_xy expects two vectors".into());
    }
    (|| {
        let first = args.first().expect("arity checked");
        let second = &args[1];
        let Value::List(first) = first else {
            return Err("fighter.math.angle_xy expects a 3-element first vector".into());
        };
        let Value::List(second) = second else {
            return Err("fighter.math.angle_xy expects a 2-element second vector".into());
        };
        if first.len() != 3 || second.len() != 2 {
            return Err("fighter.math.angle_xy expects 3- and 2-element vectors".into());
        }
        let first = [
            number(&first[0], "angle_xy")?,
            number(&first[1], "angle_xy")?,
            number(&first[2], "angle_xy")?,
        ];
        let second = [
            number(&second[0], "angle_xy")?,
            number(&second[1], "angle_xy")?,
        ];
        let result = kinematics::angle_xy(first, second);
        Ok::<_, String>(Value::F32(result))
    })()
}

pub fn facing(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("fighter.math.facing expects one argument".into());
    }
    let value = number(&args[0], "facing")?;
    Ok(Value::F32(if value >= 0.0 { 1.0 } else { -1.0 }))
}

pub fn register() -> Result<(), String> {
    static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
    REGISTERED
        .get_or_init(|| {
            register_native_value_module(
                "_skirmish_math",
                [
                    ("sin", sin, 1),
                    ("cos", cos, 1),
                    ("atan2", atan2, 2),
                    ("angle_xy", angle_xy, 2),
                    ("facing", facing, 1),
                ],
            )
            .map_err(|error| error.to_string())
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use skirmish_pon_runtime::{Program, SourceBundle, StandardLibrary};
    use std::sync::Mutex;

    static PON_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn verified_stdlib() -> skirmish_pon_runtime::MaterializedStandardLibrary {
        let archive_path = std::env::var("SKIRMISH_PON_STDLIB_ARCHIVE")
            .expect("SKIRMISH_PON_STDLIB_ARCHIVE must identify the verified stdlib archive");
        let digest = std::env::var("SKIRMISH_PON_STDLIB_SHA256")
            .expect("SKIRMISH_PON_STDLIB_SHA256 must identify the verified stdlib archive");
        assert_eq!(digest.len(), 64, "stdlib SHA-256 must be 64 hex characters");
        let expected: [u8; 32] = (0..32)
            .map(|index| {
                u8::from_str_radix(&digest[index * 2..index * 2 + 2], 16)
                    .expect("stdlib SHA-256 must contain only hexadecimal characters")
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("stdlib SHA-256 must contain 32 bytes");
        let archive =
            std::fs::read(archive_path).expect("verified stdlib archive must be readable");
        let root = tempfile::tempdir().expect("stdlib materialization root");
        StandardLibrary::from_archive(&archive[..], expected)
            .expect("verified stdlib archive must pass its SHA-256")
            .materialize(root.keep())
            .expect("verified stdlib archive must materialize")
    }

    fn f32_value(value: Result<Value, String>) -> f32 {
        match value.expect("native math callback failed") {
            Value::F32(value) => value,
            value => panic!("expected f32, got {value:?}"),
        }
    }

    #[test]
    fn trig_callbacks_preserve_compatibility_bits() {
        for input in [0.0_f32, 0.25, 1.0, -2.5, std::f32::consts::PI] {
            assert_eq!(
                f32_value(sin(&[Value::F32(input)])).to_bits(),
                trig::sinf(input).to_bits()
            );
            assert_eq!(
                f32_value(cos(&[Value::F32(input)])).to_bits(),
                trig::cosf(input).to_bits()
            );
        }
    }

    #[test]
    fn vector_and_direction_callbacks_preserve_compatibility_bits() {
        let first = [1.0, 0.0, 0.0];
        let second = [0.0, 1.0];
        let actual = f32_value(angle_xy(&[
            Value::List(first.iter().copied().map(Value::F32).collect()),
            Value::List(second.iter().copied().map(Value::F32).collect()),
        ]));
        assert_eq!(
            actual.to_bits(),
            kinematics::angle_xy(first, second).to_bits()
        );
        assert_eq!(
            f32_value(atan2(&[Value::F32(1.0), Value::F32(-1.0)])).to_bits(),
            trig::atan2f(1.0, -1.0).to_bits()
        );
        assert_eq!(
            f32_value(facing(&[Value::F32(-0.0)])).to_bits(),
            1.0_f32.to_bits()
        );
        assert_eq!(
            f32_value(facing(&[Value::F32(f32::NAN)])).to_bits(),
            (-1.0_f32).to_bits()
        );
    }

    #[test]
    fn python_wrappers_cross_the_native_pon_abi() {
        let _guard = PON_TEST_LOCK.lock().unwrap();
        register().expect("native math module registration failed");
        let stdlib = verified_stdlib();
        let bundle = SourceBundle::new("native-math-test-v1")
            .with_file(
                "fighter/actions.py",
                include_str!("../../../scripts/api/fighter/actions.py"),
            )
            .unwrap()
            .with_file(
                "fighter/transitions.py",
                include_str!("../../../scripts/api/fighter/transitions.py"),
            )
            .unwrap()
            .with_file(
                "fighter/__init__.py",
                include_str!("../../../scripts/api/fighter/__init__.py"),
            )
            .unwrap()
            .with_file(
                "fighter/api.py",
                include_str!("../../../scripts/api/fighter/api.py"),
            )
            .unwrap()
            .with_file(
                "fighter/compat.py",
                include_str!("../../../scripts/api/fighter/compat.py"),
            )
            .unwrap()
            .with_file(
                "fighter/events.py",
                include_str!("../../../scripts/api/fighter/events.py"),
            )
            .unwrap()
            .with_file(
                "fighter/registry.py",
                include_str!("../../../scripts/api/fighter/registry.py"),
            )
            .unwrap()
            .with_file(
                "fighter/math.py",
                include_str!("../../../scripts/api/fighter/math.py"),
            )
            .unwrap();
        let source = r#"
from fighter import math
from fighter.compat import f32

def probe():
    angle = f32(0.25)
    return [float(math.sin(angle)), float(math.cos(angle)),
            float(math.atan2(f32(1), f32(-1))),
            float(math.angle_xy([f32(1), f32(0), f32(0)], [f32(0), f32(1)])),
            float(math.facing(f32(-0.0)))]
"#;
        let root = std::env::temp_dir().join(format!("skirmish-pon-math-{}", std::process::id()));
        let materialized = bundle.materialize(&root).unwrap();
        let mut program = Program::new(source, "native_math.py", ["probe"])
            .with_standard_library(&stdlib)
            .prepare_for_thread_in_bundle(&materialized)
            .expect("Python math wrapper failed to compile");
        let Value::List(values) = program
            .invoke("probe", &[])
            .expect("Python math wrapper failed")
        else {
            panic!("probe did not return a list");
        };
        let expected = [
            trig::sinf(0.25),
            trig::cosf(0.25),
            trig::atan2f(1.0, -1.0),
            kinematics::angle_xy([1.0, 0.0, 0.0], [0.0, 1.0]),
            1.0,
        ];
        assert_eq!(values.len(), expected.len());
        for (actual, expected) in values.iter().zip(expected) {
            let Value::F32(actual) = actual else {
                panic!("native result was not f32");
            };
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
