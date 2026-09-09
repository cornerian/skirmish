# Skirmish

Port the pinned doldecomp/melee source into safe, concise Rust. Preserve observable
semantics before reducing code. Prefer standard collections and established
libraries where their behavior matches. Never replace a subsystem with a stub
and call it translated; report partial function coverage explicitly.

- Upstream source: `../External/melee` relative to the Code category, or the path
  passed to `skirmish inventory`. Revision is recorded in `upstream.lock.json`.
- Keep game images and extracted assets outside Git. Reference C snapshots are
  small test fixtures only. Preserve their exact contents and provenance.
- Keep builds in `/mnt/shared/tmp/skirmish-target` (set `CARGO_TARGET_DIR`), with
  `/tmp/skirmish-target` as a sandbox fallback. Durable test outputs belong in
  `/mnt/archive/runs`; disposable output belongs in `/mnt/shared/tmp`.
- Use `xonsh --no-rc` for shell work. Rust uses Cargo; Python helpers, if needed,
  use uv, and JavaScript helpers use bun.
- Run `cargo fmt --check`, `cargo test --locked`,
  `cargo test --locked --features c-oracle`, and
  `cargo clippy --locked --all-targets --all-features -- -D warnings`.
- C reference tests must use upstream code, with ABI adaptations documented.
  Host C agreement does not establish PowerPC or whole-game equivalence.
- Commit and push substantial verified milestones to `origin/main`, as requested.
  The old history is archived on `archive/pre-rust-rewrite-20260909`; `main`
  starts with a fresh root commit, per the user's explicit instruction.
