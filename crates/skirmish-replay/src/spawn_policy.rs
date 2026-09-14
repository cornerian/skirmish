//! Match-start spawn position policy.
//!
//! A fighter's spawn position at match start is *recording environment*
//! state, not purely stage data: the retail game resolves it once, per
//! match, from `VsSceneController`'s per-slot `spawn_point` (defaulting to
//! the slot index itself) through `Stage_80224E64`
//! (`gr/stage.c`, which composes the map's spawn markers) and
//! `Ground_801C2D24` (`gr/ground.c`); `gm/gmvs.c`'s `getSpawnPoint` (~1754)
//! and the per-slot spawn-assignment loop (~1905-1935) are the call site
//! that turns each active player slot into one of the stage's exported
//! points, in ascending slot order. That is what
//! `game::simulation::spawn` already reproduces via
//! `data.stage.spawns[player]` (`docs/architecture.md`), the pack's own,
//! vanilla NTSC 1.02 map points.
//!
//! Two other codesets are known to start a real recording somewhere else:
//!
//! - Slippi *netplay* builds (confirmed for 3.9.0) inject UnclePunch's
//!   "Neutral Spawns" table (`External/NeutralSpawn/NeutralSpawn.asm`,
//!   injection `8016e510`) instead of the vanilla points, for the six
//!   legal stages. [`SLIPPI_NEUTRAL_SINGLES`] is that table's singles rows,
//!   as exact `f32` literals; assignment is by participant order among
//!   active slots, the same convention `data.stage.spawns[player]` already
//!   uses.
//! - At least one console-era codeset (unidentified so far) starts Dream
//!   Land recordings at a y coordinate neither table produces
//!   (`docs/parity.md`'s 2026-09-14 sections have the measurement).
//!
//! [`SpawnPolicy`] makes the choice of starting point part of the match
//! initialization data, not an assumption baked into the resource pack:
//! `Vanilla` is the unchanged default (the pack's own `stage.spawns`, used
//! for every non-replay match); `SlippiNeutral` resolves the table above;
//! `Explicit` carries caller-supplied positions, which is what
//! `make-initialization` fills from a real replay's own frame -123
//! post-frame position (see `crates/cli/src/initialization.rs::build`) so a
//! validated replay always starts from what it actually recorded, not from
//! an assumption about which codeset produced it. [`classify_spawn_provenance`]
//! is the read-only diagnostic that reports, after the fact, which (if
//! either) of the two known tables an `Explicit` pair happens to equal.
use serde::{Deserialize, Serialize};

/// Which spawn positions a match begins from. Facing keeps the existing
/// rule regardless of policy: `fighter::entry::spawn_facing` (sign of the
/// resolved spawn x, `gmvs.c`'s `fn_8016DEEC`/`direction`) reads whatever
/// `data.stage.spawns` ends up holding after [`SpawnPolicy::resolve`], so a
/// policy only ever changes the positions, never the facing rule itself.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpawnPolicy {
    /// The resource pack's own `stage.spawns`, unchanged. Default for every
    /// non-replay match; behaviour predates this type.
    Vanilla,
    /// UnclePunch's Neutral Spawns singles table ([`SLIPPI_NEUTRAL_SINGLES`]),
    /// resolved from `data.stage.name` at construction time.
    SlippiNeutral,
    /// Caller-supplied per-participant positions, in the same
    /// participant-order convention as `data.stage.spawns`
    /// (ports sorted ascending assigned to index 0/1).
    Explicit { spawns: [[f32; 2]; 2] },
}

impl SpawnPolicy {
    /// Resolve to concrete per-participant spawn points. `vanilla` is the
    /// resource pack's own `data.stage.spawns`, passed through unchanged
    /// for [`SpawnPolicy::Vanilla`] and used as the fallback identity for
    /// every other variant's own resolution.
    pub fn resolve(
        &self,
        stage_name: &str,
        vanilla: [[f32; 2]; 2],
    ) -> anyhow::Result<[[f32; 2]; 2]> {
        match self {
            SpawnPolicy::Vanilla => Ok(vanilla),
            SpawnPolicy::SlippiNeutral => slippi_neutral_singles(stage_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "no Slippi Neutral Spawns singles entry for stage {stage_name:?}; known \
                     stages: {:?}",
                    SLIPPI_NEUTRAL_SINGLES
                        .iter()
                        .map(|(slug, _)| *slug)
                        .collect::<Vec<_>>()
                )
            }),
            SpawnPolicy::Explicit { spawns } => Ok(*spawns),
        }
    }
}

/// Lowercase, ASCII-fold and hyphenate a display name into the kebab-case
/// slug used as a key below (e.g. "Yoshi's Story" -> "yoshis-story"). A
/// deliberately independent copy of `crates/cli/src/initialization.rs`'s
/// `slug` (this crate must not depend on the `cli` crate); keep the two in
/// sync if the naming convention ever changes.
fn stage_slug(name: &str) -> String {
    let mut out = String::new();
    let mut prev_hyphen = false;
    for ch in name.chars() {
        let ch = match ch {
            '\'' | '\u{2019}' => continue,
            other => other,
        };
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_hyphen = false;
        } else if !prev_hyphen && !out.is_empty() {
            out.push('-');
            prev_hyphen = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// UnclePunch's Neutral Spawns table, singles rows
/// (`External/NeutralSpawn/NeutralSpawn.asm`, injection `8016e510`), as
/// exact `f32` literals. Only the first two entries of each stage's row are
/// kept: singles assigns participants to row index 0/1 by participant order
/// among active slots, the same convention `data.stage.spawns[player]`
/// already uses, and a two-player match never reaches indices 2/3.
///
/// Keyed by the same kebab-case slug `crates/cli/src/initialization.rs`'s
/// `STAGE_EXTERNAL_IDS` uses (external stage ids, for cross-reference: FD
/// 0x20, BF 0x1F, YS 0x08, DL 0x1C, FoD 0x02, PS 0x03).
pub const SLIPPI_NEUTRAL_SINGLES: &[(&str, [[f32; 2]; 2])] = &[
    ("final-destination", [[-60.0, 10.0], [60.0, 10.0]]),
    ("battlefield", [[-38.8, 35.2], [38.8, 35.2]]),
    ("yoshis-story", [[-42.0, 26.6], [42.0, 28.0]]),
    ("dream-land", [[-46.6, 37.2], [47.4, 37.3]]),
    ("fountain-of-dreams", [[-41.25, 21.0], [41.25, 27.0]]),
    ("pokemon-stadium", [[-40.0, 32.0], [40.0, 32.0]]),
];

/// UnclePunch's Neutral Spawns table, teams rows -- documented for
/// completeness alongside [`SLIPPI_NEUTRAL_SINGLES`], but currently unused:
/// team matches are not implemented (`ensure!(!start.is_teams, ...)` in
/// both `initialization::build` and `match_validation::validate`), so no
/// caller has a two-team match to classify or resolve against this table
/// yet.
pub const SLIPPI_NEUTRAL_TEAMS: &[(&str, [[f32; 2]; 4])] = &[
    (
        "final-destination",
        [[-60.0, 10.0], [-20.0, 10.0], [60.0, 10.0], [20.0, 10.0]],
    ),
    (
        "battlefield",
        [[-38.8, 35.2], [-38.8, 5.0], [38.8, 35.2], [38.8, 5.0]],
    ),
    (
        "yoshis-story",
        [[-42.0, 26.6], [-42.0, 5.0], [42.0, 28.0], [42.0, 5.0]],
    ),
    (
        "dream-land",
        [[-46.6, 37.2], [-46.6, 5.0], [47.4, 37.3], [47.4, 5.0]],
    ),
    (
        "fountain-of-dreams",
        [[-41.25, 21.0], [-41.25, 5.0], [41.25, 27.0], [41.25, 5.0]],
    ),
    (
        "pokemon-stadium",
        [[-40.0, 32.0], [-40.0, 5.0], [40.0, 32.0], [40.0, 5.0]],
    ),
];

/// Look up [`SLIPPI_NEUTRAL_SINGLES`]'s entry for a stage's display name
/// (e.g. `data.stage.name`, "Final Destination"/"Yoshi's Story"/...).
pub fn slippi_neutral_singles(stage_name: &str) -> Option<[[f32; 2]; 2]> {
    let slug = stage_slug(stage_name);
    SLIPPI_NEUTRAL_SINGLES
        .iter()
        .find(|(candidate, _)| *candidate == slug)
        .map(|(_, spawns)| *spawns)
}

/// Which known codeset (if either) a concrete, already-resolved spawn pair
/// happens to equal, for a given stage. Purely a read-only diagnostic:
/// classification never changes simulation behaviour, and "unknown" is an
/// expected, valid outcome (e.g. the console-era Dream Land gap
/// `docs/parity.md` documents), not a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnProvenance {
    /// Bit-exact with the resource pack's own `stage.spawns`.
    Vanilla,
    /// Bit-exact with [`SLIPPI_NEUTRAL_SINGLES`]'s entry for this stage.
    SlippiNeutral,
    /// Neither: a real recording, on some codeset not yet identified.
    UnknownCodeset,
}

impl SpawnProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            SpawnProvenance::Vanilla => "vanilla",
            SpawnProvenance::SlippiNeutral => "slippi_neutral",
            SpawnProvenance::UnknownCodeset => "unknown_codeset",
        }
    }
}

fn bits_eq(a: [[f32; 2]; 2], b: [[f32; 2]; 2]) -> bool {
    a[0][0].to_bits() == b[0][0].to_bits()
        && a[0][1].to_bits() == b[0][1].to_bits()
        && a[1][0].to_bits() == b[1][0].to_bits()
        && a[1][1].to_bits() == b[1][1].to_bits()
}

/// Classify `explicit` (a concrete, participant-ordered spawn pair, e.g. a
/// replay's own frame -123 post-frame positions) against `vanilla` (the
/// resource pack's own `data.stage.spawns` for the same pairing) and, when
/// `stage_name` resolves, [`SLIPPI_NEUTRAL_SINGLES`]. Vanilla is checked
/// first: for stages where the two tables coincide (e.g. Final
/// Destination), a genuinely vanilla recording is reported `Vanilla`, not
/// an ambiguous match against both.
pub fn classify_spawn_provenance(
    stage_name: &str,
    vanilla: [[f32; 2]; 2],
    explicit: [[f32; 2]; 2],
) -> SpawnProvenance {
    if bits_eq(explicit, vanilla) {
        return SpawnProvenance::Vanilla;
    }
    if let Some(neutral) = slippi_neutral_singles(stage_name)
        && bits_eq(explicit, neutral)
    {
        return SpawnProvenance::SlippiNeutral;
    }
    SpawnProvenance::UnknownCodeset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_vanilla_unchanged() {
        let vanilla = [[-1.0, 2.0], [3.0, -4.0]];
        assert_eq!(
            SpawnPolicy::Vanilla.resolve("anything", vanilla).unwrap(),
            vanilla
        );
    }

    #[test]
    fn resolves_explicit_unchanged() {
        let spawns = [[-1.0, 2.0], [3.0, -4.0]];
        assert_eq!(
            SpawnPolicy::Explicit { spawns }
                .resolve("anything", [[0.0, 0.0], [0.0, 0.0]])
                .unwrap(),
            spawns
        );
    }

    #[test]
    fn resolves_slippi_neutral_for_every_known_stage_slug() {
        for (slug, spawns) in SLIPPI_NEUTRAL_SINGLES {
            assert_eq!(
                SpawnPolicy::SlippiNeutral
                    .resolve(slug, [[0.0, 0.0], [0.0, 0.0]])
                    .unwrap(),
                *spawns
            );
        }
    }

    #[test]
    fn resolves_slippi_neutral_from_the_pack_display_name() {
        assert_eq!(
            SpawnPolicy::SlippiNeutral
                .resolve("Yoshi's Story", [[0.0, 0.0], [0.0, 0.0]])
                .unwrap(),
            [[-42.0, 26.6], [42.0, 28.0]]
        );
    }

    #[test]
    fn slippi_neutral_rejects_an_unrecognized_stage() {
        let error = SpawnPolicy::SlippiNeutral
            .resolve("Mushroom Kingdom", [[0.0, 0.0], [0.0, 0.0]])
            .unwrap_err();
        assert!(error.to_string().contains("Mushroom Kingdom"));
    }

    #[test]
    fn classifies_vanilla() {
        let vanilla = [[-60.0, 10.0], [60.0, 10.0]];
        assert_eq!(
            classify_spawn_provenance("Final Destination", vanilla, vanilla),
            SpawnProvenance::Vanilla
        );
    }

    #[test]
    fn classifies_slippi_neutral_when_distinct_from_vanilla() {
        let vanilla = [[-46.6, 37.221_5], [47.389_1, 37.321_503]];
        let neutral = [[-46.6, 37.2], [47.4, 37.3]];
        assert_eq!(
            classify_spawn_provenance("Dream Land", vanilla, neutral),
            SpawnProvenance::SlippiNeutral
        );
    }

    #[test]
    fn classifies_unknown_codeset() {
        // The measured console-era Dream Land gap: vanilla x, but a y
        // neither table produces (docs/parity.md, 2026-09-14).
        let vanilla = [[-46.6, 37.221_5], [47.389_1, 37.321_503]];
        let recorded = [[-46.6, 37.0], [47.389_1, 37.0]];
        assert_eq!(
            classify_spawn_provenance("Dream Land", vanilla, recorded),
            SpawnProvenance::UnknownCodeset
        );
    }

    #[test]
    fn classifies_unrecognized_stage_that_still_matches_vanilla() {
        let vanilla = [[1.0, 2.0], [3.0, 4.0]];
        assert_eq!(
            classify_spawn_provenance("Mushroom Kingdom", vanilla, vanilla),
            SpawnProvenance::Vanilla
        );
    }
}
