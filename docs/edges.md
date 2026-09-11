# Floor-end collision modes and the edge teeter

`skirmish::game::edge` (wiring, `src/game/edge.rs`) and `skirmish::fighter::edge`
(pure arithmetic, `src/fighter/edge.rs`) port `inline2(coll, mode)`'s three
floor-end rules from `mpcoll.c` and the Ottotto/OttottoWait edge-teeter state
from `ftCo_Ottotto.c`/`ftCo_Walk.c`. Pinned decomp rev `0bac93a5`.

## Ground collision modes (`inline2(coll, mode)`)

Every grounded collision callback picks one of three floor-end rules when the
ECB bottom's X passes the current floor line's end (`src/melee/mp/mpcoll.c`,
the `if (flags & 1) { mpColl_8004A678_Floor(...) } else if (flags & 2) {
mpColl_8004A45C_Floor(...) }` branch at mpcoll.c:3925-3965, itself reached
only once the ordinary floor sweep and its adjacent-segment walk have already
failed). Skirmish applies the same three modes in `game::collision::
floor_end_clamp`, at the exact point the existing grounded-floor projection
(`stage::project_floor`, itself already walking connected segments) fails:

- **Plain** (`mpColl_8004B108`, `ft_80082708`, mpcoll.c:3996-4001): past the
  end the fighter leaves the ground (`GA_Air`), unchanged from before this
  batch.
- **Clamp** (`mpColl_8004B2DC`, `ft_800827A0`, mpcoll.c:4008-4013;
  `mpColl_8004A45C_Floor`, mpcoll.c:3584-3652): unless a wall stands between
  the edge point (one unit inward and up) and the far ECB side
  (`mpCheckLeftWall`/`mpCheckRightWall`), the position is set to
  `edge - ecb.bottom` (X and Y), the current floor line and `grounded` state
  are kept, and `Fighter.edge_contact` is set to the crossed side for this
  frame only. No facing or stick condition. mpcoll only moves the position
  (mpcoll.c:3946-3950 sets `touching_floor = true` without touching
  velocity); `gr_vel` is untouched, so the clamp repeats every remaining
  frame the fighter tries to move past the end, without ever sliding
  through.
- **Teeter** (`mpColl_8004B4B0`, `ft_80084280`, mpcoll.c:4020-4025;
  `mpColl_8004B5C4`/`ft_800843FC` is the identical rule with a different ECB
  load, used by StopWall; `mpColl_8004A678_Floor`, mpcoll.c:3653-3736): the
  same clamp, admitted only when facing and stick allow it
  (`facing_dir == -1 && lstick_x > -0.75` for the left end, `facing_dir == 1
  && lstick_x < 0.75` for the right end -- 0.75 is mpcoll.c's own literal,
  supplied explicitly as `rules.edge.teeter_stick_limit`), and only then
  enters Ottotto (`ftCo_8009A3C8`: `Collide_Edge && !x2228_b2`; the Sandbag
  exemption `x2228_b2` is not modeled, no such character exists here).
  Otherwise (facing away, or an outward stick at or past 0.75) the fighter
  falls exactly as under Plain.

With `rules.edge` absent, Clamp still applies (it is a collision rule with
no data of its own); Teeter has no teeter states to enter and degrades to
Plain (`game::edge::mode_for_action`'s `edge_rules_present` parameter).

Skip the clamp entirely when a wall line intersects the segment from the
edge point to the far ECB side. This reuses `Stage::sweep` (already this
codebase's own translation of the wall-line crossing test, including joint
bounding and extension) rather than hand-porting `mpCheckLeftWall`/
`mpCheckRightWall`'s distinct `joint_id_skip`/`joint_id_only` traversal and
NULL-output variant; behaviorally both ask "does an active wall line cross
this segment." One simplification worth being explicit about: the source
only calls `mpCheckLeftWall`/`RightWall` once the Teeter facing/stick gate
already passed, so a malformed wall line can never affect the decision when
facing/stick would have failed anyway; `game::collision::floor_end_clamp`
always evaluates the wall query first, so a non-finite wall-line endpoint
that the source would never reach still propagates as an `Error::Physics`
here. This can only matter for malformed stage data, not for any behavior a
fighter's own input can trigger.

### Per-action table

Verified directly against the decomp for every action Skirmish implements
(see the `_Coll` callback and line range cited). Actions not implemented
here (`AppealS`, `ItemThrow`, the Sandbag-only branches, `StopWall`,
`DamageSong`, `DamageBind`) are omitted.

| Mode | Actions | `_Coll` callback | Source lines |
|---|---|---|---|
| Plain | Squat, SquatWait, SquatRv, Turn, Dash, Run, DownBound, DownWait, DownDamage, DownForward, DownBack, DownStand, GuardOn, Guard, GuardOff, GuardReflect | `ft_80083F88`/`ft_800844EC`/`ft_800845B4` (mode 0) | ft_081B.c:1027,1129,1139; ftCo_DownBound.c:250,326; ftCo_Down*.c; ftCo_Guard.c:488,557,625,1072 |
| Plain (conditional, unmodeled) | GuardSetOff | `ft_800845B4` normally, `ft_80084104` (Clamp) only while `fp->allow_sdi` | ftCo_Guard.c:838-845 |
| Clamp | Jab, Attack12, Attack13, Attack100Start/Loop/End, AttackS3\*, AttackHi3, AttackLw3, AttackS4\*, AttackHi4, AttackLw4, AttackDash, EscapeF, EscapeB, EscapeN, Catch, CatchDash, CatchCut, DownAttack, PassiveStandF/B, CliffClimb, RunTurn, Ottotto, OttottoWait | `ft_80084104` (mode 2); Catch/CatchDash use `ft_800841B8` (same mode-2 gate, with an extra release-the-grab call on ground loss before falling); RunTurn and Ottotto/OttottoWait use `ft_800827A0` directly | ft_081B.c:1043; ftCo_Down.c:77; ftCo_DownAttack.c:65; ftCo_PassiveStand.c:63; ftCo_CliffClimb.c:124-135; ftCo_Catch.c:163-179; ftCo_TurnRun.c:122-137; mpcoll.c:3584-3652 |
| Teeter | Wait, Walk, Landing, RunBrake | `ft_80084280`/`ft_800843FC` (mode 1) | ft_081B.c:1081,1121; mpcoll.c:3653-3736 |

`ftCo_TurnRun_Coll` (`RunTurn`) runs one extra step after the mode-2 clamp:
when `Collide_LeftEdge`/`Collide_RightEdge` is set, it calls
`ftCommon_8007E2FC` (unexamined; likely a speed/turn-completion reaction).
This extra call is not modeled -- `RunTurn` gets the same position clamp as
every other Clamp action, without that additional reaction.

This corrects the escape batch's "falls off a floor edge" expectation
(`docs/shield.md`, `tests/game_escape.rs`): rolls and the spot dodge use
`ft_80084104` (Clamp), so they stop exactly at a floor end instead of
sliding off it. `docs/validation.md`'s shield-escape entry is annotated with
a bracketed correction rather than rewritten, to keep the historical record
intact.

One further deviation from a literal port, documented rather than modeled:
`ft_80084280`'s own body (ft_081B.c:1093-1103) uses `ft_800827A0` (Clamp)
instead of the teeter inline whenever `fp->xF8_playerNudgeVel.x` opposes the
current facing -- an overlap-push special case. This dynamic, per-frame
override is not modeled; Teeter's facing/stick gate always applies
regardless of any concurrent nudge.

## Ottotto (245, animation 210) and OttottoWait (246, animation 211)

`ftCo_Ottotto.c`. Both new `Action` variants share Wait's action-instance
identity (`fighter::action_instance::motion_identity` has no explicit arm
for either, matching Wait's own fallthrough to `_ => 0`).

- **Entry** (`ftCo_8009A410`, `game::edge::enter`): `ChangeMotionState
  (Ottotto, Ft_MF_None)`, self velocity and ground velocity zeroed. The
  fighter stays grounded at the position the Teeter clamp already set;
  `game::collision::floor_end_clamp` calls this in the same step that
  clamps the position.
- **Animation end** (`ftCo_Ottotto_Anim`, `game::edge::update_animation`):
  once `action_frame` reaches the end of the supplied `fighter.teeter.start`
  poses, enters OttottoWait (`ftCo_8009A6B8`; the sound cue is not
  modeled). OttottoWait has no `Anim` callback -- it holds `fighter.teeter.
  wait`'s single pose indefinitely.
- **IASA** (`ftCo_Ottotto_IASA`/`ftCo_OttottoWait_IASA`, identical): checks
  SpecialS, Attack100, two unmodeled smash-adjacent predicates
  (`ftCo_800D6824`/`800D68C0`), catch, every smash and tilt, jab, shield
  entry (`ftCo_80091A4C` only -- no held-shoulder GuardOn branch), taunt
  (unmodeled), jump, dash, squat, turn, then `ftCo_Walk_CheckInput_Ottotto`.
  Skirmish exposes this as `tilt::interrupt_chain` returning `Chain::Wait`
  whenever `edge::owns_action(fighter.action)` -- the same chain Wait itself
  exposes, so every existing catch/smash/tilt/jab/shield/jump/dash/squat/
  turn dispatcher picks it up automatically through their pre-existing
  `interrupt_chain(...) == Some(Chain::Wait)` checks, with no new call sites
  needed. `locomotion::update_actions`'s shared Wait/Walk arm reuses the
  exact same squat/turn/walk logic, with one addition: entering Walk from
  Ottotto/OttottoWait additionally requires `stick_x * facing >=
  rules.edge.teeter_walk_threshold` (`ftCo_Walk_CheckInput_Ottotto`,
  `fighter::edge::teeter_walk_allowed`), ANDed with the ordinary walk
  predicate.
- **Phys** (`ftCo_Ottotto_Phys`/`ftCo_OttottoWait_Phys`): both empty. No
  friction, no movement; `simulation::move_fighter` special-cases
  `edge::owns_action(f.action)` to apply neither walk nor ordinary ground
  friction.
- **Coll** (`ftCo_Ottotto_Coll`/`ftCo_OttottoWait_Coll`, identical):
  `ft_800827A0` (Clamp). If the ground is lost, falls (the ordinary Plain
  fall-through already in `game::collision::resolve`, since neither action
  is in the Clamp/Teeter table's admission list for a *new* clamp -- their
  own mode 2 either re-clamps or, on genuine loss, is indistinguishable from
  Plain). Otherwise, once per frame after collision settles
  (`game::edge::check_exit`), compares the current position against the
  current floor's end on the *facing* side (`mpFloorGetRight` for
  `facing_dir > 0`, `mpFloorGetLeft` otherwise): `|position.x - end.x| >
  teeter_exit_distance + teeter_exit_tolerance` enters Wait (`ft_8008A2BC`).
  Since neither action has any self-driven movement, only an external
  displacement (an overlap nudge, in this profile) can make that distance
  grow while still in Ottotto/OttottoWait.
- **Slippi**: states 245/246 with animation indices 210/211
  (`crates/skirmish-replay/src/observation.rs`'s `action_state`/
  `animation_index`, filling the existing gap between `Pass => 244` and
  `FlyReflectWall => 247`, and between `244 => 209` and `247 => 212`).

## Resource shape

- `rules.edge: Option<edge::Rules { teeter_stick_limit, teeter_walk_threshold,
  teeter_exit_distance, teeter_exit_tolerance }>` (`+474`/`+478`/`+47C` plus
  the 0.75 literal, supplied so the resource is explicit).
- `fighter.teeter: Option<edge::Teeter { start: Vec<TeeterFrame>, wait:
  TeeterFrame }>`, `TeeterFrame { bones, hurtbox_states }` -- the same shape
  as `AttackFrame` minus `hitboxes`, since neither Ottotto nor OttottoWait
  can hit anything. `rules.edge` and every fighter's `teeter` are required
  together (`game::validation`), matching the existing escape/escape-air/
  grab pairing convention.
- `Fighter.edge_contact: Option<edge::EdgeSide>`: set for exactly the frame
  a Clamp or Teeter clamp held the position (`Collide_LeftEdge`/
  `Collide_RightEdge`/`Collide_Edge`, folded into one side flag since
  Skirmish does not otherwise track raw `env_flags` bits), reset every
  collision step.

## Tests

`tests/support/edge.rs` supplies invented `rules.edge`/`fighter.teeter`
values (`teeter_stick_limit` 0.75, matching the source; `teeter_walk_
threshold` 0.6, chosen above the note's own stick-0.5 examples so "stays in
Ottotto" and "walks away" are distinguishable; `teeter_exit_distance`/
`teeter_exit_tolerance` 2.0/0.5).

`tests/game_edges.rs` builds on the escape, tilt, smash, dash and jab
profiles, fighter 1 spawned far away: a forward smash's own TransN root
motion and a supplied root-motion dash attack reaching a floor end both
clamp and stay grounded (`edge_contact`, exact position, still playing out
their own animation, returning to Wait normally afterward);
`tests/game_escape.rs`'s roll test is mirrored here too. Walking toward the
end with an admissible stick (0.5, matching the design note's own example)
enters Ottotto with zero velocity, then OttottoWait once its poses are
spent, holding position exactly; an outward stick at exactly the 0.75 limit
falls instead (mpcoll's literal is exclusive); a fighter pushed into the
opposite floor end by an overlap nudge while facing away also falls; Dash
and Run past an end fall as before (Squat's own horizontal immobility makes
a standalone "squats past the end" scenario require contrived residual
velocity that adds nothing the clamp logic itself doesn't already cover, so
it is not separately exercised). From Ottotto: catch, smash, shield and jump
all fire through the exposed Wait chain, and a same-direction dash
correctly carries the fighter off the cliff it was teetering at (Dash's own
mode 0 is unaffected by `rules.edge`) rather than staying frozen; Turn is
separately confirmed reachable. Walking toward the edge below the teeter
walk threshold stays in OttottoWait. Checkpoints restore both Ottotto and
OttottoWait exactly. Invalid `rules.edge`/`fighter.teeter` combinations are
rejected before a match exists. With `rules.edge = None`, Wait/Walk simply
fall off an end while a smash still clamps.

Not covered by an integration test: the exit-to-Wait distance check firing
from a genuine external push while still in Ottotto/OttottoWait. Engineering
one without disrupting the walk that reaches Ottotto in the first place, or
relying on Turn's own facing flip (which leaves the Ottotto/OttottoWait
action on the same frame it fires, before the exit check can run against
it), proved impractical within this batch's scope; the pure
`fighter::edge::exit_distance_exceeded` check is unit-tested directly, and
every Ottotto/OttottoWait integration test already holds position at
distance 0 for many frames without a spurious exit, exercising the same
code path's negative case.

`crates/cli/tests/replay_match.rs`'s `file_backed_teeter_walk_then_jump_
matches_and_detects_the_first_outward_stick_frame` walks into a floor end
(states 245 then 246, animations 210/211), holds, then jumps out (Jump,
airborne), matches its own Peppi bytes, and reports a `Mismatch` at the
exact row the walk crosses the edge when that row's stick sample is pushed
out to precisely 0.75 instead of 0.5.

A pre-existing bug was found and fixed as part of this batch, since it
directly affects jumping out of Ottotto/OttottoWait (though it is not unique
to them): `locomotion::update_actions` computes `interruptible_tilt` once,
before dispatching the jump-squat entry; entering `Action::JumpSquat` did
not return, so the shared Wait/Walk arm below (whose guard still matched on
the stale `interruptible_tilt`, unaware `f.action` had just changed) ran
again on the same frame and could overwrite the fresh JumpSquat with Walk
whenever the jump press carried an admissible walk stick. Every Wait-chain
IASA is a `RETURN_IF` chain that returns immediately once a jump fires
(`ftCo_Wait_IASA`: `RETURN_IF(ftCo_Jump_CheckInput(gobj))`, before
`ftCo_Dash_CheckInput`/`ftCo_800D5FB0`/`ftCo_Turn_CheckInput`/the walk check
ever run), so `update_actions` now returns immediately after the jump-squat
entry too. No other action entry in this function has the same stale-guard
exposure: every other transition (Dash, Squat, Turn, Walk, Run, RunBrake)
happens from within its own exclusive arm of the second `match f.action`
dispatch, which cannot fall through to a stale guard within the same
function call. `tests/game_edges.rs`'s
`jump_squat_overwrites_walk_only_after_it_wins_not_before` covers an
interruptible smash pose, an interruptible jab pose and Ottotto itself, all
pressing jump with an admissible walk stick and reaching JumpSquat.

## Oracle

`tests/oracle/edge_floor.c` (alias `edge_floor` -> the existing `mpcoll`
snapshot, `tests/oracle/adapters.json`) pins `mpColl_8004A45C_Floor` and
`mpColl_8004A678_Floor` with `mpLib_80054ED8`/`mpLineGetKind`/`mpFloorGetLeft`/
`mpFloorGetRight`/`mpLib_8004DD90_Floor` (always reporting no continuation --
the only configuration Skirmish's own project-failure precondition reaches)
and `mpCheckLeftWall`/`mpCheckRightWall` (a single scripted bit) stubbed.
`tests/edge_differential.rs` compares the pure `fighter::edge::resolve`/
`clamped_position` helpers against it over 512 proptest cases (holding
`ecb.bottom.x` at exactly 0.0, the real invariant every ECB loader in
`src/collision/ecb.rs` enforces -- an earlier draft that randomized it
surfaced a spurious mismatch tracing back to this precondition, since the
source's own `coll->cur_pos.x` comparison and Skirmish's `position[0] +
ecb.bottom[0]` only agree under it) plus exact boundaries (`x == edge.x`
both sides, `stick == ±0.75`, facing mismatch, wall blocked).

`tests/oracle/ottotto.c` pins `ftCo_8009A3C8`, `ftCo_8009A410`,
`ftCo_Ottotto_IASA`, `ftCo_Ottotto_Coll`, `ftCo_8009A6B8` and
`ftCo_OttottoWait_Coll` (`tests/oracle/original/ottotto.c`), with every
`ftCo_Ottotto_IASA` callee stubbed and logged. `tests/ottotto_differential.rs`
compares the complete traced call order (a `RETURN_IF` chain over 512
arbitrary scripted-answer bitmasks) against a literal Rust mirror of the
pinned order, and the shared Coll fall/exit decision against
`fighter::edge::exit_distance_exceeded`, plus exact boundaries and the two
entry motion ids.

`tests/oracle/walk.c` pins `ftCo_Walk_CheckInput_Ottotto`
(`tests/oracle/original/walk.c`, a new snapshot -- the existing `ftwalk`
adapter pins `ftwalkcommon.c`'s free functions, a different file from
`ftCo_Walk.c`), with `ftWalkCommon_800DFC70` (the ordinary walk predicate
this function ANDs its own gate with) scripted rather than re-extracted.
`tests/walk_differential.rs` compares `fighter::edge::teeter_walk_allowed`
against it over 512 proptest cases plus exact boundaries.
