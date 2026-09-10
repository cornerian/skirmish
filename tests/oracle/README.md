# Native C references

`original/` stores one exact snapshot per upstream file. `sources.json` records
its pinned path and SHA-256; provenance tests reject unlisted or altered fixtures.
Git attributes preserve their original line endings and whitespace, including
whitespace already present upstream; adapters and Rust code use normal checks.

Each C snapshot produces `<stem>_original.inc` in Cargo's build output directory.
An optional `<stem>.functions.json` selects complete original functions; the
`ftcommon` selection is named `physics.functions.json`. Platform includes are
removed and host declarations come from the corresponding C adapters. Documented
bytecode pointer adaptations remain in `build.rs`.

The stage adapter replaces GameCube pointer subtraction with native logical
line indices, preserving the original eight-byte index calculation. It also
exposes the complete static `mpRemap2d` body used by `mpGetSpeed`; generated
tests compare its mixed float/double arithmetic before match-level carry tests.
Source
definitions are selected past forward declarations. ECB subdivision and ordinary
air knockback decay use exact excerpts of larger callbacks, checked against the
pinned snapshots by tests. Host shims provide arrays, sampled joint positions and
scalar vector normalization; they do not reproduce the whole engine environment.

`adapters.json` maps additional adapter names to existing snapshot stems. Their
own function selections produce separate includes without duplicating source:
physics, hitlag, and locomotion all read the single `original/ftcommon.c` snapshot.
The generated includes are build artifacts and are never checked into Git.

The adapters document their supported scalar/environment inputs. Native C
agreement does not establish equivalence of the unported full game scheduler.

The damage adapters also select the original armor, every-hitlag and exit-hitlag
callbacks. Armor checks cover the ordinary two-channel subtraction/minimum path;
metal, squat knockback, ice, charge and model-scale modifiers are disabled in the
host environment. Displacement checks cover main-stick SDI and ASDI with explicit
coefficients. C-stick override, LR callbacks and collision-flag side effects are
disabled; static collision response is tested through the native match instead.
The horizontal stick-age branch is an exact excerpt of `Fighter_procInput`,
verified against the preserved source before host compilation.

`shield_collision` selects complete `lbColl_80006E58`, the matrix-aware narrow
phase shared by ordinary body hurtboxes and shields. Differential tests cover
closest axes, directional matrix radius, surface contact, overlap and
broadphase behavior; native match tests own the evaluated hurt-bone matrix and
scheduler routing.

`positional_angle` preserves the complete angle-362 branch from `ftcoll.c` and
uses the exact `MTXRadToDeg` macro from the separately pinned Dolphin SDK matrix
header. Its host adapter supplies only finite hurt endpoints and contact points;
match tests own the matrix narrow phase and damage-transition composition.

`fly_reflect` selects complete `ftCo_800C18A8` and compiles it with complete
`lbVector_Add_xy` and `lbVector_Mirror` from the separately pinned `lbvector.c`.
The adapter stubs effects, camera, action entry, skeleton placement, collision
services and sound selection. It compares the retained self-plus-knockback
mirror/scale arithmetic, cleared self velocity, reflected facing and byte
lockout. Match tests own contact eligibility, floor priority, line correction
and action duration; the adapter does not claim the complete callback graph.

`passive_wall` selects complete `ftCo_800C1E0C`. Its minimal host fighter and
common-data layouts expose the jump-press age, current vertical stick, input
window and upward-stick threshold. Differential tests compare the strict age
boundary and inclusive stick boundary, including arbitrary binary32 values.
Native match tests own shoulder eligibility, wall/ceiling contact, action entry,
fighter-specific launch speeds and timing.

`stale_queue` and `stale_damage` preserve plstale.c and ft_0881.c. Their selected
whole functions use minimal host Fighter/table layouts and thread-local player,
coefficient, debug and instance globals. The damage function's unused instance
argument is retained. The core tests cover fighter-owned updates; item ownership
routing is not included. Native match tests exercise creation-time hitbox damage
caching and the separate unstaled integer knockback term.

The aerial adapters select complete functions from ft_0DF1.c, ftCo_AttackAir.c,
ftCo_LandingAir.c and the existing ftcommon.c snapshot. Host transition callbacks
capture selected lag or animation rate without running the JObj/motion graph.
Landing-lag comparisons use valid aerial IDs with the script lag flag enabled;
basic/auto-cancel landing remains a caller decision. Generated lag divisions stay
within C's defined float-to-int range. Runtime/platform.h's comparison-based ABS
macro is retained so negative-zero angle behavior is preserved.

`shield_guard` preserves ftCo_Guard.c. The shield adapter selects complete radius,
drain, stun/animation-rate/push, and displacement routines, plus getEnvDmg from
the existing ftcoll.c snapshot and GrabMash from ftcommon.c. Minimal layouts and
thread-local common data replace engine storage. Graphics/audio/statistics calls
are explicit no-result stubs; their outputs do not feed the compared arithmetic.
Full shield lifecycle and matrix contacts are exercised through native matches.

The clank adapters select ftColl_8007699C with its complete inlineA0/inlineA1
helpers, lbColl_80008688, ftCommon_800804A0 and ftCo_Rebound entry/physics from
exact snapshots. Stable nonzero integer identities replace pointer equality;
the twelve victim entries, old timers, replacement cursor, both pending response
accumulators and the second-side candidate mask are compared. The type-3 victim
path is supported; other victim registration types and the parent pair scan are
outside these tests. The rebound motion and friction hooks expose scalar outputs
without claiming the full action graph. Test common data is thread-local.

Clank kernels accept the ordinary non-Slash fighter-hit branch only. Effect and
audio outputs are omitted. ftColl_800784B4's Slash-vs-Slash branch would consume
HSD_Randi(3); that branch must preserve its shared RNG consumption before it is
supported, even in a headless match. These scalar tests do not establish global
RNG parity of the effect engine. See `docs/clanks.md` for caller responsibilities.

`nudge` selects the complete ftCommon_8007E0E4, 8007DD7C, 8007DFD0 and 8007F8B4
functions from the existing ftcommon.c snapshot. Thread-local host entities
provide player ownership, an explicit follower-to-leader mapping and the
already-resolved mpLineGetPrev/Next results. The nudge calculations themselves
are unchanged. Tests cover ordered lists, eligibility asymmetries, follower
depth bias and the original depth recenter/cap order. The safe kernel rejects
invalid physical inputs/references and nonfinite effective positions/results.
These comparisons do not include the outer scheduler or GameCube data layout.

The menu adapter compiles `mnmain.c` input decoding, confirming-port arbitration,
availability/count predicates, navigation helpers, and all ten branch callbacks.
It retains the accessed integer widths and copies the original menu enums from
`mn_forward.h`. Tests check the timer helper and input enum against the pristine
`mn_inlines.h`, and PAD masks against the pristine `dolphin_pad.h`. Rendering,
audio, object allocation, and scene scheduling are no-ops; external submenu
initializers record their identity. Callback comparisons therefore stop at the
delegation boundary, before the submenu's own initialization side effects.

`down_attack` selects complete `ftCo_800986B0` buffered-tech eligibility from
ftCo_DownAttack.c. The adapter supplies only the input-lock result, current and
previous physical-L/R press ages, and the two common-data boundaries. Match
tests own floor contact and the subsequent Passive/Down action graph.

`knockdown_input_differential` reuses the pinned `ft_0DF1.c` snapshot behind the
`aerial_input` adapter and selects complete `ftCo_800DF644` and `ftCo_800DF678`.
The adapter supplies current/previous C-stick samples plus explicit thresholds;
tests compare upward and horizontal fresh-edge predicates over arbitrary
binary32 values. Match tests own DownWait priority and recovery transitions.

`passive_stand` selects complete `ftCo_80098928`. Its host adapter supplies the
already-tested buffered-tech result and captures the requested forward/backward
motion ID; the original callback owns the comparison-based absolute value,
inclusive stick threshold and stick/facing product. Match tests own floor
contact, sampled TransN root motion, bone poses, ECB changes and action timing.

`throw_input` preserves ftCo_Throw.c and selects the three complete static-inline
main-stick crossing predicates used by `ftCo_800DD1E4`. The host adapter supplies
only the current/previous stick samples and each common-data threshold. The Rust
selector composes those exact predicates with C-stick freshness and facing;
native match tests own catch contact, direction priority, bone attachment,
scripted release and the following damage lifecycle.

`hit_direction` retains the exact fighter-position assignment from
`ftColl_8007A06C` and captured-victim assignment from `ftCo_800DDDE4`. Its tests
prove both statements appear verbatim in the pinned `ftcoll.c` and
`ftCo_Throw.c` snapshots, then compare arbitrary binary32 positions and facings
bit for bit. Native match tests own launch-vector composition, final victim
facing, grounded projection and the DownDamage override.

`ledge_option` preserves ftCo_CliffClimb.c and selects complete
`ftCo_8009AAFC`. The host adapter supplies its main/C-stick identity, input-ready
flag, X value, precomputed angle, facing, common angle boundary and cooldowns.
Stub transitions report climb or fall while the original callback owns every
comparison, return value and cooldown write. Native match tests own endpoint
eligibility, bone attachment and the six-state action lifecycle.

`rebirth` preserves ft_0D4D.c and selects complete `ftCo_Rebirth_Phys` and
`ftCo_RebirthWait_Phys`. The host adapter exercises their ordinary leader
branches with an already-resolved platform target and sampled current position.
Differential tests compare both velocity components bit for bit across arbitrary
binary32 positions and every positive signed frame count, including the two
callbacks' distinct multiplication order. Native match tests own stock loss,
entry, platform wait, input/timeout release and invulnerability; follower
velocity copying and moving stage spawn offsets remain outside this adapter.

`special_input` preserves the complete `ftCo_800D67C4` neutral-special input
predicate. The host adapter supplies the fresh button mask, main stick and two
common-data thresholds. Generated comparisons retain its strict axis bounds and
comparison-based ABS behavior. Native match tests own action priority, grounded
and airborne animation, contact, terrain conversion and input rearming.

`death` preserves the complete `ftCo_800D3158` blast-line selector. The host
adapter records its selected death callback and compares line order, top
eligibility, ice variants, screen chance and exact HSD RNG consumption. Native
match tests own the resource-driven death action timelines and stock lifecycle.
