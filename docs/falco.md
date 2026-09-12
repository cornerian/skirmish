# Falco: registering a second character on the shared Fox/Falco move code

Pinned decomp rev `0bac93a5`. This batch makes Falco a playable character
(`game::characters::Specials::Falco`) on top of the side/up/down special
code Fox already has (`game::characters::fox::{side,up,down}`), so an
exported Falco pack (`fighters/falco.json`) loads and Falco recordings can
be measured against it. It does not add any new move behavior: every
Falco-carrying source file cited below already dispatches through Fox's own
callbacks, so this is wiring and data, not new game logic.

## What Falco shares vs. what is his own

`ftFc_Init_MotionStateTable[ftFx_MS_SelfCount]` (`src/melee/ft/kinds/
ftFalco/ftfalco.c:23-370`) is Falco's own motion-state table, and every one
of its ~30 special-move entries (`ftFx_MS_SpecialNStart`..`ftFx_MS_
SpecialAirLwTurn`, ids 341-369) points at the *identical* `ftFx_Special*_
{Anim,IASA,Phys,Coll}` callback Fox's own table (`ftfox.c`) uses — not a
`ftFc_*` equivalent. `ftFc_Init_LoadSpecialAttrs` (`ftfalco.c`) forwards
straight to `ftFx_Init_LoadSpecialAttrs`, and `ftFc_Init_OnLoad`'s call to
`ftFx_Init_OnLoadForFalco` (`ftfox.c:481-483`) is just `PUSH_ATTRS(fp,
ftFox_DatAttrs)` — the same attribute struct shape Fox's own `ftFx_Init_
OnLoad` uses, loaded from Falco's own `PlFc.dat` instead of Fox's `PlFx.dat`.
So Falco's side, up and down specials are code-identical to Fox's; only the
numeric attributes differ, which is exactly why `game::characters::
Specials::Falco` reuses `fox::side::SideSpecial`/`fox::up::UpSpecial`/
`fox::down::DownSpecial` verbatim as its own field types rather than
defining Falco-specific copies (see that enum's own doc comment in
`src/game/characters/mod.rs`).

The one exception found by grepping every `FTKIND_FALCO` branch under
`src/melee/ft/kinds/ftFox/`: `ftFox_SpecialS_CreateGhostItem`
(`ftfoxspecials.c:247-262`) spawns `It_Kind_Fox_Illusion` for Fox and
`It_Kind_Falco_Phantasm` for Falco. That item is confirmed hitbox-free and
GFX-only (`fox::side`'s own module doc: every one of its `Coll` callbacks
unconditionally returns `false`) and stays entirely unmodeled here, so this
is a purely cosmetic difference with no observable effect on this
simulator. The up special (Fire Bird, `ftfoxspecialhi.c`) and down special
(Reflector, `ftfoxspeciallw.c`) have no `FTKIND_FALCO`/`FTKIND_FOX` branch
at all — genuinely one shared implementation, not a coincidence of this
port's own abstraction.

Falco's neutral special (Laser, `It_Kind_Falco_Laser`) is now wired, as of
the 2026-09-13 batch covered in its own section below
("Falco's neutral special (Laser): now wired").

## Registry wiring

- `game::characters::Specials` gains a `Falco { neutral, side, up, down }`
  variant, structurally identical to `Fox`'s own. Its accessor methods
  (`neutral`/`fox_side`/`fox_up`/`fox_down`) now match either variant, so
  every existing call site in `game::specials`'s move dispatch and
  `game::validation` (already written against those accessors, never
  against `Specials::Fox` directly) needed no change at all.
- `game::characters::moves` dispatches `Specials::Falco` to the same
  `fox::MOVES` slice `Specials::Fox` uses.
- `game::characters::fox::CHARACTER_IDS` (the external Slippi CSS ids that
  resolve through `fox::slippi_ids`) gains Falco's own id, 20
  (`crates/cli/src/initialization.rs`'s `CHARACTER_EXTERNAL_IDS`) alongside
  Fox's 2. Falco's *internal* fighter kind, `FTKIND_FALCO`
  (`ft/forward.h:112`), is the unrelated number 22 — a stale doc comment in
  this same file previously conflated the two (calling Falco "22" while
  describing the external-id table); this batch corrects it. A test in
  `crates/skirmish-replay/src/observation.rs` had the identical mix-up in
  its own comment (calling an *unregistered* external id "Falco 22" when 22
  is actually Dr. Mario's external id); also corrected, with a Falco (20)
  case added alongside the existing Fox (2) one.
- The tag on `Specials` itself (`#[serde(tag = "character")]`) needed a
  `#[serde(alias = ...)]` on both variants: `fighters/fox.json`'s own
  standalone sample writes `specials.character` capitalized (`"Fox"`), but
  every export that also carries a Falco fighter (`fighters/falco.json`,
  and the composed `fox-falco-fd`/`falco-fox-fd` pairing `match-data.json`
  files) writes it lowercase for *both* characters (`"fox"`/`"falco"`).
  Rather than have the exporter re-normalize already-published packs, both
  spellings are now accepted.

## Validation and tests

- `tests/game_falco_specials.rs` builds a synthetic match (reusing the
  existing invented `fox-{side,up,down}-special.json` fixtures, wired
  through `Specials::Falco` instead of `Specials::Fox`) and checks: a Falco
  fighter loads into a real `Match`; each of the four specials (including
  `neutral`, wired since the 2026-09-13 batch below, previously `None`)
  dispatches from a Falco fighter exactly as it does from a Fox one; and
  `game::characters::slippi_ids` resolves the same `(state, animation)`
  pair for external ids 2 (Fox) and 20 (Falco) on every special action,
  while an unregistered id (22, Dr. Mario) resolves nothing. This exercises
  the wiring, not Falco's own real numeric attributes — that is the
  real-replay ratchet's job below, and (for `neutral` specifically) `tests/
  game_falco_neutral_special.rs`'s own end-to-end coverage (see "Falco's
  neutral special (Laser): now wired" further down).
- `crates/cli/tests/real_parity_falco_fox_fd.rs` runs `make-initialization`
  against the exported `falco-fox-fd` pairing and a real Falco-vs-Fox Final
  Destination recording (`tests/fixtures/slippi/parity/falco-fox-fd.slp`,
  `12_45_21 Falco + [HAMB] Fox (FD).slp` from the CC0-1.0 `erickfm/
  slippi-public-dataset-v3.7` corpus, ports P3/P4, Falco first), then
  ratchets `validate-replay --report`'s first divergent frame against
  `falco-fox-fd-baseline.json`. Skips (does not fail) without
  `SKIRMISH_GAMEPLAY_DATA`, per `docs/gameplay-export.md`. This is the
  first parity measurement for any Falco recording: 93 frames match
  (-123 through -31, the pre-game Entry warp-in), and the first divergence
  is frame -30, P4 (Fox), field `action_age` (expected `1.0`, actual
  `0.0`) — see the baseline file's own note for what was and was not ruled
  out diagnosing it; it was not chased further in this batch, since it
  looks unrelated to Falco's own move data (both fighters' `entry.
  start_frames` are identical, and it is not simply a non-P1-seating
  artifact, since `real_parity_fox_fd_4.rs`'s own P2/P4 pairing matches much
  further).

## What the registration batch did not claim

No new gameplay behavior was added or changed for either character by the
registration batch above; it was purely registering a second character onto
already-existing, already-tested move code. `docs/fox-side-special.md`/
`fox-up-special.md`/`fox-down-special.md`'s own "Fox/Falco" titling already
anticipated this. The real-replay measurement's frame -30 divergence is
reported, not fixed.

## Falco's neutral special (Laser): now wired

Pinned decomp rev `0bac93a5`, same as every citation above. Gameplay export
v10 (`/mnt/archive/datasets/melee/skirmish-gameplay/v10-snapshot-20260913/
fighters/falco.json`) carries `specials.neutral` for Falco for the first
time, in the exact same schema `characters::fox::neutral::NeutralSpecial`
already defines for Fox (`docs/fox-neutral-special.md`): this batch wires
it through `Specials::Falco.neutral`, no schema change needed.

### The laser item: one C item, not two

The task brief for this batch expected a distinct `itfalcolaser.c` (by
analogy with the side special's own Fox/Falco-specific ghost item). Reading
the pinned decomp shows this is not the case, in three independent places:

- **No such file exists.** An exhaustive `grep -rl` of the whole pinned
  `src/melee` tree for `Falco.*[Ll]aser` outside `melee/it/forward.h`'s own
  enum finds nothing but a stray comment in `ftKirby/ftkirby.c` (Kirby's
  copy ability) and a comment in `melee/it/it_3F2F.c` (see next point) — no
  `.c`/`.h` file of Falco's own for the laser item anywhere.
- **`melee/it/it_3F2F.c`'s own per-item-kind logic table** (lines 330-349 of
  the pinned snapshot, now itself pinned whole as `tests/oracle/original/
  it_3F2F.c`, `sources.json`) gives `It_Kind_Fox_Laser` and
  `It_Kind_Falco_Laser` (`melee/it/forward.h:181-182`, adjacent enum
  values) **byte-identical stanzas**: both list `it_803F67D0` (the same
  state table) and the same seven `itFoxLaser_Logic94_*` callbacks
  (clank/reflect/absorb/shield-bounce/hit-shield/event), verbatim, comment
  aside. `tests/falco_laser_table_differential.rs`'s own
  `fox_and_falco_laser_dispatch_stanzas_are_byte_identical` extracts both
  stanzas from the pinned snapshot and asserts they match textually — a
  real, automated regression against a future decomp update actually
  splitting the two, not merely an assertion in prose.
- **`Article::x4_specialAttributes`'s own struct** (`FoxLaserAttr`,
  `melee/it/itCharItems.h:265-276`) has no Falco-specific counterpart
  either: it is reused verbatim as the specialAttributes type for whichever
  Article a given `Item_Kind` resolves to, Fox's and Falco's laser alike,
  each simply reading its own DAT-sourced instance of the same ten-float
  layout.

So Falco's Laser is not a second implementation of the fired item at all —
it is the *same* C item Fox's laser already is, spawned with a different
`Item_Kind` constant (`It_Kind_Falco_Laser = 55`, confirmed both from
`forward.h`'s declaration order immediately after `It_Kind_Fox_Laser`'s
exporter-confirmed `54`, and independently from a real recording's own
`sid.Item.FALCO_LASER == 55` in py-slippi's item-type enum, see "Real-
recording cross-check" below) purely so the spawn can reach Falco's own
`Article` (his own DAT-sourced attributes/hitbox command stream), plus a
cosmetic SFX pick (`ftFx_SpecialN_FireBlasterShot`'s only `FTKIND_FALCO`
branch, `foxSFX`/`falcoSFX`, `ftfoxspecialn.c:566-585`) — unmodeled, exactly
like every other purely-cosmetic Fox/Falco difference this project has
already found (the side special's ghost item, `docs/fox-side-special.md`).
`ftFox_SpecialN_FireBlasterShot`'s `switch (ftLib_GetKind(gobj))` on
`FTKIND_FOX`/`FTKIND_FALCO` is the only Falco-kind branch anywhere in
`ftfoxspecialn.c` that actually executes for the neutral special's own fired
item (the task brief's cited `ftfoxspecialn.c:185,573-679` line range spans
this SFX switch and the surrounding cosmetic-gun `ftFx_Throw_Anim` code
already covered by `docs/fox-neutral-special.md`'s own "gun model, cosmetic"
section — nothing there is laser-hitbox logic).

`game::projectile::ProjectileKind` gains a `FalcoLaser` variant purely as an
observation/replay label (`characters::fox::neutral::drain_pending_shot`
now checks whether `data.specials` is `Specials::Falco` and spawns that
kind instead of `FoxLaser`); every function in `game::projectile` was
already generic over `kind` before this batch and needed no change, matching
the finding above.

### The one genuine gameplay difference: attributes and hitbox data

Falco's laser is data-different from Fox's in exactly the ways
`characters::fox::neutral::{Attributes,Laser}` already has fields for
(`fighters/falco.json`, gameplay export v10):

| Field | Fox | Falco |
|---|---|---|
| `attributes.angle` | `0.0` | `0.0` |
| `attributes.speed` | `7.0` | `5.0` (slower) |
| `attributes.landing_lag` | `0.0` | `0.0` |
| `laser.lifetime` | `35.0` frames | `100.0` frames (much longer) |
| hitbox `growth`/`fixed`/`base` (all four) | `0`/`0`/`0` | `100`/`5`/`0` |
| hitbox `damage` (all four) | `3` | `3` |
| hitbox centers/radii (ids 0-2) | `x=-0.7812,-3.6442978,-6.5073957`, radius `1.1718` | identical |
| hitbox id 3 (center/radius) | `x=-14.0616`, radius `1.5624` (widened) | `x=-9.3744`, radius `1.1718` (uniform, shorter reach) |
| `neutral_thresholds` | `[0.6, 0.55]` | `[0.6, 0.55]` |

The nonzero `growth`/`fixed` is the one field this batch's own read of the
source (`docs/fox-neutral-special.md`'s "Where the hitbox numbers live")
already flagged as necessarily data, not code: `ftColl`/`ftfoxspecialn.c`
never distinguish Fox's and Falco's laser hitbox values, they live purely
in each character's own Article command stream. This matches Melee
community knowledge that Falco's laser flinches noticeably harder than
Fox's (real, if modest, knockback/hitstun vs. Fox's pure flinch); with this
project's own invented `tests/fixtures/game/integration-match.json` rules
(not real Melee constants), the resulting knockback vector is nonzero but
does not itself cross the universal one-frame hitstun minimum
(`fighter::combat::initial_hitstun`) — `tests/
game_falco_neutral_special.rs`'s own `falcos_laser_hits_with_real_knockback_
unlike_foxs_all_zero_laser` proves the vector, not a specific frame count,
for exactly this reason.

### Real-recording cross-check

`tests/fixtures/slippi/parity/falco-fox-fd.slp` (Slippi `2.0.1`, confirmed
directly: `any(f.items for f in game.frames)` is `False` throughout)
predates Slippi's own item-event support and cannot be used for this.
Scanning `/mnt/archive/datasets/melee/slippi-public-dataset-v3.7/data/
FALCO/batch_00/` and `batch_01/` for Slippi `>= 3.0` recordings with any
item frames at all (py-slippi, installed into a scratch `uv venv`, not
committed) finds five candidates, all Slippi `3.9.0`; `19_39_37 Falco + Fox
(DL).slp` (Falco vs. Fox, Dreamland) has 16 independent `FALCO_LASER`
(`sid.Item.FALCO_LASER == 55`, confirming `It_Kind_Falco_Laser`'s numeric
value independently of the decomp's own declaration-order count) spawn
instances. Confirms, independent of source-reading:

- **Speed**: every grounded/level shot's `velocity` is exactly `(5.0, 0.0)`
  (or `(-5.0, -0.0)` facing left) for its entire flight — `attributes.speed
  == 5.0`, exactly the exporter's own value, now doubly confirmed.
- **Motion**: `position` advances by exactly `velocity` every frame, no
  drift — the plain `position += velocity` model, same as Fox's own laser.
- **Lifetime**: every instance's first-observed `timer` is `99.0`,
  decrementing by exactly `1.0`/frame — consistent with a **100-frame**
  lifetime and matching the exporter's own `laser.lifetime == 100.0` (the
  same off-by-one-on-first-observation pattern Fox's own `34.0`-for-35-frame
  cross-check already established).
- **The same open "laser angling" question Fox's own doc already flagged**:
  one instance (`spawn_id 35`) has an oblique, non-axis-aligned velocity,
  `(1.4139, -4.7959)`, magnitude exactly `5.0` (i.e. still Falco's own
  confirmed speed, just not fired level) — this batch did not find the
  source function that produces this either (see `docs/
  fox-neutral-special.md`'s own "Open question, not resolved"); now
  independently reproduced for Falco, so it is a real, character-
  independent gap in this project's own source-reading, not a Fox-specific
  puzzle.
- **A discrepancy worth flagging for the concurrent hit-registration
  diagnosis, not fixed here**: that same oblique instance's `timer` jumps
  from `97.0` directly to `1.0` on its very next observed frame, then the
  item disappears the frame after. Reading `itFoxlaser_UnkMotion1_Coll`
  (`itfoxlaser.c:98-107`, pinned) shows why: a terrain hit
  (`it_8029C4D4`/`it_8026E9A4`) does not despawn the item immediately, it
  calls `it_80275158(item_gobj, 1.0F)` (set remaining lifetime to exactly
  one frame) and restores the item's pre-collision `pos`, letting it exist
  one more frame before the ordinary lifetime-expiry check removes it.
  `game::projectile::step` (`src/game/projectile.rs`, the file `fox-fd-2`'s
  own concurrent batch is diagnosing) instead returns `Outcome::Despawn`
  immediately on a terrain-line hit, with no one-frame grace period. This
  batch did not change that shared code (out of this batch's own scope,
  per the task brief), but records the finding here and would point any
  future hit-registration work at `itfoxlaser.c:98-107` verbatim before
  assuming the immediate-despawn model is exact.
- Real recordings this batch checked did not happen to include a captured
  fighter-hit instance (`item.damage` stayed `0` in every one of the 16
  spawn instances sampled from this file) to independently cross-check the
  `3%` damage value against — Fox's own doc already established `3%` from
  a different real recording (`fox-fd-3.slp`'s own `FOX_LASER` instances),
  and this port's exported hitbox data (`damage: 3` for both characters)
  matches that value for Falco too; not independently re-confirmed from a
  Falco-side recording in this batch, reported rather than chased further.

### Tests

`tests/game_falco_neutral_special.rs` (new): the shared state machine
dispatches through `Specials::Falco`, a spawned laser is tagged `Projectile
Kind::FalcoLaser` (not `::FoxLaser`), travels at Falco's own slower `5.0`
speed, hits with the same `3%` damage and no piercing but a genuinely
nonzero knockback vector unlike Fox's all-zero laser, and still bounces off
a shield (a light smoke test — the shield-bounce/Reflector paths are
entirely generic over `Projectile`, already fully covered through Fox by
`tests/game_fox_neutral_special_reflect.rs`, and this batch's own table-
identity test above confirms the pinned source treats both characters'
lasers identically at the C level, so this batch does not re-derive that
whole suite a second time for Falco). `tests/game_falco_specials.rs`'s own
fixture now wires `neutral` (previously `None`); its "no neutral special"
test became `falco_plays_his_own_neutral_special`. `tests/
falco_laser_table_differential.rs` is the new C-oracle work: the table-
identity check above (ungated, pure text comparison, no linking needed) and
a `#[cfg(feature = "c-oracle")]` known-values check that the already-pinned
generic laser-spawn oracle function (`itfoxlaser.functions.json`'s own
`it_8029C504`, exercised through the existing `tests/oracle/fox_laser.c`
adapter) reproduces Falco's own exported spawn attributes exactly, not just
Fox's (the existing `fox_laser_differential.rs` proptest ranges already
happened to cover Falco's numbers without asserting them by name).

### Known gaps

- The terrain-despawn one-frame-grace discrepancy above is reported, not
  fixed, per this batch's own scope (the shared `game::projectile::step`
  hit/terrain pipeline, not the Falco-specific data path).
- The "laser angling" open question (`docs/fox-neutral-special.md`) remains
  open for both characters; this batch's own laser still only fires at the
  fixed per-facing angle.
- Falco's own real-recording cross-check did not happen to capture a
  fighter-hit instance; the `3%` damage value is exporter- and Fox-
  recording-confirmed, not independently re-confirmed from a Falco-side
  recording.
- `neutral_thresholds` (`[0.6, 0.55]`, identical to Fox's own) and the
  `script`-driven arming/fire-timing trace (`fighters/falco.json` carries
  its own `specials.neutral.script`, structurally the same as Fox's) are
  wired and validated but not independently cross-checked against a real
  Falco recording's own `state_age` the way Fox's `fox-fd-3.slp` cross-check
  did (`docs/fox-neutral-special.md`); `falco-fox-fd.slp`'s own Slippi
  `2.0.1` format cannot supply that (no item events at all), and this batch
  did not locate a suitable newer Falco recording with matching enough
  conditions to isolate a single Loop cycle's own `state_age` at spawn the
  way the Fox cross-check did. Reported, not chased further.
