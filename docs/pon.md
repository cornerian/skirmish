# Pon integration status

This page records the in-progress integration of
[can1357/pon](https://github.com/can1357/pon), pinned at commit
`ab9067dbd2899c64c4d67a4bc27b8ad49472b126`. Pon is embedded as Rust native
code; it is not a PyO3 or maturin extension and does not add a CPython
dependency to the game.

The embedded runtime now imports the class based API from the installable
[pure-Python fighter SDK](../scripts/api/README.md), and focused tests
compile the public package and exercise the typed native ABI through
`Program`. The retained [`tools/pon-probe`](../tools/pon-probe/README.md)
remains a small standalone embedding smoke test, while `crates/pon-runtime`
and `tests/game_scripting.rs` cover the integrated host seam.

The native bridge is now connected to the match lifecycle and combat path.
`crates/script-runtime/tests/stdlib_gameplay.rs` covers a full class export
and bound callback with the verified stdlib, while the CLI startup path
streams, verifies, and materializes an explicitly configured archive before
game startup. Release bundles may instead place the executable-relative
resources below `resources/pon-stdlib/`: `pon-stdlib.tar.gz` and the pinned
`identity.json` manifest. Build that layout outside the repository with:

```xonsh
uv run --no-project tools/package_skirmish_runtime.py \
  --executable /path/to/skirmish \
  --stdlib-archive /path/to/pon-stdlib.tar.gz \
  --output /tmp/skirmish-release
```

The default loader resolves those resources beside the current executable,
independent of the working directory, and verifies the archive and identity
before startup. Explicit `--pon-stdlib` plus `--pon-stdlib-sha256` options
override the packaged default. A developer executable with no resource
directory keeps its existing development behavior; an incomplete resource
directory fails closed rather than falling back to a checkout.

## SDK import audit

The SDK's direct imports are mostly ordinary CPython library modules:
`dataclasses`, `types`, `typing`, and `enum` are pure-Python imports, while
`struct` is a pure-Python wrapper over Pon's native `_struct` module. The API's
`math` import resolves to Pon's curated native `math` module. The package's
transitive imports then reach more pure-Python library files such as
`inspect`, `abc`, `collections`, `copy`, `functools`, and `re`, plus the native
accelerators those modules expect. The SDK package itself therefore does not
constitute a self-contained Pon standard library.

The current runtime resolver searches curated native modules first, then
installed/source roots, and finally the CPython 3.14 `Lib` tree. If
`PON_STDLIB_PATH` is set, it is authoritative; a missing directory disables
the stdlib root. Otherwise the resolver may find the tree relative to the
compiled Pon checkout or the current working directory. That behavior is
appropriate for development and tests, but it does not let a compiled game
load the SDK from an unrelated working directory without a Python installation
or developer checkout. A controlled release must provide the pure-Python
stdlib as read-only resources and pass its root through the host's bundle
interface; the smallest safe bundle is the content-pinned closure required by
the SDK, with Pon's native registry satisfying the native portion.

This dependency is observable today: running the public package conformance
test with `PON_STDLIB_PATH` set to a nonexistent path fails at
`ModuleNotFoundError: No module named 'dataclasses'`. The normal focused test
passes only when the pinned checkout's vendored `Lib` tree is discoverable.
The checkout currently contains 1,845 files occupying about 38 MiB; it is a
release input to hash and bundle, not a tree to copy into this repository or a
developer home directory.

## Release stdlib bundle

`tools/package_pon_stdlib.py` packages an explicit `Lib` input into a
deterministic gzip tarball outside the repository. It rejects missing or
malformed `REVISION` metadata, symlink entries, and changed input when
`--expect-identity` is supplied. It excludes `test/`, `tests/`,
`site-packages/`, `__pycache__/`, bytecode, and host extension binaries. Run it with `uv` and an
explicit source path:

```xonsh
uv run --no-project tools/package_pon_stdlib.py \
  --source /path/to/pon-conformance/vendor/cpython-3.14/Lib \
  --license /path/to/cpython/LICENSE \
  --output /tmp/skirmish-stdlib-release-proof/pon-stdlib.tar.gz
```

The archive reader contract is `manifest.json` plus files below `stdlib/`.
The manifest format is `skirmish-pon-stdlib-v1` and contains the pinned Pon
revision, CPython `REVISION`, sorted per-file `path`/`bytes`/`sha256` records,
the `LICENSE` record, aggregate `identity`, and explicit exclusion rules. A release resource
loader should verify the aggregate identity and each file hash before exposing
the read-only root to Pon. The current proof artifact (733 Python source files
plus the license) is
`/tmp/skirmish-stdlib-release-proof/pon-stdlib-a.tar.gz`, SHA-256
`5c7ce12a21f4d5ae49a8cb2c425911bc4de863f427e0ff174fb74ff7bf63639c`;
its CPython v3.14.0 license entry has SHA-256
`b0e25a78cffb43f4d92de8b61ccfa1f1f98ecbc22330b54b5251e7b6ba010231` and
the manifest identity is
`b14b1766c1138f9aad84664dbbd4f52242b874a1003810a7ec69410cc5f83782`;
repeating the command produced the same bytes and hash. This bundle is a
release input for the controlled host interface. The CLI automatically loads
and verifies this packaged archive when it is placed beside the executable;
the broader release import-root isolation remains an open integration gate.

Pon lowers Python syntax to native code through Ruff and Cranelift. The probe
uses Pon's runtime and garbage collector, so its result is limited to the
single-thread smoke path exercised there. It does not establish performance,
memory safety, rollback compatibility, resource loading, or gameplay parity.

The native Rust event, action, collision, physics, projectile, and motion paths
remain authoritative for gameplay. A focused clean-process full Fox load now
passes in `tests/pon_runtime_bundle.rs` with the explicitly verified stdlib
archive. Complete gameplay parity and async move rollback are not yet
verified. The acceptance
requirements in [the integration plan](pon-integration-plan.md) remain the bar
for replacing those native paths.
