# Local validation provenance

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
