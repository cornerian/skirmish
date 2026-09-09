//! Terminal and semantic-trace adapters for the native menu branch library.

use crate::{
    menus::{Action, MenuState, Unlocks, controller::Controllers, input},
    trace::{Record, SCHEMA},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::BTreeMap,
    io::{BufRead, Write},
};

/// One deterministic adapter tick. `held` contains HSD digital button bits
/// for ports 0..3. Resume explicitly returns from an unported destination.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Controllers { held: [u32; 4] },
    Resume,
}

struct Session {
    menu: MenuState,
    controllers: Controllers,
}

impl Session {
    fn new(unlocks: Unlocks) -> Self {
        Self {
            menu: MenuState::new(unlocks),
            controllers: Controllers::default(),
        }
    }

    fn tick(&mut self, held: [u32; 4]) -> Option<Action> {
        let frames = self.controllers.poll(held);
        self.menu.step_for_port(
            input::decode(input::aggregate(&frames)),
            input::confirming_port(&frames),
        )
    }

    /// Human commands are discrete presses after transition cooldowns. The
    /// JSONL adapter below deliberately performs no implicit ticks.
    fn settle(&mut self) {
        while self.menu.snapshot().pending.is_none() && self.menu.snapshot().cooldown > 0 {
            self.tick([0; 4]);
        }
    }
}

/// Browse the implemented branches without a renderer or raw-terminal mode.
/// EOF exits cleanly, including when driven through a pipe.
pub fn interactive(
    unlocks: Unlocks,
    mut reader: impl BufRead,
    mut writer: impl Write,
) -> Result<()> {
    let mut session = Session::new(unlocks);
    writeln!(writer, "Skirmish menus — terminal navigation preview")?;
    writeln!(
        writer,
        "up/down, enter, back, quit (press Enter after each command)."
    )?;
    let mut line = String::new();
    loop {
        session.settle();
        let snapshot = session.menu.snapshot();
        writeln!(writer, "\n{}", snapshot.menu.title())?;
        for entry in snapshot.menu.entries() {
            let available = snapshot.menu.is_available(entry.index, unlocks);
            writeln!(
                writer,
                "{} {}{}",
                if snapshot.selection == entry.index {
                    ">"
                } else {
                    " "
                },
                entry.label,
                if available { "" } else { " (locked)" },
            )?;
        }
        if snapshot.pending.is_some() {
            writeln!(
                writer,
                "Destination requested; this screen is not implemented yet. Use back to return."
            )?;
        }
        write!(writer, "> ")?;
        writer.flush()?;
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let command = line.trim().to_ascii_lowercase();
        if matches!(command.as_str(), "q" | "quit" | "exit") {
            break;
        }
        let held = match command.as_str() {
            "up" | "u" | "w" | "\x1b[a" => input::pad::UP,
            "down" | "d" | "s" | "\x1b[b" => input::pad::DOWN,
            "" | "enter" | "confirm" | "a" => input::pad::A,
            "start" => input::pad::START,
            "back" | "b" | "esc" | "escape" | "\x1b" => input::pad::B,
            _ => {
                writeln!(writer, "Use up, down, enter, back, or quit.")?;
                continue;
            }
        };
        if snapshot.pending.is_some() {
            if held == input::pad::B {
                session.menu.resume();
                session.tick([0; 4]);
            } else {
                writeln!(writer, "Use back to return to the menu, or quit.")?;
            }
            continue;
        }
        session.tick([held as u32, 0, 0, 0]);
        session.tick([0; 4]);
    }
    Ok(())
}

/// Emit the existing semantic trace schema. Empty or malformed scenarios fail
/// without an End record; destinations remain pending until explicit Resume.
pub fn run(unlocks: Unlocks, reader: impl BufRead, mut writer: impl Write) -> Result<MenuState> {
    let mut session = Session::new(unlocks);
    write_record(
        &mut writer,
        &Record::Header {
            schema: SCHEMA,
            upstream_commit: "0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9".into(),
            scenario: "native-menu-branches-v1".into(),
            initial_state: BTreeMap::from([
                ("menu".into(), json!(session.menu.snapshot())),
                ("unlocks".into(), json!(unlocks)),
            ]),
        },
    )?;
    let mut frame = 0;
    for line in reader.lines() {
        ensure!(frame < 1_000_000, "scenario exceeds 1000000 frames");
        let command: Command = serde_json::from_str(&line?)
            .with_context(|| format!("invalid menu input on line {}", frame + 1))?;
        let mut events = vec![];
        match command {
            Command::Controllers { held } => {
                if let Some(action) = session.tick(held) {
                    events.push(json!(action));
                }
            }
            Command::Resume => {
                ensure!(
                    session.menu.resume(),
                    "menu input line {}: no pending destination to resume",
                    frame + 1
                );
                // Release adapter-held buttons; returning itself is this tick.
                session.controllers.poll([0; 4]);
                events.push(json!({ "kind": "resumed" }));
            }
        }
        write_record(
            &mut writer,
            &Record::Frame {
                frame,
                inputs: BTreeMap::from([("command".into(), json!(command))]),
                state: BTreeMap::from([("menu".into(), json!(session.menu.snapshot()))]),
                events,
            },
        )?;
        frame += 1;
    }
    ensure!(frame > 0, "scenario supplied no frames");
    write_record(&mut writer, &Record::End { frames: frame })?;
    writer.flush()?;
    Ok(session.menu)
}

fn write_record(writer: &mut impl Write, record: &Record) -> Result<()> {
    serde_json::to_writer(&mut *writer, record)?;
    writeln!(writer)?;
    Ok(())
}
