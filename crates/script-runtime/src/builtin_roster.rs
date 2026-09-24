//! The small trusted bridge between bundled fighter bytes and roster identity.
//!
//! Fighter scripts intentionally do not declare their own identity.  The
//! native loader therefore needs one source of provenance when it is used
//! directly, outside the higher-level game loader.  Matching both the exact
//! embedded bytes and their private filename keeps arbitrary Pon programs
//! from selecting a roster entry by setting a private global or by choosing a
//! familiar filename.

#[derive(Clone, Copy)]
struct BuiltinSource {
    filename: &'static str,
    module: &'static str,
    source: &'static str,
}

// Keep this table private to the runtime.  The game crate has its own richer
// roster table; duplicating only the immutable source identity here avoids a
// dependency cycle while preserving a trusted direct-CompiledProgram path.
const BUILTIN_SOURCES: &[BuiltinSource] = &[
    BuiltinSource {
        filename: "captain.py",
        module: "captain",
        source: include_str!("../../../scripts/fighters/captain.py"),
    },
    BuiltinSource {
        filename: "donkey_kong.py",
        module: "donkey_kong",
        source: include_str!("../../../scripts/fighters/donkey_kong.py"),
    },
    BuiltinSource {
        filename: "fox.py",
        module: "fox",
        source: include_str!("../../../scripts/fighters/fox.py"),
    },
    BuiltinSource {
        filename: "game_and_watch.py",
        module: "game_and_watch",
        source: include_str!("../../../scripts/fighters/game_and_watch.py"),
    },
    BuiltinSource {
        filename: "kirby.py",
        module: "kirby",
        source: include_str!("../../../scripts/fighters/kirby.py"),
    },
    BuiltinSource {
        filename: "bowser.py",
        module: "bowser",
        source: include_str!("../../../scripts/fighters/bowser.py"),
    },
    BuiltinSource {
        filename: "link.py",
        module: "link",
        source: include_str!("../../../scripts/fighters/link.py"),
    },
    BuiltinSource {
        filename: "luigi.py",
        module: "luigi",
        source: include_str!("../../../scripts/fighters/luigi.py"),
    },
    BuiltinSource {
        filename: "mario.py",
        module: "mario",
        source: include_str!("../../../scripts/fighters/mario.py"),
    },
    BuiltinSource {
        filename: "marth.py",
        module: "marth",
        source: include_str!("../../../scripts/fighters/marth.py"),
    },
    BuiltinSource {
        filename: "mewtwo.py",
        module: "mewtwo",
        source: include_str!("../../../scripts/fighters/mewtwo.py"),
    },
    BuiltinSource {
        filename: "ness.py",
        module: "ness",
        source: include_str!("../../../scripts/fighters/ness.py"),
    },
    BuiltinSource {
        filename: "peach.py",
        module: "peach",
        source: include_str!("../../../scripts/fighters/peach.py"),
    },
    BuiltinSource {
        filename: "pikachu.py",
        module: "pikachu",
        source: include_str!("../../../scripts/fighters/pikachu.py"),
    },
    BuiltinSource {
        filename: "ice_climbers.py",
        module: "ice_climbers",
        source: include_str!("../../../scripts/fighters/ice_climbers.py"),
    },
    BuiltinSource {
        filename: "jigglypuff.py",
        module: "jigglypuff",
        source: include_str!("../../../scripts/fighters/jigglypuff.py"),
    },
    BuiltinSource {
        filename: "samus.py",
        module: "samus",
        source: include_str!("../../../scripts/fighters/samus.py"),
    },
    BuiltinSource {
        filename: "yoshi.py",
        module: "yoshi",
        source: include_str!("../../../scripts/fighters/yoshi.py"),
    },
    BuiltinSource {
        filename: "zelda.py",
        module: "zelda",
        source: include_str!("../../../scripts/fighters/zelda.py"),
    },
    BuiltinSource {
        filename: "sheik.py",
        module: "sheik",
        source: include_str!("../../../scripts/fighters/sheik.py"),
    },
    BuiltinSource {
        filename: "falco.py",
        module: "falco",
        source: include_str!("../../../scripts/fighters/falco.py"),
    },
    BuiltinSource {
        filename: "young_link.py",
        module: "young_link",
        source: include_str!("../../../scripts/fighters/young_link.py"),
    },
    BuiltinSource {
        filename: "dr_mario.py",
        module: "dr_mario",
        source: include_str!("../../../scripts/fighters/dr_mario.py"),
    },
    BuiltinSource {
        filename: "roy.py",
        module: "roy",
        source: include_str!("../../../scripts/fighters/roy.py"),
    },
    BuiltinSource {
        filename: "pichu.py",
        module: "pichu",
        source: include_str!("../../../scripts/fighters/pichu.py"),
    },
    BuiltinSource {
        filename: "ganondorf.py",
        module: "ganondorf",
        source: include_str!("../../../scripts/fighters/ganondorf.py"),
    },
];

/// Return the trusted private module name for one exact bundled source.
pub(crate) fn module_for(source: &str, filename: &str) -> Option<&'static str> {
    BUILTIN_SOURCES
        .iter()
        .find(|builtin| {
            builtin.filename == filename
                && (builtin.source == source || source_with_game_marker(source, builtin.source))
        })
        .map(|builtin| builtin.module)
}

// The game facade historically appended this marker before handing a source
// to CompiledProgram.  Recognize that one trusted wrapper so the lower-level
// constructor can take ownership of marker injection without breaking the
// existing game path.  The marker's value is never trusted or read.
fn source_with_game_marker(source: &str, builtin: &str) -> bool {
    let Some((prefix, marker)) = source.rsplit_once("\n__skirmish_canonical_module__ = ") else {
        return false;
    };
    if prefix != builtin {
        return false;
    }
    if marker == "None" {
        return true;
    }
    let Some(module) = marker
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
        .or_else(|| {
            marker
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
        })
    else {
        return false;
    };
    !module.is_empty()
        && module
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::module_for;

    #[test]
    fn only_exact_bundled_bytes_resolve() {
        let source = include_str!("../../../scripts/fighters/yoshi.py");
        assert_eq!(module_for(source, "yoshi.py"), Some("yoshi"));
        assert_eq!(
            module_for(&format!("{source}\n# changed"), "yoshi.py"),
            None
        );
        assert_eq!(module_for(source, "mario.py"), None);
    }

    #[test]
    fn game_wrapper_is_supported_without_trusting_marker_value() {
        let source = include_str!("../../../scripts/fighters/yoshi.py");
        let wrapped = format!("{source}\n__skirmish_canonical_module__ = None");
        assert_eq!(module_for(&wrapped, "yoshi.py"), Some("yoshi"));
        let spoofed = format!("{source}\n__skirmish_canonical_module__ = 'mario'");
        assert_eq!(module_for(&spoofed, "yoshi.py"), Some("yoshi"));
    }
}
