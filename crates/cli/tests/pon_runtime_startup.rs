use std::{fs, process::Command};
use tempfile::TempDir;

const ARCHIVE: &str = "/tmp/skirmish-stdlib-release-proof/pon-stdlib-final-sorted.tar.gz";
const SHA256: &str = "5c7ce12a21f4d5ae49a8cb2c425911bc4de863f427e0ff174fb74ff7bf63639c";

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_skirmish"))
}

fn staged_release_with_archive(archive: &std::path::Path) -> (TempDir, std::path::PathBuf) {
    let stage = tempfile::tempdir().unwrap();
    let executable = stage.path().join("skirmish");
    fs::copy(env!("CARGO_BIN_EXE_skirmish"), &executable).unwrap();
    let resources = stage.path().join("resources/pon-stdlib");
    fs::create_dir_all(&resources).unwrap();
    fs::copy(archive, resources.join("pon-stdlib.tar.gz")).unwrap();
    fs::write(resources.join("identity.json"), format!(
        "{{\"archive\":\"pon-stdlib.tar.gz\",\"archive_sha256\":\"{SHA256}\",\"format\":\"skirmish-pon-stdlib-release-v1\",\"identity\":\"b14b1766c1138f9aad84664dbbd4f52242b874a1003810a7ec69410cc5f83782\"}}\n"
    )).unwrap();
    (stage, executable)
}

fn staged_invalid_release() -> (TempDir, std::path::PathBuf) {
    let stage = tempfile::tempdir().unwrap();
    let executable = stage.path().join("skirmish");
    fs::copy(env!("CARGO_BIN_EXE_skirmish"), &executable).unwrap();
    let resources = stage.path().join("resources/pon-stdlib");
    fs::create_dir_all(&resources).unwrap();
    fs::write(resources.join("pon-stdlib.tar.gz"), b"invalid archive").unwrap();
    fs::write(resources.join("identity.json"), format!(
        "{{\"archive\":\"pon-stdlib.tar.gz\",\"archive_sha256\":\"{SHA256}\",\"format\":\"skirmish-pon-stdlib-release-v1\",\"identity\":\"{}\"}}\n",
        "b14b1766c1138f9aad84664dbbd4f52242b874a1003810a7ec69410cc5f83782"
    )).unwrap();
    (stage, executable)
}

#[test]
#[ignore = "requires the finalized verified Pon stdlib release artifact"]
fn copied_release_uses_executable_relative_stdlib_from_unrelated_cwd() {
    let (_stage, executable) = staged_release_with_archive(std::path::Path::new(ARCHIVE));
    let cwd = tempfile::tempdir().unwrap();
    let data = cwd.path().join("fighter.json");
    let mut fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/game/integration-match.json"
    ))
    .unwrap();
    fixture["rules"]["countdown_frames"] = serde_json::json!(0);
    let callback_source = r#"
from skirmish import (AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves,
    GrabMoves, GroundedMoves, LedgeMoves, Move, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves, hook, register)
class EntryMove(Move):
    @hook.press("B")
    def entered(self, fighter, context):
        fighter.state.counter = fighter.state.counter + 1
        return True
class State:
    counter: int = 0
ordinary = EntryMove()
@register
class NativeFighter(Fighter):
    name = "native_preparation"
    attributes = Attributes
    state = State
    specials = SpecialMoves(ordinary, ordinary, ordinary, ordinary)
    aerials = AerialMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary)
    smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary)
    throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)
"#;
    fixture["fighters"][0]["script"] = serde_json::json!({
        "version": "pon-v2",
        "source": callback_source,
        "dependencies": {}
    });
    fs::write(&data, serde_json::to_vec(&fixture).unwrap()).unwrap();
    let mut inputs = String::new();
    for _ in 0..300 {
        inputs.push_str(
            "[{\"buttons\":512,\"stick\":[0.0,0.0]},{\"buttons\":0,\"stick\":[0.0,0.0]}]\n",
        );
    }
    let input = cwd.path().join("inputs.jsonl");
    fs::write(&input, inputs).unwrap();
    let output = Command::new(&executable)
        .current_dir(cwd.path())
        .env_clear()
        .env("PON_STDLIB_PATH", cwd.path().join("missing-developer-root"))
        .args(["run-match", "--data"])
        .arg(&data)
        .stdin(std::process::Stdio::from(fs::File::open(&input).unwrap()))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("frame"));
    let records: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("run-match emitted malformed JSONL"))
        .collect();
    let counters: Vec<i64> = records
        .iter()
        .filter(|record| record["kind"] == "frame")
        .map(|record| {
            record["state"]["match"]["fighters"][0]["script_state"]["counter"]
                .as_i64()
                .expect("frame state omitted script counter")
        })
        .collect();
    assert!(
        counters.len() == 300 && counters.iter().all(|counter| *counter == 1),
        "Pon callback counter was not exactly one for all frames: {counters:?}"
    );
}

#[test]
fn packaged_archive_bad_hash_fails_closed() {
    let (_stage, executable) = staged_invalid_release();
    let manifest = executable
        .parent()
        .unwrap()
        .join("resources/pon-stdlib/identity.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    value["archive_sha256"] = serde_json::Value::String("0".repeat(64));
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    let output = Command::new(&executable)
        .env_clear()
        .args(["inventory", "."])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SHA-256"));
}

#[test]
fn packaged_identity_manifest_missing_fails_closed() {
    let (_stage, executable) = staged_invalid_release();
    fs::remove_file(
        executable
            .parent()
            .unwrap()
            .join("resources/pon-stdlib/identity.json"),
    )
    .unwrap();
    let output = Command::new(&executable)
        .env_clear()
        .args(["inventory", "."])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("identity manifest is missing"));
}

#[test]
fn archive_requires_digest_pair() {
    let output = cli()
        .args(["--pon-stdlib", ARCHIVE, "demo-match"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--pon-stdlib-sha256") && stderr.contains("required arguments"),
        "{stderr}"
    );
}

#[test]
fn digest_requires_archive_pair() {
    let output = cli()
        .args(["--pon-stdlib-sha256", SHA256, "demo-match"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--pon-stdlib") && stderr.contains("required arguments"),
        "{stderr}"
    );
}

#[test]
fn malformed_digest_fails_before_archive_access() {
    let output = cli()
        .args([
            "--pon-stdlib",
            "/path/that/does/not/exist.tar.gz",
            "--pon-stdlib-sha256",
            "not-a-sha256",
            "demo-match",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--pon-stdlib-sha256 must contain exactly 64 hexadecimal characters"),
        "{stderr}"
    );
}

#[test]
fn unicode_digest_fails_without_panicking() {
    let malformed_digest = format!("{}éa", "a".repeat(61));
    let output = cli()
        .args([
            "--pon-stdlib",
            "/path/that/does/not/exist.tar.gz",
            "--pon-stdlib-sha256",
            &malformed_digest,
            "demo-match",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--pon-stdlib-sha256 must contain exactly 64 hexadecimal characters"),
        "{stderr}"
    );
}

#[test]
#[ignore = "requires the finalized verified Pon stdlib release artifact"]
fn verified_archive_configures_before_native_demo_match() {
    let output = cli()
        .args([
            "--pon-stdlib",
            ARCHIVE,
            "--pon-stdlib-sha256",
            SHA256,
            "demo-match",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
