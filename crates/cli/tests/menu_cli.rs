use skirmish::menus::{Menu, Unlocks, input::pad};
use skirmish_cli::menu_cli::{self, Command};
use skirmish_equivalence::trace;
use std::{
    io::{Cursor, Write},
    process::{Command as Process, Stdio},
};

fn run_cli(command: &str, input: &[u8]) -> std::process::Output {
    let mut child = Process::new(env!("CARGO_BIN_EXE_skirmish"))
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn script(commands: impl IntoIterator<Item = Command>) -> Vec<u8> {
    let mut bytes = vec![];
    for command in commands {
        serde_json::to_writer(&mut bytes, &command).unwrap();
        bytes.push(b'\n');
    }
    bytes
}

fn held(button: u64) -> Command {
    Command::Controllers {
        held: [button as u32, 0, 0, 0],
    }
}

#[test]
fn terminal_browses_a_branch_requests_a_destination_and_returns() {
    let output = run_cli("menus", b"down\nenter\nenter\nback\nback\nquit\n");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("> Melee"), "{text}");
    assert_eq!(text.matches("Destination requested;").count(), 1, "{text}");
    assert!(text.contains("Use back to return."));
    let eof = run_cli("menus", b"");
    assert!(eof.status.success());
}

#[test]
fn native_controller_frames_do_not_reconfirm_when_a_is_held_through_transition() {
    let commands = std::iter::repeat_n(held(0), 20)
        .chain([held(pad::DOWN), held(0)])
        .chain(std::iter::repeat_n(held(pad::A), 32));
    let input = script(commands);
    let mut expected = vec![];
    let state = menu_cli::run(Unlocks::default(), Cursor::new(&input), &mut expected).unwrap();
    assert_eq!(state.snapshot().menu, Menu::Versus);
    assert_eq!(state.snapshot().selection, 0);
    assert!(state.snapshot().pending.is_none());
    let actual = run_cli("run-menus", &input);
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(
        trace::compare(Cursor::new(expected), Cursor::new(actual.stdout))
            .unwrap()
            .frames,
        54
    );
}

#[test]
fn trace_resume_returns_to_origin_and_preserves_menu_selection() {
    let commands = std::iter::repeat_n(held(0), 20)
        .chain([held(pad::DOWN), held(0), held(pad::A)])
        .chain(std::iter::repeat_n(held(0), 5))
        .chain([held(pad::START)]);
    let mut input = script(commands);
    let state = menu_cli::run(Unlocks::default(), Cursor::new(&input), vec![]).unwrap();
    assert_eq!(state.snapshot().menu, Menu::Versus);
    assert!(state.snapshot().pending.is_some());
    input.extend(script([Command::Resume]));
    let state = menu_cli::run(Unlocks::default(), Cursor::new(&input), vec![]).unwrap();
    assert_eq!(state.snapshot().menu, Menu::Versus);
    assert_eq!(state.snapshot().selection, 0);
    assert_eq!(state.snapshot().cooldown, 5);
    assert!(state.snapshot().pending.is_none());
}

#[test]
fn invalid_scenarios_never_emit_a_success_end_record() {
    for input in [
        "",
        "{\"kind\":\"controllers\",\"held\":[0]}\n",
        "{\"kind\":\"controllers\",\"held\":[0,0,0,0],\"unknown\":true}\n",
        "{\"kind\":\"resume\"}\n",
        "{\"kind\":\"controllers\",\"held\":[0,0,0,0]}\nnot-json\n",
    ] {
        let output = run_cli("run-menus", input.as_bytes());
        assert!(!output.status.success(), "{input}");
        assert!(
            !String::from_utf8(output.stdout)
                .unwrap()
                .contains("\"kind\":\"end\"")
        );
    }
}
