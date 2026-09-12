# Pre-"GO" input lock and match clock (design note)

Evidence from the parity replay (`tests/fixtures/slippi/parity/fox-fd.slp`,
pre-frame controller samples vs post-frame states):
- P4 holds the main stick at (-0.99, 0) from frame -47 through -41 while in
  EntryEnd (324) and then Fall (29, frames -44..-41): its x stays exactly
  60.00 and its facing is unchanged. Ordinary Fall with a held stick drifts,
  so controller input is ignored during this window.
- P4's stick is neutral from -40; P1's first non-neutral sample is at -37
  (stick +0.86) and P1 enters Dash on that same post-frame. Slippi's
  documented convention is that players gain control at frame -39 ("GO"),
  84 frames after the first frame (-123). The clock (`time_limit_frames`)
  starts at frame 0, 123 frames after the first frame.
- Physics runs normally during the locked window: P1 exits EntryEnd into
  Fall at -59 with gravity applied, lands at -49, and idles; the fighters
  are simulated, only their controllers are neutral.

Current Skirmish (main e77e2fb, match-start batch ad67669):
`Phase::Countdown` runs only `entry::update_animation` and otherwise
freezes fighters for `rules.countdown_frames` frames (the legacy synthetic
fixture asserts frozen positions during Countdown, and its
`countdown_frames` is 2), and the gameplay export sets
`countdown_frames = 123`. That reproduces neither the fall/landing during
the countdown nor the input lock.

## Behaviour to implement
- When `rules.entry` is `Some` (the real-data profile): the match simulates
  every frame fully from the first frame; for the first
  `rules.entry.input_lock_frames` frames (default 84, serde default; not on
  disc) each fighter's controller is replaced by a neutral `Controller`
  before input dispatch (and `previous_input` bookkeeping sees neutral, so
  nothing is "fresh" on the first controlled frame unless the source says
  otherwise: check how the pad copy behaves at the unlock by reading the
  decomp gate); the clock/`Phase::Playing` transition (`Event::Started`,
  `remaining_frames` decrement) happens after `rules.countdown_frames`
  (123) frames as now.
- When `rules.entry` is `None`: keep the legacy frozen Countdown exactly
  (existing tests must not change).
- Find and cite the decomp gate. Candidates: the VS-mode frame logic in
  `gm/gm_1601.c` (`gm_Scene_Vs_OnFrame` neighbours, the "Ready/GO" timer),
  the pad source used by `Player_80032828`/`pl/player.c`, the fighter pad
  copy in `ft/fighter.c` (`Fighter_8006A1BC` area, `x221D_b3`),
  `gm_801A45E8` bits, or `HSD_PadMasterStatus` gating in `gm/gmmain_lib.c`.
  If the exact constant is found (expect 84 or an equivalent "-39"), use it
  as the default and cite it; if not, keep 84 with the replay evidence
  cited and say the decomp citation is pending.

## Resource shape
`Rules.entry.input_lock_frames: u32` (serde default 84; validated
`<= countdown_frames`), no exporter change needed.

## Tests
Native: a fighter with `rules.entry` set, stick held during the locked
window, falls and lands without drifting or turning; the first controlled
frame acts; legacy fixture unchanged; validation. Real-file measurement:
copy `/mnt/archive/datasets/melee/skirmish-gameplay/v2/` (current export)
to `/mnt/shared/tmp/skirmish-gameplay-v2-lock/`, run `skirmish
make-initialization` + `validate-replay --report`, and record the first
divergent frame and field in `docs/parity.md` (do not commit the copy; the
baseline file stays until pack v2 is published).

## Implementation (2026-09-11)

`Rules.entry.input_lock_frames: u32` (`game::entry::EntryRules`, serde
default 84 via `default_input_lock_frames`, validated
`<= Rules.countdown_frames` in `game::validation::validate`). `game::
simulation::advance` (`src/game/simulation.rs`) gained two changes, both
gated on `data.rules.entry.is_some()` so `rules.entry.is_none()` matches are
byte-for-byte unaffected:

1. **The `Phase::Countdown` early return is skipped**, not narrowed further:
   the previous match-start batch already ran `entry::update_animation`
   inside that early return so the warp-in's own timers progressed during
   Countdown, but everything else (physics, collision, landing, combat)
   stayed frozen for `rules.countdown_frames` frames. This batch instead
   captures `was_countdown` (the phase *before* this frame's own Countdown
   -> Playing transition) and, when `rules.entry` is `Some`, falls through
   to the complete per-frame pipeline instead of returning early -- so
   Entry/EntryStart/EntryEnd, the fall into ordinary `Action::Fall`, and any
   landing all run for real during the pre-"GO" period, exactly as Melee's
   own recording shows. `state.remaining_frames`'s decrement and the
   time-limit finish check are now gated on `!was_countdown` (previously
   implicit, since the whole function returned before reaching them): the
   match clock still only starts counting once a frame begins already in
   `Phase::Playing`, matching `rules.countdown_frames` unchanged from before
   this batch.
2. **`inputs` is replaced with `[Controller::default(); 2]`** for both
   players whenever `state.next_frame <= entry.input_lock_frames`, before
   any other use of `inputs` in the function -- dispatch, hitlag sampling,
   and the `fighter.previous_input = input` bookkeeping all see the neutral
   controller during the lock, so a button held through the unlock frame
   registers as freshly pressed against a neutral `previous_input` (the
   simplest reading available given the decomp citation below is pending;
   see "Open question" for what would change it). `state.next_frame` is
   already the 1-based frame number of the step being computed (incremented
   in `Match::step` before `advance` runs), so `input_lock_frames == 84`
   locks steps 1..=84 and the first real controller reaches dispatch on
   step 85 -- matching the replay's frame -123 (step 1) through frame -40
   (step 84), with control at frame -39 (step 85).

**Decomp gate: not found, replay evidence cited instead, as the design note
allowed.** Searched and read in full: `gm/gmvs.c`'s `gm_Scene_Vs_OnFrame`
and its `fn_8016CD98`/`fn_8016CFE0` neighbors (the VS-mode per-frame
dispatch and the `frame_count`/HUD-timer advance -- `frame_count` itself
counts up from a negative value toward `-1` there, consistent with the
clock's own starting point, but nothing in that file gates *pad* input by
frame count); `ft/fighter.c`'s `Fighter_Spaghetti_8006AD10` (the per-fighter
pad-copy callback, `HSD_GObj_SetupProc`'d unconditionally at fighter
creation, running every frame from spawn with no frame-count or phase gate
found in its body) and its `x221D_b3`-gated double-buffer branch (a pad
*history* selector, not an input suppressor); `gm/gm_1A45.c`'s
`gm_801A45E8` (bit-flag queries for pause/camera/HUD state, none of them
frame-count-gated); `sysdolphin/baselib/controller.c`'s
`HSD_PadMasterStatus`; `pl/player.c`'s `Player_80032828` (a pose-array
setter, unrelated to input). None of these implement a "first N frames
force neutral input" gate. This is consistent with `docs/match-start.md`'s
own prior conclusion that Entry/EntryStart/EntryEnd's `_IASA` callbacks are
unconditionally empty -- but that conclusion only explains why input has no
effect *during those three states*; it does not explain this batch's new
replay evidence (P4 holds the stick through part of ordinary `Fall`, at
-44..-41, well past EntryEnd, with zero drift), which requires something
broader than an empty `_IASA`. `input_lock_frames` therefore keeps the
default `84` backed only by the replay's own frame arithmetic (frame -39 is
84 frames after the recording's first frame, -123, matching Slippi's
publicly documented "GO" convention), with the decomp citation left
**pending** rather than guessed at.

**Open question, left for a future batch if the gate is ever found:**
whether the real pad-copy leaves a non-neutral `previous_input` behind at
the unlock frame (which would change whether a held button reads as
"fresh" at frame -39) is exactly the kind of detail a located gate would
settle; absent it, this batch takes the simpler, explicitly-flagged default
(neutral `previous_input` throughout the lock).

**C-oracle:** none added. `docs/validation.md`'s three-level framework
(`docs/parity.md`) reserves differential (C-oracle) coverage for a Rust
function that ports a specific pinned decomp function; since no decomp
function implements this gate (above), there is nothing to pin a
differential test against. The existing `entry_differential.rs` suite
(EntryRules's other fields) is unaffected -- `input_lock_frames` is
consumed only by `game::simulation::advance`, not by any function under
oracle comparison.

**Real-file measurement, first pass (2026-09-11, gameplay export v2,
patched nothing -- v2 already ships `rules.entry`/`countdown_frames: 123`/
per-fighter `trophy_scale`/`entry.start_frames`, unlike the v1 pack the
match-start batch had to patch locally):** copying
`/mnt/archive/datasets/melee/skirmish-gameplay/v2/` to
`/mnt/shared/tmp/skirmish-gameplay-v2-lock/` and running
`make-initialization` + `validate-replay --report` against the pinned
`fox-fd.slp` still reported the first divergent frame as -123, unmoved, for
the same pre-existing, unrelated reason `docs/parity.md` already recorded
for the match-start batch: the differing field was `shield` (expected
`0x42700000` = 60.0; Skirmish reported `0x00000000`), because that copy of
v2 had no `rules.shield`/per-fighter `shield` resource at all.

**The `shield` gap closed independently, mid-batch, moving the divergence
to a new field this batch also owns.** The published export
(`/mnt/archive/datasets/melee/skirmish-gameplay/v2/`) gained
`rules.shield`/per-fighter `shield` data (unrelated to this batch; a
separate exporter update). Re-copying it and re-running the same two
commands reports the first divergent frame still at **-123**, but the
differing field is now **`state_flags.dead`** (expected `0x01`, Skirmish
`0x00`) -- Slippi's flags byte 4, bit `0x40` (`Fighter::x221F_b1`), which
this same match-start sequence sets and clears and had not yet been
modeled. Fixed in this batch (not a separate one, since it is the entry
warp-in's own observable state, uncovered only once `shield` stopped
masking it):

- `ft_0C31.c:46` (`ftCo_800C61B0`): `fp->x221F_b1 = true`, set
  immediately after `Fighter_ChangeMotionState(gobj, ftCo_MS_Entry, ...)`.
- `fighter.c:1066`: `Fighter_ChangeMotionState` unconditionally clears
  `x221F_b1 = 0` on every motion change (confirmed directly, alongside the
  `fighter.c:752` sibling clear in a different reset path) -- so the
  EntryStart transition clears it again, with nothing to set it back.
- `game::simulation::enter` (the shared per-action-transition function
  every `Action` change already funnels through, Skirmish's own mirror of
  `Fighter_ChangeMotionState`) now unconditionally sets
  `fighter.death.hidden = false` alongside its other unconditional resets
  (`skip_floor`, `body_state`, `smash`, ...): `fighter.death.hidden` is
  already Skirmish's existing runtime bit for this exact Slippi flag (see
  `crates/skirmish-replay/src/observation.rs`'s `state_flags`,
  `fighter.death.hidden || matches!(action, Respawn | Eliminated)`),
  previously written only by `death::begin` for blast deaths.
  `game::entry::enter` (the Entry-specific spawn function) now sets
  `fighter.death.hidden = true` immediately after its own
  `simulation::enter(fighter, Action::Entry)` call, mirroring the source's
  own "ChangeMotionState, then set the flag back" order exactly.
  `game::death::begin` is reordered (`simulation::enter` first, then the
  `fighter.death = State { ..., hidden: !delayed_disappearance, ... }`
  assignment) so the new unconditional reset does not clobber its own
  intentional set -- verified this does not change any existing behavior,
  since nothing between the two statements previously read `fighter.death`.
- `tests/game_entry.rs`'s
  `the_dead_flag_bit_is_set_through_entry_and_clears_at_entrystart` pins the
  replay-verified sequence: set at spawn and through every remaining Entry
  frame, cleared starting at EntryStart's first frame, and still cleared
  through EntryEnd and the ordinary Fall it exits into.

**Real-file measurement, second pass (2026-09-11, same v2 export, now with
`rules.shield` and the dead-flag fix above):** the first divergent frame is
still **-123**, but the field is now **`position.x`** for **P4** (expected
`0x42700000` = 60.0, Skirmish reports `0x41a00000` = 20.0). Both fields this
batch and the match-start batch own -- the entry sequence's states/
positions/ages and the dead flag -- now agree with the recording on frame
-123 (the divergence moved past both in turn as each was fixed, first past
`shield`, then past `state_flags.dead`, landing on `position.x`). This new
gap is a stage-spawn coordinate mismatch (P4's expected spawn x is 60, not
20) unrelated to either batch: neither models or reads stage spawn
coordinates from anything but the resource's own `stage.spawns` array, so
this is exporter/pack data, not simulator logic, and is left for whoever
owns that data next. `fox-fd-baseline.json` is left unmoved (still -123);
moving it is a future batch's call once `position.x` (or whatever the
next-found field is) is fixed.

## Open question resolved (2026-09-11, real-replay parity loop, `fox-fd-4.slp`)

The "open question" above -- whether the real pad copy leaves a non-neutral
`previous_input` behind at the unlock frame -- is answered by a second real
recording, `tests/fixtures/slippi/parity/fox-fd-4.slp` (`14_56_00 [C2] Fox +
Fox (FD).slp`, same corpus, ports P2/P4, `docs/parity.md`). Unlike `fox-fd.slp`
(whose P4 happens to go neutral on the stick at -40, one frame before its own
unlock at -39, so the original neutral-`previous_input` guess was never
exercised there), `fox-fd-4.slp`'s P4 holds the stick down continuously from
-46 through and past its unlock frame, also -39. The recording shows P4
continuing an ordinary gravity-only fall at -39->-38 (`position.y` steps by a
constant, accelerating amount matching every other frame of that fall), but
Skirmish reported a much larger drop there: `game::simulation`'s fast-fall
edge check (`!f.fast_fall && f.velocity[1] < 0.0 && input.stick[1] <=
-threshold && f.previous_input.stick[1] > -threshold`) read `previous_input`
as the neutral controller this batch's original implementation left behind
for every locked frame, so a stick that had been held down since before the
lock read as a fresh press exactly at unlock, wrongly entering fast-fall a
frame early. Confirmed directly: patching `game::simulation::advance` to
keep `previous_input` tracking the real, un-neutralized samples throughout
the lock (only *dispatch* still sees the neutral controller) moved
`fox-fd-4.slp`'s `checked_frames` from 84 to 85 (`first_divergent_frame`
from -39 to -38) with no other change, isolating this as the sole cause.

This means the real pad copy does *not* stop tracking during the lock --
only the fighters' own dispatch does -- the opposite of this batch's
original, explicitly-flagged default. `game::simulation::advance` now
captures `raw_inputs` before the neutralization and uses it for every
`fighter.previous_input` assignment in the function; dispatch (`inputs`)
is unchanged. The decomp gate for the lock itself is still not found (the
search above remains current); this only resolves which of the two readings
of "neutral for the locked frames" the pad copy takes, not the lock's own
citation.
