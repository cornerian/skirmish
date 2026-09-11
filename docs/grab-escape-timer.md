# Grab escape timer: standings and handicap (design note)

Pinned decomp rev 0bac93a5. Skirmish flattens the capture escape timer into
`Rules.grab.escape.timer_base + percent * timer_percent_scale`
(`src/game/grab.rs:1040`). The source computes it from six common-data
constants plus two match-state inputs, so a real match cannot be reproduced
from a constant. This batch models the real formula and its inputs.

Sources: `src/melee/ft/kinds/ftCommon/ftCo_CapturePulled.c:18-33`
(`ftCo_800DA824`: `slot = standing + 1` (f32); `value = x364 - slot; value =
x360 * value; temp = x35C - handicap; temp = x358 * temp + x354; temp +=
value; return percent * x368 + temp` — keep this evaluation order in f32),
`ft/types.h:266-271` (`ftCommonData` x354..x368), `pl/player.c:936-939`
(`Player_80033BB8` = `gm_8016C5C0(slot)`), `gm/gmvs.c:971-984`
(`gm_8016C5C0`: refreshes the standings table when the frame changed, then
returns `x58[slot].x5` in non-team play, or the team's standing in teams),
`gm/gm_1601.c:2945+` (`gm_80166378`: fills `player_standings[i]` with stocks,
percent, falls, KOs, then ranks them: READ the ranking loop and reproduce
its ordering and tie rule exactly; the expected result for two players
with equal stocks and percent is that both hold standing 0),
`pl/player.c:855-870` (`Player_GetHandicap`), `mn/mncharsel.c:4290`,
`gm/gm_1601.c:3502`, `gm/gmmain_lib.c:833` (handicap is 9 whenever the
handicap rule is off, which is the case for the parity replay and for
tournament play; when on, each player carries their chosen 1..9 value).
Other readers of the same standing value, to share one helper:
`ft/ftCo_800C7590.c:44-52` and `ft/ftCo_800C78B0.c:46-54` (the two
`CaptureLeadead`/`Leadead` variants: `x728 * (x72C - (standing + 1))` plus
`x720 * (x724 - handicap) + x71C`, then `percent * x730`),
`ft/kinds/ftCommon/ftCo_DamageBind.c:40-46`.

## Behaviour
- `standing(state, player)` computed on demand from the current match
  state per `gm_80166378`'s ordering (stocks, then percent, then whatever
  the loop uses next; ties share the better standing). Two-player only for
  now, but write it over `state.fighters`.
- `handicap(player)`: from a new per-player match setting (default 9).
- Capture timer = the source formula with those inputs; `timer_base` stays
  as the legacy path when the new formula fields are absent, so existing
  fixtures are untouched.
- The Leadead and DamageBind readers are out of scope (no Kirby/item
  captures modeled); cite them in the doc as sharing the helper later.

## Resource shape
`Rules.grab.escape.formula: Option<EscapeFormula { base: f32 (x354),
handicap_scale: f32 (x358), handicap_max: f32 (x35C), rank_scale: f32
(x360), rank_max: f32 (x364), percent_scale: f32 (x368) }>`; when present
it takes precedence over `timer_base`/`timer_percent_scale` (validate both
ways). `MatchData.players: Option<[PlayerSettings { handicap: u8 }; 2]>`
(default handicap 9; `make-initialization` may later fill it from the
replay's player settings if Slippi records them; check `peppi`'s game
start for a handicap field and use it when present). The exporter already
writes the six constants to its `rules.json` sidecar as
`grab.escape_formula`; after this batch it embeds them.

## Oracle
Pin `ftCo_800DA824` (extract from `ftCo_CapturePulled.c`; stub
`Player_80033BB8`/`Player_GetHandicap` with scripted values) and compare
the timer bit-exactly across percent, handicap 1..9 and standings 0..3.

## Tests
Standing ordering and ties from stocks/percent; the formula at the
replay's settings (handicap 9, standing 0) equals the flattened value the
exporter wrote (75.0 for Fox's data: verify by decoding, do not hardcode
without the derivation); legacy path unchanged; validation of the new
fields; a self-recorded replay regression where a grab at differing
stocks yields a different escape timer.

## Implemented (this batch)

`ftCo_800DA824` itself is `crate::fighter::grab::escape_timer` (pure f32
math, `src/fighter/grab.rs`), keeping the source's exact evaluation order.
`crate::game::grab::capture_timer` (`src/game/grab.rs`) calls it with
`escape.formula`'s six constants plus this frame's `standing`/`handicap`
when `formula` is `Some`, otherwise keeps the legacy flattened
`timer_base + percent * timer_percent_scale` path unchanged.
`Rules.grab.escape: EscapeRules` gained `formula: Option<EscapeFormula>`
(the six named constants, `base`/`handicap_scale`/`handicap_max`/
`rank_scale`/`rank_max`/`percent_scale` for `x354`/`x358`/`x35C`/`x360`/
`x364`/`x368`); `grab::validate` checks the six fields are finite and that
their worst-case combination (percent 999, any handicap/standing) stays
under the same 1,000,000 ceiling the legacy path already enforces.

`MatchData` gained `players: Option<[PlayerSettings; 2]>`
(`PlayerSettings { handicap: u8 }`, default 9), validated to `1..=9` when
present; `crate::game::grab::handicap` reads it, defaulting to 9 (handicap
rule off) when the field is absent, matching `mn/mncharsel.c:4290`/
`gm/gm_1601.c:3502`/`gm/gmmain_lib.c:833`. `crates/cli/src/initialization.
rs::build` now unconditionally fills `data.players` from the replay's
`GameStart.players[_].handicap` (peppi 2.1.2's `game::Player::handicap` is
a required `u8`, not gated by Slippi version, so this is always a real
recorded value, not a placeholder).

**The standing helper and what it actually reads.** `gm_8016C5C0`
(`gm/gmvs.c:971-984`) returns `x58[slot].x5` — but this file's own field
names are raw byte offsets into the *same* layout `gm_1601.c` names
`struct MatchPlayerData` (`gm/types.h:638-692`), and offset `0x5` there is
`is_big_loser`, not a distinct `x5` field. `is_big_loser` is filled by
`fn_80165AC0` (`gm_1601.c`, right after `gm_80166378`'s own population
loop at `:2945+`): for player `i`, it counts opponents `j` whose `score`
is *strictly* greater than `i`'s own, so 0 is best and a tie leaves both
players at 0 (matching the note's own worked example). `score` itself
comes from `fn_8016588C`, which branches on match kind
(`MatchKind_Time`/`Stock`/`Coin`/`Bonus`, `gm_1601.c:2742-2772`); Skirmish
only models `MatchKind_Stock` (`MatchData.rules.stocks`), whose branch
(`arg0->x5 == 1`) is: remaining stocks if nonzero, else a deeply negative
sentinel (`frame_count / 60 + 0xFF000001`, sign-extended, clamped to
`+/-(2^24 - 1)`) so a longer-lived eliminated player still outranks an
earlier one.

**Correction against the design note above.** The note's phrasing
("`gm_80166378`... fills `player_standings[i]` with stocks, percent,
falls, KOs, then ranks them") reads as if `percent` participates in the
tie rule. It does not, for Stock matches specifically: `percent` (and
falls/KOs) are populated into `player_standings` for the *other* match
kinds' own `score` branches (Time's falls/KOs-weighted score, Coin's coin
count) and for the end-of-match results screen, but the Stock branch
`fn_8016588C` actually takes at `arg0->x5 == 1` reads only `stocks` (and,
once eliminated, survival time) — never `percent`. The note's own worked
example (equal stocks *and* percent give standing 0 for both) still holds
under this reading, since it does not depend on percent to reach that
result; this batch's `standing()` (`src/game/grab.rs`) implements the
Stock branch only, and its own doc comment records this correction.
Because Skirmish ends a match once a fighter reaches zero stocks, the
elimination branch is coded (for parity with the source) but unreachable
from a live grab.

`standing` and `handicap` take plain `[u8; 2]`/`u32`/`Option<&
[PlayerSettings; 2]>` rather than the full `MatchState`/`MatchData`, so
they are unit-tested directly in `src/game/grab.rs`'s own `#[cfg(test)]`
module (ties, ordering) without constructing a full `Fighter`.

**Oracle.** `tests/oracle/escape_formula.c` (alias `escape_formula ->
capture_pulled` in `tests/oracle/adapters.json`, reusing the existing
`ftCo_CapturePulled.c` snapshot) stubs `Player_80033BB8`/
`Player_GetHandicap` as scripted `_Thread_local` globals and exposes
`oracle_escape_formula`. `tests/escape_formula_differential.rs` compares
it bit-for-bit against `escape_timer` across arbitrary `f32` constants,
percent in `0.0..999.0`, handicap `1..=9` and standing `0..=3`.

**Real constants confirmed.** Fox's real `ftCommonData` constants, already
extracted to `/mnt/archive/datasets/melee/skirmish-gameplay/v2/rules.json`
(`grab.escape_formula`) by the exporter: `base` (x354) 30.0,
`handicap_scale` (x358) 8.0, `handicap_max` (x35C) 9.0, `rank_scale`
(x360) 15.0, `rank_max` (x364) 4.0, `percent_scale` (x368) 1.6. At the
parity replay's settings (handicap 9, standing 0), the formula reduces
exactly to `75.0 + percent * 1.6`, matching the flattened `timer_base`/
`timer_percent_scale` this same exporter already wrote for Fox — verified
by direct derivation in `src/fighter/grab.rs`'s
`escape_timer_at_the_real_fox_constants_and_replay_settings_matches_the_
flattened_value` test, not hardcoded. That JSON dataset directory does
not itself embed `escape_formula` into a `MatchData`'s `rules.grab.escape`
yet (only into the standalone `rules.json` sidecar); wiring it into the
exported `match-data.json` is exporter-side follow-up, out of scope here.

`crates/cli/tests/replay_match.rs` gains
`physical_z_drives_file_backed_grab_capture_at_differing_stocks_with_the_
real_formula`: fighter 0 jabs fighter 1 into the top blast zone (costing
exactly one stock), then grabs it once it returns to active play; with
the real Fox constants installed, the grabbed fighter's `escape_timer`
matches `escape_timer(..., standing=1, handicap=9)` and differs from the
equal-standing (0) value, and the recorded frames still match their own
re-encoded bytes.
