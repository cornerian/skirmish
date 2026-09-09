# Local validation provenance

The 2026-09-09 native runtime, physics, input and replay validation run is stored
outside Git at:

`/mnt/archive/runs/skirmish-rust-port-20260909-v2`

It contains command/exit-code metadata and debug, optimized, lint and Rust-only
test logs. The earlier `skirmish-rust-port-20260909-v1` directory also contains a
hash inventory of all 2,441 upstream C/C++/header/assembly files. The input source
revision and toolchain version are recorded in `validation.json`. The upstream
revision is `0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`. Original test snapshots are
tracked separately in `tests/oracle/sources.json` and checked for exact coverage
and SHA-256 equality by the test suite.

Build products remain in `/mnt/shared/tmp/skirmish-target`. Local absolute paths
are provenance, not build requirements; CI builds in its own temporary directory.
No Melee game image or full-game behavior comparison participated in this run.
