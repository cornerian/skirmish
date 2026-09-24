//! Native source-compatible math exposed to embedded fighter scripts.

use skirmish_compat_math::{kinematics, trig};
use skirmish_pon_runtime::{Value, register_native_value_module};
use std::sync::OnceLock;

fn number(value: &Value, name: &str) -> Result<f32, String> {
    match value {
        Value::Int(value) => Ok(*value as f32),
        Value::F32(value) => Ok(*value),
        _ => Err(format!("fighter.math.{name} expects a number")),
    }
}

fn unary(args: &[Value], name: &str, operation: fn(f32) -> f32) -> Result<Value, String> {
    let [value] = args else {
        return Err(format!("fighter.math.{name} expects one argument"));
    };
    Ok(Value::F32(operation(number(value, name)?)))
}

pub fn sin(args: &[Value]) -> Result<Value, String> {
    unary(args, "sin", trig::sinf)
}

pub fn cos(args: &[Value]) -> Result<Value, String> {
    unary(args, "cos", trig::cosf)
}

pub fn atan2(args: &[Value]) -> Result<Value, String> {
    let [y, x] = args else {
        return Err("fighter.math.atan2 expects two arguments".into());
    };
    Ok(Value::F32(trig::atan2f(
        number(y, "atan2")?,
        number(x, "atan2")?,
    )))
}

pub fn angle_xy(args: &[Value]) -> Result<Value, String> {
    let [first, second] = args else {
        return Err("fighter.math.angle_xy expects two vectors".into());
    };
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
    Ok(Value::F32(kinematics::angle_xy(first, second)))
}

pub fn facing(args: &[Value]) -> Result<Value, String> {
    let [value] = args else {
        return Err("fighter.math.facing expects one argument".into());
    };
    // The source predicate is `value < 0 ? -1 : 1`.  Keep the comparison in
    // this direction so NaN follows the source's false branch (+1), while
    // both positive and negative zero remain facing right.
    Ok(Value::F32(if number(value, "facing")? < 0.0 {
        -1.0
    } else {
        1.0
    }))
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

    fn f32_value(value: Result<Value, String>) -> f32 {
        match value.expect("native math callback failed") {
            Value::F32(value) => value,
            value => panic!("expected f32, got {value:?}"),
        }
    }

    #[test]
    fn callbacks_preserve_shared_compatibility_bits() {
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
        assert_eq!(
            f32_value(atan2(&[Value::F32(1.0), Value::F32(-1.0)])).to_bits(),
            trig::atan2f(1.0, -1.0).to_bits()
        );
        assert_eq!(
            f32_value(angle_xy(&[
                Value::List(vec![Value::F32(1.0), Value::F32(0.0), Value::F32(0.0)]),
                Value::List(vec![Value::F32(0.0), Value::F32(1.0)]),
            ]))
            .to_bits(),
            kinematics::angle_xy([1.0, 0.0, 0.0], [0.0, 1.0]).to_bits()
        );
        assert_eq!(
            f32_value(facing(&[Value::F32(-0.0)])).to_bits(),
            1.0_f32.to_bits()
        );
        assert_eq!(
            f32_value(facing(&[Value::F32(0.0)])).to_bits(),
            1.0_f32.to_bits()
        );
        assert_eq!(
            f32_value(facing(&[Value::F32(f32::NAN)])).to_bits(),
            1.0_f32.to_bits()
        );
    }
}
