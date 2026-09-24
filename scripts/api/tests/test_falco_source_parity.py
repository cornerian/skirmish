"""Focused source contracts for Falco's fighter-specific initialization."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import ArticleId, export_definition  # noqa: E402
from fighters.falco import Falco, FalcoParameters  # noqa: E402
from fighters.fox import Fox  # noqa: E402


class FalcoSourceParityTests(unittest.TestCase):
    def test_ftfalco_onload_wall_jump_and_laser_identity_are_exported(self):
        falco = FalcoParameters()
        fox = Fox.parameters()
        self.assertTrue(falco.can_walljump)
        self.assertEqual(falco.article_id, ArticleId.FALCO_LASER)
        self.assertEqual(falco.phantasm_article_id, 57)
        self.assertNotEqual(falco.article_id, fox.article_id)

        exported = export_definition(Falco).as_dict()
        self.assertTrue(exported["parameters"]["can_walljump"])
        self.assertEqual(exported["parameters"]["phantasm_article_id"], 57)
        self.assertEqual(exported["parameters"]["article_id"], ArticleId.FALCO_LASER)


if __name__ == "__main__":
    unittest.main()
