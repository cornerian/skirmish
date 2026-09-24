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
        self.assertEqual(PikachuParameters().data_name, "ftDataPikachu")
        self.assertEqual(PikachuParameters().animation_data_file, "PlPkAJ.dat")
        self.assertEqual(PikachuParameters().thunder_jolt_sound, 240076)
        self.assertEqual(
            Pikachu.costume_files,
            ("PlPkNr.dat", "PlPkRe.dat", "PlPkBu.dat", "PlPkGr.dat"),
        )
        self.assertEqual(
            Pikachu.costume_joint_files,
            (
                "PlyPikachu5K_Share_joint",
                "PlyPikachu5KRe_Share_joint",
                "PlyPikachu5KBu_Share_joint",
                "PlyPikachu5KGr_Share_joint",
            ),
        )
        self.assertEqual(
            Pikachu.costume_material_animation_files,
            (
                "PlyPikachu5K_Share_matanim_joint",
                "PlyPikachu5KRe_Share_matanim_joint",
                "PlyPikachu5KBu_Share_matanim_joint",
                "PlyPikachu5KGr_Share_matanim_joint",
            ),
        )
        self.assertEqual(
            Pikachu.demo_motion_files,
            (
                "ftDemoResultMotionFilePikachu",
                "ftDemoIntroMotionFilePikachu",
                "ftDemoEndingMotionFilePikachu",
                "ftDemoViWaitMotionFilePikachu",
            ),
        )


if __name__ == "__main__":
    unittest.main()
