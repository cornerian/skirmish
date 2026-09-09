# Local validation provenance

The 2026-09-09 stage, ECB, swept-contact and damage physics validation run is stored
outside Git at:

`/mnt/archive/runs/skirmish-physics-20260909-v4`

It contains command/exit-code metadata and debug, optimized, lint and Rust-only
test logs, plus debug/optimized synthetic match traces and their comparison.
The earlier `skirmish-headless-match-20260909-v3` contains the first native match
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
