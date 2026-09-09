# Native C references

`original/` stores one exact snapshot per upstream file. `sources.json` records
its pinned path and SHA-256; provenance tests reject unlisted or altered fixtures.
Git attributes preserve their original line endings and whitespace, including
whitespace already present upstream; adapters and Rust code use normal checks.

Each C snapshot produces `<stem>_original.inc` in Cargo's build output directory.
An optional `<stem>.functions.json` selects complete original functions; the
`ftcommon` selection is named `physics.functions.json`. Platform includes are
removed and host declarations come from the corresponding C adapters. Documented
bytecode pointer adaptations remain in `build.rs`.

The stage adapter replaces GameCube pointer subtraction with native logical
line indices, preserving the original eight-byte index calculation. Source
definitions are selected past forward declarations. ECB subdivision and ordinary
air knockback decay use exact excerpts of larger callbacks, checked against the
pinned snapshots by tests. Host shims provide arrays, sampled joint positions and
scalar vector normalization; they do not reproduce the whole engine environment.

`adapters.json` maps additional adapter names to existing snapshot stems. Their
own function selections produce separate includes without duplicating source:
physics, hitlag, and locomotion all read the single `original/ftcommon.c` snapshot.
The generated includes are build artifacts and are never checked into Git.

The adapters document their supported scalar/environment inputs. Native C
agreement does not establish equivalence of the unported full game scheduler.
