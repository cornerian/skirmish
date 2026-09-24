"""Source contract for Pikachu's native archive identity."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighters.pikachu import Pikachu, PikachuParameters


class PikachuMetadataTests(unittest.TestCase):
    def test_initializer_archive_metadata_matches_ftpikachu(self):
        self.assertIs(Pikachu.parameters, PikachuParameters)
        self.assertEqual(PikachuParameters().data_file, "PlPk.dat")
        self.assertEqual(PikachuParameters().animation_data_file, "PlPkAJ.dat")
        self.assertEqual(
            Pikachu.costume_files,
            ("PlPkNr.dat", "PlPkRe.dat", "PlPkBu.dat", "PlPkGr.dat"),
        )


if __name__ == "__main__":
    unittest.main()
