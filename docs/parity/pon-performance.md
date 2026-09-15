# Pon callback performance

The manual benchmark in
`crates/pon-runtime/tests/callback_performance.rs` compares one cached,
compiled Pon class-bound callback with handwritten Rust performing the same
integer event transform (`value + 1`). Pon compilation happens once before
warmup; every measured iteration reuses the same prepared callback and uses the
same modulo-97 integer inputs. Both paths must produce the same checksum.

Run it with the pinned Pon source and an external release target directory:

```sh
env CARGO_HOME=/tmp/skirmish-pon-cargo \
  CARGO_TARGET_DIR=/tmp/skirmish-pon-target \
  cargo test --release --manifest-path crates/pon-runtime/Cargo.toml \
  --test callback_performance -- --ignored --nocapture --test-threads=1
```

The test reports seven samples of 20,000 iterations after a 2,000-iteration
warmup, their medians, and the Pon/Rust ratio. This measures the typed facade,
Pon object boxing/unboxing, native call bridge, and trivial callback body. It
does not measure a fighter workload, event scheduling, resource access,
rollback, or a complete match step. The result is machine-dependent evidence,
not a performance guarantee or CI threshold, and it is not a CPython
comparison.

Measured on 2026-09-15 with the pinned commit, optimized release build, 20,000
iterations per sample, seven samples, and 2,000 warmup iterations:

```text
Pon median:  9,233,859 ns  (461.7 ns/callback)
Rust median:    18,081 ns  (0.90 ns/transform)
Pon/Rust:          510.69x
checksum:       6,951,913
```

The Pon path is about 511 times slower for this tiny integer bridge workload.
That gap includes the typed facade, boxed object allocation and conversion,
native dispatch, and callback execution; it is not a prediction of full fighter
performance. The checksum assertion passed, but the absolute values and ratio
are one machine snapshot.
