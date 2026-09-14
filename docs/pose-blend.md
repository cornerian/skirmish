# Joint-pose blending across a motion-state entry

Pinned decomp: Fox/Falco/common fighter code under `/mnt/shared/Projects/Code/External/melee/src`.
Sources: `ft/fighter.c:1236-1300` (`Fighter_ChangeMotionState`), `ft/ftanim.c:314-428,925-940`
(`ftAnim_8006E9B4`, `ftAnim_8006EBE8`, `ftAnim_8006FE9C`), `melee/lb/lb_00B0.c:469-550`
(`lb_8000C490`), `sysdolphin/baselib/mtx.c:412-434` (`HSD_MtxSRTQuat`), `sysdolphin/baselib/jobj.c:138-196`
(`HSD_JObjMakeMatrix`), `dolphin/mtx/mtx.c` (`C_MTXQuat`/`MTXQuat`, `C_MTXScale`, `PSMTXTrans`).

## Mechanism

Melee does not snap a fighter's model to its new figatree instantly on every
motion-state entry. `Fighter_ChangeMotionState` calls `ftAnim_8006EBE8(gobj,
anim_start, anim_rate, blend)` with `blend = (anim_blend == -1) ? 0 :
anim_blend ? anim_blend : (*unk_byte_ptr)[0]`, where `unk_byte_ptr =
&fp->x28[fp->anim_id]` is a per-subaction `u8[2]` (byte 0: the default blend
frame count). `ftAnim_8006EBE8` sets `x8A4_animBlendFrames = blend`,
`x8A8_anim_frame = 0`.

Every anim-phase frame, before the script and the destination state's own
anim callback (`fighter.c:1684-1694`), `ftAnim_8006E9B4` runs. With `blend >
0`: `x8A8 += framerate` (1.0 for every animation this codebase samples); `t =
(blend <= x8A8) ? 1 : framerate / (framerate + (blend - x8A8))`, `t_inv = 1 -
t`; the figatree is applied to a separate "anim" skeleton; then
`ftAnim_8006FE9C(fp, FtPart_TransN, t, t_inv)` blends every part from
`TransN` down (bone 0, `TopN`, is never touched -- it keeps the root-facing
rotation `simulation::pose` already sets) through `lb_8000C490`: translation
and scale are `a*t + b*t_inv`; rotation copies the anim (figatree) value
directly when both sides are already plain Euler and every component
differs by `<= 1e-4`, otherwise both are converted to quaternions
(`EulerToQuat`), the model's previous-frame quaternion is negated if its
squared-difference sum from the anim quaternion exceeds its squared-sum sum,
and `HSD_QuatLib_8037EF28(anim, model_prev, out, t_inv)` produces the blended
quaternion, with `JOBJ_USE_QUATERNION` set on that joint from then on. When
`t` reaches `1` the pose equals the figatree exactly, and `blend == 0`
(default byte absent, `-1` passed explicitly, or a fresh action that never
gets an anim-phase update before this frame's own pose read) skips blending
entirely, matching this codebase's pre-batch behavior byte for byte.

## Scope of this batch

**Wired generically:** `game::pose_blend` (`src/game/pose_blend.rs`) owns the
per-fighter blend state (`Fighter::pose_blend`, checkpointed like every other
physics field) and the recursion itself. `game::simulation::enter` (the
single choke point every action-entry path in this codebase already calls,
the same role `Fighter_ChangeMotionState` plays in the source) arms the
blend on every motion change. `game::simulation::update_pose_blend`, called
once per active fighter per frame at the very top of `advance`'s per-player
loop -- before the hitlag branch's own `pose`/`collision::sample` call, the
active-fighter branch's own pose reads, and any state transition this frame
-- resolves the pack's default byte (see below) and advances the recursion;
`game::simulation::pose` then reads the resulting blended joints instead of
the raw figatree sample whenever a blend is active.

**Explicit call-site values: reported, not wired.** The full decomp rule has
three cases (`-1` forces 0, a nonzero literal overrides, otherwise use the
subaction's default byte). Only the third case is wired generically here,
via `data::MovementPoses.blend_frames: Option<u8>` (a new pack field,
`#[serde(default)]`, so packs v10..v13 -- which carry none -- resolve to `0`
and blend exactly as before this batch). A grep of every direct
`Fighter_ChangeMotionState(` call site across `ft/fighter.c`, `ft/ftanim.c`,
every top-level `ft/*.c`, `ft/kinds/ftCommon/*.c`, `ft/kinds/ftFox/*.c` and
`ft/kinds/ftFalco/*.c` (292 call sites) found only three passing a literal
`-1` or nonzero blend, none in Fox's or Falco's own code:

| file:line | blend | context |
| --- | --- | --- |
| `ft/ft_0D4D.c:128` | `-1.0f` | `ftCo_800D4FF4`, the death/rebirth respawn transition (msid `0xC`) |
| `ft/kinds/ftCommon/ftCo_Fall.c:78` | `-1.0F` | `ftCo_Fall_Enter_YoshiEgg`, falling after being spat out of Yoshi's egg |
| `ft/kinds/ftCommon/ftCo_ItemParasolOpen.c:66` | `10.0f` | `ftCo_800CEFE0`, an alternate/duplicate parasol-open path (`ft_800CEF08`'s sibling, which uses the ordinary default-byte `0.0f`) |

None of the three is reachable from this codebase's implemented action set
(no Yoshi egg, no held-item parasol mechanic, and the respawn transition
already uses `simulation::spawn`'s own from-scratch `Fighter` construction
rather than an ordinary `enter` call). `simulation::enter` therefore always
calls `pose_blend::arm` with `explicit: None`; `arm`'s parameter already
carries the contract for a future batch to thread a real explicit value
through if a modeled action ever needs one.

**Per-subaction byte pair, mirroring the exporter exactly.** Every
pose-bearing profile carries its own `blend_frames: u8`/`dynamics_variant:
u8` pair (`data::Blend`; `dynamics_variant` is `ftCo_8009E7B4`'s unrelated
cosmetic jiggle-bone selector, kept only for schema fidelity, never
consulted here), matching `fp->x28[]`'s real per-subaction shape and the
`skirmish-assets` exporter's own `blend_bytes`/`fighter::Subaction` field
pair (`14eaf99`, `src/gameplay/minimal.rs`) field for field:

- Every attack-shaped profile (`data::Attack`, used directly by jab, tilts,
  smashes, dash attack, ledge attacks, the knockdown get-up attack, every
  aerial's `Move.attack`, and every special phase via `Phase{ground, air}:
  Attack`) carries `blend_frames`/`dynamics_variant` as plain sibling
  fields next to `frames`, matching `build_attack`'s own JSON object shape
  verbatim.
- Every other pose-bearing struct carries the same pair, named to match the
  exporter's own (sometimes prefixed) JSON keys exactly: `wall_jump::
  Attributes`, `ledge::Motion` (plus `phase2_blend_frames`/
  `phase2_dynamics_variant` for `Jump.motion`'s concatenated second phase),
  `ledge::Parameters.wait_blend_frames`/`wait_dynamics_variant` (`wait`
  itself stays a bare `Frame`), `escape::RollMotion`, `escape::
  SpotDodgeMotion`, `escape_air::Parameters` (main frames, plus
  `landing_poses_blend_frames`/`dynamics_variant`), `edge::Teeter`
  (`start_`/`wait_` prefixed), `taunt::TauntAnimation`, `clank::Animation`
  (`poses_` prefixed; Rebound only -- ReboundStop samples a frozen state
  snapshot, not a figatree, so it has no pack byte), `aerial::Move.
  landing_poses_blend_frames`/`dynamics_variant`, `grab::Catch`, `grab::
  Pummel` (`poses_` prefixed), `grab::CaptureDamage` (`high_`/`low_`
  prefixed), `grab::Escape` (`catch_cut_poses_`/`capture_cut_poses_`
  prefixed), `grab::Throw` (`poses_` prefixed), `damage::KnockdownAttributes.
  passive_poses_blend_frames`/`dynamics_variant`, `damage::
  ProneRecoveryAttributes` (`bound_poses_`/`wait_poses_`/`stand_poses_`
  prefixed; the optional `damage_poses` field has no pack byte -- the
  exporter does not populate it either, `docs/pose-blend.md` per its own
  `build_prone_recovery` doc comment), `damage::FloorTechMotion`, `damage::
  SurfaceResponseAttributes` (`wall_poses_`/`ceiling_poses_` prefixed),
  `damage::SurfaceTechAttributes` (`passive_wall_poses_`/
  `passive_wall_jump_poses_`/`passive_ceiling_poses_` prefixed).
- `data::MovementPoses` and `damage::DamagePoseAttributes` each carry a
  same-shaped `blend: Option<_>` sidecar (`MovementPosesBlend`,
  `DamagePosesBlend`) instead of widening their own bare bone-array fields,
  matching the exporter's own `"blend": {...}` sidecar convention exactly
  (one `Blend` entry per named movement field; `ground`/`air`/`fly`-shaped
  for damage poses).

Every one of these fields is `#[serde(default)]` (or the containing
`Option` defaults to absent), so packs v10..v13 -- which carry none of
them -- resolve every profile to `Blend::default()` (`blend_frames: 0`) and
behave exactly as before this batch.

`game::pose_blend::resolve_default_byte` (called once per fighter per frame
from `simulation::update_pose_blend`) picks the byte from whichever profile
the current action actually uses, mirroring `simulation::local_pose`'s own
dispatch chain arm for arm -- each pose-source module (`grab`, `ledge`,
`wall_jump`, `damage`, `escape`, `escape_air`, `edge`, `taunt`, `clank`,
`aerial`, `movement`) exposes a `blend_frames`-named sibling to its own
`pose`-selecting function, reading the identical profile's own byte instead
of its bones. This was corrected after an initial version of this batch
shipped a single `MovementPoses.blend_frames` shared across every movement
sub-motion, which would have blended Dash (real byte `0`) using Wait's real
byte (`6`) or vice versa; the real per-subaction table (Fox: `Wait1`/
`WalkSlow`/`WalkMiddle`/`WalkFast`/`Run` at `6`, `Dash`/`Landing`/every
aerial/every SpecialN phase at `0`) makes that distinction load-bearing.

Verified end to end against a hand-injected `movement_poses`/`blend`
fragment (`/mnt/shared/tmp/skirmish-pose-blend-scratch/
movement_poses_wait6_dash0.json`, mirroring the exporter's own shape with
`wait.blend_frames = 6`/`dash.blend_frames = 0`) through the real
`Match::step` pipeline, not just `game::pose_blend`'s own isolated unit
tests: `tests/game_dash.rs`'s new `dash_snaps_and_the_later_wait_reentry_
blends_over_the_packs_own_six_frame_byte` drives a real Wait -> Dash ->
AttackDash -> Wait sequence and confirms Dash's own entry snaps instantly
while the later Wait re-entry blends from AttackDash's settled pose over
exactly six frames, landing on `rest + (target - rest) * (k / 6)` after the
k-th frame -- the same linear-ramp closed form `game::pose_blend`'s own
`blend_t_sequence_is_exactly_1_over_n_counting_down` unit test derives
directly from the recursion.

## bones::Pose and the C oracle

`collision::bones::LocalTransform` gained `rotation_quaternion: Option<[f32;
4]>` (`JOBJ_USE_QUATERNION`); `srt` dispatches to the new `srt_quat`
(`HSD_MtxSRTQuat`, `mtx.c:412-434`) when set, composing `MTXScale`/`MTXConcat`/
`MTXQuat`/`PSMTXTrans` through the already-pinned `concat` (`C_MTXConcat`)
exactly as the source's own call sequence does. `MTXQuat` resolves to
`C_MTXQuat` (`sysdolphin/baselib/mtx.h:88`'s own `#define MTXQuat
C_MTXQuat`), already pinned in `tests/oracle/original/sdk_mtx.c` from an
earlier batch; `MTXScale`/`PSMTXTrans` are definitionally an assignment-only
diagonal/translation matrix (`dolphin/mtx/mtx.c:706-723,766-781`), so no
floating-point operation exists for the PS-asm and scalar variants to
disagree on.

`tests/oracle/bones_pose.c` extracts the real `HSD_MtxSRTQuat` (previously
an `abort()` stub, since the oracle's own driver never set
`JOBJ_USE_QUATERNION`) and `C_MTXQuat`/`C_MTXScale` (added to
`sdk_mtx.functions.json`, defined once in `bones.c`, linked via `extern` from
`bones_pose.c`, matching the existing `C_MTXConcat` pattern). `HSD_JObj.rotate`
widens from `Vec3` to `Quaternion` (the real field's own type -- the Euler
branch's call site already casts it to `Vec3*` in the pinned `jobj.c`
extraction). `oracle_bones_pose` takes an added `use_quaternion`/`quaternion`
pair of flat arrays and sets `JOBJ_MTX_DIRTY | JOBJ_USE_QUATERNION` per
joint accordingly. `tests/bones_differential.rs` adds
`quaternion_srt_tracks_original_with_host_trig_free_tolerance` (a property
test over generated scales/quaternions/translations, no parent scale, direct
against a single-joint oracle call -- `HSD_MtxSRTQuat` has no `sinf`/`cosf`
of its own, so there is no host-libm-vs-MSL trig noise to tolerate here
unlike the Euler `srt` proptest) and
`quaternion_joint_matches_oracle_under_a_scaled_parent` (a synthetic
two-joint hierarchy exercising the parent-scale-compensated branch through
`Pose::evaluate` end to end, mirroring
`hierarchical_capsule_endpoints_match_original_matrix_operations`'s existing
style for the Euler path).

## Validation

With every pack's `blend_frames` absent, `resolve_pending` always resolves to
`0`, `pose_blend::advance` always takes the `frames == 0` branch and returns
the raw target unchanged, and `simulation::pose`'s own `blend_frames > 0`
gate never consults the stored blend either way -- byte-for-byte identical
to this codebase's pre-batch pose sampling, decoupled from whether
`update_pose_blend` ran (see its own doc comment on the ordering caveat).

Confirmed two ways: the full native (`cargo test --workspace`) and C-oracle
(`--features c-oracle`) suites pass unchanged, and the real-replay ratchet
(`crates/cli/tests/real_parity.rs`, `SKIRMISH_GAMEPLAY_DATA` pointed at
`v13-snapshot-20260914`) was run against both this commit and the
unmodified base (`8160023`, a separate worktree, untouched): every one of
the nine recordings' `outcome.frame`/`checked_frames` is bit-for-bit
identical between the two.

| recording | frame | checked_frames | status vs. checked-in baseline |
| --- | --- | --- | --- |
| fox-fd | 5 | 128 | within baseline |
| fox-fd-2 | -6 | 117 | **regressed** (baseline -5) |
| fox-fd-3 | -32 | 91 | within baseline |
| fox-fd-4 | -14 | 109 | **regressed** (baseline -5) |
| fox-bf | 25 | 148 | **regressed** (baseline 174) |
| fox-ys | -113 | 10 | within baseline |
| fox-fod | -118 | 5 | within baseline |
| fox-dl | -49 | 74 | within baseline |
| fox-ps | -6 | 117 | within baseline |

The three regressions (fox-fd-2, fox-fd-4, fox-bf) are pre-existing at base
`8160023`, unrelated to this batch: `real_parity` already fails there with
the identical `frame`/`checked_frames` values, confirmed by running the
same test against that commit's own separate, unmodified worktree. This
batch does not fix, chase, or otherwise touch that gap; it only establishes
that pose blending did not move it.
