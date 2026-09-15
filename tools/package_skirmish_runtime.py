"""Stage a Skirmish executable and its authenticated Pon stdlib resource."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import tarfile
from pathlib import Path


class PackageError(ValueError):
    pass


def package(executable: Path, archive: Path, output: Path) -> dict[str, str]:
    if not executable.is_file() or executable.is_symlink():
        raise PackageError(f"executable must be a regular file: {executable}")
    if not archive.is_file() or archive.is_symlink():
        raise PackageError(f"stdlib archive must be a regular file: {archive}")
    try:
        with tarfile.open(archive, "r:gz") as tar:
            member = tar.extractfile("manifest.json")
            if member is None:
                raise PackageError("stdlib archive is missing manifest.json")
            manifest = json.load(member)
    except (OSError, KeyError, tarfile.TarError, json.JSONDecodeError) as error:
        raise PackageError(f"cannot read stdlib archive manifest: {error}") from error
    identity = manifest.get("identity")
    if not isinstance(identity, str) or len(identity) != 64:
        raise PackageError("stdlib manifest has no valid identity")
    output.mkdir(parents=True, exist_ok=True)
    target = output / executable.name
    shutil.copy2(executable, target)
    target.chmod(target.stat().st_mode | 0o111)
    resources = output / "resources" / "pon-stdlib"
    resources.mkdir(parents=True, exist_ok=True)
    archive_target = resources / "pon-stdlib.tar.gz"
    shutil.copy2(archive, archive_target)
    archive_sha256 = hashlib.sha256(archive_target.read_bytes()).hexdigest()
    identity_manifest = {
        "format": "skirmish-pon-stdlib-release-v1",
        "archive": archive_target.name,
        "archive_sha256": archive_sha256,
        "identity": identity,
    }
    (resources / "identity.json").write_text(json.dumps(identity_manifest, indent=2, sort_keys=True) + "\n")
    return {"executable": str(target), "archive_sha256": archive_sha256, "identity": identity}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", required=True, type=Path)
    parser.add_argument("--stdlib-archive", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        print(json.dumps(package(args.executable, args.stdlib_archive, args.output), indent=2, sort_keys=True))
    except (OSError, PackageError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
