//! Verified-Pon loading coverage for every bundled fighter source.
//!
//! This test deliberately uses the release stdlib artifact instead of the
//! direct `Registry::builtins()` path: the fighter authoring API imports
//! `dataclasses`, which is intentionally not part of the facade's embedded
//! SDK.  The source table below mirrors `game::script::BUILTIN_SCRIPTS` and
//! uses compile-time includes so the test never discovers files at runtime.

use std::fs::File;

use skirmish_script_runtime::{
    CompiledProgram, NativeValue, SourceBundle, StandardLibrary, configure_standard_library,
};

const STDLIB_ARCHIVE: &str = "/tmp/captain-slippi-init-20260917-export/pon-stdlib.tar.gz";
const STDLIB_ARCHIVE_SHA256: &str =
    "91aab4eb859690ebe1ecf8e3e21957378fea86b1ebac0a61e7559145f64c642d";

/// Keep this table in the same order and with the same identities as the
/// canonical `game::script::BUILTIN_SCRIPTS` table.  Each source is embedded
/// at compile time; no filesystem scan or resource fallback is permitted.
const BUILTIN_FIGHTERS: [(&str, &str, &str); 26] = [
    (
        "captain-falcon",
        "captain.py",
        include_str!("../../../scripts/fighters/captain.py"),
    ),
    (
        "donkey-kong",
        "donkey_kong.py",
        include_str!("../../../scripts/fighters/donkey_kong.py"),
    ),
    (
        "fox",
        "fox.py",
        include_str!("../../../scripts/fighters/fox.py"),
    ),
    (
        "game-and-watch",
        "game_and_watch.py",
        include_str!("../../../scripts/fighters/game_and_watch.py"),
    ),
    (
        "kirby",
        "kirby.py",
        include_str!("../../../scripts/fighters/kirby.py"),
    ),
    (
        "bowser",
        "bowser.py",
        include_str!("../../../scripts/fighters/bowser.py"),
    ),
    (
        "link",
        "link.py",
        include_str!("../../../scripts/fighters/link.py"),
    ),
    (
        "luigi",
        "luigi.py",
        include_str!("../../../scripts/fighters/luigi.py"),
    ),
    (
        "mario",
        "mario.py",
        include_str!("../../../scripts/fighters/mario.py"),
    ),
    (
        "marth",
        "marth.py",
        include_str!("../../../scripts/fighters/marth.py"),
    ),
    (
        "mewtwo",
        "mewtwo.py",
        include_str!("../../../scripts/fighters/mewtwo.py"),
    ),
    (
        "ness",
        "ness.py",
        include_str!("../../../scripts/fighters/ness.py"),
    ),
    (
        "peach",
        "peach.py",
        include_str!("../../../scripts/fighters/peach.py"),
    ),
    (
        "pikachu",
        "pikachu.py",
        include_str!("../../../scripts/fighters/pikachu.py"),
    ),
    (
        "ice-climbers",
        "ice_climbers.py",
        include_str!("../../../scripts/fighters/ice_climbers.py"),
    ),
    (
        "jigglypuff",
        "jigglypuff.py",
        include_str!("../../../scripts/fighters/jigglypuff.py"),
    ),
    (
        "samus",
        "samus.py",
        include_str!("../../../scripts/fighters/samus.py"),
    ),
    (
        "yoshi",
        "yoshi.py",
        include_str!("../../../scripts/fighters/yoshi.py"),
    ),
    (
        "zelda",
        "zelda.py",
        include_str!("../../../scripts/fighters/zelda.py"),
    ),
    (
        "sheik",
        "sheik.py",
        include_str!("../../../scripts/fighters/sheik.py"),
    ),
    (
        "falco",
        "falco.py",
        include_str!("../../../scripts/fighters/falco.py"),
    ),
    (
        "young-link",
        "young_link.py",
        include_str!("../../../scripts/fighters/young_link.py"),
    ),
    (
        "dr-mario",
        "dr_mario.py",
        include_str!("../../../scripts/fighters/dr_mario.py"),
    ),
    (
        "roy",
        "roy.py",
        include_str!("../../../scripts/fighters/roy.py"),
    ),
    (
        "pichu",
        "pichu.py",
        include_str!("../../../scripts/fighters/pichu.py"),
    ),
    (
        "ganondorf",
        "ganondorf.py",
        include_str!("../../../scripts/fighters/ganondorf.py"),
    ),
];

fn digest(hex: &str) -> [u8; 32] {
    (0..32)
        .map(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

fn source_bundle(filename: &str, source: &str) -> SourceBundle {
    SourceBundle::new("fighter-api-roster-conformance-v1")
        .with_file(filename, source)
        .expect("fighter source module path")
}

#[test]
#[ignore = "requires the captain-slippi verified Pon stdlib artifact"]
fn every_builtin_fighter_loads_with_verified_pon_stdlib() {
    let archive_path =
        std::env::var("SKIRMISH_PON_STDLIB_ARCHIVE").unwrap_or_else(|_| STDLIB_ARCHIVE.to_owned());
    let archive_sha256 = std::env::var("SKIRMISH_PON_STDLIB_SHA256")
        .unwrap_or_else(|_| STDLIB_ARCHIVE_SHA256.to_owned());
    let archive = File::open(&archive_path)
        .unwrap_or_else(|error| panic!("verified Pon stdlib archive {archive_path}: {error}"));
    let library = StandardLibrary::from_archive(archive, digest(&archive_sha256))
        .unwrap_or_else(|error| panic!("strictly validate verified Pon stdlib archive: {error}"));
    let cache_root =
        std::env::temp_dir().join(format!("skirmish-pon-roster-{}", std::process::id()));
    let materialized = library
        .materialize(&cache_root)
        .expect("materialize verified Pon stdlib");
    configure_standard_library(materialized).expect("configure verified Pon stdlib");

    assert_eq!(BUILTIN_FIGHTERS.len(), 26);
    for (character_key, filename, source) in BUILTIN_FIGHTERS {
        eprintln!("loading bundled fighter {character_key} ({filename})");
        let program = CompiledProgram::new_with_bundle(
            source,
            filename,
            [],
            Some(source_bundle(filename, source)),
        )
        .unwrap_or_else(|error| panic!("{character_key} ({filename}) failed to load: {error}"));

        let NativeValue::Dict(definition) = program
            .export_metadata()
            .unwrap_or_else(|error| panic!("{character_key} metadata export failed: {error}"))
        else {
            panic!("{character_key} did not export exactly one fighter definition");
        };
        assert_eq!(
            definition.get("name"),
            Some(&NativeValue::String(character_key.into())),
            "{character_key} exported a definition for the wrong fighter"
        );

        let callbacks = program
            .callback_keys()
            .unwrap_or_else(|error| panic!("{character_key} callback metadata failed: {error}"));
        assert!(
            !callbacks.is_empty(),
            "{character_key} exported no callback metadata"
        );
    }
}
