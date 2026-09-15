//! Generic, bounded native helpers shared by lifecycle and resource hosts.
//!
//! These helpers intentionally know only the validation ABI. Resource names,
//! fighter identities, and attack representations remain owned by the host;
//! object and sequence values are resolved through `NativeHost` paths so a
//! validation pass does not clone the cached resource tree.

use super::starlark::value::{Error, NativeHost, NativeObject, NativeValue};
use std::collections::BTreeMap;
const MAX_NUMBER: f32 = 1_000_000.0;
const MAX_ATTACK_FRAMES: i64 = 4_096;
const MAX_COMMAND_VALUE: i64 = 4_294_967_295;
const NUMBER_NAMES: &[&str] = &["nonnegative"];
const FIELD_NAMES: &[&str] = &["finite", "nonnegative", "positive", "nonzero"];

/// Dispatch one generic validation builtin. `None` means the path belongs to
/// another host namespace.
pub(crate) fn call<H: NativeHost + ?Sized>(
    host: &mut H,
    path: &str,
    args: &[NativeValue],
) -> Result<Option<NativeValue>, Error> {
    let value = match path {
        "math.sin" => math_unary(args, crate::compat::math::trig::sinf, "math.sin")?,
        "math.cos" => math_unary(args, crate::compat::math::trig::cosf, "math.cos")?,
        "math.atan2" => math_atan2(args)?,
        "math.angle_xy" => math_angle_xy(args)?,
        "math.facing" => math_facing(args)?,
        "validation.finite" => NativeValue::Bool(finite(args)?),
        "validation.number" => NativeValue::Bool(number(args)?),
        "validation.fields" => NativeValue::Bool(fields(host, args)?),
        "validation.attack" => NativeValue::Bool(attack(host, args)?),
        "validation.command_trace" => NativeValue::Bool(command_trace(host, args)?),
        "validation.hitboxes" => NativeValue::Bool(hitboxes(host, args)?),
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn math_unary(
    args: &[NativeValue],
    function: fn(f32) -> f32,
    name: &str,
) -> Result<NativeValue, Error> {
    let [value] = args else {
        return Err(Error::Host(format!("{name} expects one argument")));
    };
    Ok(NativeValue::F32(function(math_number(value, name)?)))
}

fn math_atan2(args: &[NativeValue]) -> Result<NativeValue, Error> {
    let [y, x] = args else {
        return Err(Error::Host("math.atan2 expects two arguments".into()));
    };
    Ok(NativeValue::F32(crate::compat::math::trig::atan2f(
        math_number(y, "math.atan2 y")?,
        math_number(x, "math.atan2 x")?,
    )))
}

fn math_angle_xy(args: &[NativeValue]) -> Result<NativeValue, Error> {
    let [a, b] = args else {
        return Err(Error::Host("math.angle_xy expects two vectors".into()));
    };
    let a = math_vector::<3>(a, "math.angle_xy first vector")?;
    let b = math_vector::<2>(b, "math.angle_xy second vector")?;
    let result = crate::compat::math::kinematics::angle_xy(a, b);
    Ok(NativeValue::F32(result))
}

fn math_facing(args: &[NativeValue]) -> Result<NativeValue, Error> {
    let [value] = args else {
        return Err(Error::Host("math.facing expects one argument".into()));
    };
    // Keep the source comparison rather than using signum: NaN is unordered
    // and therefore follows the same negative-facing branch as the authoring
    // helper (`value >= 0` is false).
    Ok(NativeValue::F32(
        if math_number(value, "math.facing")? >= 0.0 {
            1.0
        } else {
            -1.0
        },
    ))
}

fn math_number(value: &NativeValue, name: &str) -> Result<f32, Error> {
    match value {
        NativeValue::Int(value) => Ok(*value as f32),
        NativeValue::F32(value) => Ok(*value),
        _ => Err(Error::Host(format!("{name} expects a number"))),
    }
}

fn math_vector<const N: usize>(value: &NativeValue, name: &str) -> Result<[f32; N], Error> {
    let values = match value {
        NativeValue::List(values) if values.len() == N => values,
        NativeValue::Vec2(values) if N == 2 => {
            return Ok(std::array::from_fn(|index| values[index]));
        }
        _ => return Err(Error::Host(format!("{name} expects a {N}-element vector"))),
    };
    values
        .iter()
        .map(|value| math_number(value, name))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| Error::Host(format!("{name} expects a {N}-element vector")))
}

/// Dispatch validation helpers while preserving keyword arguments at the
/// native boundary. The compiler catalog defines these names; this function
/// performs the final whitelist and positional mapping before execution.
pub(crate) fn call_named<H: NativeHost + ?Sized>(
    host: &mut H,
    path: &str,
    args: &[NativeValue],
    named: &BTreeMap<String, NativeValue>,
) -> Result<Option<NativeValue>, Error> {
    if !path.starts_with("validation.") {
        return Ok(None);
    }
    let mut positional = args.to_vec();
    let named_position = match path {
        "validation.number" => NUMBER_NAMES,
        "validation.fields" => FIELD_NAMES,
        "validation.finite"
        | "validation.attack"
        | "validation.command_trace"
        | "validation.hitboxes" => &[],
        _ => return Ok(None),
    };
    for name in named.keys() {
        let Some(index) = named_position
            .iter()
            .position(|candidate| *candidate == name.as_str())
        else {
            return Err(Error::Host(format!("unknown validation argument `{name}`")));
        };
        let index = index + 1;
        while positional.len() <= index {
            positional.push(if path == "validation.fields" {
                NativeValue::List(Vec::new())
            } else {
                NativeValue::None
            });
        }
        positional[index] = named[name].clone();
    }
    call(host, path, &positional)
}

fn finite(args: &[NativeValue]) -> Result<bool, Error> {
    let [value] = args else {
        return Err(Error::Host("validation.finite expects one argument".into()));
    };
    Ok(matches!(value, NativeValue::Int(_))
        || matches!(value, NativeValue::F32(value) if value.is_finite()))
}

fn number(args: &[NativeValue]) -> Result<bool, Error> {
    if !(1..=2).contains(&args.len()) {
        return Err(Error::Host(
            "validation.number expects one or two arguments".into(),
        ));
    }
    let nonnegative = match args.get(1) {
        None => false,
        Some(NativeValue::Bool(value)) => *value,
        Some(_) => {
            return Err(Error::Host(
                "validation.number nonnegative must be a boolean".into(),
            ));
        }
    };
    let Some(value) = numeric(args.first().expect("arity checked")) else {
        return Ok(false);
    };
    Ok(value.is_finite() && value.abs() <= MAX_NUMBER && (!nonnegative || value >= 0.0))
}

fn fields<H: NativeHost + ?Sized>(host: &mut H, args: &[NativeValue]) -> Result<bool, Error> {
    if !(1..=5).contains(&args.len()) {
        return Err(Error::Host(
            "validation.fields expects one to five arguments".into(),
        ));
    }
    let record = &args[0];
    let mut categories: [&[NativeValue]; 4] = [&[], &[], &[], &[]];
    for (index, category) in categories.iter_mut().enumerate() {
        if let Some(value) = args.get(index + 1) {
            let Some(values) = sequence_values(value) else {
                return Ok(false);
            };
            *category = values;
        }
    }
    for (category, mode) in categories.into_iter().zip([
        FieldMode::Finite,
        FieldMode::Nonnegative,
        FieldMode::Positive,
        FieldMode::Nonzero,
    ]) {
        for name in category {
            let Some(name) = string(name) else {
                return Ok(false);
            };
            let Some(value) = field(host, record, name)? else {
                return Ok(false);
            };
            if !check_field(&value, mode) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[derive(Clone, Copy)]
enum FieldMode {
    Finite,
    Nonnegative,
    Positive,
    Nonzero,
}

fn check_field(value: &NativeValue, mode: FieldMode) -> bool {
    let Some(value) = numeric(value) else {
        return false;
    };
    if !value.is_finite() {
        return false;
    }
    match mode {
        FieldMode::Finite => value.abs() <= MAX_NUMBER,
        FieldMode::Nonnegative => value.abs() <= MAX_NUMBER && value >= 0.0,
        FieldMode::Positive => value.abs() <= MAX_NUMBER && value > 0.0,
        FieldMode::Nonzero => value.abs() <= MAX_NUMBER && value != 0.0,
    }
}

fn attack<H: NativeHost + ?Sized>(host: &mut H, args: &[NativeValue]) -> Result<bool, Error> {
    let [context, path] = args else {
        return Err(Error::Host(
            "validation.attack expects context and path".into(),
        ));
    };
    let Some(path) = string(path) else {
        return Ok(false);
    };
    if path.is_empty() {
        return Ok(false);
    }
    let Some(context) = object(context) else {
        return Ok(false);
    };
    let valid = host_bool_call(
        host,
        &format!("{}.validate_attack", context.path),
        &[NativeValue::String(path.into())],
    )?;
    if !valid {
        return Ok(false);
    }
    let frames = host.call(
        &format!("{}.frames", context.path),
        &[NativeValue::String(path.into())],
    )?;
    Ok(matches!(frames, NativeValue::Int(value) if (1..=MAX_ATTACK_FRAMES).contains(&value)))
}

fn command_trace<H: NativeHost + ?Sized>(
    host: &mut H,
    args: &[NativeValue],
) -> Result<bool, Error> {
    let [context, command_path, pose_path] = args else {
        return Err(Error::Host(
            "validation.command_trace expects context and two paths".into(),
        ));
    };
    let (Some(context), Some(command_path), Some(pose_path)) =
        (object(context), string(command_path), string(pose_path))
    else {
        return Ok(false);
    };
    if command_path.is_empty() || pose_path.is_empty() {
        return Ok(false);
    }
    let resource = host.call(
        &format!("{}.resource", context.path),
        &[NativeValue::String(command_path.into())],
    )?;
    if matches!(resource, NativeValue::None) {
        return Ok(false);
    }
    let Some(command_rows) = field(host, &resource, "cmd_vars")? else {
        return Ok(false);
    };
    let Some(interrupt_rows) = field(host, &resource, "allow_interrupt")? else {
        return Ok(false);
    };
    let frames = host.call(
        &format!("{}.frames", context.path),
        &[NativeValue::String(pose_path.into())],
    )?;
    let NativeValue::Int(frame_count) = frames else {
        return Ok(false);
    };
    if !(1..=MAX_ATTACK_FRAMES).contains(&frame_count) {
        return Ok(false);
    }
    let command_count = array_length_call(host, context, command_path)?;
    let interrupt_count =
        array_length_call(host, context, &format!("{command_path}.allow_interrupt"))?;
    if command_count != Some(frame_count as usize) || interrupt_count != Some(frame_count as usize)
    {
        return Ok(false);
    }
    for index in 0..frame_count as usize {
        let Some(row) = sequence_item(host, &command_rows, index)? else {
            return Ok(false);
        };
        if sequence_length(host, &row)? != Some(4) {
            return Ok(false);
        }
        if sequence_values(&row).is_none() {
            let Some(row_object) = object(&row) else {
                return Ok(false);
            };
            for column in 0..4 {
                let value = host.get(&format!("{}[{column}]", row_object.path))?;
                if !valid_command_value(&value) {
                    return Ok(false);
                }
            }
        }
        if let Some(row_values) = sequence_values(&row)
            && row_values.iter().any(|value| !valid_command_value(value))
        {
            return Ok(false);
        }
        let Some(interrupt) = sequence_item(host, &interrupt_rows, index)? else {
            return Ok(false);
        };
        if !matches!(interrupt, NativeValue::Bool(_)) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn hitboxes<H: NativeHost + ?Sized>(host: &mut H, args: &[NativeValue]) -> Result<bool, Error> {
    let [value] = args else {
        return Err(Error::Host(
            "validation.hitboxes expects one argument".into(),
        ));
    };
    let Some(length) = sequence_length(host, value)? else {
        return Ok(false);
    };
    if !(1..=4).contains(&length) {
        return Ok(false);
    }
    for index in 0..length {
        let Some(hitbox) = sequence_item(host, value, index)? else {
            return Ok(false);
        };
        let Some(radius) = field(host, &hitbox, "radius")? else {
            return Ok(false);
        };
        if !check_field(&radius, FieldMode::Nonnegative) {
            return Ok(false);
        }
        let angle = field(host, &hitbox, "angle")?.or(field(host, &hitbox, "angle_degrees")?);
        let Some(angle) = angle else {
            return Ok(false);
        };
        if !is_finite_number(&angle) {
            return Ok(false);
        }
        let Some(center) = field(host, &hitbox, "center")? else {
            return Ok(false);
        };
        if sequence_length(host, &center)? != Some(3) {
            return Ok(false);
        }
        for coordinate in 0..3 {
            let Some(value) = sequence_item(host, &center, coordinate)? else {
                return Ok(false);
            };
            if !is_finite_number(&value) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn numeric(value: &NativeValue) -> Option<f32> {
    match value {
        NativeValue::Int(value) => Some(*value as f32),
        NativeValue::F32(value) => Some(*value),
        _ => None,
    }
}

fn is_finite_number(value: &NativeValue) -> bool {
    matches!(value, NativeValue::Int(_))
        || matches!(value, NativeValue::F32(value) if value.is_finite())
}

fn string(value: &NativeValue) -> Option<&str> {
    match value {
        NativeValue::String(value) => Some(value),
        _ => None,
    }
}

fn object(value: &NativeValue) -> Option<&NativeObject> {
    match value {
        NativeValue::Object(value) => Some(value),
        _ => None,
    }
}

fn sequence_values(value: &NativeValue) -> Option<&[NativeValue]> {
    match value {
        NativeValue::List(values) => Some(values),
        _ => None,
    }
}

fn field<H: NativeHost + ?Sized>(
    host: &mut H,
    value: &NativeValue,
    name: &str,
) -> Result<Option<NativeValue>, Error> {
    if let NativeValue::Dict(values) = value {
        return Ok(values.get(name).cloned());
    }
    let Some(object) = object(value) else {
        return Ok(None);
    };
    match host.get(&format!("{}.{}", object.path, name)) {
        Ok(value) => Ok(Some(value)),
        Err(Error::Host(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn sequence_length<H: NativeHost + ?Sized>(
    host: &mut H,
    value: &NativeValue,
) -> Result<Option<usize>, Error> {
    if let Some(values) = sequence_values(value) {
        return Ok(Some(values.len()));
    }
    let Some(object) = object(value) else {
        return Ok(None);
    };
    match host.get(&format!("{}.length", object.path)) {
        Ok(NativeValue::Int(value)) => Ok(usize::try_from(value).ok()),
        Ok(_) | Err(Error::Host(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn sequence_item<H: NativeHost + ?Sized>(
    host: &mut H,
    value: &NativeValue,
    index: usize,
) -> Result<Option<NativeValue>, Error> {
    if let Some(values) = sequence_values(value) {
        return Ok(values.get(index).cloned());
    }
    let Some(object) = object(value) else {
        return Ok(None);
    };
    match host.get(&format!("{}[{index}]", object.path)) {
        Ok(value) => Ok(Some(value)),
        Err(Error::Host(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn host_bool_call<H: NativeHost + ?Sized>(
    host: &mut H,
    path: &str,
    args: &[NativeValue],
) -> Result<bool, Error> {
    Ok(matches!(host.call(path, args)?, NativeValue::Bool(true)))
}

fn array_length_call<H: NativeHost + ?Sized>(
    host: &mut H,
    context: &NativeObject,
    path: &str,
) -> Result<Option<usize>, Error> {
    match host.call(
        &format!("{}.array_length", context.path),
        &[NativeValue::String(path.into())],
    )? {
        NativeValue::Int(value) => Ok(usize::try_from(value).ok()),
        _ => Ok(None),
    }
}

fn valid_command_value(value: &NativeValue) -> bool {
    match value {
        NativeValue::None => true,
        NativeValue::Int(value) => (0..=MAX_COMMAND_VALUE).contains(value),
        // JSON-backed resources expose numbers as f32. Accept an exactly
        // integral representation here, while retaining the command ABI's
        // unsigned 32-bit bounds. The round-trip check rejects f32 values
        // which rounded an out-of-range integer (e.g. 4_294_967_295).
        NativeValue::F32(value) if value.is_finite() && value.fract() == 0.0 && *value >= 0.0 => {
            let integer = *value as i64;
            (0..=MAX_COMMAND_VALUE).contains(&integer) && integer as f32 == *value
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_trace_accepts_integral_json_numbers_within_u32_bounds() {
        assert!(valid_command_value(&NativeValue::None));
        assert!(valid_command_value(&NativeValue::Int(1)));
        assert!(valid_command_value(&NativeValue::F32(1.0)));
        assert!(valid_command_value(&NativeValue::F32(65_536.0)));
        assert!(!valid_command_value(&NativeValue::F32(1.5)));
        assert!(!valid_command_value(&NativeValue::F32(-1.0)));
        assert!(!valid_command_value(&NativeValue::F32(f32::INFINITY)));
    }

    struct TestHost;

    impl NativeHost for TestHost {
        fn get(&mut self, path: &str) -> Result<NativeValue, Error> {
            Err(Error::Host(format!("unexpected get {path}")))
        }

        fn set(&mut self, path: &str, _value: NativeValue) -> Result<(), Error> {
            Err(Error::Host(format!("unexpected set {path}")))
        }

        fn call(&mut self, path: &str, _args: &[NativeValue]) -> Result<NativeValue, Error> {
            Err(Error::Host(format!("unexpected call {path}")))
        }
    }

    fn bool_result(value: Option<NativeValue>) -> bool {
        matches!(value, Some(NativeValue::Bool(true)))
    }

    #[test]
    fn number_preserves_optional_flag_and_bounds() {
        let mut host = TestHost;
        assert!(bool_result(
            call(&mut host, "validation.number", &[NativeValue::Int(4)]).unwrap()
        ));
        assert!(!bool_result(
            call(
                &mut host,
                "validation.number",
                &[NativeValue::Int(-4), NativeValue::Bool(true)],
            )
            .unwrap()
        ));
        assert!(!bool_result(
            call(
                &mut host,
                "validation.number",
                &[NativeValue::F32(1_000_001.0)],
            )
            .unwrap()
        ));
    }

    #[test]
    fn math_builtins_use_the_game_f32_implementations() {
        let mut host = TestHost;
        let NativeValue::F32(sine) = call(
            &mut host,
            "math.sin",
            &[NativeValue::F32(std::f32::consts::FRAC_PI_2)],
        )
        .unwrap()
        .unwrap() else {
            panic!("math.sin must return f32");
        };
        assert_eq!(
            sine,
            crate::compat::math::trig::sinf(std::f32::consts::FRAC_PI_2)
        );

        let NativeValue::F32(angle) = call(
            &mut host,
            "math.atan2",
            &[NativeValue::F32(1.0), NativeValue::F32(1.0)],
        )
        .unwrap()
        .unwrap() else {
            panic!("math.atan2 must return f32");
        };
        assert_eq!(angle, crate::compat::math::trig::atan2f(1.0, 1.0));

        let NativeValue::F32(xy_angle) = call(
            &mut host,
            "math.angle_xy",
            &[
                NativeValue::List(vec![
                    NativeValue::F32(1.0),
                    NativeValue::F32(0.0),
                    NativeValue::F32(0.0),
                ]),
                NativeValue::List(vec![NativeValue::F32(0.0), NativeValue::F32(1.0)]),
            ],
        )
        .unwrap()
        .unwrap() else {
            panic!("math.angle_xy must return f32");
        };
        assert_eq!(
            xy_angle,
            crate::compat::math::kinematics::angle_xy([1.0, 0.0, 0.0], [0.0, 1.0])
        );
    }

    #[test]
    fn angle_xy_preserves_nan_behavior() {
        let mut host = TestHost;
        let value = call(
            &mut host,
            "math.angle_xy",
            &[
                NativeValue::List(vec![
                    NativeValue::F32(f32::NAN),
                    NativeValue::F32(0.0),
                    NativeValue::F32(0.0),
                ]),
                NativeValue::List(vec![NativeValue::F32(1.0), NativeValue::F32(0.0)]),
            ],
        )
        .unwrap()
        .unwrap();
        assert!(matches!(value, NativeValue::F32(value) if value.is_nan()));
    }

    #[test]
    fn facing_uses_nonnegative_comparison_for_nan_and_signed_zero() {
        let mut host = TestHost;
        assert_eq!(
            call(&mut host, "math.facing", &[NativeValue::F32(2.0)])
                .unwrap()
                .unwrap(),
            NativeValue::F32(1.0)
        );
        assert_eq!(
            call(&mut host, "math.facing", &[NativeValue::F32(-0.0)])
                .unwrap()
                .unwrap(),
            NativeValue::F32(1.0)
        );
        assert_eq!(
            call(&mut host, "math.facing", &[NativeValue::F32(f32::NAN)])
                .unwrap()
                .unwrap(),
            NativeValue::F32(-1.0)
        );
    }

    #[test]
    fn fields_reject_missing_and_malformed_values() {
        let mut host = TestHost;
        let record = NativeValue::Dict(BTreeMap::from([
            ("speed".into(), NativeValue::F32(2.0)),
            (
                "character".into(),
                NativeValue::String("test_fighter".into()),
            ),
        ]));
        let names = NativeValue::List(vec![NativeValue::String("speed".into())]);
        assert!(bool_result(
            call(
                &mut host,
                "validation.fields",
                &[record.clone(), names.clone()],
            )
            .unwrap()
        ));
        let mut named = BTreeMap::new();
        named.insert("nonnegative".into(), names.clone());
        assert!(bool_result(
            call_named(
                &mut host,
                "validation.fields",
                std::slice::from_ref(&record),
                &named,
            )
            .unwrap()
        ));
        let missing = NativeValue::List(vec![NativeValue::String("angle".into())]);
        assert!(!bool_result(
            call(&mut host, "validation.fields", &[record, missing]).unwrap()
        ));
        assert!(!bool_result(
            call(
                &mut host,
                "validation.fields",
                &[
                    NativeValue::Dict(BTreeMap::from([(
                        "speed".into(),
                        NativeValue::String("bad".into()),
                    )])),
                    names
                ],
            )
            .unwrap()
        ));
    }

    #[test]
    fn hitboxes_validate_generic_shape_without_character_fields() {
        let mut host = TestHost;
        let hitbox = NativeValue::Dict(BTreeMap::from([
            ("radius".into(), NativeValue::F32(1.0)),
            ("angle".into(), NativeValue::F32(45.0)),
            (
                "center".into(),
                NativeValue::List(vec![
                    NativeValue::F32(0.0),
                    NativeValue::F32(1.0),
                    NativeValue::F32(0.0),
                ]),
            ),
        ]));
        assert!(bool_result(
            call(
                &mut host,
                "validation.hitboxes",
                &[NativeValue::List(vec![hitbox])],
            )
            .unwrap()
        ));
        let malformed = NativeValue::Dict(BTreeMap::from([
            ("radius".into(), NativeValue::F32(1.0)),
            ("angle".into(), NativeValue::F32(45.0)),
            (
                "center".into(),
                NativeValue::List(vec![NativeValue::F32(0.0), NativeValue::F32(1.0)]),
            ),
        ]));
        assert!(!bool_result(
            call(
                &mut host,
                "validation.hitboxes",
                &[NativeValue::List(vec![malformed])],
            )
            .unwrap()
        ));
    }
}
