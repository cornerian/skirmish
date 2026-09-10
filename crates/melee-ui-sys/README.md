# Melee UI native source boundary

This crate is the narrow unsafe boundary for compiling and executing the pinned
doldecomp Melee menu source on a native host. It does not contain a rewritten
menu. With the `source` feature, `build.rs` finds the upstream checkout through
`SKIRMISH_MELEE_SOURCE` or the workspace's documented `../../External/melee`
location, requires that checkout to be clean and exactly at the pinned revision,
verifies `mnmain.c` byte-for-byte against the oracle snapshot, and compiles that
complete source file with a host ABI prelude.

Only functions whose dependencies have real host implementations are exposed.
Linker section collection discards unreachable original functions; extending
the API requires implementing the newly reachable HSD/game boundary rather than
adding success stubs. The current slice executes the original digit and menu
light-index helpers. JObj/archive/scheduler integration remains explicit work.
