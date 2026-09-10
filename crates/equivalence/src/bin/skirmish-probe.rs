//! Executable differential probe for translated RNGs, not a game executable.
use anyhow::{Context, Result, ensure};
use serde_json::json;
use skirmish::random::{HsdRng, MslRng};
use skirmish_equivalence::trace::{Record, f32_bits};
use std::{
    collections::BTreeMap,
    io::{self, Write},
};

fn main() -> Result<()> {
    let input = io::read_to_string(io::stdin())?;
    let fields: Vec<_> = input.split_whitespace().collect();
    ensure!(fields.len() == 3, "expected: seed:u32 frames:u32 bound:i32");
    let seed = fields[0].parse::<u32>().context("seed")?;
    let frames = fields[1].parse::<u32>().context("frame count")?;
    let bound = fields[2].parse::<i32>().context("bound")?;
    ensure!(
        (1..=1_000_000).contains(&frames),
        "frames must be in 1..=1000000"
    );
    let mut out = io::BufWriter::new(io::stdout().lock());
    writeln!(
        out,
        "{}",
        serde_json::to_string(&Record::Header {
            schema: 1,
            upstream_commit: "0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9".into(),
            scenario: "rng-probe-v1".into(),
            initial_state: BTreeMap::from([
                ("seed".into(), json!(seed)),
                ("frames".into(), json!(frames)),
                ("bound".into(), json!(bound))
            ]),
        })?
    )?;
    let (mut hsd, mut msl) = (HsdRng::new(seed), MslRng::new(seed));
    for frame in 0..frames {
        let rand = hsd.rand();
        let randf = hsd.randf();
        let randi = hsd.randi(bound);
        let msl_rand = msl.rand();
        let record = Record::Frame {
            frame: u64::from(frame),
            inputs: BTreeMap::from([("bound".into(), json!(bound))]),
            state: BTreeMap::from([
                ("hsd.rand".into(), json!(rand)),
                ("hsd.randf".into(), f32_bits(randf)),
                ("hsd.randi".into(), json!(randi)),
                ("hsd.seed".into(), json!(hsd.seed())),
                ("msl.rand".into(), json!(msl_rand)),
                ("msl.seed".into(), json!(msl.seed())),
            ]),
            events: vec![],
        };
        writeln!(out, "{}", serde_json::to_string(&record)?)?;
    }
    writeln!(
        out,
        "{}",
        serde_json::to_string(&Record::End {
            frames: u64::from(frames)
        })?
    )?;
    out.flush()?;
    Ok(())
}
