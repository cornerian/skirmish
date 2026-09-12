# ECB timing: the fox-fd.slp -51/-49 landing gap

This note records a full investigation into why, against the real recording
`tests/fixtures/slippi/parity/fox-fd.slp` (P1 Fox, with the v2 gameplay
pack's `movement_poses` spliced in), Skirmish reports P1 landing at frame
-51 while the recording lands at -49. It cites the pinned decomp
(`/mnt/shared/Projects/Code/External/melee`, rev `0bac93a5e`) throughout.
One real, decomp-cited behavior gap was found and fixed (`ecb_lock` not
applied on a plain walk/dash-off-an-edge transition); it does not move this
particular divergence. The divergence itself is traced to a data
limitation already flagged by the prior movement-poses batch
(`docs/parity.md`, `docs/movement-poses.md`), and is *not* fixable by any
purely Skirmish-side timing or lock correction -- confirmed here by
mathematical derivation from the decomp's own clamp arithmetic and by
runtime instrumentation of the actual simulation.

## The recording

P1 (Fox), `TopN`/position.y, frames -52..-49: `1.7649` (Fall, airborne),
`-0.3051` (airborne), `-2.6051` (airborne), `0.0001` (Landing, grounded).
Velocity accumulates smoothly (~-0.23/frame added to an already-falling
rate) across all four frames -- confirmed bit-identical against Skirmish's
own physics via runtime instrumentation (see "What actually happens in
Skirmish" below) -- so the divergence is purely about when *collision*
resolves the floor crossing, not about gravity or position integration.

Fox enters ordinary Fall (Slippi action state 29) at frame -59, from
`EntryEnd` (state 324, Slippi's match-start warp-in tail), confirmed
directly against the replay:

```
-60 EntryEnd frame 29  pos.y=10.045
-59 Fall     frame 0   pos.y=9.815
...
-52 Fall     frame 7   pos.y=1.7649
-51 Fall     frame 8   pos.y=-0.3051
-50 Fall     frame 9   pos.y=-2.6051
-49 Landing  frame 0   pos.y=0.0001
```

## Hypothesis 1 (given): stale joint matrices from process order

`fighter.c:895-910` registers `Fighter_procMap` (collision) at priority 6
and `Fighter_8006C5F4` (the JObj matrix setup, `ft_80089B08`) at priority
7. The natural hypothesis is that collision, running first, samples joint
world matrices left over from the *previous* frame's matrix setup while
subtracting the *current* frame's position, producing a one-frame-stale
ECB bottom that would explain a lagging landing.

This does not hold up: `lb_00B0.c:105` (`lb_8000B1CC`) calls
`HSD_JObjSetupMatrix(arg0)` itself, on demand, at read time, for any joint
with a parent -- it does not depend on `Fighter_8006C5F4` having already
run this frame. For the *root* joint specifically (no parent), it instead
returns the raw local translation directly (`HSD_JObjGetTranslation`),
which `Fighter_procMap` (`fighter.c:2480-2515`) has already set to the
*current* frame's `fp->cur_pos` before calling `coll_cb`. So there is no
inherent one-frame staleness in the joint sampling itself.

## Hypothesis 2: the real `ecb_lock`/bottom-lock mechanism

`mpcoll.c:624-639` (`mpColl_LoadECB_inline`) and `mpcoll.c:345-455`
(`mpColl_LoadECB_JObj`) show a genuine lock: when `x130_flags &
CollData_X130_Locked` is set, the newly computed `desired_ecb.bottom` is
thrown away and the previous value is written back
(`ecb.rs`'s `State::finish_load` already ports this exactly). This bit is
driven by `fp->ecb_lock`, decremented once per frame at the top of
`Fighter_procMap` (`fighter.c:2483-2489`) with `ftCommon_UnlockECB`
(`ftcommon.c:509-513`) clearing the lock at zero.

`ftcommon.c:515-525` (`ftCommon_8007D5D4`) is the only setter relevant to
ordinary falling: `ecb_lock = 10`, `x130_flags |= Locked`. Grepping every
call site (`ftCo_Jump.c:158`, `ftCo_JumpAerial.c` x5, `ftCo_Pass.c:81,93`,
`ftCo_Fall.c:68`, plus ~40 special-move/capture/item sites) turns up
exactly one relevant to plain falling: `ftCo_Fall_Enter`
(`ftCo_Fall.c:47-70`):

```c
if (!ftCo_Fall_inline(gobj)) {
    Fighter_ChangeMotionState(gobj, ftCo_MS_Fall, ...);
    ...
    if (fp->ground_or_air == GA_Ground) {
        ftCommon_8007D5D4(fp);
    }
}
```

The lock only engages if the fighter was still `GA_Ground` at the instant
Fall is entered -- i.e. exactly "just walked/dashed off an edge", not a
jump (which sets `ecb_lock` itself, via `ftCo_Jump.c`/`ftCo_JumpAerial.c`)
and not a Fall entered while already airborne.

**Skirmish had this exact case unimplemented.** `src/game/collision.rs`
locks the ECB on grounded-jump-launch (`game/locomotion.rs`, 3 sites) and
on platform pass-through (`begin_pass_as`, `collision.rs:79-81`, both
already correct and cited), but the plain "lost floor support this frame,
falling through to ordinary `Action::Fall`" branch
(`collision.rs`, inside the `if f.grounded { ... }` block, previously
ending at `simulation::enter(f, Action::Fall);`) did not set `ecb_lock`/
`bottom_locked`. Fixed in this batch (see the commit and
`collision.rs`'s new comment) with a new test,
`dashing_off_an_edge_locks_the_ecb_bottom_for_ten_frames`
(`tests/game_locomotion.rs`), which also confirms an asymmetry the fix
needed to preserve: a jump's lock (set during an earlier-priority
per-frame callback, before `Fighter_procMap`'s own decrement runs) is
already visible to that same frame's decrement and reads back as 9 on the
entry frame (see `late_air_jump_relocks_the_ecb_and_keeps_its_action_past_the_apex`,
already in the suite); this collision-detected transition's lock (set
*inside* the same call that *is* the decrement's `coll_cb`) is not
touched by that frame's decrement and reads back as 10, only starting to
count down the following frame. Skirmish's frame order
(`advance_ecb_lock` before `collision::sample`/`resolve`, `move_fighter`
before `advance_ecb_lock`) reproduces this distinction without any special
casing, because the fix lives inside `collision::resolve` -- after that
frame's `advance_ecb_lock` call -- exactly mirroring where the decomp's
own transition happens relative to `fp->ecb_lock--`.

**This fix does not move the fox-fd divergence.** Fox enters Fall from
`EntryEnd` (`ftcommon.c:596-604`, `ftCommon_8007D92C`: `if
(ground_or_air == GA_Air) ftCo_Fall_Enter(...)`), and he is airborne
throughout the match-start warp-in (position never reaches the stage
during `Entry`/`EntryStart`/`EntryEnd` in this recording) -- so
`ftCo_Fall_Enter`'s own `ground_or_air == GA_Ground` check is false and
`ftCommon_8007D5D4` never runs for this transition. Confirmed empirically
(temporary debug instrumentation, removed before committing): with the fix
applied, `f.ecb.bottom_locked` is `false` for every frame from `EntryEnd`
through `Fall` up to the landing frame in this replay -- the fix is real
and correct, but orthogonal to this specific gap.

## Verifying the six-joint list

Fox's real, disc-decoded ECB source (`fighters/fox.json`'s
`collision_box` in the v2 pack) is:

```json
{"indices": [58, 0, 30, 13, 6, 4], "flags": 0, "parameters": {...}}
```

`ft_081B.c:33-67` (`ft_80081B38`) resolves these six joints from
`ftData.x44` (`ftData_x44_t`, `types.h:584-595`: six `s16` `Fighter_Part`
ids) through `bones[part_id].joint` (`fp->parts`, the per-character
part-to-joint table, `ftparts.c:695-698`'s `ftParts_GetBoneIndex`) --
*not* through `bones->joint` (part 0, always the root, stored separately
as `x108_joint` and never read by `mpColl_LoadECB_JObj`'s sampling loop).
So a literal `0` among the six is not automatically "the root by
construction" -- it means whichever part id sits at that position
genuinely resolves to joint 0 for this character.

The exporter (`/mnt/shared/tmp/skirmish-assets-gameplay`,
`src/gameplay/minimal.rs::build_ecb_collision_box`,
`src/gameplay/common.rs::PartsTable::joint_for_part`) reads this same
table directly out of `PlCo.dat` (`co_archive`, not `fx_archive` -- the
exporter's own comment at `export.rs:353-360` documents a *different*,
already-fixed archive-selection bug that would have zeroed *all six*
joints, not just one, and confirms today's code passes the correct
archive). The exporter's own recorded cross-check
(`export.rs:953-975`, echoed in the pack's `manifest.json` under
`ecb_part_ids_cross_check`) reads, verbatim:

> Fox's six ftData.x44 ECB source part ids and their resolved parts-table
> joints, in decomp order: part 41 -> joint 58, **part 55 -> joint 0**,
> part 25 -> joint 30, part 13 -> joint 13, part 7 -> joint 6, part 4 ->
> joint 4. Part id 55 (index 1) resolves to joint 0, the skeleton root --
> confirmed real, disc-decoded data...

So this is genuine per-character game data, not an exporter bug: **do not
fix the exporter** (per the batch's own instruction) -- there is nothing
to fix here. Independently, `skirmish-assets-gameplay/src/gameplay/
figatree.rs` and `docs/gameplay-export.md` (search `TransN is skeleton
joint index 1, not joint 0`) record that joint 0's real FigaTree track was
directly inspected and **has zero channels** -- it is a structural root
bone with no animation curve at all, confirmed from the binary format
itself, not assumed. Every exported pose's bone 0 translation is `[0, 0,
0]` for exactly this reason (verified directly against `fighters/fox.json`
`movement_poses.fall`, all 9 samples).

## Why the six-joint list forces `bottom == position` every frame

`mpColl_LoadECB_JObj` (`mpcoll.c:345-455`, ported faithfully in
`collision/ecb.rs::State::load_joints`) computes, in local (position-
relative) coordinates:

```
bottom_y = min over the six joints of (joint.y - position.y)   // <= 0, since joint 0 contributes exactly 0
if flags & 4 == 0 { bottom_y -= 2.0 }                            // Fox's flags == 0, so this always applies
if flags & 1 == 0 { if bottom_y < 0.0 { bottom_y = 0.0 } }        // Fox's flags == 0, so this branch runs
```

Since joint 0 (offset exactly `0`, proven above) is always one of the six
samples, `bottom_y` before the final clamp is always `<= 0 - 2 = -2`,
so the clamp always fires and `bottom_y` is always exactly `0`. World
bottom = `position.y + bottom_y = position.y`, unconditionally, every
frame, regardless of pose, and regardless of whether the bottom is locked
(a lock just freezes this same always-`0` value at whatever it last was,
which is also always `0`). No frame-order, staleness, or lock fix changes
this -- it is forced by the joint list and the decomp's own clamp
arithmetic, both independently verified above.

Confirmed by instrumenting the real simulation (temporary
`SKIRMISH_ECB_DEBUG` eprintln in `collision::resolve`, removed before
committing) while re-running `validate-replay` against the spliced pack:
`ecb.current.bottom` reads `[0.0, 0.0]` on every single frame from
`EntryEnd` through `Fall` up to and including the landing frame, and
`f.position` matches the recording's values to 5+ decimal places the
whole way (`Fall frame=7 pos=[-60.0, 1.7649204]`, `Fall frame=8
pos=[-60.0, -0.30507958]`) -- physics/gravity are correct; only the
landing-frame decision differs.

## Conclusion: not fixable from Skirmish's side with what is modeled here

Given `bottom == position` is forced by real, verified per-character game
data (not a bug, not stale/lagged data, not missing a lock), Skirmish's
existing collision code -- which faithfully implements
`mpColl_LoadECB_JObj`'s arithmetic -- necessarily detects the floor
crossing the instant `position.y` itself goes non-positive, i.e. frame
-51. For the real game to delay this to -49, its actual floor-crossing
*test* (the specific `mpColl_800xxxxx_Floor` callback selected by Fox's
current `coll_cb`, not the ECB *shape* arithmetic examined above) must use
some additional per-frame quantity this investigation did not reach --
candidates considered and not confirmed:

- The Entry/EntryEnd fixed collision box (`ftCo_EntryEnd_Coll`,
  `ft_0C31.c:306-322`, `mv.co.entry.x2C`) pins its *world* bottom at the
  original spawn height `y0` for the whole warp-in, rather than tracking
  the falling position the way Skirmish's `entry.rs` deliberately
  simplifies it (see that file's own comment at lines 234-248). Traced
  through: this cannot be the missing two frames here, because position
  never approaches the floor during `Entry`/`EntryStart`/`EntryEnd` in
  this recording, so the pinned-vs-tracked distinction cannot affect
  anything before the `EntryEnd -> Fall` handoff, and by one frame into
  ordinary `Fall` both models converge to the same `bottom == position`
  value regardless.
- `ftCo_Fall_inline` (`ftCo_Fall.c:31-42`) can redirect Fall's entry
  through `ftCo_80090780`/`ftCo_800C5D34` instead of the normal body
  examined above, gated on `fp->x2224_b2` (a freeze/hitstun-adjacent flag
  set by `ft_0C8C.c:43`, unrelated to a match-start warp-in in this
  recording) or `ftCo_800C5240`. Not traced to completion; `x2224_b2` is
  false in this scenario by every setter found, so this is unlikely to be
  the missing mechanism, but the alternate branch's own body was not read.
- The specific floor-check callback selected via `fp->coll_cb`/the motion
  state table's `coll_cb` field for ordinary Fall was not individually
  traced among the ~40 near-identical `mpColl_800xxxxx_Floor` wrapper
  functions in `mpcoll.c`; one of them may carry velocity- or
  state-dependent tolerance this note did not reach.

This is the same root cause the movement-poses batch already flagged
(`docs/parity.md`'s "Current measurement" entry, `docs/movement-poses.md`):
a data/joint-list limitation of the already-pinned ECB batch, not a
Skirmish behavior bug reachable by a timing fix -- now confirmed by an
independent derivation from the decomp's own arithmetic, by the exporter's
own real-figatree-verified joint-0-is-channel-less finding, and by direct
runtime instrumentation of the affected frames. Closing it needs either
new per-character joint data (there is none to add: joint 0's presence is
verified-correct, real data) or tracing the specific floor-check callback
chain beyond what this batch reached -- both out of scope for a further
timing/lock fix.
