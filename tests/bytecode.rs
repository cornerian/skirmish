use proptest::prelude::*;
use skirmish::{
    bytecode::{Error, evaluate},
    random::HsdRng,
};

fn literal(word: u32) -> Vec<u8> {
    let mut code = vec![6];
    code.extend(word.to_be_bytes());
    code
}

fn expression(words: &[u32], opcode: u8) -> Vec<u8> {
    let mut code: Vec<_> = words.iter().flat_map(|&word| literal(word)).collect();
    code.extend([opcode, 1]);
    code
}

fn eval(code: &[u8]) -> u32 {
    evaluate(Some(code), &[], &mut HsdRng::new(1))
        .unwrap()
        .to_bits()
}

fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= expected.abs().max(1.0) * 2e-6,
        "actual {actual:?}, expected {expected:?}"
    );
}

#[test]
fn stack_operands_and_control_flow() {
    assert_eq!(
        evaluate(None, &[], &mut HsdRng::new(1)).unwrap().to_bits(),
        0
    );
    assert_eq!(eval(&expression(&[0x0123_4567], 0)), 0x0123_4567);
    let mut args = vec![0.0; 257];
    args[256] = -12.5;
    assert_eq!(
        evaluate(Some(&[2, 1, 0, 1]), &args, &mut HsdRng::new(1)),
        Ok(-12.5)
    );
    for (depth, expected) in [(0, 20), (1, 10)] {
        let mut code = literal(10);
        code.extend(literal(20));
        code.extend([0x3c, depth, 1]);
        assert_eq!(eval(&code), expected);
    }
    let mut code = literal(10);
    code.extend(literal(20));
    code.extend([5, 1, 1]);
    assert_eq!(eval(&code), 10);
    assert_eq!(eval(&[5, 255, 6, 0, 0, 0, 30, 1]), 30);
    // Conditional branches inspect bits: even the float -0.0 is true.
    for (condition, expected) in [(0, 11), (1, 22), (0x8000_0000, 22)] {
        let mut code = literal(condition);
        code.extend([3, 0, 8]);
        code.extend(literal(11));
        code.extend([4, 0, 5]);
        code.extend(literal(22));
        code.push(1);
        assert_eq!(eval(&code), expected);
    }
}

#[test]
fn all_unary_operations() {
    for (opcode, input, expected) in [
        (7, (-7.75_f32).to_bits(), (-7_i32) as u32),
        (8, (-123_i32) as u32, (-123_f32).to_bits()),
        (9, 12.5_f32.to_bits(), (-12.5_f32).to_bits()),
        (0x0a, 7, (-7_i32) as u32),
        (0x15, (-12.5_f32).to_bits(), 12.5_f32.to_bits()),
        (0x28, (-123_i32) as u32, 123),
        (0x31, 0, 1),
        (0x31, 2, 0),
    ] {
        assert_eq!(
            eval(&expression(&[input], opcode)),
            expected,
            "opcode {opcode:#x}"
        );
    }
    for (opcode, input, expected) in [
        (0x0d, 30.0_f32, 0.5),
        (0x0e, 60.0, 0.5),
        (0x0f, 45.0, 1.0),
        (0x10, 0.5, 30.0),
        (0x11, 0.5, 60.0),
        (0x12, 1.0, 45.0),
        (0x13, 1.0, 0.0),
        (0x14, 0.0, 1.0),
        (0x16, 144.0, 12.0),
    ] {
        close(
            f32::from_bits(eval(&expression(&[input.to_bits()], opcode))),
            expected,
        );
    }
    for opcode in [0x0b, 0x0c] {
        let mut rng = HsdRng::new(0x1234_5678);
        let expected = if opcode == 0x0b {
            rng.randi(2) as u32
        } else {
            rng.randf().to_bits()
        };
        let mut actual_rng = HsdRng::new(0x1234_5678);
        let actual = evaluate(Some(&expression(&[0], opcode)), &[], &mut actual_rng).unwrap();
        assert_eq!(actual.to_bits(), expected);
        assert_eq!(actual_rng.seed(), rng.seed());
    }
}

#[test]
fn binary_operations_preserve_order_and_signedness() {
    for (opcode, expected) in [
        (0x17, 8.0_f32),
        (0x18, 4.0),
        (0x19, 12.0),
        (0x1a, 3.0),
        (0x1b, 0.0),
        (0x21, 36.0),
        (0x22, 2.0),
        (0x23, 6.0),
        (0x26, 71.56505),
    ] {
        close(
            f32::from_bits(eval(&expression(
                &[6_f32.to_bits(), 2_f32.to_bits()],
                opcode,
            ))),
            expected,
        );
    }
    for (opcode, expected) in [
        (0x1c, -5),
        (0x1d, -13),
        (0x1e, -36),
        (0x1f, -2),
        (0x20, -1),
        (0x24, -9),
        (0x25, 4),
        (0x29, 1),
        (0x2a, 0),
        (0x2b, 1),
        (0x2c, 0),
        (0x2d, 0),
        (0x2e, 1),
        (0x2f, 1),
        (0x30, 1),
        (0x32, 0),
        (0x39, 4),
        (0x3a, -9),
        (0x3b, -13),
    ] {
        assert_eq!(
            eval(&expression(&[(-9_i32) as u32, 4], opcode)) as i32,
            expected,
            "opcode {opcode:#x}"
        );
    }
    for (opcode, expected) in [
        (0x33, 0),
        (0x34, 1),
        (0x35, 0),
        (0x36, 1),
        (0x37, 0),
        (0x38, 1),
    ] {
        assert_eq!(
            eval(&expression(&[6_f32.to_bits(), 2_f32.to_bits()], opcode)),
            expected
        );
    }
    for (a, b) in [(0, 0), (0, 7), (7, 0), (7, 3)] {
        for (opcode, expected) in [
            (0x2f, a != 0 && b != 0),
            (0x30, a != 0 || b != 0),
            (0x32, (a == 0) != (b == 0)),
        ] {
            assert_eq!(eval(&expression(&[a, b], opcode)), u32::from(expected));
        }
    }
    let mut rng = HsdRng::new(1);
    assert_eq!(
        eval(&expression(&[10, 20], 0x27)),
        (10 + rng.randi(11)) as u32
    );
}

#[test]
fn signed_zeros_nans_and_wrapping_integer_edges() {
    let nan = 0x7fc0_1234;
    for bits in [0_u32, 0x8000_0000, nan] {
        assert_eq!(eval(&expression(&[bits], 0x15)), bits);
        assert_eq!(eval(&expression(&[bits], 9)), bits ^ 0x8000_0000);
    }
    for opcode in [0x22, 0x23] {
        for (left, right) in [(0, 0x8000_0000), (0x8000_0000, 0), (nan, 0), (0, nan)] {
            assert_eq!(eval(&expression(&[left, right], opcode)), left);
        }
    }
    for a in [0.0_f32, -0.0, 1.0, -1.0, f32::NAN] {
        for b in [0.0_f32, -0.0] {
            assert_eq!(
                eval(&expression(&[a.to_bits(), b.to_bits()], 0x26)),
                (if a >= 0.0 { 90.0_f32 } else { -90.0_f32 }).to_bits()
            );
        }
    }
    for opcode in 0x33..=0x38 {
        assert_eq!(
            eval(&expression(&[nan, 0], opcode)),
            u32::from(opcode == 0x38)
        );
    }
    assert_eq!(
        eval(&expression(&[i32::MAX as u32, 1], 0x1c)),
        i32::MIN as u32
    );
    assert_eq!(
        eval(&expression(&[i32::MIN as u32, 1], 0x1d)),
        i32::MAX as u32
    );
    assert_eq!(eval(&expression(&[u32::MAX, 2], 0x1e)), u32::MAX - 1);
    for opcode in [0x0a, 0x28] {
        assert_eq!(
            eval(&expression(&[i32::MIN as u32], opcode)),
            i32::MIN as u32
        );
    }
}

#[test]
fn malformed_programs_are_errors() {
    let run = |code: &[u8]| evaluate(Some(code), &[], &mut HsdRng::new(1));
    assert_eq!(run(&[]), Err(Error::MissingReturn));
    assert_eq!(run(&[0]), Err(Error::MissingReturn));
    for code in [
        &[2, 0][..],
        &[3],
        &[4, 0],
        &[5],
        &[6, 0, 0, 0],
        &[0x3c],
        &[0xff],
    ] {
        assert_eq!(run(code), Err(Error::TruncatedOperand(0)));
    }
    for opcode in (7..=0x3b).chain([1, 3, 0x3c]) {
        let code = [opcode, 0, 0];
        assert!(
            matches!(run(&code), Err(Error::StackUnderflow { .. })),
            "opcode {opcode:#x}"
        );
    }
    assert_eq!(
        run(&[2, 0, 1]),
        Err(Error::InvalidArgument { pc: 0, index: 1 })
    );
    assert_eq!(
        run(&[4, 0, 1]),
        Err(Error::InvalidJump { pc: 0, target: 4 })
    );
    assert_eq!(run(&[4, 0, 0]), Err(Error::MissingReturn));
    assert_eq!(run(&[0xff, 0]), Err(Error::Unimplemented(0)));
    assert_eq!(
        run(&[0x3d]),
        Err(Error::InvalidOpcode {
            pc: 0,
            opcode: 0x3d
        })
    );
    for f in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 2147483648.0] {
        assert_eq!(
            run(&expression(&[f.to_bits()], 7)),
            Err(Error::InvalidFloatCast(5))
        );
    }
    assert_eq!(
        eval(&expression(&[(-2147483648.0_f32).to_bits()], 7)),
        i32::MIN as u32
    );
    for opcode in [0x1f, 0x20] {
        for words in [[123, 0], [i32::MIN as u32, u32::MAX]] {
            assert_eq!(
                run(&expression(&words, opcode)),
                Err(Error::InvalidDivision(10))
            );
        }
    }
}

proptest! {
    #[test]
    fn arbitrary_bytes_never_panic(code in prop::collection::vec(any::<u8>(), 0..256)) {
        let _ = evaluate(Some(&code), &[0.0, 1.0, -1.0], &mut HsdRng::new(1));
    }

    #[test]
    fn all_literal_words_round_trip(word: u32) {
        prop_assert_eq!(eval(&expression(&[word], 0)), word);
    }
}

#[cfg(feature = "c-oracle")]
#[allow(unsafe_code)]
mod oracle {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());
    unsafe extern "C" {
        fn HSD_ByteCodeEval(code: *const u8, args: *const f32, count: i32) -> f32;
        fn oracle_hsd_set_seed(seed: u32);
        fn oracle_hsd_get_seed() -> u32;
    }

    fn compare(code: &[u8], args: &[f32], seed: u32, approximate: bool) {
        let _guard = LOCK.lock().unwrap();
        let mut rng = HsdRng::new(seed);
        // Rust rejects undefined inputs before they can reach the C oracle.
        let actual = evaluate(Some(code), args, &mut rng).unwrap();
        // SAFETY: callers supply valid terminating programs and in-bounds args;
        // the mutex serializes the oracle's global random seed.
        let (expected, expected_seed) = unsafe {
            oracle_hsd_set_seed(seed);
            let result = HSD_ByteCodeEval(code.as_ptr(), args.as_ptr(), args.len() as i32);
            (result, oracle_hsd_get_seed())
        };
        assert_eq!(rng.seed(), expected_seed);
        if approximate && actual.is_finite() && expected.is_finite() {
            close(actual, expected);
        } else if approximate && actual.is_nan() && expected.is_nan() {
            // Arithmetic NaN payloads/signs are not certified across libm ports.
        } else {
            assert_eq!(actual.to_bits(), expected.to_bits(), "code {code:02x?}");
        }
    }

    #[test]
    fn every_opcode_agrees_with_upstream_host_c() {
        for opcode in 7..=0x3b {
            let unary = matches!(opcode, 7..=0x16 | 0x28 | 0x31);
            let integer =
                matches!(opcode, 8 | 0x0a | 0x1c..=0x20 | 0x24..=0x25 | 0x27..=0x32 | 0x39..=0x3b);
            let words = if integer {
                [3, 7]
            } else {
                [0.25_f32.to_bits(), 0.5_f32.to_bits()]
            };
            compare(
                &expression(&words[..if unary { 1 } else { 2 }], opcode),
                &[],
                1234,
                matches!(opcode, 0x0d..=0x14 | 0x16 | 0x21 | 0x26),
            );
        }
        compare(
            &[0, 2, 0, 0, 2, 0, 1, 0x3c, 1, 5, 1, 0x17, 1],
            &[3.5, 8.5],
            1,
            false,
        );
        compare(&[5, 255, 6, 1, 2, 3, 4, 1], &[], 1, false);
        for condition in [0_u32, 1, 0x8000_0000] {
            let mut code = literal(condition);
            code.extend([3, 0, 8]);
            code.extend(literal(11));
            code.extend([4, 0, 5]);
            code.extend(literal(22));
            code.push(1);
            compare(&code, &[], 1, false);
        }
    }

    #[test]
    fn edge_values_agree_with_upstream_host_c() {
        let values = [
            0_u32,
            0x8000_0000,
            1_f32.to_bits(),
            (-1_f32).to_bits(),
            0x7fc0_1234,
        ];
        for left in values {
            for right in values {
                for opcode in [0x22, 0x23, 0x26, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38] {
                    compare(&expression(&[left, right], opcode), &[], 17, opcode == 0x26);
                }
            }
            for opcode in [9, 0x15] {
                compare(&expression(&[left], opcode), &[], 17, false);
            }
        }
    }

    proptest! {
        #[test]
        fn integer_and_logic_operations_match_c(a: i32, b: i32, seed: u32) {
            for opcode in [0x1c, 0x1d, 0x1e, 0x24, 0x25, 0x27, 0x29, 0x2a,
                0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30, 0x32, 0x39, 0x3a, 0x3b] {
                compare(&expression(&[a as u32, b as u32], opcode), &[], seed, false);
            }
            if b != 0 && (a != i32::MIN || b != -1) {
                for opcode in [0x1f, 0x20] {
                    compare(&expression(&[a as u32, b as u32], opcode), &[], seed, false);
                }
            }
        }

        #[test]
        fn finite_float_operations_match_c(a in -1000_f32..1000_f32, b in 1_f32..1000_f32) {
            for opcode in [0x17, 0x18, 0x19, 0x1a, 0x1b, 0x22, 0x23, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38] {
                compare(&expression(&[a.to_bits(), b.to_bits()], opcode), &[], 1, false);
            }
            for opcode in [7, 9, 0x15] {
                compare(&expression(&[a.to_bits()], opcode), &[], 1, false);
            }
        }
    }
}
