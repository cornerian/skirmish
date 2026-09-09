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
line indices, preserving the original eight-byte index calculation. Source
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
