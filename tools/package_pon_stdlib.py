"""Build a deterministic release bundle for Pon's pure-Python stdlib.

The source is an explicit CPython ``Lib`` directory from the pinned Pon
checkout. The output is intended for a release staging area such as
``/tmp/skirmish-stdlib-release-proof``; this tool never modifies the source
tree or installs anything. It packages only Python source files and records
the source hashes in ``manifest.json``.

Example (run with ``uv run --no-project``):

    uv run --no-project tools/package_pon_stdlib.py \
      --source /path/to/pon-conformance/vendor/cpython-3.14/Lib \
      --license /path/to/cpython/LICENSE \
      --output /tmp/skirmish-stdlib-release-proof/pon-stdlib.tar.gz
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
import sys
import tarfile
from pathlib import Path, PurePosixPath

PON_REVISION = "ab9067dbd2899c64c4d67a4bc27b8ad49472b126"
CPYTHON_REVISION = "v3.14.0"
_EXCLUDED_DIRECTORIES = ("__pycache__", "site-packages", "test", "tests")
_EXCLUDED_SUFFIXES = (".dll", ".dylib", ".pyd", ".so")


class PackageError(ValueError):
    """Raised when a stdlib input cannot be safely packaged."""


def _canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def _source_files(source: Path) -> tuple[str, list[tuple[str, Path]], list[str]]:
    if source.is_symlink() or not source.is_dir():
        raise PackageError(f"stdlib source directory does not exist: {source}")
    revision_file = source.parent / "REVISION"
    if not revision_file.is_file() or revision_file.is_symlink():
        raise PackageError(f"stdlib source must have a regular sibling REVISION file: {revision_file}")
    revision = revision_file.read_text(encoding="utf-8").strip()
    if revision != CPYTHON_REVISION:
        raise PackageError(f"unsupported CPython REVISION {revision!r}; expected {CPYTHON_REVISION}")

    files: list[tuple[str, Path]] = []
    excluded: list[str] = []
    for root, directories, names in os.walk(source, followlinks=False):
        root_path = Path(root)
        kept_directories: list[str] = []
        for directory in sorted(directories):
            path = root_path / directory
            if path.is_symlink():
                raise PackageError(f"stdlib source contains symlink: {path}")
            if directory in _EXCLUDED_DIRECTORIES:
                excluded.append(str(path.relative_to(source).as_posix()) + "/")
            else:
                kept_directories.append(directory)
        directories[:] = kept_directories
        for name in sorted(names):
            path = root_path / name
            if path.is_symlink():
                raise PackageError(f"stdlib source contains symlink: {path}")
            relative = path.relative_to(source).as_posix()
            if any(part in _EXCLUDED_DIRECTORIES for part in PurePosixPath(relative).parts):
                excluded.append(relative)
                continue
            if path.suffix.lower() in _EXCLUDED_SUFFIXES or path.suffix.lower() in {".pyc", ".pyo"}:
                excluded.append(relative)
                continue
            if path.suffix != ".py":
                excluded.append(relative)
                continue
            if not path.is_file():
                raise PackageError(f"stdlib source entry is not a regular file: {path}")
            files.append((relative, path))
    if not files:
        raise PackageError(f"stdlib source contains no Python files: {source}")
    return revision, files, sorted(excluded)


def _snapshot(files: list[tuple[str, Path]], license_path: Path) -> tuple[list[tuple[str, bytes]], bytes]:
    if license_path.is_symlink() or not license_path.is_file():
        raise PackageError(f"license must be a regular file: {license_path}")
    snapshot = []
    for relative, path in files:
        if path.is_symlink() or not path.is_file():
            raise PackageError(f"stdlib source entry changed during packaging: {path}")
        snapshot.append((relative, path.read_bytes()))
    snapshot.sort(key=lambda item: item[0])
    return snapshot, license_path.read_bytes()


def _manifest(revision: str, files: list[tuple[str, bytes]], license_bytes: bytes, excluded: list[str], pon_revision: str) -> dict[str, object]:
    records = [
        {
            "path": relative,
            "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
        }
        for relative, data in files
    ]
    license_record = {
        "path": "LICENSE",
        "bytes": len(license_bytes),
        "sha256": hashlib.sha256(license_bytes).hexdigest(),
    }
    identity_payload = {
        "pon_revision": pon_revision,
        "cpython_revision": revision,
        "license": license_record,
        "files": records,
    }
    identity = hashlib.sha256(_canonical_json(identity_payload)).hexdigest()
    return {
        "format": "skirmish-pon-stdlib-v1",
        "pon_revision": pon_revision,
        "cpython_revision": revision,
        "root": "stdlib",
        "file_count": len(records),
        "license": license_record,
        "files": records,
        "identity": identity,
        "excluded": {
            "directories": list(_EXCLUDED_DIRECTORIES),
            "suffixes": list(_EXCLUDED_SUFFIXES) + [".pyc", ".pyo"],
            "files": excluded,
        },
    }


def _tar_bytes(manifest: dict[str, object], files: list[tuple[str, bytes]], license_bytes: bytes) -> bytes:
    manifest_bytes = json.dumps(manifest, indent=2, sort_keys=True).encode() + b"\n"
    output = io.BytesIO()
    with gzip.GzipFile(fileobj=output, mode="wb", mtime=0, filename="") as compressed:
        with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
            entries = [("manifest.json", manifest_bytes), ("LICENSE", license_bytes)]
            entries.extend((f"stdlib/{relative}", data) for relative, data in files)
            for name, data in entries:
                info = tarfile.TarInfo(name)
                info.size = len(data)
                info.mode = 0o644
                info.mtime = 0
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                archive.addfile(info, io.BytesIO(data))
    return output.getvalue()


def package(source: Path, license_path: Path, output: Path, *, pon_revision: str = PON_REVISION, expected_identity: str | None = None) -> dict[str, object]:
    if pon_revision != PON_REVISION:
        raise PackageError(f"unsupported Pon revision {pon_revision!r}; expected pinned {PON_REVISION}")
    source_resolved = source.resolve()
    output_resolved = output.resolve()
    protected_inputs = {license_path.resolve(), (source.parent / "REVISION").resolve()}
    if output_resolved in protected_inputs:
        raise PackageError(f"output must not overwrite an input file: {output}")
    try:
        output_resolved.relative_to(source_resolved)
    except ValueError:
        pass
    else:
        raise PackageError(f"output must be outside the stdlib source tree: {output}")
    revision, paths, excluded = _source_files(source)
    files, license_bytes = _snapshot(paths, license_path)
    manifest = _manifest(revision, files, license_bytes, excluded, pon_revision)
    if expected_identity is not None and manifest["identity"] != expected_identity:
        raise PackageError(
            f"source identity changed: expected {expected_identity}, got {manifest['identity']}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(_tar_bytes(manifest, files, license_bytes))
    return {
        "format": manifest["format"],
        "file": output.name,
        "sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
        "bytes": output.stat().st_size,
        "identity": manifest["identity"],
        "file_count": manifest["file_count"],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path, help="CPython Lib directory")
    parser.add_argument("--license", required=True, type=Path, help="exact CPython license text to include")
    parser.add_argument("--output", required=True, type=Path, help="deterministic .tar.gz output outside the source tree")
    parser.add_argument("--expect-identity", help="reject changed input unless its manifest identity matches this hash")
    args = parser.parse_args(argv)
    try:
        print(json.dumps(package(args.source, args.license, args.output, expected_identity=args.expect_identity), indent=2, sort_keys=True))
    except (OSError, PackageError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    sys.exit(main())
