# Optional roster benchmark

Run from the repository root:

```sh
PYTHONPATH=scripts/api:scripts uv run --offline python scripts/api/benchmarks/roster_dispatch.py
```

The command measures two separate workloads:

* `roster_load_all` imports each `scripts/fighters/*.py` module and runs the
  same validated export path used by the authoring loader.
* `callback_dispatch` invokes one exported callback through the current
  `skirmish._loader.dispatch` path.

Each workload reports the median, minimum, maximum, and every raw sample in
nanoseconds. `--repeats N` changes the number of measured samples and
`--warmup N` controls discarded calls before measurement. Record Python
version, CPU, repository revision, command arguments, and raw output alongside
any result you publish. This benchmark is optional and is intentionally not
collected by `pytest` or the Rust benchmark harness.

There is currently no host-C or embedded-Pon command exposing this same
operation with matching inputs and lifecycle. The benchmark therefore reports
only the current Python authoring path; its timings must not be compared with
the decomp, Pon, or complete gameplay runtime. The Pon embedding probe under
`tools/pon-probe` measures a different JIT workload and is not a baseline for
these numbers.
