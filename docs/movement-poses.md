# Per-frame poses for movement states (design note)

Evidence from the parity replay (`fox-fd.slp`, P1, frames -59..-49): Fall
begins at -59 from y 9.8149 and accumulates gravity every frame (-0.46,
-0.69, ... -2.07, -2.30 per frame); at -51 TopN is already -0.3051 and at
-50 it is -2.6051 while the fighter is still airborne; Landing (42) comes at
-49 with TopN snapped to 0.0001.

Skirmish before this batch (`src/game/simulation.rs:1728`, `pose()`): attacks
pose from their per-frame `AttackFrame.bones`; every other action used the
static rest pose `FighterData.bones`. The ECB (`CollisionBox::Bones`,
`src/game/collision.rs`, `ecb::load_joints`) therefore never changed shape
during Wait/Walk/Dash/Run/Jump/Fall/Landing/Squat.... Melee poses every
state from its figatree every frame (`ftAnim_8006EBA4`) and reads the ECB's
six bones from that pose (`ft_081B.c:36-68`). This batch adds that missing
per-frame pose for every movement (non-attack) action this codebase already
models the timing of: `src/game/movement.rs`.

## Resource shape
`FighterData.movement_poses: Option<MovementPoses>` where each field is a
`Vec<Vec<Bone>>` (one local-bone list per animation frame, exactly the
`AttackFrame.bones` layout), keyed by the sub-motion it comes from: `wait`
(Wait1_0), `walk_slow`, `walk_middle`, `walk_fast`, `turn`, `turn_run`,
`dash`, `run`, `run_brake`, `knee_bend`, `jump_f`, `jump_b`,
`jump_aerial_f`, `jump_aerial_b`, `fall`, `fall_f`, `fall_b`,
`fall_aerial`, `fall_aerial_f`, `fall_aerial_b`, `fall_special`,
`fall_special_f`, `fall_special_b`, `landing`, `landing_fall_special`,
`squat`, `squat_wait`, `squat_rv`, `pass`, `ottotto`, `ottotto_wait`,
`entry_start`. Optional fields for sub-motions Skirmish does not model yet
may be omitted; every field's frame count and bone count (equal to
`FighterData.bones.len()`) is validated
(`game::validation::validate_movement_poses`, reusing
`validate_animation_pose`).

## Behaviour (implemented)
`simulation::pose` calls `movement::pose` after every other pose source
(grab/ledge/wall-jump/damage/escape/edge/taunt/rebound/attack/landing) and
before the final rest-pose fallback, so an absent `MovementPoses` (or an
absent field within it) is exactly the pre-batch behavior for that action.
`movement::pose` selects `movement_poses.<field>[frame]`, where `<field>`
and `frame` follow the exact same per-action rules
`crates/skirmish-replay/src/observation.rs`'s `action_age` already
establishes for the reported Slippi state age:

- Walk/Run read their own continuous frame counters
  (`fighter.locomotion.walk.frame`/`run.frame`), already advanced and
  wrapped earlier in `simulation::advance` than any collision phase.
- Wait reads `fighter.idle.frame`, and only while `fighter.idle.animation
  == 2` (Wait1_0); every other idle sub-motion `game::idle` can cycle to
  keeps the rest pose, since it has no track of its own here.
- Every other mapped action reads `fighter.action_frame` directly: this
  module runs during the frame's own collision phase, strictly before the
  shared end-of-frame `action_frame += 1` (`simulation.rs`'s later
  per-player loop), so the value it sees is already the same pre-increment
  value `observation.rs`'s general rule (`action_frame.saturating_sub(1)`)
  has to reconstruct from the post-increment field it only sees after
  `advance` returns. Dash is the one exception (`observation.rs` already
  documents it): `ftCo_Dash_Enter` (`ftCo_Dash.c:48-63`) calls `ftAnim_
  8006EBA4(gobj)` itself on entry, one frame ahead of every other action's
  convention for its whole duration, so this module reads `action_frame +
  1` for Dash alone.
- Jump/JumpAerial select the `_f`/`_b` field via `fighter.locomotion.
  jump_backward` (set once at launch, matching Slippi's own JumpF/JumpB,
  JumpAerialF/B ids).
- Fall selects `fall_aerial` when `fighter.locomotion.fall_aerial`, else
  `fall`; FallSpecial selects `fall_special`. `ftCo_Fall_Anim_Inner`'s
  continuous air-drift blend between the neutral/forward/backward
  figatrees (`ftCo_Fall.c:110-172`) is not modeled: the `_f`/`_b` fields for
  Fall/FallAerial/FallSpecial are validated if supplied but never selected,
  reserved for a future blend batch.
- Loop vs. hold: Fall, FallAerial, FallSpecial, SquatWait and OttottoWait
  persist indefinitely, so their pose index wraps
  (`game::movement::loop_period`, see below); every other mapped action is
  bounded by an existing frame threshold that transitions it away before
  its own `action_frame` can exceed the matching pose array
  (`game::locomotion::update_actions` runs, and applies any such
  transition, earlier in `simulation::advance` than the collision phase
  that reads the pose), so holding at the last supplied frame is a
  defensive clamp for those, except EntryStart, which genuinely holds its
  own figatree's last frame for the rest of its 30-frame action duration
  (matching `observation.rs`'s own EntryStart cap).
- Root motion (TransN) is untouched by this batch: every supplied frame's
  bone 0 has translation `[0.0; 3]` (the same convention `AttackFrame`
  poses already use), so `simulation::pose`'s root-matrix composition from
  `Fighter.position`/`facing` is the only translation applied; nothing here
  double-applies it.

## The loop-wrap discovery (supersedes this note's original re-entry guess)
This note originally speculated that the replay's frame -51 (see below) was
a discrete `ftCo_Fall_Enter` re-entry -- i.e., that some decomp call site
re-runs `Fighter_ChangeMotionState` on an already-falling fighter -- and
listed candidates to trace (`ftCo_Fall.c`'s callbacks, `ft_081B.c`'s
`ft_80083E64_inline`/`ft_80081A00`, `ftcommon.c`'s `ftCommon_8007D5D4`/
`_8007D7FC`/`_8007D6A4`, `mpcoll.c`'s ECB clear-on-load, the `x2219_b1`
flag). Tracing those did not find a caller: `ftCo_Fall_Coll`'s own dispatch
(`ft_800831CC`/`ft_80083090_inline`/`mpColl_80047E14`) only reaches
`ftWallJump_8008169C`/`ftCliffCommon_80081298` on a non-contact frame,
neither of which re-enters Fall; the JumpAerial re-jump check embedded in
Fall's own IASA chain (`ftCo_800CB870`) needs a fresh jump input this
recording's P1 never gives; `ftCommon_CheckFallFast`/`FallFast` clamp
velocity directly, which the recording's own smooth, unbroken gravity
accumulation across frame -51 rules out; `ft_80081A00` is item-pickup-only;
and the flagged Entry-related sites (`ft_0C31.c`, `x2219_b1`,
`ftCommon_8007D5D4`'s `ecb_lock`) turned out irrelevant once direct replay
inspection (extending the dump back to frame -75) showed this fall is
P1's own match-start entry drop (`EntryEnd` at 324 through frame -60, Fall
at -59 via `ftCo_EntryEnd_Anim`'s `ftCommon_8007D92C` on the entry timer's
own expiry), not a mid-air jump -- so `ecb_lock`, which only ordinary/aerial
jump entries set, was never armed.

The simpler, correct explanation needs no such caller at all: Fall's own
recorded state age is exactly `0,1,2,3,4,5,6,7,0,1,2,...` -- an **eight**-
frame period, not nine, wrapping the frame after age 7, never reaching 8.
`movement_poses.fall` has exactly nine exported samples. That is precisely
what a nine-sample, closed-loop animation (`ftAnim`'s own loop flag; sample
8 re-evaluates the same point in the figatree as sample 0, the standard
convention for a cyclic clip) produces: an `n`-sample loop wraps every
`n - 1` frames, not `n`. No `Fighter_ChangeMotionState` call, no
`ftCo_Fall_Enter`, is needed to explain "state age resets while staying in
the same action, velocity untouched" -- it is simply what a looping
`cur_anim_frame` does, and Slippi's `state_age` mirrors `cur_anim_frame`
directly. `game::movement::loop_period(frame_count) = frame_count - 1`
implements this (used both by `movement::pose`'s own index and by
`observation.rs`'s `action_age` for Fall/FallAerial/FallSpecial/SquatWait/
OttottoWait, so the reported state age matches the pose actually sampled).
This is implemented and tested
(`crates/skirmish-replay/src/observation.rs`'s
`looping_movement_pose_frames_selects_the_active_sub_motion_and_wraps_one_short_of_the_sample_count`).

## Why the landing-frame divergence remains (measured, not fixed by this batch)
Splicing the real pack's `movement_poses` into `fox-fd/match-data.json` and
re-running `make-initialization`/`validate-replay` against
`fox-fd.slp` reproduces the *same* first divergence `docs/parity.md`
already recorded before this batch: frame -51, field `action_state`,
expected `0x001d` (Fall) vs. actual `0x002a` (Landing) -- Skirmish still
lands two frames early. This batch's poses do not move that frame, and
direct instrumentation explains why: Fox's exported
`collision_box.indices` is `[58, 0, 30, 13, 6, 4]`, and index `0` is the
skeleton's own root joint, whose translation is `[0.0; 3]` in *every*
supplied pose (rest, attack, or movement -- the same TransN-stripping
convention every pose in this dataset follows, see above). Bone 0's world
position is therefore always exactly `Fighter.position` (the root matrix
`simulation::pose` composes from it), contributing an ECB-sample offset of
exactly `0.0` regardless of animation. `ecb::load_joints`'s own bottom
computation (`if bottom < 0.0 { bottom = 0.0 }`, mirroring
`mpColl_LoadECB_JObj`'s identical clamp, `mpcoll.c:425-433`) can then never
produce a bottom offset above `0.0` either: the six-joint minimum can never
exceed bone 0's own `0.0` contribution. So Fox's ECB bottom, under this
export's joint selection, is pinned to exactly `position.y` for every pose
-- the ground-contact sweep (`collision::resolve`'s `floor_query`, built
from `f.ecb.current.bottom`/`f.ecb.previous.bottom`) is therefore
mathematically equivalent to a bare "does raw position cross the floor"
test, identical to the pre-batch rest-pose behavior, however the movement
pose tucks the other five joints. Confirmed directly: `tests/
game_movement_poses.rs`'s synthetic fixture (a `CollisionBox::Bones` config
that samples a *non-root* bone instead) shows the pose-driven ECB delay
mechanism works correctly when the sampled joints are not pinned to the
root; the limitation is specific to this dataset's Fox `collision_box.
indices`, from the earlier, already-pinned ECB batch, not to this batch's
own selection/wiring. Revisiting Fox's `collision_box.indices` (a different
batch's data, out of scope here per this note's own "no new pinned
arithmetic beyond `mpColl_LoadECB_JObj`" rule) is the next step toward
closing this specific divergence.

## Tests
`tests/game_movement_poses.rs`: a synthetic `CollisionBox::Bones` fixture
(sampling a non-root bone, unlike Fox's real export) whose Fall pose sits
above the rest pose lands measurably later than the rest pose does
(`a_fall_pose_whose_sampled_bone_sits_above_the_rest_pose_lands_later`); an
explicit empty `MovementPoses` behaves identically to `None`
(`an_empty_movement_poses_resource_keeps_the_rest_pose_fallback`);
validation rejects an empty frame list and a bone-count mismatch.
`crates/skirmish-replay/src/observation.rs`'s
`looping_movement_pose_frames_...` pins the loop-period arithmetic (9
samples -> period 8) against the replay-recorded numbers. Real-file
measurement with the spliced `v2` pack is recorded in `docs/parity.md`.

## Oracle
No new pinned arithmetic beyond `mpColl_LoadECB_JObj` (already pinned in
the ECB batch); the loop-wrap arithmetic (`n - 1`) is a data-shape
convention (closed-loop sample export), not original-game arithmetic, so
nothing new needs pinning for it either.
