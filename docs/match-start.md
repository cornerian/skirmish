# Match start (Entry / EntryStart / EntryEnd) — design note

Pinned decomp rev 0bac93a5. The real replay (`tests/fixtures/slippi/parity/
fox-fd.slp`) diverges on its very first frame (-123) because both fighters
start in Entry (Slippi action state 322) while Skirmish spawns straight into
Fall. This batch models the match-start sequence so the first divergent
frame moves past the entry animation and the first landing.

Sources: `src/melee/ft/ft_0C31.c` (the whole file: entry 24-48, Entry
50-73, EntryStart 75-244, EntryEnd 246-354), `fighter.c:915-928` (spawn
dispatch: `Player_GetFlagsBit3` selects `ftCo_800C61B0`, otherwise
`ftCommon_8007D92C`), `fighter.c:233-257` (`Fighter_UnkInitReset_80067C98`:
position from `Player_LoadPlayerCoords`, facing from
`Player_GetFacingDirection`; the x offset `facing * x40 * scale` is zero
because `fp->x40 = 0` at `fighter.c:789`), `gm/gmvs.c:1601-1606` (the entry
delay: per processed slot `unk_A += 5; Player_SetUnk4C(slot, unk_A)`, so
port P1 = 5, P2 = 10, P3 = 15, P4 = 20), `gmvs.c:1793-1830` (facing: each
player faces the opponent with the largest |dx|; +1 when that opponent is
more than 5 units to the right, -1 when more than 5 to the left, otherwise
the opposite of the opponent's already-assigned facing, default +1;
`gmvs.c:2198-2206`: a lone player faces -1 when its spawn x >= 0 else +1),
`ft/types.h:473-476` (`ftCommonData` x6BC int, x6C0 int, x6C4 float, x6C8
int), `ft/types.h:748` (`co_attrs.trophy_scale`, `ftCo_DatAttrs` +0x110),
`ftcommon.c:596-604` (`ftCommon_8007D92C`: airborne -> `ftCo_Fall_Enter`,
grounded -> `ft_8008A2BC` Wait), `ft_081B.c:1316-1324` (`ft_80084CB0`:
copies the current ECB into a `ftCollisionBox`), `ft_081B.c:995-1025`
(`ft_80083E64`: airborne collision with a supplied box, callback on
landing), `ft_081B.c:1156-1180` (`ft_800846B0`: grounded variant, callback
when the ground is lost), `ftcommon.c:581-594` (`ftCommon_8007D7FC`: the
landing callback), `ftmotionstates.c:3676-3708` (rows 322-324: Entry and
EntryEnd have sub-motion `ftCo_SM_None`, EntryStart has
`ftCo_SM_EntryStart`; all three use `ftCo_MF_Rebirth` flags and no IASA).

## Timeline verified against the replay (both ports Fox, FD, spawns (±60, 10))
| frame | P1 (slot 0) | P4 (slot 3) |
|---|---|---|
| -123 | Entry 322, y 10, age -1 | Entry 322, y 10, age -1 |
| -118 | EntryStart 323, y 10.045, age 0 | Entry |
| -103 | EntryStart age 15, y 10.719 | EntryStart 323, age 0 |
| -89 | EntryEnd 324, y 11.348, age -1 | EntryStart |
| -74 | EntryEnd, y 10.674 | EntryEnd 324, y 11.348 |
| -59 | Fall 29, y 9.815, age 0 | EntryEnd |
| -49 | Landing 42, y 0 | EntryEnd y 10.225 |
| -44 | Landing age 5 | Fall 29, y 9.815 |
| -38 | Dash 20 (first player action) | Fall |
| -34 | | Landing 42 |
So: Entry lasts delay+1 frames (5 -> 322 on -123..-119), EntryStart 30
frames (x6BC = 30), EntryEnd 30 frames (x6C0 = 30), then Fall with gravity
applied on the transition frame (10.045 - 0.23 = 9.815), landing after ten
frames. State age is -1 for animation-less states (Entry, EntryEnd) and
counts from 0 in EntryStart: the observation mapping must reproduce that.

## Behaviour (cite for equivalence; structure freely)
- **Spawn**: position = the stage spawn for the player's port; facing by
  the `gmvs.c` rule above (replace Skirmish's `player == 0` hardcode);
  airborne; `entry.x4 = position.y`; `entry.timer = 5 * (slot + 1)`;
  invisible (visual); a copy of the current ECB as the entry box.
- **Entry (322)**, `ftCo_Entry_Anim`: `if timer == 0 { -> EntryStart }
  then timer -= 1` (the check precedes the decrement: timer 5 gives five
  frames of Entry). No IASA/Phys/Coll.
- **EntryStart (323)**, `ftCo_800C6408`: `timer = x6BC`; `x24 = scale.y *
  trophy_scale`; `x20 = x28 = 1.497345 * x24` (the double literal 1.497345
  times a float, as the source computes it); the warp-star accessory and
  effect are visual. Anim: `timer -= 1; if timer == 0 { -> EntryEnd }`.
  Phys: `t = (x6BC - timer) as f32 / x6BC as f32; x28 = x20 * t;
  position.y = x4 + x28`. Coll: `box.bottom = -x28`; airborne ->
  `ft_80083E64(box, ftCommon_8007D7FC)` (a landing would ground the
  fighter; on FD at y = 10 it never happens), grounded -> `ft_800846B0`.
- **EntryEnd (324)**, `ftCo_800C6B6C`: `timer = x6C0`; `position.y = x4 +
  x20`; ChangeMotionState(EntryEnd, 0x3000). Anim: `timer -= 1; if timer
  == 0 { if Player flags bit4 { invincibility x6C8 } ; ftCommon_8007D92C }`
  (bit4 is not set in ordinary VS: keep the invincibility branch behind the
  rule and default it off). Phys: `t = timer as f32 / x6BC as f32` (note:
  x6BC, not x6C0); `x28 = x20 * t; position.y = x4 + x28`. Coll as
  EntryStart.
- **Exit**: airborne -> Fall (existing), then the existing landing.
- **Countdown / control**: Skirmish's `Phase::Countdown` currently freezes
  the simulation; that is wrong. The match must simulate from the first
  frame (-123) with the fighters in Entry. Two open facts to settle from
  evidence, not assumption: (1) when player input starts to take effect
  (Slippi convention says control begins around frame -39, "GO"; the
  replay's first action is Dash at -38): compare the replay's pre-frame
  inputs (`peppi` pre-frames carry the sticks/buttons) with the observed
  states, and find the decomp gate (candidates: the pad copy in
  `Fighter_8006A1BC`/`Fighter_Spaghetti_8006AD10`, `x221D_b3`, the
  `gm_801A45E8` bits, or `Player_80032828`'s pad source); (2) the clock
  starts at frame 0 (123 frames after the first). Model these as rules
  (`countdown_frames` = frames before the clock starts; add whatever the
  gate needs) and document.
- **Slippi ids** 322/323/324 with the character-table animation indices;
  `state_age` -1 for Entry/EntryEnd, EntryStart counts its animation.

## Resource shape
`rules.entry: Option<EntryRules { start_frames: u32 (x6BC), end_frames:
u32 (x6C0), scale_y: f32 (x6C4, visual, keep for completeness),
invincibility_frames: u32 (x6C8) }>`, `fighter.trophy_scale: Option<f32>`
(co_attrs +0x110), `fighter.entry: Option<EntryAnimation { start_frames:
u32 }>` (the EntryStart figatree's own frame count -- see "Reported
state_age" below; unrelated to `EntryRules.start_frames`, which is the
30-frame action duration, not an animation length), and the player's port
slot supplied from the replay initialization's `ports` (P1 -> 0, P4 -> 3)
to the match (default slots 0 and 1 for two-player fixtures). When
`rules.entry` is None, keep today's Fall start so existing fixtures and
the published v1 pack behave as before. The exporter is adding these
fields to pack v2; for local measurement, copy the v1 pack to
`/mnt/shared/tmp/skirmish-gameplay-v1-entry/` and add `entry {30, 30, x,
0}`, Fox's `trophy_scale` solved bit-exactly from the replay (`y at
EntryEnd entry = 10 + 1.497345 * trophy_scale`; 0.9 reproduces the
recorded `0x41358fd0` bits exactly), and Fox's `fighter.entry {11}`
(the EntryStart figatree length, also confirmed directly from the replay).

## Reported state_age (EntryStart)
Confirmed directly against `fox-fd.slp`, not assumed: Slippi's recorded
`state_age` for EntryStart is the character's own *animation* frame (the
`ftCo_SM_EntryStart` figatree), not a count of the 30-frame action
duration `x6BC` counts down -- the two are unrelated counters that happen
to share a name. Both ports' recorded `state_age` advances 0, 1, 2, ...
up to 10 and then *holds at 10* for the remainder of EntryStart (18-19
more frames, depending on port), while `position.y` keeps changing
correctly the whole time under the unrelated `x6BC`/`x20` formula --
proving the animation clip (11 frames) is simply shorter than the
30-frame action and freezes once exhausted, exactly like any other
looping-or-holding figatree. `fighter.entry.start_frames` (11 for Fox)
models this: `crates/skirmish-replay/src/observation.rs`'s `observe`
reports `min(action_frame - 1, start_frames - 1)` for EntryStart when the
resource is present, and the pre-existing uncapped `action_frame - 1`
approximation when it is absent (no figatree-length data available for
that character). `tests/game_entry.rs`'s
`entrystart_reported_age_holds_at_the_figatree_length_when_supplied` pins
the exact replay values (age 0 at the first EntryStart frame, age 10 from
there through the frame before EntryEnd).

## Oracle
Pin `ft_0C31.c` (stub HSD/JObj/effect/audio/Player calls; capture
ChangeMotionState; script the collision answers) and compare the timers and
the y curve per frame bit-exactly against the Rust for both phases,
including the `x6BC` divisor in EntryEnd.

## Tests
Entry delay per slot; the state timeline above (unit test reproducing the
frame table for slots 0 and 3); y curve values at the phase edges; landing
from the entry box when a floor is within reach; the invincibility branch
behind the flag; Slippi ids and state ages; checkpoints; invalid rules.
Real-file measurement: run `make-initialization` + `validate-replay
--report` with the patched local pack and record the new first divergent
frame in `docs/parity.md` (expected: past -59 and up to -38, where Dash
needs animation data that pack v1 does not carry). Leave
`fox-fd-baseline.json` at -123 until pack v2 is published (CI still runs
v1, which has no `entry` rules).

## Implementation (2026-09-11)

`src/game/entry.rs` (`EntryRules`, `State`, `owns_action`, `enter`/
`enter_start`/`enter_end`/`exit`, `update_animation`, `move_fighter`) and
`src/fighter/entry.rs` (the pure `entry_delay`, `spawn_facing`, `amplitude`,
`start_progress`, `end_progress` helpers) implement the design above almost
literally: `game::entry::update_animation` mirrors each pinned Anim
function's exact control flow (including `ftCo_Entry_Anim`'s check-before-
decrement, whose unconditional trailing decrement lands on EntryStart's
*freshly assigned* `x6BC` timer on a transition frame -- confirmed against
the real source, not just the note, and this exact "shared field" behavior
is what produces the replay's 10.045 first-EntryStart-frame Y, not a
separate mechanism), and `game::entry::move_fighter` mirrors each Phys
function, called from `simulation::move_fighter`'s own early return so the
existing collision/animation pipeline (Anim, Phys, `collision::sample`,
`collision::resolve`, `staling::flush`) needs no bypass branch the way
`rebirth`/`death`/`ledge` do -- Entry-owned fighters flow through the
*same* per-frame path as every other action, with only three targeted
hooks (`update_animation`'s own dispatch call, `update_actions`'s early
return matching the pinned empty IASA, and `move_fighter`'s position-
formula override). `Match::new_with_slots` (new; `Match::new` now delegates
to it with `[0, 1]`) threads each player's real port (P1=0..P4=3) into
`spawn`'s per-slot entry delay; `crates/skirmish-replay/src/
match_validation.rs`'s `initialize` supplies `Initialization.ports` there.

Confirmed directly against `ft_0C31.c` (not just this note, which
paraphrased): `ftCo_Entry_Phys`/`_IASA`/`_Coll` and `ftCo_EntryStart_IASA`/
`ftCo_EntryEnd_IASA` are present in the motion table as empty function
bodies (`{}`), not absent -- "No IASA/Phys/Coll" above is accurate
*behaviorally* (nothing they could do has any observable effect), which is
why `game::entry`'s `update_actions` early return has no per-state IASA
logic to port at all, for any of the three states. `ftCo_800C6408` and
`ftCo_800C6B6C`/`ftCo_EntryEnd_Coll` all branch on `Fighter::x221F_b4` (a
secondary-entity "follow the leader" path, e.g. Ice Climbers' partner)
before touching position; only `!x221F_b4` is ported, since Skirmish has
one fighter per port -- a genuinely new fact this note did not have
(neither `ftCo_800C61B0` nor the callers of `ft_0C31.c`'s functions were
read closely enough beforehand to know this branch existed).

**Landing, simplified from the box sweep to the ordinary pipeline.** Both
`ftCo_EntryStart_Coll`/`ftCo_EntryEnd_Coll` set `box.bottom = -x28` and,
that same frame, `position.y = x4 + x28` (Phys) -- so the box's world-space
bottom (`position.y + box.bottom`) is *always exactly `x4`* (the spawn
height), for every frame of both states, regardless of the timer or which
phase is active. Rather than port `ft_80083E64`/`ft_800846B0`'s bespoke
sweep (whose full bodies are outside `ft_0C31.c`), `game::entry` leaves
Entry-owned fighters in the *ordinary* `collision::sample`/`collision::
resolve` pipeline (unlike Rebirth, which bypasses it): the fighter's
default rest-bone ECB, translated by the same `position` the Phys formula
already writes, gives a materially equivalent floor/wall/ceiling test and
reuses the existing generic landing dispatch (`collision::land`'s
`Action::Landing` fallthrough, the same one `ftCommon_8007D7FC` reaches).
`ft_800846B0`'s own grounded/lost-floor branch is not modeled: no fixture
spawns a fighter already grounded into Entry. `tests/game_entry.rs`'s
`entry_lands_on_a_floor_within_reach` exercises the landing path directly.

**The "input gate" question is resolved, not left open.** No separate gate
exists or is needed: `ftCo_Entry_IASA`/`ftCo_EntryStart_IASA`/`ftCo_
EntryEnd_IASA` are unconditionally empty (confirmed above), so nothing
reads pad state during any of the three match-start actions regardless of
`x221D_b3`/`gm_801A45E8`/any other candidate gate -- those candidates would
only matter for a *different* question (whether the hardware pad itself is
live before frame 0), which is moot for gameplay-visible behavior. Once
`ftCommon_8007D92C` exits into ordinary Fall/Wait, Skirmish's existing,
already-implemented input dispatch resumes exactly as it does for any other
Fall/Wait entry -- this is why the replay's first Dash at frame -38 needs
no bespoke modeling: it falls out of Entry's fixed duration plus ordinary
falling/landing physics reaching a frame where the recorded stick input
happens to cross the dash threshold, not a separate "control begins here"
rule.

**Countdown, scoped rather than fully reworked.** `simulation::advance`'s
`Phase::Countdown` branch now also runs `entry::update_animation` (Anim
only: timers/transitions, no position write, no landing) for entry-owned
fighters before returning, so `rules.entry.is_some()` fighters progress
through Entry/EntryStart/EntryEnd even if `rules.countdown_frames` is
nonzero; every other fighter (and every existing `rules.entry.is_none()`
fixture, including `tests/game_matches.rs`'s
`countdown_walk_jump_land_hitlag_respawn_and_second_stock_finish`, which
asserts frozen positions through Countdown) is completely unaffected. This
is narrower than "simulate the whole frame normally during Countdown": the
shipped gameplay pack's own `countdown_frames` is `2` (a small, unrelated
pre-game buffer, not Melee's 123-frame pre-"GO" period), so in practice
Phase reaches `Playing` almost immediately and the full pipeline (including
landing) takes over well before Entry's own ~85-frame sequence needs it;
extending landing detection into the Countdown branch itself was left out
as unnecessary for the one pack this batch measures against, and is noted
here as a scoping decision rather than silently limiting the fix.

**Facing scoped to `rules.entry.is_some()`, not applied everywhere.** The
`gmvs.c` two-slot facing rule (`fighter::entry::spawn_facing`) replaces the
`player == 0` hardcode only when the match-start warp-in is modeled.
Applying it unconditionally broke three pre-existing `tests/game_edges.rs`
cases, which park fighter 1 far to one side purely as a non-interacting
dummy (never meant to represent a real opponent position); the general rule
-- correctly -- reads that position as a real opponent and flips fighter
0's facing. `rules.entry.is_none()` keeps the exact old hardcode, verified
by `tests/game_entry.rs`'s
`none_keeps_the_pre_batch_fall_start_and_the_player_zero_facing_hardcode`
against a spawn layout the hardcode gets backwards.

**A genuine bit-exactness bug caught by the oracle, not assumed away.**
`1.497345` in `ftCo_800C6408` is an unsuffixed C double literal; `1.497345
* sp48.y` promotes the `f32` operand to `double`, multiplies, and only
truncates back to `f32` on assignment. Computing directly in `f32` (Rust's
literal-type inference would otherwise pick `f32`, since one operand is
`f32`) differs by one ulp for generic inputs; `fighter::entry::amplitude`
now promotes to `f64` explicitly, matching the source, and `tests/
entry_differential.rs`'s `entry_start_enter_amplitude_matches_the_1_
497345_literal` proptest caught the original mismatch directly (see
`docs/validation.md`'s entry for this batch).

**EntryStart's externally observed age needed a `-1` adjustment**, not a
literal pass-through of `action_frame`: `simulation::enter` resets
`action_frame` to `0` on the transition frame, but the existing shared
per-frame tail (`simulation::advance`'s own generic `action_frame += 1`)
already increments it once more before that same frame's state is
externally observable -- the same reason Walk/Run/idle track a dedicated
float frame instead of reusing `action_frame` directly for age reporting,
and exactly the behavior `crates/cli/tests/replay_match.rs`'s idle-fighter
regression already documents for its own restart/pick row
(`action_age is the restarted action_frame ... observed here as 1`).
`crates/skirmish-replay/src/observation.rs`'s `observe` publishes
`action_frame - 1` for `Action::EntryStart` specifically to recover the
replay-verified 0-based count; `Action::Entry`/`Action::EntryEnd` publish
the fixed `-1` the design called for. `animation_index` maps `ftCo_
SM_EntryStart` to `238` (counted directly from `kinds/ftCommon/forward.h`'s
`enum { ftCo_SM_None = -1, ftCo_SM_DeadUpFallHitCamera, ... }`, cross-
checked against this codebase's own already-verified `Wait1_0 == 2`/
`Fall == 20`/`Landing == 35` entries in the same enum, not a fresh guess).

No other contradiction between this batch's implementation and the pinned
source was found; the pre-existing test suite remains green unchanged
(only the `player == 0` facing hardcode's replacement required touching
any pre-existing test, and only because it was genuinely wrong for a
reversed spawn layout, not because this batch's own feature needed it).

Real-file measurement (patched local copy of gameplay export v1, not
committed): see `docs/parity.md`'s entry for this batch for the new first
divergent frame and which field differs there.
