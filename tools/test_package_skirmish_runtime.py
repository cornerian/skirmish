import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("package_skirmish_runtime.py")
SPEC = importlib.util.spec_from_file_location("package_skirmish_runtime", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class PackageRuntimeTests(unittest.TestCase):
    def test_stages_executable_and_pinned_identity_manifest(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            executable = root / "skirmish"
            executable.write_bytes(b"binary")
            archive = root / "stdlib.tar.gz"
            import tarfile
            with tarfile.open(archive, "w:gz") as tar:
                manifest = json.dumps({"identity": "a" * 64}).encode()
                info = tarfile.TarInfo("manifest.json")
                info.size = len(manifest)
                tar.addfile(info, __import__("io").BytesIO(manifest))
            output = root / "release"
            result = MODULE.package(executable, archive, output)
            identity = json.loads((output / "resources/pon-stdlib/identity.json").read_text())
            self.assertEqual(identity["identity"], "a" * 64)
            self.assertEqual(result["archive_sha256"], __import__("hashlib").sha256((output / "resources/pon-stdlib/pon-stdlib.tar.gz").read_bytes()).hexdigest())
            self.assertTrue((output / "skirmish").stat().st_mode & 0o111)

    def test_missing_manifest_is_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            executable = root / "skirmish"
            executable.write_bytes(b"binary")
            archive = root / "stdlib.tar.gz"
            import tarfile
            with tarfile.open(archive, "w:gz"):
                pass
            with self.assertRaises(MODULE.PackageError):
                MODULE.package(executable, archive, root / "release")


if __name__ == "__main__":
    unittest.main()
