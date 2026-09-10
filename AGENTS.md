# Skirmish

Reproduce Melee's observable behavior exactly in safe, concise, maintainable
Rust. The finished presentation must be completely identical to the pinned game:
assets, composition, transforms, animation, visibility, materials, timing, text,
audio cues, input response, transitions, and scene handoffs have no intentional
approximations. Verify visual work against reference captures and report every
remaining difference explicitly.

Implementation parity is not the goal. Do not recreate HSD/GObj internals or
preserve source structure when a smaller, clearer design produces the same
observable result. Prefer modular, data-driven systems and established libraries
where their behavior matches. Keep archive decoding, immutable resources,
animation evaluation, scene instances, input, menu flow, rendering, audio, and
destination services behind explicit interfaces. Use stable source identities
plus runtime instance identities so cloned objects remain distinguishable.

Treat extensibility and modding as first-class requirements. Menu definitions,
assets, transitions, destinations, themes, and optional elements should be
replaceable or additive through declarative data and narrow extension points,
not hardcoded branch tables or forks of the engine. The original C source is a
behavioral specification and a focused test oracle; execute it in the product
only when that is simpler and cleaner than an equivalent Rust implementation.
Never replace a subsystem with a stub and call it complete; report partial
coverage explicitly.

Hard requirement: native modern-machine builds, execution, and tests must never
require an ISO, DOL, emulator, or GameCube runtime. Use native game resources,
host-compiled original C, generated scenarios and future replay observations.

Visual conversion and reconstruction are owned by the separate resource project,
`skirmish-assets`; raw original-file extraction remains an independent library.
Do not build a temporary custom menu or asset-import UI while the original menu
runtime is incomplete. Consume the resource project's converted exports as they
become available without duplicating its conversion pipeline or reorganizing its
directories. Vector artwork and procedural visual effects are
presentation resources. Bone animation, collision geometry, action scripts and
gameplay-affecting effects must retain their simulation semantics. See
`docs/resources.md` for the consumer requirements; the final export layout is
owned by the resource task.

- Upstream source: `../../External/melee` relative to this project, or the path
  passed to `skirmish inventory`. Revision is recorded in `upstream.lock.json`.
- Do not introduce disc/executable dependencies. Reference C snapshots are
  small test fixtures only. Preserve their exact contents and provenance.
- Keep builds in `/mnt/shared/tmp/skirmish-target` (set `CARGO_TARGET_DIR`), with
  `/tmp/skirmish-target` as a sandbox fallback. Durable test outputs belong in
  `/mnt/archive/runs`; disposable output belongs in `/mnt/shared/tmp`.
- Use `xonsh --no-rc` for shell work. Rust uses Cargo; Python helpers, if needed,
  use uv, and JavaScript helpers use bun.
- Run `cargo fmt --all --check`, `cargo test --locked --workspace`,
  `cargo test --locked --workspace --features c-oracle`, and
  `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`.
- C reference tests must use upstream code, with ABI adaptations documented.
  Host C agreement does not establish PowerPC or whole-game equivalence.
- Prefer integration tests of input-to-observable behavior across subsystem
  boundaries while refactoring. Add focused original-C unit/differential tests
  where a Rust function deliberately retains exact source-function parity,
  especially arithmetic edge cases; do not mirror refactored implementation
  structure in unit tests. Missing behavior and missing independent fixtures
  must be reported separately from passing coverage.
- Commit substantial verified milestones and push the active feature branch to
  `origin` continually, as requested. Do not push directly to `main` unless the
  user explicitly asks for that destination. The old history is archived on
  `archive/pre-rust-rewrite-20260909`; `main` starts with a fresh root commit.
