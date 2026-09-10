//! HSD expression bytecode, translated from `sysdolphin/baselib/bytecode.c`.
//!
//! Stack entries are raw 32-bit words, operands are big endian, and jumps are
//! relative to the byte after their operand. Float functions use `libm`; their
//! final bits and NaN payloads are not certified against the PowerPC runtime.
//! Integer arithmetic wraps as on PowerPC. Invalid float casts and integer
//! division, which are undefined in the reference C, return explicit errors.

use super::math;
use crate::random::HsdRng;

const DEG_TO_RAD: f64 = 0.017453292519943295;
const RAD_TO_DEG: f64 = 57.29577951308232;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("bytecode ended without returning a value")]
    MissingReturn,
    #[error("truncated operand at byte {0}")]
    TruncatedOperand(usize),
    #[error("opcode {opcode:#04x} at byte {pc} needs {needed} stack values")]
    StackUnderflow {
        pc: usize,
        opcode: u8,
        needed: usize,
    },
    #[error("argument {index} at byte {pc} does not exist")]
    InvalidArgument { pc: usize, index: usize },
    #[error("jump at byte {pc} leaves the bytecode (target {target})")]
    InvalidJump { pc: usize, target: usize },
    #[error("unknown opcode {opcode:#04x} at byte {pc}")]
    InvalidOpcode { pc: usize, opcode: u8 },
    #[error("opcode 0xff at byte {0} is unimplemented in the reference")]
    Unimplemented(usize),
    #[error("float-to-integer conversion at byte {0} is outside i32 range")]
    InvalidFloatCast(usize),
    #[error("integer division at byte {0} is undefined (zero divisor or overflow)")]
    InvalidDivision(usize),
}

/// Evaluate an expression, updating its explicitly owned random generator.
///
/// `None` preserves the reference's null-bytecode result of positive zero.
/// A present but empty slice is malformed. `args` contains raw float values;
/// bytecode can reinterpret their bits as integers. Returning an integer word
/// likewise returns the float with those bits, without a numeric conversion.
pub fn evaluate(bytecode: Option<&[u8]>, args: &[f32], rng: &mut HsdRng) -> Result<f32, Error> {
    let Some(code) = bytecode else { return Ok(0.0) };
    let mut stack = Vec::<u32>::new();
    let mut cursor = 0;
    while let Some(&opcode) = code.get(cursor) {
        let pc = cursor;
        cursor += 1;
        let operand_bytes = match opcode {
            5 | 0x3c | 0xff => 1,
            2..=4 => 2,
            6 => 4,
            _ => 0,
        };
        let end = cursor + operand_bytes;
        let operand = code
            .get(cursor..end)
            .ok_or(Error::TruncatedOperand(pc))?
            .iter()
            .fold(0_u32, |value, &byte| (value << 8) | u32::from(byte));
        cursor = end;
        let needed = match opcode {
            1 | 3 | 7..=0x16 | 0x28 | 0x31 => 1,
            0x17..=0x27 | 0x29..=0x30 | 0x32..=0x3b => 2,
            0x3c => operand as usize + 1,
            _ => 0,
        };
        if stack.len() < needed {
            return Err(Error::StackUnderflow { pc, opcode, needed });
        }
        match opcode {
            0 => {}
            1 => return Ok(f32::from_bits(stack[stack.len() - 1])),
            2 => stack.push(
                args.get(operand as usize)
                    .ok_or(Error::InvalidArgument {
                        pc,
                        index: operand as usize,
                    })?
                    .to_bits(),
            ),
            3 | 4 => {
                let take = opcode == 4 || stack.pop().unwrap() != 0;
                if take {
                    cursor += operand as usize;
                    if cursor > code.len() {
                        return Err(Error::InvalidJump { pc, target: cursor });
                    }
                }
            }
            // Removing a null linked-list head is a no-op in the reference.
            5 => stack.truncate(stack.len().saturating_sub(operand as usize)),
            6 => stack.push(operand),
            7..=0x3b => {
                let right = stack.pop().unwrap();
                let result = if needed == 1 {
                    unary(opcode, right, rng, pc)?
                } else {
                    binary(opcode, stack.pop().unwrap(), right, rng, pc)?
                };
                stack.push(result);
            }
            0x3c => stack.push(stack[stack.len() - needed]),
            0xff => return Err(Error::Unimplemented(pc)),
            _ => return Err(Error::InvalidOpcode { pc, opcode }),
        }
    }
    Err(Error::MissingReturn)
}

fn unary(opcode: u8, word: u32, rng: &mut HsdRng, pc: usize) -> Result<u32, Error> {
    let f = f32::from_bits(word);
    let i = word as i32;
    Ok(match opcode {
        7 => {
            if !(-2147483648.0..2147483648.0).contains(&f) {
                return Err(Error::InvalidFloatCast(pc));
            }
            (f as i32) as u32
        }
        8 => (i as f32).to_bits(),
        9 => word ^ 0x8000_0000,
        0x0a => i.wrapping_neg() as u32,
        0x0b => rng.randi(2) as u32,
        0x0c => rng.randf().to_bits(),
        0x0d => libm::sinf((DEG_TO_RAD * f64::from(f)) as f32).to_bits(),
        0x0e => libm::cosf((DEG_TO_RAD * f64::from(f)) as f32).to_bits(),
        0x0f => libm::tanf((DEG_TO_RAD * f64::from(f)) as f32).to_bits(),
        0x10 => degrees(libm::asinf(f)),
        0x11 => degrees(libm::acosf(f)),
        0x12 => degrees(libm::atanf(f)),
        0x13 => libm::logf(f).to_bits(),
        0x14 => libm::expf(f).to_bits(),
        0x15 => {
            if f < 0.0 {
                word ^ 0x8000_0000
            } else {
                word
            }
        }
        0x16 => libm::sqrtf(f).to_bits(),
        0x28 => i.wrapping_abs() as u32,
        0x31 => u32::from(word == 0),
        _ => unreachable!("unary opcode classified by evaluate"),
    })
}

fn degrees(radians: f32) -> u32 {
    ((RAD_TO_DEG * f64::from(radians)) as f32).to_bits()
}

fn binary(opcode: u8, left: u32, right: u32, rng: &mut HsdRng, pc: usize) -> Result<u32, Error> {
    let (a, b) = (f32::from_bits(left), f32::from_bits(right));
    let (i, j) = (left as i32, right as i32);
    Ok(match opcode {
        0x17 => (a + b).to_bits(),
        0x18 => (a - b).to_bits(),
        0x19 => (a * b).to_bits(),
        0x1a => (a / b).to_bits(),
        0x1b => libm::fmodf(a, b).to_bits(),
        0x1c => left.wrapping_add(right),
        0x1d => left.wrapping_sub(right),
        0x1e => left.wrapping_mul(right),
        0x1f => i.checked_div(j).ok_or(Error::InvalidDivision(pc))? as u32,
        0x20 => i.checked_rem(j).ok_or(Error::InvalidDivision(pc))? as u32,
        0x21 => libm::powf(a, b).to_bits(),
        0x22 => math::min(a, b).to_bits(),
        0x23 => math::max(a, b).to_bits(),
        0x24 => i.min(j) as u32,
        0x25 => i.max(j) as u32,
        0x26 => math::atan2_degrees(a, b).to_bits(),
        0x27 => i.wrapping_add(rng.randi(j.wrapping_sub(i).wrapping_add(1))) as u32,
        0x29 => u32::from(i < j),
        0x2a => u32::from(i > j),
        0x2b => u32::from(i <= j),
        0x2c => u32::from(i >= j),
        0x2d => u32::from(i == j),
        0x2e => u32::from(i != j),
        0x2f => u32::from(i != 0 && j != 0),
        0x30 => u32::from(i != 0 || j != 0),
        0x32 => u32::from((i == 0) != (j == 0)),
        0x33 => u32::from(a < b),
        0x34 => u32::from(a > b),
        0x35 => u32::from(a <= b),
        0x36 => u32::from(a >= b),
        0x37 => u32::from(a == b),
        0x38 => u32::from(a != b),
        0x39 => left & right,
        0x3a => left | right,
        0x3b => left ^ right,
        _ => unreachable!("binary opcode classified by evaluate"),
    })
}
