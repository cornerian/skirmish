# Behavior tests during the Rust refactor

Integration tests are the primary contract: supply initial state, resources and
controller inputs, advance the real native match, and assert observable results.
Internal functions and module boundaries can change without rewriting that
contract. Unit/differential tests remain useful where a Rust function retains
an exact original-C counterpart or arithmetic compatibility boundary. Existing
`tests/*_differential.rs` use content-pinned source snapshots, with ABI adaptations
documented in `tests/oracle`; they do not establish whole-game equivalence.

`tests/replay_match.rs` writes synthetic `.slp` files through Peppi, loads actual
files, and advances `game::Match` using their controller inputs. It exercises
movement, jumps, landing, a jab, damage, explicit checkpoint restoration, and
the command-line executable. C-stick ASDI cases check inclusive selection,
main-stick-only DI, hitlag freeze and checkpoint branching; the file-backed
version detects a changed C-stick at the first affected frame. Deliberate late post-state corruption and changed
controller input produce first-divergence failures. Altered recorded pre-state
and RNG values cannot reset the simulator. These expected recordings come from
the native implementation itself, so this is harness validation; independent
Melee observations must supply the fidelity oracle. See [replays.md](replays.md)
for the exact selected fields and initialization contract.

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

The movement/combat milestone activates the source-backed dash, standing turn,
crouch, ordinary double jump, tap-jump/input-window, armor, main-stick SDI/ASDI,
platform-drop and ordinary top-KO scenarios. Focused match integration targets
in the root `tests/game_*.rs` suite extend these beyond the original single gap scenario:

| Passing target | Additional contracts |
| --- | --- |
| `locomotion` | Input age, launch timing, turn/crouch lifecycle, jump exhaustion and restoration |
| `input_history` | Shared input ages through hitlag and recovery, fresh re-presses and conflicting resource rejection |
| `hitlag_displacement` | Threshold boundaries, input held before damage, attacker/victim callbacks, expiry ordering, armor channels, checkpoint suffixes and transactional errors |
| `platform_drop` | Supporting-line skip, stacked and solid floors, high-speed substeps, fresh versus held down input and checkpoint history |
| `blast_zones` | Strict upward-knockback/top-position thresholds, self-velocity jumps, grounded crossings, side/bottom KOs and checkpoint restoration |

These scenarios use supplied synthetic coefficients and poses. Passing them
establishes those behavioral contracts; authentic animation, full callback order
and unported character/state branches still need independent validation.
`damage_differential` additionally compares the retained armor arithmetic,
stick-age branch and displacement callbacks with pinned original C. Adapters
explicitly disable unrelated state/environment branches.

| Required behavior | Integration cases |
| --- | --- |
| Dash, standing turn, crouch and release | `conformance_actions`: forward flick, backward tilt, down-stick lifecycle |
| Double jump, neutral and directional aerial attacks | `conformance_actions`: fresh airborne jump and aerial selection |
| Grounded and airborne specials | `conformance_actions`: neutral-B dispatch |
| Grab and throw | `conformance_actions`: paired capture and throw release |
| Ledge catch, hang, climb, jump, attack, roll, drop | `conformance_actions`: individual ledge scenarios |
| Tap jump and stick-age input windows | `conformance_actions`: upward flick and gradual tilt versus flick |
| Clanks and remaining combat responses | `conformance_combat`: simultaneous attacks and remaining modifiers |
| Throws, SDI, ASDI, techs, knockdown | `conformance_combat`: victim release, hitlag displacement, landing responses |
| Platform drop and fighter pushing | `conformance_stage`: downward platform input and exact bounded X/Z push before movement |
| Moving platforms/remapping, ECB corner and squeeze response | `conformance_stage`: independent reference cases |
| Conditional top KOs and rebirth platform | `conformance_stage`: ordinary upper-boundary jump and stock-loss lifecycle |
| Star/screen deaths | `conformance_stage`: independent death lifecycle reference |
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
