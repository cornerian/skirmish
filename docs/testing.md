# Behavior tests during the Rust refactor

Integration tests are the primary contract: supply initial state, resources and
controller inputs, advance the real native match, and assert observable results.
Internal functions and module boundaries can change without rewriting that
contract. Unit/differential tests remain useful where a Rust function retains
an exact original-C counterpart or arithmetic compatibility boundary. Existing
`tests/*_differential.rs` use content-pinned source snapshots, with ABI adaptations
documented in `tests/oracle`; they do not establish whole-game equivalence.

`crates/cli/tests/replay_match.rs` writes synthetic `.slp` files through Peppi, loads actual
files, and advances `game::Match` using their controller inputs. It exercises
movement, jumps, landing, a jab, damage, explicit checkpoint restoration, and
the command-line executable. C-stick ASDI cases check inclusive selection,
main-stick-only DI, hitlag freeze and checkpoint branching; the file-backed
version detects a changed C-stick at the first affected frame. A table-driven
file-backed case independently corrupts every reported action, timer, position,
damage, shield, stock, airborne, jump, velocity and hitlag field and requires the
first divergence at that row. Slippi 2.0, 3.5 and 3.8 fixtures prove the report's
version-dependent field set. Separate Peppi-written replay cases drive physical
B through neutral-special combat and physical Z through grab/capture, then remove
each recorded input and require divergence on that frame. Deliberate late post-state corruption and changed
controller input produce first-divergence failures. Altered recorded pre-state
and RNG values cannot reset the simulator. These expected recordings come from
the native implementation itself, so this is harness validation; independent
Melee observations must supply the fidelity oracle. See [replays.md](replays.md)
for the exact selected fields and initialization contract.

## GitHub Actions suites

Three independent workflows run on pushes, pull requests, and manual dispatch:

| Workflow | Coverage |
| --- | --- |
| [Unit tests](../.github/workflows/unit-tests.yml) | Workspace library/binary unit tests and doctests; formatting and Clippy |
| [Integration tests](../.github/workflows/integration-tests.yml) | Workspace integration targets, including original-C differential tests, except the Slippi file targets below; debug/release match-trace comparison |
| [System tests (Slippi file parity)](../.github/workflows/system-tests.yml) | `peppi-adapter`'s `replays`, plus `skirmish-cli`'s `slippi_cli`, `slippi_corpus` and `replay_match` |

Every suite retains default debug, C-oracle debug, and C-oracle release runs.
Integration targets are discovered from Cargo metadata, so new integration test
files are included automatically. Explicit `--test` selection avoids rerunning
library/binary unit tests in the integration workflow. When adding a system test
target, add it to the system workflow and the integration workflow's exclusion
set. All workflows share native SDL3/ALSA and Rust setup.

The system suite covers Slippi parsing, command-line import, file-backed match
validation and first-divergence detection using synthetic recordings. It also
checks hashes and import summaries for [ten original archived replays](../tests/fixtures/slippi/README.md),
including explicit rejection of the unsupported 1.7.1 format. Import regressions
do not establish native simulation parity against those recordings. The
in-memory validator tests remain in the integration suite. Ignored conformance
and graphics-dependent tests retain their existing opt-in behavior.

## Passing, unimplemented and blocked

The normal suite contains passing regressions for implemented behavior.
`tests/conformance_*.rs` also register every gameplay gap discussed in the
coverage audit. Missing capabilities have explicit `#[ignore = "unimplemented:
..."]` reasons. Their scenarios drive `Match::step` and assert required behavior;
running them now exposes an unsupported input or a differing result. They are
not `should_panic` tests that turn missing functionality into a pass.

Other cases require independently sourced resources/reference traces and are
marked `blocked`. They execute the same complete scenario comparator when those
files exist, and fail clearly when a fixture is absent. An ignored or blocked
case is **not passing coverage**. Remove the ignore attribute when both the
implementation and its independent evidence are available.

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo test --locked --workspace
cargo test --locked --workspace --features c-oracle
# Expected to fail until the corresponding behavior/reference is supplied:
cargo test --locked --test conformance_actions -- --ignored
cargo test --locked --test conformance_combat -- --ignored
cargo test --locked --test conformance_stage -- --ignored
cargo test --locked --test conformance_resources -- --ignored
```

## Coverage map

`game_aerial_actions` supplies explicit invented resources for five ordinary
aerials and their landing animations. It covers both-facing selection, fresh
versus held C-stick, attack/jump priority, interrupt and landing flags, animated
contacts and one-shot facing changes through hitlag, checkpoint restoration,
strict aggregate-trigger L-cancel timing and invalid-resource rejection. Its
neutral/directional conformance cases run normally. `replay_match` also reads a
synthetic Peppi file through aerial and landing transitions, and detects a
changed C-stick at the first affected frame.

`fighter::aerial` isolates ordinary C-stick freshness, main/C-stick aerial
selection, folded stick angles and landing arithmetic. `aerial_differential`
uses unchanged original function bodies and explicit synthetic coefficients;
it covers held/reversed C-stick input, neutral and angle boundaries, signed
zeros, L-cancel truncation/window edges, and `(end_frame + 0.1) / lag` animation
rate. Main/C-stick angles use `atan2(y, ABS(x))`; host libm comparisons allow a
small numerical tolerance for general inputs. Undefined lag float-to-int
conversions are errors instead of executing undefined C behavior.

Animation completion is a separate state contract. `ftAnim_IsFramesRemaining`
checks eligible parts' animation flags (and selects the blended skeleton when
active). `lb_8000B074` reads `AOBJ_NO_ANIM`; the non-looping branch in
`HSD_AObjInterpretAnim` sets that flag when `end_frame <= curr_frame`, after
animation processing. First-play skips frame advancement. No numerical epsilon
is used for completion. These graph/callback semantics are not implemented by
the aerial scalar helpers. L-cancel's input age is likewise supplied by the
caller: x67F tracks a fresh aggregate virtual-LR edge, not just physical L/R.

The stale-move batch enables repeated-jab conformance. `stale_differential`
compares original queue, identity and damage functions, including the tenth
slot's duplicate suppression, nine-entry weighting, holes, 16-bit wrap, debug
bypass and exceptional float behavior. `game_staling` checks percent versus
knockback damage, hitbox caching and instance deduplication, simultaneous trades,
death/reset lifecycle and checkpoint replay with wholly synthetic resources.

The [ordinary clank profile](clanks.md) adds native `game_clank` scenarios and
enables equal-grounded-jab conformance. Original-C kernels compare response and
victim-table mutations; match tests cover source slot order, staled fractional
damage, one-sided priority, body-hit precedence, frozen poses, delayed recoil,
group history, resource rejection and restored recovery. These fixtures are
synthetic. Items, capture rules, Slash RNG and the full original graph remain
outside the declared profile.

The [damage-floor profile](damage-floor.md) promotes both tumble-landing
conformance scenarios. `game_damage_floor` exercises buffered neutral tech,
directional tech and missed-tech rolls, attack/roll/stand priority, fresh C-stick
history, sampled TransN motion and bone-derived ECBs, get-up attack contact,
prone DownDamage launch/landing and countdown recovery, repeat lockout, exact
configured recovery durations, non-tumble landings, DamageFall carry, invalid
data, serialization, checkpoint replay and reset. The damage-surface integration
resource also covers DamageFall fast-fall and special/aerial/jump dispatch on
both sides of the hitstun boundary.
`damage_floor_differential` compares the retained eligibility predicate with
the complete pinned original C function over generated timer, boundary and lock
inputs. `floor_tech_roll_differential` compares complete `ftCo_80098928` with a
stubbed tech result over selector boundaries and arbitrary binary32 values.
`knockdown_input_differential` compares complete `ftCo_800DF644` and
`ftCo_800DF678` C-stick predicates over boundaries and arbitrary binary32 values.
`down_damage_differential` compares complete `ftCo_8009F0F0`, including its
strict threshold and face-down motion-selector quirk.

The [ordinary damage-motion profile](damage-motion.md) adds
`game_damage_motion` coverage for every grounded/airborne knockback level and
hurtbox height, strict threshold equality, repeated-hit reselection, sampled
hurtbox/ECB geometry, animation versus hitstun completion, final-pose holding,
airborne physics and special/aerial/jump dispatch across the hitstun boundary,
checkpoint replay and malformed resources. `damage_motion_differential` runs
arbitrary binary32 values through Rust and the byte-for-byte source level
selection and 2x4x3 motion table in its host C adapter.

The [grounded launch profile](grounded-launch.md) adds `game_ground_launch`
coverage for flat and sloped tangent projection, the exact pi/2 departure
boundary, fly departure and bounce, hitlag freezing, ground-friction decay,
serialization, invalid resources and checkpoint replay. `game_damage_floor`
also verifies the explicit-motion DownDamage fly branch.
`ground_launch_differential` compares the complete retained source branch and
extracted `lbVector_Angle` over arbitrary binary32 inputs.

[Damage-direction coverage](hit-direction.md) adds `game_hit_direction` for
both players attacking from either side, the equal-X tie, grounded tangent
composition and checkpoint replay. `game_damage_floor` verifies that DownDamage
launches away while preserving its prior-facing override, and `game_grab`
checks throw-facing assignment. `hit_direction_differential` compares fighter
and throw assignments over 512 arbitrary binary32 inputs with verbatim pinned C.
`game_positional_angle` drives angle 362 through matrix contact for three
quadrants, the vertical tie and checkpoint replay. Its exact helper has focused
unit coverage, while `positional_angle_differential` compares 512 arbitrary
finite geometries against the verbatim source branch and pinned SDK degree
conversion.

[Matrix-aware hurtbox contact](hurtbox-contact.md) adds
`game_hurtbox_geometry`, where a scaled bone extends collision along one axis
and contracts it along another. Existing `game_swept_hitboxes` and
`game_damage_motion` cases ensure swept attack centers and action-selected hurt
bones enter that path. The shared primitive is already covered by
`shield_geometry` integration and `shield_collision_differential` against the
complete retained C routine.
`game_hurtbox_eligibility` covers enabled, disabled and intangible capsules,
the independent grabbable property, scanning past an ineligible first entry,
legacy resource defaults and directional matrix scale in the grab scheduler.
`game_hurtbox_states` drives those states through a target's sampled attack
frames for damage and grab contact. It also covers legacy base-state inheritance,
explicit frame override, rejection of partial samples and checkpoint replay over
the eligibility transition.

The [damage-surface profile](damage-surfaces.md) adds `game_damage_surface`
coverage for wall and ceiling launch reflection, neutral/jump wall techs, both
wall orientations, ceiling input motion, exact configured action durations,
both reflected and tech sampled bone poses, delayed neutral-to-jump conversion,
floor/wall collision priority, both cross-surface reflection chains during
lockout, post-freeze special/aerial/jump dispatch, reflected-action gravity,
drift, fast-fall and special/aerial/jump hitstun boundaries, ceiling
non-interruption, all three tech floor-landing paths, both reflected-action
knockdown landings and shared-state cleanup, shoulder repeat lockout,
disabled/unmet profiles, moving-wall tech and reflected-action response,
DamageFall air-action and fast-fall hitstun boundaries, malformed resources and
checkpoint replay.
`damage_reflect_differential`
compares the retained velocity/facing/lockout portion of complete
`ftCo_800C18A8`, including its two complete vector helpers, over arbitrary
binary32 values. `wall_tech_differential` compares complete `ftCo_800C1E0C`
jump selection across timer boundaries and arbitrary binary32 values.

The [grab profile](grabs.md) promotes all three paired grab/throw conformance
scenarios. `game_grab` covers distinct standing/dash bone-sampled contact,
Dash/Run/Turn/Squat entry, retained momentum, miss recovery, all four throw
directions, pummel priority and repeat lifecycle, grounded-low/airborne-high
capture families, split captured-damage poses, one captured-damage event, shared
hitlag, sampled holder/victim attachment motion, threshold lifts, swept floor
conversion with preserved action time, paired fractional
throw timing for heavy/light victims and weight-independent directions, independent
CaptureDamage completion, passive and button/stick/analog-shoulder-mashed
escape, logical shoulder rearming, timer freeze,
release motion and cut actions, main/C-stick priority and fresh-edge history,
held-victim input suppression, checkpoint suffixes,
simultaneous ordering, target policy, KO cleanup and invalid resources. Focused
unit tests cover the exact `fn_800DA4C0` A-bit predicate and complete
`ftCommon_GrabMash` mutation. `grab_differential` compares the three exact
`ftCo_800DD1E4` main-stick threshold predicates; `grab_mash_differential`
compares arbitrary binary32 timers, coefficients and input/latch state against
the complete pinned C function. `capture_alignment_differential` compares the
complete position and strict scaled-height result of `fn_800DAD18` over arbitrary
binary32 inputs.

The [ledge profile](ledges.md) promotes all six ledge conformance scenarios.
`game_ledge` covers endpoint flags/connectivity, both sides, eligibility,
stable occupancy, bone attachment, action priority and completion, jump launch,
ordinary attack contact, incoming-damage release, strict quick/slow percent
selection, distinct wait/motion/jump/attack resources, timeout, cooldown/regrab,
checkpoint replay, KO cleanup and invalid resources. `ledge_differential`
compares the complete retained `ftCo_8009AAFC` callback, including its strict
float branches and drop-cooldown side effect, against pinned C. It also compares
the original `ftCo_8009AB9C` quick/slow selector over arbitrary binary32 values.

The [rebirth profile](rebirth.md) promotes the stock-loss platform conformance
scenario. `game_rebirth` covers both slots, reset/event state, each travel frame,
static waiting, timeout and button/trigger/main-stick/C-stick releases,
invulnerability, checkpoint replay and invalid resources. `rebirth_differential`
compares the two complete original leader physics callbacks bit for bit while
retaining their distinct floating-point operand order.

The [neutral-special profile](specials.md) promotes both remaining executable
action conformance scenarios. `game_special` covers strict fresh-B selection for
both slots, priority over competing inputs, rearming, sampled combat, air
physics, landing conversion, checkpoints and malformed resources.
`special_differential` compares the complete source neutral-input predicate over
generated button, stick and threshold values.

The [blast-death profile](deaths.md) adds `game_death` coverage for all four
normal directions, forced top deaths, exact timers, star ascent, screen
approach/hold/fall phases, delayed and final-stock loss, ignored input,
checkpoint replay and malformed resources. `death_differential` compares the
complete original blast selector and post-call HSD seed over generated states.
The independent star/screen conformance fixture remains blocked until a trace
from an independent producer exists.

The [stage-motion profile](stage-motion.md) adds `game_stage_motion` coverage
for affine and cyclic line samples, grounded carry after self movement, hitlag,
airborne detachment and relanding, current-geometry access, countdown timing,
checkpoint replay and malformed resources. `stage_differential` now compares
the complete original `mpRemap2d` body across generated binary32 endpoints and
points, including clamping and degenerate-line behavior. Its independent
moving-platform trace remains blocked until a separate producer exists.

The [ECB response profile](ecb-response.md) adds `game_ecb_response` coverage
for four-sided moving compression, horizontal and grounded vertical squeeze,
next-frame shape restoration, stable contact IDs, deterministic checkpoint
replay, stationary-airborne landing, one-way direction filtering and tangential
motion rejection. The squeeze helpers already run against their complete pinned
original C bodies in `ecb_differential`. Independent scheduler traces remain
blocked until a separate producer exists.

The movement/combat milestone activates the source-backed dash, running and
standing turns, crouch, ordinary double jump, tap-jump/input-window, armor,
main-stick SDI/ASDI, platform-drop and ordinary top-KO scenarios. Focused match integration targets
in the root `tests/game_*.rs` suite extend these beyond the original single gap scenario:

| Passing target | Additional contracts |
| --- | --- |
| `locomotion` | Input age, launch timing, standing and running-turn lifecycle, Run and RunBrake trigger priority, velocity-gated marker freeze, animation-time carry, ordinary and five-aerial-jump exhaustion/restoration, multijump held-input markers, bone turn/facing, drift and checkpoint replay |
| `input_history` | Shared input ages through hitlag and recovery, fresh re-presses and conflicting resource rejection |
| `hitlag_displacement` | Threshold boundaries, input held before damage, attacker/victim callbacks, expiry ordering, armor channels, checkpoint suffixes and transactional errors |
| `platform_drop` | Supporting-line skip, stacked and solid floors, high-speed substeps, fresh versus held down input and checkpoint history |
| `blast_zones` | Strict upward-knockback/top-position thresholds, self-velocity jumps, grounded crossings, side/bottom KOs and checkpoint restoration |
| `death` | Directional action timers, star/screen selection and motion, delayed/final stock loss, RNG/checkpoint state and invalid resources |
| `stage_motion` | Affine/cyclic collision lines, current geometry, grounded/self-motion/hitlag carry, air detachment/relanding, checkpoints and invalid resources |
| `ecb_response` | Four-sided moving compression, ECB restoration, moving-floor landing, one-way direction and tangential-motion rejection, checkpoint replay |
| `damage_floor` | Neutral/directional techs, full face-up/down knockdown and get-up suffix, DownBound attack buffering, prone DownDamage reactions, sampled root motion and bones, exact recovery protection, combat vulnerability, input history, invalid profiles and checkpoint replay |
| `damage_surface` | Wall/ceiling reflection and techs, neutral/jump choice, both wall orientations, ceiling motion, action timing, collision priority, repeat lockout, disabled/invalid profiles and checkpoint replay |
| `damage_motion` | All ordinary ground/air/fly selectors, sampled hurtbox/ECB poses, dual animation/hitstun completion, repeated-hit reselection, invalid profiles and checkpoint replay |
| `ground_launch` | Flat/sloped floor retention and tangent projection, departure and fly bounce boundaries, hitlag freeze, scalar friction, prone DownDamage override, invalid profiles and checkpoint replay |
| `hit_direction` | Both fighter positions and player slots, equal-X tie, victim facing, grounded projection, throw assignment, prone override and checkpoint replay |
| `hurtbox_geometry` | Matrix-aware body contact, directional bone scale, long-axis hit and short-axis miss |
| `hurtbox_eligibility` | Damage/grab state filtering, grabbable flag, later-entry scan, legacy defaults and matrix-aware grabs |
| `hurtbox_states` | Frame-sampled damage/grab eligibility, base inheritance, explicit override, invalid partial samples and checkpoint replay |
| `positional_angle` | Angle-362 matrix contact, three launch quadrants, vertical tie, validation and checkpoint replay |

These scenarios use supplied synthetic coefficients and poses. Passing them
establishes those behavioral contracts; authentic animation, full callback order
and unported character/state branches still need independent validation.
`damage_differential` additionally compares the retained armor arithmetic,
stick-age branch and displacement callbacks with pinned original C.
`locomotion_differential` compares the multijump root turn, including arbitrary
integer state and binary32 facing/yaw, with its complete pinned C callback.
Adapters explicitly disable unrelated state/environment branches.

| Required behavior | Integration cases |
| --- | --- |
| Dash, standing turn, crouch and release | `conformance_actions`: forward flick, backward tilt, down-stick lifecycle |
| Double jump, neutral and directional aerial attacks | `conformance_actions`: fresh airborne jump and aerial selection |
| Grounded and airborne neutral specials | `conformance_actions` and `game_special` (implemented paired profile) |
| Grab, pummel, escape and throw | `game_grab`, `conformance_actions` and `conformance_combat`: standing/dash/pivot capture, bone-lift/floor-loss/floor-contact capture-family transfer, sampled captured-damage reaction, repeated pummel, exact mash timer, cut release and throw release (implemented profile) |
| Ledge catch, hang, climb, jump, attack, roll, drop | `conformance_actions` and `game_ledge` (implemented static-endpoint profile) |
| Tap jump and stick-age input windows | `conformance_actions`: upward flick and gradual tilt versus flick |
| Clanks and remaining combat responses | `conformance_combat`: simultaneous attacks and remaining modifiers; `game_damage_surface` covers ordinary wall/ceiling reflection and techs |
| Throws, SDI, ASDI, techs, knockdown | `conformance_combat`: victim release and implemented hitlag/floor/surface responses |
| Platform drop and fighter pushing | `conformance_stage`: downward platform input and exact bounded X/Z push before movement |
| Moving platforms/remapping | `game_stage_motion` (implemented native profile); `conformance_stage` remains blocked on an independent reference |
| ECB corner and moving-surface squeeze response | `game_ecb_response` (implemented opposing-surface profile); `conformance_stage` remains blocked on an independent reference |
| Conditional top KOs and rebirth platform | `conformance_stage`: ordinary upper-boundary jump and stock-loss lifecycle |
| Star/screen deaths | `game_death` (implemented native profile); `conformance_stage` remains blocked on an independent reference |
| Authentic skeletons/bind parents, animation tracks, hurtbox/ECB attachments | `conformance_resources`: independent native-resource scenarios |
| Complete action scripts and stage topology | `conformance_resources`: independent command/topology scenarios |
| Nana simulation | `conformance_resources`: independent follower scenario; parser follower regressions already run normally |
| Original callback order and RNG consumption/restoration | `conformance_resources`: independent schedule/RNG scenarios |

## Independent reference fixtures

Set `SKIRMISH_CONFORMANCE_DIR` to a directory containing one subdirectory per
named reference case. Each contains `data.json` (native `MatchData`),
`inputs.jsonl` (one controller pair per frame), `expected.jsonl` (a complete
semantic trace), and `manifest.json`. Keep durable generated runs outside Git;
small, attributed reference fixtures can be checked in after review.

The manifest records schema, case ID, seed, provenance and SHA-256 identities for
all three files. Provenance identifies the independent producer/source revision;
a native simulator recording of itself is accepted only by the fixture-loader
regression test, not by the blocked fidelity cases. Reference loading verifies
file hashes, runs the real match, and compares the complete trace, including
exact float-bit strings. The optional checkpoint-restore scenario replays its
remaining inputs from the saved native state. No replay observations are copied
into the simulator to force agreement. See
[`reference_scenario.rs`](../tests/support/reference_scenario.rs) for the precise
manifest schema and validation rules.
