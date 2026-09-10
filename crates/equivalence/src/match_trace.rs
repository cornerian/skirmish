//! Native match adapter for the executable/trace comparison machinery.
use crate::trace::{Record, SCHEMA};
use anyhow::{Result, ensure};
use serde::Serialize;
use serde_json::{Value, json};
use skirmish::game::{self, Controller, Phase, State, data::MatchData};
use std::{collections::BTreeMap, io::Write};

/// The match schema contains f32 fields only. Serialization promotes them to
/// f64 exactly; convert those numbers back to f32 bits before writing traces.
/// The simulator validates finiteness before producing an observation.
pub fn float_bits(value: &impl Serialize) -> Result<Value> {
    fn encode(value: &mut Value) {
        match value {
            Value::Number(n) if n.is_f64() => {
                *value = Value::String(format!(
                    "f32:{:08x}",
                    (n.as_f64().unwrap() as f32).to_bits()
                ));
            }
            Value::Array(values) => values.iter_mut().for_each(encode),
            Value::Object(values) => values.values_mut().for_each(encode),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(value)?;
    encode(&mut value);
    Ok(value)
}

/// Each successful callback supplies exactly one frame. EOF ends a bounded
/// scenario; extra inputs after match completion are an error. No `end` record
/// is emitted on malformed input, physics errors or an empty scenario.
pub fn run(
    data: MatchData,
    seed: u32,
    mut next_input: impl FnMut(&State) -> Result<Option<[Controller; 2]>>,
    mut output: impl Write,
) -> Result<State> {
    let mut game = game::Match::new(data, seed)?;
    write(
        &mut output,
        &Record::Header {
            schema: SCHEMA,
            upstream_commit: "0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9".into(),
            scenario: "experimental-headless-match-v1".into(),
            initial_state: BTreeMap::from([
                ("match".into(), float_bits(game.state())?),
                (
                    "resources_sha256".into(),
                    json!(game.resource_id().map(|b| format!("{b:02x}")).concat()),
                ),
                ("profile".into(), json!(game.data().profile)),
                ("provenance".into(), json!(game.data().provenance)),
            ]),
        },
    )?;
    let mut frame = 0;
    while let Some(input) = next_input(game.state())? {
        ensure!(frame < 1_000_000, "scenario exceeds 1000000 frames");
        let state = game.step(input)?;
        let mut observation = float_bits(state)?;
        observation.as_object_mut().unwrap().remove("events");
        write(
            &mut output,
            &Record::Frame {
                frame,
                inputs: BTreeMap::from([("controllers".into(), float_bits(&input)?)]),
                state: BTreeMap::from([("match".into(), observation)]),
                events: state.events.iter().map(float_bits).collect::<Result<_>>()?,
            },
        )?;
        frame += 1;
    }
    ensure!(frame > 0, "scenario supplied no frames");
    write(&mut output, &Record::End { frames: frame })?;
    output.flush()?;
    Ok(game.state().clone())
}

fn write(output: &mut impl Write, record: &Record) -> Result<()> {
    serde_json::to_writer(&mut *output, record)?;
    writeln!(output)?;
    Ok(())
}

/// Scripted integration fixture: player one repeatedly jabs a passive opponent.
/// This is a lifecycle demonstration, not an RL policy or autonomous player.
pub fn demo_input(state: &State) -> Option<[Controller; 2]> {
    if matches!(state.phase, Phase::Finished { .. }) {
        return None;
    }
    Some([
        Controller {
            buttons: if matches!(state.phase, Phase::Playing) && state.next_frame.is_multiple_of(2)
            {
                game::BUTTON_A
            } else {
                0
            },
            ..Controller::default()
        },
        Controller::default(),
    ])
}
