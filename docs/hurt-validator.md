# Blaster hurtbox-timing measurement tool

`crates/cli/src/hurt_validator.rs` (`skirmish hurt-validator` subcommand).
Read this alongside `docs/validation.md`'s "early-hit" investigation entries
(2026-09-14, "pose-sample-index investigation") and its four-constraint
overlap table: Skirmish registers Blaster hits one frame early in several
console (Slippi 2.0.1) recordings where laser item positions are not
recorded, leaving the victim's own hurtbox reach as the leading open
hypothesis. Slippi 3.x replays record the laser's own item position every
frame, which removes the laser's own geometry as an unknown and leaves the
victim's hurtbox reach under Skirmish's pose model as the only thing left to
measure against ground truth.

## What it does

For every recorded Fox-laser item (Slippi item type 54) in a directory of
eligible `.slp` files (Slippi 3.x, both players Fox and/or Falco, stage one
of FD/BF/YS/FoD/DL/PS, and a matching `skirmish-assets` gameplay-export pack
directory available):

1. Reconstructs the laser's own per-frame capsules from its *recorded* item
   position (never simulated): the shooter's pack `specials.neutral.
   laser.hitboxes` (four fixed capsules staggered along local -X), offset
   and grown by the same factor `game::projectile::step` applies
   (`scale += |speed| / 11.25` per frame, capped at `laser.scale`), swept
   from the previous recorded frame's item position (degenerate on the
   spawn frame, since no earlier recorded position exists then).
2. Reconstructs the *other* fighter's hurtbox capsules every frame purely
   from the recorded post-frame observation (action state, state age,
   position, facing, airborne) -- not a full driven simulation.
3. Tests contact with `collision::shield::capsule_matrix` (the same
   primitive the live engine's projectile-vs-fighter path calls) at
   `broadphase_scale = 3.0` (the real `lbColl_80006E58` item-vs-fighter
   scale; the live `game::projectile::step` call site currently passes
   `1.0`, an unrelated pre-existing simplification this tool does not
   modify or inherit -- see the task instruction that asked for `3.0` here).
4. Compares the first frame with simulated contact against the first frame
   the recording shows the hit: victim `percent` increases with
   `last_hit_by` equal to the shooter's port *and* that specific laser
   instance's own recorded position is within a plausible reach of the
   victim on that frame (this last condition matters: a shooter can land an
   unrelated hit -- a second laser instance, a jab, an aerial -- on the same
   frame an earlier laser instance is merely still coasting through open
   space many units away; `percent`/`last_hit_by` alone cannot tell those
   apart, and an earlier version of this tool without the proximity check
   produced exactly that false attribution twice in the real run below).
   Falls back to the laser's last recorded frame if no such frame exists
   but it disappeared within 8 units of the victim.

Events are excluded (not analyzed, but tallied) when: the laser was
reflected (its recorded `owner` changes port across its lifetime), it hit a
shield (the victim occupies a Guard/ShieldBreak action state at or before
the recorded hit), the victim was intangible (recorded `hurtbox_state ==
2`), no recorded hit could be established at all, or the victim's action at
the recorded hit frame has no pose reconstruction (next section).

## Coverage and why it stops there

`simulation::pose`/`local_pose`/`hurtbox_state` are `pub(crate)` inside the
`skirmish` library crate and are not reachable from this external CLI-tool
crate without editing simulator source, which this batch does not do (the
task's own "tool + docs only" constraint). Every helper in
`hurt_validator.rs` is instead a small, cited, from-scratch port built only
from `pub` items (`bones::Pose::evaluate_with_root`, `BoneCapsule::
transform`, `movement::loop_period`, the public `MatchData`/`FighterData`
schema) that reproduces the *same* mapping, restricted to what
`game::movement::pose`'s own match arms already select from `action_frame`
alone, plus `LandingFallSpecial` (via `escape_air::pose`'s `landing_elapsed`
indexing, itself already confirmed bit-exact to the recorded `state_age` by
`docs/validation.md`'s 2026-09-14 entry): **Wait** (bind-pose fallback when
the idle sub-motion, not recoverable from recorded fields, is unknown),
**Walk** (15/16/17), Turn, RunTurn, **Dash**, Run, RunBrake, **JumpSquat**
(24, i.e. KneeBend), Jump (25/26), JumpAerial (27/28), Fall (29/32),
FallSpecial, Landing, **LandingFallSpecial** (43), Squat, SquatWait,
SquatRv, Pass, Ottotto, OttottoWait, EntryStart.

Every other recorded victim action -- grabs, ledge, wall-jump, damage/
hitstun (including every `DamageFly*`/`DamageFall` tumble state), dodges/
rolls, shield, attacks, specials, death/respawn, entry, prone -- needs
additional `Fighter` sub-state (grab target, ledge timer, attack-frame index
tables, damage-motion trajectory, ...) that the recorded post-frame fields
alone do not determine, and is skipped with the skipped state id tallied in
the report, per the task's own "if the mapping is not available for some
actions, say which and skip them" instruction.

## Usage

```
SKIRMISH_GAMEPLAY_DATA=/mnt/archive/datasets/melee/skirmish-gameplay/v15-snapshot-20260916 \
  skirmish hurt-validator \
  --dataset-dir /mnt/archive/datasets/melee/slippi-public-dataset-v3.7/data/FOX \
  --time-budget-seconds 1200 \
  --report /path/to/report.json
```

`--pack-root` overrides `SKIRMISH_GAMEPLAY_DATA`. `--limit N` caps the
number of *eligible* files processed. Packs are loaded once per matchup
directory and cached for the run (each `match-data.json` is ~360 MB of
JSON; loading all packs a single run might touch takes a few seconds each,
not per-file).

## Real run, 2026-09-14

`SKIRMISH_GAMEPLAY_DATA=v15-snapshot-20260916`, dataset directory as above
(the task's own default), full time budget, no `--limit`. Runtime: **3.3
seconds** wall clock (release build) -- the entire eligible corpus, not a
20-minute sample.

| files considered | eligible (version/stage/characters/pack pairing) | processed | failed | recorded Fox-laser items seen |
| --- | --- | --- | --- | --- |
| 182 | 13 | 13 | 0 | 445 |

Only 13 of the 182 `.slp` files under the default directory pass every
filter: Slippi 3.x, both players Fox and/or Falco, stage in
{FD,BF,YS,FoD,DL,PS}, *and* a matching pack directory actually present in
the v15 snapshot. The snapshot only carries Fox/Falco packs for {FD, BF, DL,
FoD, PS, YS} dittos and Fox-vs-Falco for FD alone; several real Fox-vs-Falco
recordings on FoD/PS in the dataset have no matching pack and are correctly
excluded ("pack pairing available").

Event disposition (445 recorded laser items total):

| disposition | count |
| --- | --- |
| analyzed (contact test run) | 0 |
| no recorded hit (miss, off-stage, or an unrelated same-frame hit correctly rejected by the proximity gate) | 283 |
| victim action has no pose mapping (skipped) | 162 |
| reflected | 0 |
| shield contact | 0 |
| victim intangible | 0 |

Skipped victim actions, by recorded Slippi state id:

| state id | name | count |
| --- | --- | --- |
| 38 | DamageFall | 1 |
| 90 | DamageFlyTop (not modeled by Skirmish's `Action` enum at all -- see below) | 160 |
| 186 | DownStand (prone) | 1 |

**Headline finding: zero analyzable events, for a real, load-bearing
reason, not a tool defect.** Every one of the 162 laser hits this run could
positively attribute to a specific recorded laser instance (proximity-gated
`percent`/`last_hit_by` match) landed on a victim already in a damage/
hitstun/prone state -- overwhelmingly `DamageFlyTop` (state 90), the tumble
state a fighter enters after already taking a strong hit. This matches real
Blaster usage: a landed laser is disproportionately the second or third hit
of a string on an opponent who is already airborne and reacting, not a
free-standing Wait/Walk/Dash target. None of the 13 eligible replays'
laser-hit events landed on a victim in Wait(14), Walk(15-17), Dash(20),
JumpSquat(24) or LandingFallSpecial(43) -- the five actions the task calls
out for per-event detail -- so those tables are genuinely empty for this
corpus, not merely unpopulated by an early exit.

Separately, `DamageFlyTop` (Slippi state 90) has no corresponding
`skirmish::game::Action` variant at all: this is a real, pre-existing
Skirmish coverage gap (the simulator's own `Action` enum has no tumble-state
variant beyond `Damage`/`DamageFall`), not something this tool's own
"pose-family" restriction chose to skip -- confirmed by reading
`crates/skirmish-replay/src/observation.rs`'s `action_state` forward table
in full: no arm produces `90`.

**What would be needed to get a nonzero sample.** A larger or differently
distributed 3.x corpus with more free-standing (non-comboed) Blaster
poking, and/or extending `simulation`'s own damage-pose family (native
`src/`, out of this batch's "tool + docs only" scope) so a tool like this
one could reconstruct `DamageFlyTop`/`DamageFall` hurtboxes from recorded
`state_age`/velocity/knockback alone -- itself a nontrivial project, since
those poses depend on the launch trajectory's own DI/knockback history, not
just elapsed time.

## Known limitations (beyond the coverage list above)

- `frame_before_x_overlap` is this tool's own diagnostic (an axis-aligned X
  interval overlap over every hitbox/hurtbox pair), not a call into any
  engine primitive -- `collision::sweep::closest_axes` is
  `pub(super)`-scoped inside `skirmish::collision` and not reachable from
  here either. It is a reasonable, simple proxy ("how close were the
  closest capsules on X"), not the exact rounded-capsule silhouette.
- Walk's own float animation counter (`locomotion.walk.frame`) only ever
  advances in the real engine while the pack supplies `movement.
  walk_animation`; this tool mirrors that exactly (rate-scaled from
  recorded `state_age` when present, left at its `0.0` default otherwise)
  rather than guessing a rate.
- `JumpAerial`'s root yaw (`multi_jump_yaw`) and `death.camera_offset` are
  left at zero; both default to zero in the real engine too and neither
  affects any of the five actions this task calls out by name.
