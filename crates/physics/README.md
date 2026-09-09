# physics

An independently buildable Rust translation of selected movement
functions from `src/melee/ft/ftcommon.c` at the revision in `../../upstream.lock.json`.
It supports headless consumers and has no renderer or platform dependencies.

`Movement` owns only the fighter fields read or written by these functions.
Its attributes are caller-provided game data; this crate invents no character
constants, stage geometry, collision results, or frame scheduler. Acceleration
methods set the original acceleration fields and do not silently integrate them.
Each public method documents its original function name. Shared implementation
removes duplicate C arithmetic while preserving comparison order and signed zero.
Movement methods are allocation-free. `bones::Pose` owns allocated hierarchy
tables and resolves sampled Euler poses into world matrices, including HSD parent
scale compensation and classical scaling. Bone-attached capsule endpoints use
the original scalar matrix formulas; radius scale is supplied explicitly.

The bone module does not sample HSD animation or implement quaternion joints, IK,
RObj constraints, independent matrices, or AObj translation overrides. Its libm
trigonometry is tested numerically against host C; matrix concatenation and point
transforms are tested bitwise. Nonuniform ellipsoid collision needs additional
narrow-phase logic and is not approximated by increasing a capsule radius.

`combat` supplies the source capsule/sphere narrow phase, base knockback,
hitlag and initial hitstun counter. Common coefficients and modifiers are explicit
inputs. `sweep::capsule_capsule` translates `lbColl_80006094` from
`src/melee/lb/lbcollision.c`, returning collision and both closest centerline
points for isotropic world-space capsules. A moving sphere uses its previous and
current centers as endpoints. The original near-parallel endpoint selection,
degenerate thresholds, early-rejection output behavior, and floating-point order
are preserved; generic geometry libraries do not reproduce these rules.
`point_segment` and `point_segment_xy` retain the source projection helpers,
including NaN output on zero-length segments. This is not time-of-impact solving
or the matrix-dependent hurt/shield pipeline `lbColl_80006E58`.
Combat scheduling, stale moves, armor and throws remain outside these helpers.

`damage` implements ordinary airborne knockback decay, directional influence,
velocity merging and launch-angle selection, including 361 thresholds and
conditional timer writes. Common coefficients are caller supplied. DI timing,
ASDI/SDI, bounce, grounded projection and damage callbacks are separate concerns.

`stage` translates directed floor, ceiling and both wall queries, neighbor
selection, endpoint extension, filtering and surface projection. Line
and joint arrays retain traversal order and flags. Normals use Dolphin's scalar
C normalization, not the PowerPC reciprocal-root estimate. Stage motion,
previous-frame remapping and moving-joint callbacks remain unported.

`ecb` owns environmental collision-box history, fixed or six-joint sampling,
normalization, locked bottoms, interpolation, squeeze restoration, swept bounds
and source movement subdivision. These are reusable primitives; the full
fighter collision callback graph remains unported. Optional `serde` support
serializes native geometry and checkpoint state without a rendering dependency.

`locomotion::jump_velocity` translates the launch velocity calculation from
`ftCo_800CB110`; its caller owns motion flags, timers, and sound events.
`locomotion::walk` translates walking acceleration, tapering, and ground
projection and returns the source animation target. Material friction is an
explicit input; this module neither queries stage geometry nor integrates the
ground velocity. Native C comparisons execute the original selected functions
with jump sounds disabled and supplied material data.

These helpers are not a complete game simulation. The source function allowlist
for differential testing is `../../tests/oracle/physics.functions.json`; other
functions in `ftcommon.c` remain untranslated. The C oracle uses verbatim selected
functions from a pinned snapshot with a compact host representation of the fields
they access. Host agreement does not prove whole-game or PowerPC equivalence.

Build and verify from this directory:

```console
$CARGO_TARGET_DIR = "/mnt/shared/tmp/skirmish-target"
cargo build --locked
cargo test --locked
```

Run the original C comparisons from the repository root:

```console
$CARGO_TARGET_DIR = "/mnt/shared/tmp/skirmish-target"
cargo test --locked --features c-oracle --test physics_differential --test bones_differential --test locomotion_differential --test combat_differential --test sweep_differential --test damage_differential --test stage_differential --test ecb_differential
```

Use `/tmp/skirmish-target` when the shared scratch directory is not writable.
