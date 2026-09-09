# Local validation provenance

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
There are 32 executable missing-behavior scenarios and 13 cases blocked on
independent reference fixtures. The audit invokes all 45 explicitly and records
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
