"""Package a local gameplay-export directory into the tarball CI downloads.

Run with `uv run --no-project tools/package_gameplay_export.py --source DIR
--version v1 --output OUT.tar.gz --skirmish-cli PATH/TO/skirmish`. Stdlib
only; no project dependencies (the `--skirmish-cli` binary is an already-
built Skirmish CLI, `cargo build -p skirmish-cli --release`, not a Python
dependency).

The producer (`skirmish-assets`) writes a directory shaped like
`<source>/<pairing>/match-data.json` plus each pairing's own `manifest.json`,
a top-level `manifest.json`/`rules.json` and `stages/*.json` (see
docs/gameplay-export.md's "Durable location" section and
`crates/cli/tests/real_parity.rs`'s doc comment for the exact layout). This
script does not validate that shape beyond requiring at least one pairing
directory (a direct child of `--source` containing its own
`match-data.json`) to exist; `real_parity.rs` is the consumer that enforces
the rest at test time.

For each pairing directory, this script runs `skirmish pack convert` (see
`crates/cli/src/pack.rs`) to produce a compact, lossless `match-data.bin`
sibling and packages *that* instead of the multi-hundred-megabyte
`match-data.json` -- every loader that accepts `--match-data` prefers a
`.bin` file when present (`skirmish_cli::pack::discover_match_data`), so
nothing downstream needs to know the tarball changed shape. The source
directory's own top-level duplicate `match-data.json` (a copy of one
pairing's file that historically also lived at the export root) and any
other large sidecar data (e.g. a `fighters/*.json` dump) are intentionally
left out of the tarball: only `match-data.bin` per pairing, each pairing's
own `manifest.json`, and the top-level `manifest.json`/`rules.json`/
`stages/*.json` are small enough, and are all that `real_parity.rs`,
`make-initialization` and `validate-replay` actually read. This script never
modifies `--source` itself; every conversion is written to a temporary
staging directory that is packaged and discarded.

The reviewer runs this once against the real export
(`/mnt/archive/datasets/melee/skirmish-gameplay/<version>/`), uploads the
resulting tarball to `https://huggingface.co/datasets/cornerian/skirmish-datapacks`
under `gameplay/<version>/<file>`, and pastes the printed JSON over
`tests/fixtures/slippi/parity/gameplay-export.lock.json`'s placeholder
(`"sha256": "pending"`), which is what tells `system-tests.yml` to stop
skipping the download step.

Chooses `.tar.gz` (not `.tar.zst`) specifically so this script stays within
Python's standard library (`tarfile`'s gzip support); no `zstandard`
dependency is introduced for a one-off packaging step.
"""

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

# Top-level files/directories (relative to --source) that are small and are
# copied into the tarball verbatim, alongside each pairing's converted
# match-data.bin and its own manifest.json.
KEPT_TOP_LEVEL_FILES = ("manifest.json", "rules.json")
KEPT_TOP_LEVEL_DIRS = ("stages",)


def find_pairings(source: Path) -> list[Path]:
    """Direct subdirectories of `source` that hold their own
    `match-data.json`, e.g. `<source>/fox-fd/`."""
    return sorted(
        child
        for child in source.iterdir()
        if child.is_dir() and (child / "match-data.json").is_file()
    )


def convert_pairing(skirmish_cli: Path, pairing: Path, staging: Path) -> None:
    json_path = pairing / "match-data.json"
    bin_path = staging / pairing.name / "match-data.bin"
    bin_path.parent.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(
        [str(skirmish_cli), "pack", "convert", str(json_path), str(bin_path)],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(
            f"skirmish pack convert failed for {json_path}:\n{result.stdout}\n{result.stderr}"
        )
    manifest = pairing / "manifest.json"
    if manifest.is_file():
        shutil.copy2(manifest, staging / pairing.name / "manifest.json")


def stage(source: Path, skirmish_cli: Path, staging: Path) -> None:
    pairings = find_pairings(source)
    if not pairings:
        raise SystemExit(
            f"no pairing directories (a subdirectory with its own match-data.json) found under {source}"
        )
    for pairing in pairings:
        convert_pairing(skirmish_cli, pairing, staging)

    for name in KEPT_TOP_LEVEL_FILES:
        candidate = source / name
        if candidate.is_file():
            shutil.copy2(candidate, staging / name)

    for name in KEPT_TOP_LEVEL_DIRS:
        candidate = source / name
        if candidate.is_dir():
            shutil.copytree(candidate, staging / name, dirs_exist_ok=True)


def package(source: Path, output: Path, version: str, skirmish_cli: Path) -> dict:
    if not source.is_dir():
        raise SystemExit(f"source directory does not exist: {source}")
    if not skirmish_cli.is_file():
        raise SystemExit(f"--skirmish-cli does not exist: {skirmish_cli}")

    with tempfile.TemporaryDirectory(prefix="skirmish-gameplay-export-") as staging_name:
        staging = Path(staging_name)
        stage(source, skirmish_cli, staging)

        entries = sorted(p for p in staging.rglob("*") if p.is_file())
        if not entries:
            raise SystemExit(f"nothing to package after staging {source}")

        output.parent.mkdir(parents=True, exist_ok=True)
        with tarfile.open(output, "w:gz") as tar:
            for entry in entries:
                tar.add(entry, arcname=str(entry.relative_to(staging)))

    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    return {
        "version": version,
        "file": output.name,
        "sha256": digest,
        "bytes": output.stat().st_size,
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path, help="Local export directory to package")
    parser.add_argument("--version", required=True, help="Export version, e.g. v1 (matches the HF path segment)")
    parser.add_argument("--output", required=True, type=Path, help="Tarball path to write, e.g. skirmish-gameplay-v1.tar.gz")
    parser.add_argument(
        "--skirmish-cli",
        required=True,
        type=Path,
        help="Path to a built skirmish CLI binary (cargo build -p skirmish-cli --release), used for `pack convert`",
    )
    args = parser.parse_args(argv)

    lock = package(args.source, args.output, args.version, args.skirmish_cli)
    print(json.dumps(lock, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
