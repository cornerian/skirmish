# Behavior tests during the Rust refactor

Integration tests are the primary contract: supply initial state, resources and
controller inputs, advance the real native match, and assert observable results.
Internal functions and module boundaries can change without rewriting that
contract. Unit/differential tests remain useful where a Rust function retains
an exact original-C counterpart or arithmetic compatibility boundary. Existing
`tests/*_differential.rs` use content-pinned source snapshots, with ABI adaptations
documented in `tests/oracle`; they do not establish whole-game equivalence.

`tests/replay_match.rs` writes synthetic `.slp` files through Peppi, loads actual
files, and advances `arena::Match` using their controller inputs. It exercises
movement, jumps, landing, a jab, damage, explicit checkpoint restoration, and
the command-line executable. Deliberate late post-state corruption and changed
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

| Required behavior | Integration cases |
| --- | --- |
| Dash, standing turn, crouch and release | `conformance_actions`: forward flick, backward tilt, down-stick lifecycle |
| Double jump, neutral and directional aerial attacks | `conformance_actions`: fresh airborne jump and aerial selection |
| Grounded and airborne specials | `conformance_actions`: neutral-B dispatch |
| Shield raise/release, grab, throw | `conformance_actions`: shoulder lifecycle, paired capture, throw release |
| Ledge catch, hang, climb, jump, attack, roll, drop | `conformance_actions`: individual ledge scenarios |
| Tap jump and stick-age input windows | `conformance_actions`: upward flick and gradual tilt versus flick |
| Stale moves, clanks, armor, shield interaction | `conformance_combat`: repeated attacks, simultaneous attacks, armored target, guarding target |
| Throws, SDI, ASDI, techs, knockdown | `conformance_combat`: victim release, hitlag displacement, landing responses |
| Platform drop and fighter pushing | `conformance_stage`: downward platform input and opposing grounded movement |
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
