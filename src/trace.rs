//! Strict semantic trace comparison, independent of executable layout.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, io::BufRead};

pub const SCHEMA: u32 = 1;

/// Adapters must encode floating-point state as bit strings (e.g. `f32:80000000`),
/// and pointers as stable object IDs. Native addresses and decimal floats do not
/// define a portable, exact state comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Record {
    Header {
        schema: u32,
        upstream_commit: String,
        scenario: String,
        initial_state: BTreeMap<String, Value>,
    },
    Frame {
        frame: u64,
        inputs: BTreeMap<String, Value>,
        state: BTreeMap<String, Value>,
        events: Vec<Value>,
    },
    End {
        frames: u64,
    },
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub frames: u64,
    pub scenario: String,
}

fn read_record(reader: &mut impl BufRead, line: &mut usize) -> Result<Option<Record>> {
    let mut text = String::new();
    if reader.read_line(&mut text)? == 0 {
        return Ok(None);
    }
    *line += 1;
    let record: Record =
        serde_json::from_str(&text).with_context(|| format!("invalid trace line {line}"))?;
    validate_observations(&record).with_context(|| format!("invalid trace line {line}"))?;
    Ok(Some(record))
}

fn validate_value(value: &Value, path: &str) -> Result<()> {
    match value {
        Value::Number(number) => ensure!(
            !number.is_f64(),
            "{path}: decimal floats are forbidden; encode exact f32:/f64: bits"
        ),
        Value::String(value) => {
            for (prefix, width) in [("f32:", 8), ("f64:", 16)] {
                if let Some(bits) = value.strip_prefix(prefix) {
                    ensure!(
                        bits.len() == width && bits.bytes().all(|byte| byte.is_ascii_hexdigit()),
                        "{path}: {prefix} requires exactly {width} hexadecimal digits"
                    );
                }
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_value(value, &format!("{path}/{index}"))?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                validate_value(value, &format!("{path}/{key}"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_observations(record: &Record) -> Result<()> {
    let (fields, path) = match record {
        Record::Header { initial_state, .. } => {
            (vec![("initial_state", initial_state)], "header".into())
        }
        Record::Frame {
            frame,
            inputs,
            state,
            events,
        } => {
            let path = format!("frame {frame}");
            for (index, event) in events.iter().enumerate() {
                validate_value(event, &format!("{path}/events/{index}"))?;
            }
            (vec![("inputs", inputs), ("state", state)], path)
        }
        Record::End { .. } => return Ok(()),
    };
    for (field, values) in fields {
        for (key, value) in values {
            validate_value(value, &format!("{path}/{field}/{key}"))?;
        }
    }
    Ok(())
}

/// Returns the first differing semantic field, ignoring JSON object key order.
fn difference(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            for key in a.keys().chain(b.keys()) {
                let next = format!("{path}/{key}");
                match (a.get(key), b.get(key)) {
                    (Some(a), Some(b)) => {
                        if let Some(diff) = difference(a, b, &next) {
                            return Some(diff);
                        }
                    }
                    _ => return Some(format!("{next}: field missing on one side")),
                }
            }
            None
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => a
            .iter()
            .zip(b)
            .enumerate()
            .find_map(|(i, (a, b))| difference(a, b, &format!("{path}/{i}"))),
        _ if a == b => None,
        _ => Some(format!("{path}: reference={a}, candidate={b}")),
    }
}

/// Compare complete, nonempty traces with contiguous frame numbers starting at 0.
/// A matching prefix, empty pair, or duplicate frame is never a passing test.
pub fn compare(mut reference: impl BufRead, mut candidate: impl BufRead) -> Result<Comparison> {
    let (mut left_line, mut right_line) = (0, 0);
    let left = read_record(&mut reference, &mut left_line)?.context("empty reference trace")?;
    let right = read_record(&mut candidate, &mut right_line)?.context("empty candidate trace")?;
    let Record::Header {
        schema,
        upstream_commit,
        scenario,
        ..
    } = &left
    else {
        bail!("reference trace must start with a header");
    };
    ensure!(*schema == SCHEMA, "unsupported trace schema {schema}");
    ensure!(!scenario.is_empty(), "scenario must not be empty");
    ensure!(
        upstream_commit.len() == 40 && upstream_commit.bytes().all(|b| b.is_ascii_hexdigit()),
        "upstream_commit must be a full 40-character Git revision"
    );
    if let Some(diff) = difference(
        &serde_json::to_value(&left)?,
        &serde_json::to_value(&right)?,
        "header",
    ) {
        bail!("{diff}");
    }
    let scenario = scenario.clone();
    let mut frames = 0;
    loop {
        let left = read_record(&mut reference, &mut left_line)?
            .context("reference trace is truncated (missing end)")?;
        let right = read_record(&mut candidate, &mut right_line)?
            .context("candidate trace is truncated (missing end)")?;
        if let Some(diff) = difference(
            &serde_json::to_value(&left)?,
            &serde_json::to_value(&right)?,
            &format!("frame {frames}"),
        ) {
            bail!("{diff}");
        }
        match left {
            Record::Frame { frame, state, .. } => {
                ensure!(frame == frames, "expected frame {frames}, found {frame}");
                ensure!(!state.is_empty(), "frame {frame} has no observed state");
                frames += 1;
            }
            Record::End { frames: declared } => {
                ensure!(frames > 0, "zero-frame traces cannot establish agreement");
                ensure!(
                    declared == frames,
                    "end declares {declared} frames; observed {frames}"
                );
                ensure!(
                    read_record(&mut reference, &mut left_line)?.is_none(),
                    "reference has records after end"
                );
                ensure!(
                    read_record(&mut candidate, &mut right_line)?.is_none(),
                    "candidate has records after end"
                );
                return Ok(Comparison { frames, scenario });
            }
            Record::Header { .. } => bail!("unexpected header after the start of the trace"),
        }
    }
}

pub fn f32_bits(value: f32) -> Value {
    Value::String(format!("f32:{:08x}", value.to_bits()))
}
