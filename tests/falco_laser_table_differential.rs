//! Whether Falco's laser is its own C item or shares Fox's, checked against
//! the pinned source rather than assumed: `melee/it/it_3F2F.c`'s per-item-
//! kind logic-table array (`it_803F2F2C`-equivalent; the pinned source has
//! no name for this specific table, only its element type,
//! `ItemLogicTable`) holds one stanza per `It_Kind_*` (`melee/it/
//! forward.h:180-181`: `It_Kind_Fox_Laser` immediately followed by
//! `It_Kind_Falco_Laser`), each stanza a fixed list of the item's own state
//! table pointer plus its `Logic94`-family callbacks (spawn/anim/phys/coll
//! dispatch, clank/reflect/absorb/shield-bounce/hit-shield/event hooks).
//! There is no `itfalcolaser.c` anywhere in this pinned decomp rev
//! (confirmed by an exhaustive grep of `src/melee` for `Falco.*[Ll]aser`
//! outside this file and `melee/it/forward.h`'s own enum) -- the "Falco
//! laser" stanza below is text-identical to the "Fox laser" stanza right
//! above it: same state table (`it_803F67D0`), same `itFoxLaser_Logic94_*`
//! callbacks, verbatim, not merely equivalent-looking. Falco's laser is the
//! *same* C item as Fox's, differing only in which `Article`'s own DAT-
//! sourced data (attributes, hitbox command stream) a given spawn reads --
//! exactly the same "data-only difference" `docs/falco.md` already
//! established for the side/up/down specials, now confirmed for the
//! neutral special's own fired item too. `docs/falco.md` and
//! `docs/fox-neutral-special.md` cite this test as the evidence; `game::
//! projectile::ProjectileKind::FalcoLaser` exists purely so observation/
//! replay code can label a spawned laser by its owner's own character (the
//! generic `game::projectile` collision/motion code has no `match` on kind
//! at all, matching this finding).
//!
//! Pinned whole-file (`tests/oracle/original/it_3F2F.c`, `sources.json`):
//! not itself run through the C-oracle extraction pipeline (no `.functions.
//! json` of its own -- the table is data, not a function, so there is
//! nothing to extract), only read directly by this file's own `include_
//! str!`, the same way `reflect_gate_differential.rs`'s own verbatim-excerpt
//! test reads an already-pinned snapshot without linking it.

const FOX_LASER_MARKER: &str = "        // Fox laser\n";
const FALCO_LASER_MARKER: &str = "        // Falco laser\n";
const STANZA_END: &str = "    },\n";

fn stanza_after<'a>(source: &'a str, marker: &str) -> &'a str {
    let after = source
        .split(marker)
        .nth(1)
        .unwrap_or_else(|| panic!("marker not found: {marker:?}"));
    after
        .split(STANZA_END)
        .next()
        .unwrap_or_else(|| panic!("stanza did not terminate: {marker:?}"))
}

/// The generic ray-item C code (`it_8029C504`/`Item_UpdateRayAnimation`,
/// already extracted by `itfoxlaser.functions.json` and exercised through
/// `tests/oracle/fox_laser.c`'s own `oracle_laser_spawn`/`oracle_laser_
/// motion`) reproduces Falco's own exported numbers exactly, not just
/// Fox's: `fox_laser_differential.rs`'s own `known_values_match`/proptest
/// blocks already fuzz `speed`/`angle` over ranges wide enough to cover
/// Falco's `5.0`/`0.0` (`fighters/falco.json`'s own `specials.neutral.
/// attributes`, the exporter's read of Falco's `PlFc.dat` Blaster fields),
/// but never assert Falco's exact values by name; this does. `kind_in`
/// (`It_Kind_Falco_Laser`, `forward.h:182`, the enum value immediately
/// after `It_Kind_Fox_Laser`'s exporter-confirmed `54`) and `lifetime_attr`
/// (`100.0`, also exporter-confirmed) are the two fields `compare_spawn`
/// in the sibling file hard-codes to Fox's own `54`/`35.0`; this test's own
/// `oracle_laser_spawn` call passes Falco's instead, over the same
/// spawn-position/angle-normalization/facing formula.
#[cfg(feature = "c-oracle")]
mod falco_known_values {
    #![allow(unsafe_code)]

    #[link(name = "skirmish_oracle", kind = "static")]
    unsafe extern "C" {
        fn oracle_laser_spawn(
            owner_x: f32,
            owner_y: f32,
            ecb_top: f32,
            ecb_bottom: f32,
            angle_in: f32,
            speed_in: f32,
            kind_in: i32,
            lifetime_attr: f32,
            out_pos_x: *mut f32,
            out_pos_y: *mut f32,
            out_angle: *mut f32,
            out_speed: *mut f32,
            out_facing_dir: *mut f32,
            out_lifetime: *mut f32,
        );
    }

    const FALCO_LASER_ITEM_KIND: i32 = 55; // `It_Kind_Falco_Laser`, forward.h:182.
    const FALCO_SPEED: f32 = 5.0; // `fighters/falco.json`'s `specials.neutral.attributes.speed`.
    const FALCO_ANGLE: f32 = 0.0; // ...`.attributes.angle`.
    const FALCO_LIFETIME: f32 = 100.0; // ...`.laser.lifetime`.

    #[test]
    fn falco_spawn_matches_his_own_exported_attributes() {
        let (owner_x, owner_y, ecb_top, ecb_bottom) = (12.0_f32, -4.0_f32, 8.0_f32, -2.0_f32);
        let (mut px, mut py, mut oa, mut os, mut facing, mut lifetime) =
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        unsafe {
            oracle_laser_spawn(
                owner_x,
                owner_y,
                ecb_top,
                ecb_bottom,
                FALCO_ANGLE,
                FALCO_SPEED,
                FALCO_LASER_ITEM_KIND,
                FALCO_LIFETIME,
                &mut px,
                &mut py,
                &mut oa,
                &mut os,
                &mut facing,
                &mut lifetime,
            );
        }
        // `ftLib_80086990`: the fighter's own ECB vertical midpoint, exactly
        // as Fox's own `compare_spawn` already checks -- the same generic
        // function, now with Falco's own numbers.
        assert_eq!(px, owner_x);
        assert_eq!(py, owner_y + 0.5 * (ecb_top + ecb_bottom));
        assert_eq!(oa, FALCO_ANGLE);
        assert_eq!(os, FALCO_SPEED);
        assert_eq!(lifetime, FALCO_LIFETIME);
        assert_eq!(facing, 1.0, "angle 0.0 is right-facing (< pi/2)");
    }
}

#[test]
fn fox_and_falco_laser_dispatch_stanzas_are_byte_identical() {
    let source = include_str!("oracle/original/it_3F2F.c");
    let fox = stanza_after(source, FOX_LASER_MARKER);
    let falco = stanza_after(source, FALCO_LASER_MARKER);
    assert_eq!(
        fox, falco,
        "the pinned source's own \"Fox laser\"/\"Falco laser\" logic-table \
         stanzas diverged -- if a decomp update genuinely gives Falco his \
         own laser item code, ProjectileKind::FalcoLaser needs its own \
         behavior, not just its own label"
    );
    // Every callback the shared stanza names is the exact set this port's
    // own oracle already extracts for Fox (`itfoxlaser.functions.json`),
    // confirming there is no *sixth* Falco-only callback silently skipped.
    for callback in [
        "it_803F67D0",
        "itFoxLaser_Logic94_Clanked",
        "itFoxLaser_Logic94_Reflected",
        "itFoxLaser_Logic94_Absorbed",
        "itFoxLaser_Logic94_ShieldBounced",
        "itFoxLaser_Logic94_HitShield",
        "itFoxLaser_Logic94_EvtUnk",
    ] {
        assert!(
            falco.contains(callback),
            "Falco laser stanza missing expected callback {callback}"
        );
    }
}
