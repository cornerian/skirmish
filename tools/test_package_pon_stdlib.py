import importlib.util
import json
import tarfile
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("package_pon_stdlib.py")
SPEC = importlib.util.spec_from_file_location("package_pon_stdlib", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class PackagePonStdlibTests(unittest.TestCase):
    def make_source(self, root: Path) -> Path:
        source = root / "cpython" / "Lib"
        source.mkdir(parents=True)
        (source.parent / "REVISION").write_text("v3.14.0\n")
        (source.parent / "LICENSE").write_text("CPython license text\n")
        (source / "dataclasses.py").write_text("VALUE = 1\n")
        (source / "test").mkdir()
        (source / "test" / "ignored.py").write_text("VALUE = 2\n")
        (source / "__pycache__").mkdir()
        (source / "__pycache__" / "ignored.pyc").write_bytes(b"pyc")
        return source

    def test_repeated_output_is_byte_for_byte_deterministic_and_manifest_is_readable(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            source = self.make_source(root)
            first = root / "one.tar.gz"
            second = root / "two.tar.gz"
            license_path = source.parent / "LICENSE"
            first_result = MODULE.package(source, license_path, first)
            second_result = MODULE.package(source, license_path, second)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            self.assertEqual(first_result["sha256"], second_result["sha256"])
            with tarfile.open(first, "r:gz") as archive:
                manifest = json.loads(archive.extractfile("manifest.json").read())
                self.assertEqual(manifest["cpython_revision"], "v3.14.0")
                self.assertEqual(manifest["file_count"], 1)
                self.assertEqual(archive.getnames(), ["manifest.json", "LICENSE", "stdlib/dataclasses.py"])
                self.assertEqual(manifest["license"]["path"], "LICENSE")
                self.assertEqual(archive.extractfile("LICENSE").read(), b"CPython license text\n")

    def test_missing_revision_symlink_and_tampered_input_are_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            source = self.make_source(root)
            license_path = source.parent / "LICENSE"
            result = MODULE.package(source, license_path, root / "bundle.tar.gz")
            (source / "dataclasses.py").write_text("VALUE = 9\n")
            with self.assertRaises(MODULE.PackageError):
                MODULE.package(source, license_path, root / "changed.tar.gz", expected_identity=result["identity"])
            (source.parent / "REVISION").write_text("v3.14.1\n")
            with self.assertRaises(MODULE.PackageError):
                MODULE.package(source, license_path, root / "wrong-revision.tar.gz")
            (source.parent / "REVISION").write_text("v3.14.0\n")
            (source.parent / "REVISION").unlink()
            with self.assertRaises(MODULE.PackageError):
                MODULE.package(source, license_path, root / "missing-revision.tar.gz")

            source = self.make_source(root / "symlink-case")
            (source / "escape.py").symlink_to(root / "outside.py")
            with self.assertRaises(MODULE.PackageError):
                MODULE.package(source, source.parent / "LICENSE", root / "symlink.tar.gz")

    def test_output_inside_source_is_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            source = self.make_source(root)
            with self.assertRaises(MODULE.PackageError):
                MODULE.package(source, source.parent / "LICENSE", source / "bundle.tar.gz")

    def test_output_cannot_overwrite_license_or_revision(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            source = self.make_source(root)
            license_path = source.parent / "LICENSE"
            revision_path = source.parent / "REVISION"
            for output in (license_path, revision_path):
                with self.assertRaises(MODULE.PackageError):
                    MODULE.package(source, license_path, output)


if __name__ == "__main__":
    unittest.main()
