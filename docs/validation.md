# Local validation provenance

The 2026-09-10 damage-surface pose and delayed-jump batches are recorded at:

`/mnt/archive/runs/skirmish-surface-tech-poses-20260910`

`/mnt/archive/runs/skirmish-surface-tech-jump-queue-20260910`

Each isolated batch runs formatting, strict all-target/all-feature Clippy,
native workspace tests and original-C differential tests in debug and release
modes. The pose batch supplies complete wall, wall-jump and ceiling physics
skeletons, validates every sample and distinguishes ordinary wall-jump ownership
through the headless bone ECB. The delayed-jump batch covers a neutral wall
tech's queued jump transition, same-frame pose handoff, release-frame boundary
and deterministic checkpoint suffix. Exact jump selection and launch arithmetic
remain compared with their complete pinned C bodies over arbitrary binary32
inputs.

The 2026-09-10 wall-tech air-interrupt batch is recorded at:

`/mnt/archive/runs/skirmish-wall-tech-air-interrupts-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage holds actions through frozen wall
tech startup, rearms fresh input after release, dispatches neutral special,
aerial attack and double jump in source priority, applies the same branches to
ordinary wall jumps and preserves the ceiling tech's empty interrupt callback.

The 2026-09-10 surface-tech landing coverage batch is recorded at:

`/mnt/archive/runs/skirmish-surface-tech-landings-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. One match-level integration test drives neutral wall tech,
wall-jump tech and ceiling tech through gravity, floor contact, shared Landing
timing, transient surface/wall-jump cleanup and checkpoint replay.

The 2026-09-09 grounded-launch coverage batch is recorded at:

`/mnt/archive/runs/skirmish-ground-launch-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises flat and sloped tangent
projection, exact floor departure, fly bounce, hitlag freezing, scalar friction,
prone DownDamage's explicit fly override, malformed resources and checkpoint
replay. The complete retained launch branch and extracted vector-angle helper
are compared over arbitrary binary32 inputs.

The 2026-09-09 prone DownDamage coverage batch is recorded at:

`/mnt/archive/runs/skirmish-down-damage-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises both prone orientations,
strict threshold equality, the source face-down selector quirk, launch and
landing, shared countdown recovery, malformed resources and checkpoint replay.
The complete retained eligibility/selector callback is compared with pinned C.

The 2026-09-09 prone-orientation coverage batch is recorded at:

`/mnt/archive/runs/skirmish-prone-orientation-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage selects face-up and face-down from
the evaluated hip matrix, preserves the choice through checkpoints and drives
distinct bound/wait poses, both roll directions, stand poses and get-up attacks.
The strict matrix-axis/inversion selector has direct unit boundary coverage.

The 2026-09-09 floor-recovery coverage batch is recorded at:

`/mnt/archive/runs/skirmish-floor-recovery-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises the full supplied
Passive/DownBound/DownWait pose suffix, reset and checkpoint behavior for the
source A/B attack buffer, DownBound attack/roll priority, every recovery
invincibility category and combat on the first vulnerable frame. Exact selector
boundaries remain covered by Rust unit tests and the existing complete C-stick
predicate differentials.

The 2026-09-09 missed-tech recovery batch is recorded at:

`/mnt/archive/runs/skirmish-knockdown-options-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises attack/roll/stand input
priority, both roll directions, fresh and held C-stick history, exact sampled
root motion and bones, get-up attack contact, resource rejection and checkpoint
replay. The two exact C-stick predicates are compared against pinned original C
over arbitrary binary32 values.

The 2026-09-09 floor-tech roll batch is recorded at:

`/mnt/archive/runs/skirmish-floor-tech-roll-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs neutral, forward and backward
floor techs, inclusive input selection, separate sampled action durations,
TransN root motion, bone-derived ECB changes, serialization, malformed resources
and exact checkpoint replay. Direction selection compares against complete
pinned original C across arbitrary binary32 inputs. Independent scheduler traces
remain pending.

The 2026-09-09 damage-surface tech batch is recorded at:

`/mnt/archive/runs/skirmish-damage-surface-tech-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs neutral and jump wall techs,
both wall orientations, ceiling input motion, exact action timing, floor/wall
priority, repeat lockout, invalid resources and exact checkpoint replay. Wall
tech jump selection compares against complete pinned original C across timer
boundaries and arbitrary binary32 inputs. Independent scheduler traces remain
pending.

The 2026-09-09 damage-surface batch is recorded at:

`/mnt/archive/runs/skirmish-damage-surface-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs wall and ceiling reflection,
configured action timing, floor-first collision priority, omission and unmet
threshold behavior, malformed resources and exact checkpoint replay. The
retained reflection arithmetic compares against complete pinned original C and
vector-helper bodies. Independent scheduler traces remain pending.

The 2026-09-09 ECB-response batch is recorded at:

`/mnt/archive/runs/skirmish-ecb-response-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs four-sided moving compression,
horizontal and vertical squeeze, next-frame ECB restoration, moving-floor
landing, one-way direction filtering, tangential-motion rejection and exact
checkpoint replay. The reusable squeeze functions continue to compare against
their complete pinned original C bodies. Independent corner/squeeze scheduler
traces remain blocked pending a separate producer.

The 2026-09-09 stage-motion batch is recorded at:

`/mnt/archive/runs/skirmish-stage-motion-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs affine/cyclic collision-line
motion, current geometry, grounded carry after self movement and during hitlag,
airborne detachment/relanding, countdown timing, checkpoint replay and invalid
resources. Generated cases compare the complete original moving-line remap,
including exceptional binary32 inputs. The independent moving-platform trace
remains blocked pending a separate producer.

The 2026-09-09 blast-death batch is recorded at:

`/mnt/archive/runs/skirmish-deaths-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs every normal blast direction,
forced top death, star and screen phases, delayed/final stock loss, input
suppression, checkpoint replay and invalid resources. The complete original
blast selector is compared across generated states, including exact HSD RNG
consumption. Thirteen fidelity cases remain blocked on independent reference
fixtures, including the separate star/screen trace comparison.

The 2026-09-09 neutral-special batch is recorded at:

`/mnt/archive/runs/skirmish-specials-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Both formerly ignored neutral-special scenarios now pass, with
focused integration coverage for input priority and rearming, both player slots,
ground/air physics and terrain conversion, bone-attached combat, checkpoints and
invalid resources. The exact neutral input predicate is compared with complete
pinned C. The conformance audit now has no executable unimplemented scenarios;
13 cases remain blocked on independent reference fixtures.

The 2026-09-09 rebirth-platform batch is recorded at:

`/mnt/archive/runs/skirmish-rebirth-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. The stock-loss platform scenario now passes, with focused
integration coverage for both player slots, exact target travel, wait/release,
invulnerability, checkpoint restoration and invalid resources. Two complete
original leader physics callbacks are compared bit for bit. The conformance
audit now has 2 executable unimplemented scenarios and 13 cases blocked on
independent reference fixtures.

The 2026-09-09 static ledge-action batch is recorded at:

`/mnt/archive/runs/skirmish-ledge-20260909`

It validates the composed tree after the native asset-import commit with
formatting, strict all-target/all-feature Clippy, native workspace tests and
original-C differential tests in debug and release modes. Six formerly ignored
ledge scenarios now pass, with focused integration coverage for endpoint
eligibility and ownership, bone attachment, every ledge option, combat contact,
damage/KO release, cooldown/regrab and checkpoint restoration. The current
conformance audit has 3 executable unimplemented scenarios and 13 cases blocked
on independent reference fixtures.

The 2026-09-09 movement/combat coverage milestone is recorded at:

`/mnt/archive/runs/skirmish-gameplay-20260909-v6b`

It checks the isolated gameplay snapshot with formatting, strict Clippy, native
workspace tests and original-C differential tests in debug and release modes.
Native trace comparisons exercise both the original demo and explicit movement,
combat/displacement/armor and stacked-platform profiles. The run preserves its
commands, input resources, scripts, output traces and source hashes. Synthetic
debug/release agreement checks determinism, not original-game equivalence.

Eleven previously ignored scenarios are now active, accompanied by focused
integration regressions for input history, movement/action timing, displacement,
armor, platform collision and blast boundaries. The remaining audit contains
21 executable unimplemented scenarios and 13 independent-reference-blocked
cases; all 34 are invoked explicitly and remain failures rather than passing
coverage. Authentic movement poses, special-character callbacks and the full
game scheduling order are still outside this milestone.
The earlier `v6` run caught inconsistent synthetic movement/displacement
thresholds after input-history consolidation; `v6b` uses corrected explicit data.

The 2026-09-09 Peppi Slippi importer validation run is stored outside Git at:

`/mnt/archive/runs/skirmish-slippi-20260909-v5c`

It contains command/exit-code metadata and debug, optimized, lint and Rust-only
test logs, plus debug/optimized synthetic match traces and their comparison.
Peppi 2.1.2 writes wholly synthetic `.slp` fixtures covering supported versions,
ports, follower presence/absence, exact input bits, rollback, finalization,
malformed events and executable error handling. A synthetic position protocol
checks the generic transition interface. Separate file-backed scenarios drive
the real native match across walking, jumping, landing and combat, detect late
corruption and changed inputs, and ensure reference observations never reset
simulated state. Their expectations originate from the experimental native
match and do not establish independent Melee equivalence. Explicit conformance
audits report missing implementation and independent-reference fixtures as
failures, separately from the normal passing regressions; see [testing.md](testing.md).
That archived audit had 32 executable missing-behavior scenarios and 13 cases blocked on
independent reference fixtures. It invoked all 45 explicitly and recorded
their failures; default test discovery reports them as ignored. Three real
Slippi 2.0.1 corpus files are also parsed with hashes and frame counts recorded.
This smoke check covers parsing, without compatible native-match initialization
or a gameplay equivalence assertion. The earlier `v5` attempt stopped at
formatting while another task was editing menu tests. `v5b` passed in the shared
workspace; `v5c` validates the fixed commit snapshot in a temporary worktree,
separately from concurrent menu/presentation additions.
The earlier `skirmish-physics-20260909-v4` contains stage, ECB, swept-contact and
damage physics validation. `skirmish-headless-match-20260909-v3` contains the first native match
and skeletal physics milestone. `skirmish-rust-port-20260909-v2` contains the runtime/physics/input/
replay milestone. `skirmish-rust-port-20260909-v1` also contains a
hash inventory of all 2,441 upstream C/C++/header/assembly files. The input source
revision and toolchain version are recorded in `validation.json`. The upstream
revision is `0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`. Original test snapshots are
tracked separately in `tests/oracle/sources.json` and checked for exact coverage
and SHA-256 equality by the test suite.

Build products remain in `/mnt/shared/tmp/skirmish-target`. Local absolute paths
are provenance, not build requirements; CI builds in its own temporary directory.
No Melee game image or full-game behavior comparison participated in this run.
The synthetic native match reaches stock-based termination. Original-C checks
cover selected functions, while debug/optimized match agreement validates the
experimental scheduler's determinism, not Melee compatibility.
