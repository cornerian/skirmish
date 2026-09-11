# Skirmish

A Rust rewrite of [doldecomp/melee](https://github.com/doldecomp/melee), in progress.
The complete game is **not implemented** yet. This tree replaces the earlier
Skirmish prototype. Its history is archived on `archive/pre-rust-rewrite-20260909`;
`main` starts from a new root commit.

The migration prioritizes behavior over matching PowerPC assembly. Rust's
standard library replaces manual allocation and containers; established crates
provide parsing, command-line handling, math, hashing and test generation.
Game-specific arithmetic is retained when a library would change its results.

## Development

Use stable Rust and a C compiler (Peppi's compression dependencies and the
optional differential tests use native C).
The renderer workspace member needs SDL3, plus ALSA development files and
`pkg-config` on Linux; see its [setup notes](crates/renderer/README.md). In xonsh:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo test --locked --workspace
cargo test --locked --workspace --features c-oracle
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

`/tmp/skirmish-target` is a fallback when shared storage is unavailable. Building,
running and testing this project **must never require an ISO, DOL, emulator or
GameCube runtime**. It targets native execution on modern machines. Game data
will use native resources; missing data remains an explicit migration task.
Tests against host-compiled C establish agreement for the tested functions and
inputs; full-game equivalence remains unverified.

See the [integration coverage and missing-behavior tests](docs/testing.md).
Ignored conformance scenarios describe unimplemented behavior and are expected
to fail when explicitly run; their absence from the passing suite is reported.

## Current translation

The pinned source contains 2,441 C/C++/header/assembly files and 611,321 lines,
including the Dolphin SDK. `upstream.lock.json` records the revision and counts;
`cargo run --bin skirmish -- inventory /path/to/melee` produces a source/hash inventory.

| Library | Translated behavior |
| --- | --- |
| `compat::{bytecode, math, mbstring}`, `random`, `ctype`, `spline`, `id`, `quaternion` | Focused HSD and Metrowerks compatibility algorithms without a generic runtime wrapper |
| `fighter` | Movement, locomotion, neutral-special input, blast-death selection, fixed/contact-relative knockback, DI, hitlag, prone damage, damage reflection and floor/wall-tech selection |
| `collision` | Skeletal poses, environmental collision boxes and substeps, directed stage queries, exact moving-line remapping, opposing-surface squeeze, swept capsules and matrix-aware [hurtbox contact](docs/hurtbox-contact.md) with frame-sampled eligibility |
| `controller` | Original controller clamping plus optional SDL3 hot-plug input for standard gamepads and GameCube adapters |
| `menus` | Native digital input/repeat handling and ten main-menu branches with navigation, unlock rules, cooldowns and explicit scene/panel requests |
| `replay-validation` | Simulator-independent checkpoint/transition validation machinery |
| `peppi-adapter` | Peppi 2.1.2 parsing, retained replay primitives/columns, rollback and finalized-frame selection, native-validator transitions |
| `skirmish-replay` | Concrete Slippi input conversion, observation policy, and native match validation |
| `skirmish-equivalence` | Semantic traces, process comparison, native match trace adapter, and differential probes |
| `game` | Experimental native two-player match: static or resource-animated stage/ECB collision, bone-attached ledge and neutral-special actions, jab combos (second/third jab and the rapid jab) with tilts, charged smashes and the dash attack, standing/dash/pivot/shield catches, stick-directed air dodges with special landings, paired capture/pummel/mash-escape/four-direction throws, damage, DI, prone reactions, floor/wall/ceiling techs, wall/ceiling reflection, directional/star/screen KOs, stocks, airborne rebirth platforms, timeout and checkpoints |
| `renderer` | SDL3 window/events/controllers, wgpu scene and menu presentation, build-time WESL shaders, offscreen PNG output and CPAL procedural audio cues |

Start the native graphics preview with
`cargo run --locked -p renderer --bin skirmish-renderer`. It includes a procedural
demo; `--scene /path/to/scene.json` loads a `skirmish-visual-v1` asset export.
Without `--scene`, the window starts on the interactive menu. Choose
**Import Game Assets** to discover or select a local USA 1.02 ISO and install
its original files using the built-in Rust importer. Players need no terminal,
Python, Bun, or Dolphin. The [in-game import guide](docs/asset-import.md) explains
storage, validation, and the remaining visual/gameplay conversion work.
The [canonical asset tree](docs/asset-tree.md) defines resource categories and
consumers; its [complete source inventory](docs/asset-source-tree.md) lists all
1,209 supported disc files.
F1 switches menus and scene on
the same graphics surface. The [renderer crate](crates/renderer/README.md) documents controls,
offscreen capture, requirements, and material approximation limits. It does not
yet present live matches or implement original game rendering.

Most game, engine, SDK and platform code remains unported. Transcendental
tests allow a documented host-library tolerance; integer and
eligible floating-point operations use exact comparisons. The match slice uses
an explicitly synthetic fixture; a faithful Melee matchup and a training adapter
are not implemented yet.

Run a complete native headless demonstration with
`cargo run --locked --bin skirmish -- demo-match`. It emits a semantic JSONL trace
of a scripted jab sequence through stock loss, respawn and a winner. Bones and
collision run in the headless `collision` and `fighter` modules; there is no rendering dependency. See the
[match API, native-data format and coverage limits](docs/match.md). Real Fox and
stage data subsets are preserved with [provenance and missing fields](docs/native-data.md).
Optional [blast-death resources](docs/deaths.md) expose normal, star and screen
KO timelines, exact selector RNG and delayed stock loss to headless consumers.
The [shield profile](docs/shield.md) includes direct digital and analog
shield-to-Pass drops through one-way platforms, C-stick shield jumps and
resource-driven grounded rolls and spot dodges with sampled root motion, bones
and scripted hurtbox state, and A/Z shield grabs with the dash-grab buffer, all
with pinned input predicates. The [air-dodge profile](docs/air-dodge.md) adds
stick-directed EscapeAir with scripted decay, the FallSpecial continuation,
platform pass-through and the wavedash-capable special landing. The [tilt
profile](docs/tilts.md) adds forward tilts with their angle variants, up and
down tilts, the down tilt's buffered repeat and scripted interrupt windows.
The [smash profile](docs/smashes.md) adds forward smashes with their angle
variants, stick-sign facing and TransN root motion, up and down smashes,
C-stick entry, the jump-cancelled up smash, the charge state machine with
charged damage and the charging-victim knockback multiplier.
The [jab profile](docs/jabs.md) adds the second and third jab, buffered from
their scripted follow-up windows through Wait and the previous jab's
interruptible poses, and the rapid jab's press/release entry count, looping
figatree and continuation check.
The [dash-attack profile](docs/dash-attack.md) adds the Dash early/middle/late
input phases (a no-age-window forward smash and a held-shoulder forward roll
early, an A-buffered dash attack and shield entry from the middle and late
phases, and the late re-dash), the shared dash-attack/shield arms of Run's
own dispatch, and the dash attack itself with its no-A catch buffer and
TransN-or-friction physics.
The [landing profile](docs/landing.md) adds an optional interrupt window to
the ordinary Landing: once past `normal_landing_lag`, it opens the same
complete Wait chain an interruptible tilt or dash attack already exposes,
narrowed only by a single first-frame crouch entry; `LandingFallSpecial`
keeps its own separate, unaffected full lockout.
The [state-parity profile](docs/state-parity.md) corrects FallSpecial's
reported Slippi state and animation (35/26, not FallB's 31/22) and adds an
optional backward ground/aerial jump direction and the aerial-jump variant of
Fall, each reported through their own dedicated Slippi ids.
The [damage-floor profile](docs/damage-floor.md) adds neutral and directional
techs plus missed-tech rolls, standing and get-up attacks with sampled root
motion and bones.
The [ordinary damage-motion profile](docs/damage-motion.md) selects all 15
ground, air and fly reactions by post-armor knockback and contacted hurtbox
height, with headless pose sampling through hitstun.
The [grounded launch profile](docs/grounded-launch.md) keeps low-level damage on
the supporting floor, projects knockback along slopes and decays its dedicated
scalar while fly hits leave or bounce from the floor.
[Damage direction](docs/hit-direction.md) selects ordinary fighter launch from
attacker/victim positions, retains the equal-X tie, and gives throws their
separate facing-based rule.
Resource-driven [stage motion](docs/stage-motion.md) exposes current collision
geometry and carries supported fighters with the original line-remap arithmetic.
The [ECB response profile](docs/ecb-response.md) adds moving-surface penetration,
ordinary four-sided squeeze and deterministic shape restoration.
The [damage-surface profile](docs/damage-surfaces.md) composes exact reflection
and wall-tech input arithmetic with wall/ceiling ECB contacts, headless tech
motion and configured action timing.
The [ordinary wall-jump profile](docs/wall-jumps.md) adds moving-wall-relative
arming, strict away-input timing, repeated-height decay and complete sampled
physics poses without rendering.
The [floor-end and edge-teeter profile](docs/edges.md) derives one of three
grounded floor-end collision modes (plain fall-off, always-clamp, or a
facing/stick-gated teeter) from the fighter's action, adds the zero-velocity,
no-physics `Ottotto`/`OttottoWait` states with the full Wait input chain and
their own walk-away threshold and exit-to-Wait distance, and corrects the
shield-escape profile's rolls and spot dodge to clamp at a floor end instead
of falling off it.
The [walk-speed profile](docs/walk.md) subdivides Walk into WalkSlow/
WalkMiddle/WalkFast by ground velocity, each with its own Slippi id, sub-
motion and animation rate; a velocity crossing re-enters Walk mid-stride
with the animation frame remapped proportionally into the new kind's
figatree length, without changing the reported action-instance id.

Browse the translated menu branches with `cargo run --locked --bin skirmish -- menus`.
This terminal preview supports navigation and confirm/back. The renderer adds
graphical navigation; original menu artwork and destination screens remain
unported. `run-menus` accepts controller frames and
emits reproducible traces. See [menu controls and coverage](docs/menus.md).

Inspect a completed Slippi replay with
`cargo run --locked --bin skirmish -- inspect-replay /path/to/game.slp`.
Add `--finalized-only` to select its explicitly confirmed frame prefix. The
[Peppi adapter](crates/peppi-adapter) preserves recorded fields and produces
validator transitions. Compare those transitions against real native
`Match::step` calls with:

```xonsh
cargo run --locked --bin skirmish -- validate-replay /path/to/game.slp --initialization /path/to/init.json
```

The required initialization embeds native match data, a seed, explicit port
mapping, the next replay frame and deterministic warmup inputs. The command
compares the entire selected suffix, reports the first difference, and exits
unsuccessfully on mismatch or unsupported input. Its `fighter-post-v11` policy
checks action identity and age, position, facing, percent, shield, stocks,
airborne state, remaining jumps, landing state, selected fighter-state flags
(including GuardReflect, inert shield touch and the death/inactive lifecycle),
hit attribution, the retained combo counter, and the hitstun counter for two
human leaders. It also checks internal character identity, all five recorded
velocity components from Slippi 3.5 onward, hitlag from 3.8 onward, and current
animation index from 3.11 onward, and current and last-hit action-instance IDs
from 3.16 onward. Matching those fields does not certify complete state or
Melee gameplay.
The current match rules and real Fox resources remain incomplete; see
the [initialization format and comparison limits](docs/replays.md).

See [headless architecture](docs/architecture.md) and
[equivalence testing](docs/equivalence.md), plus the
[replay integration contract](docs/replays.md). Independent library projects live under
`crates/`; CLI, replay, testing, rendering, and training/coaching adapters are
kept outside the core simulation library.

`skirmish-probe` lives in `skirmish-equivalence` and is a headless RNG executable
for testing the comparison harness.
The suite compiles a separate executable from original C, compares it with the
Rust executable at two C optimization levels, and verifies that deliberate
divergence, input mutation, nonzero exit and timeout all fail. It is not a game.
