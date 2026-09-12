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

Falco's neutral special (Laser, `It_Kind_Falco_Laser`, with Falco-kind
branches of its own in `ftfoxspecialn.c:185,573-679`) is **not wired** in
this batch: `fighters/falco.json`'s `specials` object carries no `neutral`
key from the exporter yet, and neither does the paired `fighters/fox.json`
sample — the neutral shell (`game::specials::neutral`) exists in this
codebase but no pack yet supplies data through it for either character. The
`Specials::Falco` variant still carries a `neutral: Option<neutral::
Parameters>` field, parallel to `Fox`'s own, so wiring it later (once a
pack supplies the data) needs no shape change here.

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
  fighter loads into a real `Match`; each of the three shared specials
  dispatches from a Falco fighter exactly as it does from a Fox one; the
  fixture's `neutral` field is `None`; and `game::characters::slippi_ids`
  resolves the same `(state, animation)` pair for external ids 2 (Fox) and
  20 (Falco) on every special action, while an unregistered id (22, Dr.
  Mario) resolves nothing. This exercises the wiring, not Falco's own real
  numeric attributes — that is the real-replay ratchet's job, below.
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

## What this batch does not claim

No new gameplay behavior was added or changed for either character; this is
purely registering a second character onto already-existing, already-tested
move code. `docs/fox-side-special.md`/`fox-up-special.md`/
`fox-down-special.md`'s own "Fox/Falco" titling already anticipated this.
Falco's neutral special is unimplemented pending exporter data (above), and
the real-replay measurement's frame -30 divergence is reported, not fixed.
