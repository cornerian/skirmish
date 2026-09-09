# Skirmish

Port the pinned doldecomp/melee source into safe, concise Rust. Preserve observable
semantics before reducing code. Prefer standard collections and established
libraries where their behavior matches. Never replace a subsystem with a stub
and call it translated; report partial function coverage explicitly.

Hard requirement: native modern-machine builds, execution, and tests must never
require an ISO, DOL, emulator, or GameCube runtime. Use native game resources,
host-compiled original C, generated scenarios and future replay observations.

Resource extraction and visual conversion are owned by a separate task/project,
`melee-assets`. Keep this task focused on native gameplay and physics; consume
its exports as they become available without duplicating extraction or
reorganizing its directories. Vector artwork and procedural visual effects are
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
- Commit and push substantial verified milestones to `origin/main`, as requested.
  The old history is archived on `archive/pre-rust-rewrite-20260909`; `main`
  starts with a fresh root commit, per the user's explicit instruction.
