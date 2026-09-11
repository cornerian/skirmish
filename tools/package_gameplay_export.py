"""Package a local gameplay-export directory into the tarball CI downloads.

Run with `uv run --no-project tools/package_gameplay_export.py --source DIR
--version v1 --output OUT.tar.gz`. Stdlib only; no project dependencies.

The producer (`skirmish-assets`) writes a directory shaped like
`<source>/<pairing>/match-data.json` plus each pairing's own `manifest.json`
(see docs/gameplay-export.md's "Durable location" section and
`crates/cli/tests/real_parity.rs`'s doc comment for the exact layout). This
script does not validate that shape beyond requiring the directory to exist
and be non-empty; `real_parity.rs` is the consumer that enforces it at test
time.

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
import sys
import tarfile
from pathlib import Path


def package(source: Path, output: Path, version: str) -> dict:
    if not source.is_dir():
        raise SystemExit(f"source directory does not exist: {source}")
    entries = sorted(p for p in source.rglob("*") if p.is_file())
    if not entries:
        raise SystemExit(f"source directory is empty: {source}")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(output, "w:gz") as tar:
        for entry in entries:
            tar.add(entry, arcname=str(entry.relative_to(source)))
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
    args = parser.parse_args(argv)

    lock = package(args.source, args.output, args.version)
    print(json.dumps(lock, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
