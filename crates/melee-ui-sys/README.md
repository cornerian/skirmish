# Melee UI native source boundary

This crate is the narrow unsafe boundary for compiling and executing the pinned
doldecomp Melee menu source on a native host. It does not contain a rewritten
menu. With the `source` feature, `build.rs` finds the upstream checkout through
`SKIRMISH_MELEE_SOURCE` or the workspace's documented `../../External/melee`
location, requires that checkout to be clean and exactly at the pinned revision,
verifies `mnmain.c` byte-for-byte against the oracle snapshot, and compiles that
complete source file with its exact-revision animation and joint-lookup helpers
under a host ABI prelude.

Only functions whose dependencies have real host implementations are exposed.
Linker section collection discards unreachable original functions; extending
the API requires implementing the newly reachable HSD/game boundary rather than
adding success stubs. The current slice executes the original digit and menu
light-index helpers, `mn_80229B2C`, `mn_80229DC0`, and the latter's original
`fn_80229BF4` process callback. A C-owned scene runtime performs GObj/JObj
attachment and proc registration over the supplied, pinned 102-node background
and 106-node panel topologies. It executes the original preorder lookup and
process callbacks, records the configured render-callback invocation, and runs
source user-data teardown. It records the original Main-to-VS frame requests
from 400 to the 500 idle loop, but reports the pose as unevaluated until a real
archive animation-track decoder is connected.
