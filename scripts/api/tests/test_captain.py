"""Focused contract tests for the partial Captain Falcon authoring module."""

import importlib.util
import sys
import types
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))


def _load_captain():
    # Shared modules are normally supplied by AssetStore; provide the same
    # namespace for this CPython-only API contract test.
    if "fighters.captain" in sys.modules:
        return sys.modules["fighters.captain"].CaptainFalcon
    shared = types.ModuleType("shared")
    spec = importlib.util.spec_from_file_location(
        "shared.common", ROOT / "scripts" / "fighters" / "common.py"
    )
    common = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules["shared"] = shared
    sys.modules["shared.common"] = common
    spec.loader.exec_module(common)
    shared.common = common
    from fighters.captain import CaptainFalcon

    return CaptainFalcon


class CaptainFalconTests(unittest.TestCase):
    def test_exports_identity_and_resource_backed_punch(self):
        from fighter.api import export_definition

        captain = _load_captain()
        exported = export_definition(captain).as_dict()
        self.assertEqual(exported["name"], "captain-falcon")
        self.assertEqual(exported["external_ids"], [0])
        neutral = exported["movesets"]["specials"]["neutral"]
        self.assertEqual(neutral, "move_0")
        behavior = next(item for item in exported["behaviors"] if item["id"] == neutral)
        self.assertEqual(behavior["resource"], "neutral")
        self.assertEqual(exported["actions"]["special.neutral.ground"]["slippi_state"], 347)
        self.assertEqual(exported["actions"]["special.neutral.air"]["slippi_state"], 348)

    def test_inherits_all_standard_groups_without_duplicate_declarations(self):
        captain = _load_captain()
        fighter_base = captain.__mro__[1]
        self.assertEqual(fighter_base.__name__, "FighterBase")
        for group in ("aerials", "grounded", "tilts", "smashes", "grabs", "throws", "defense", "ledge", "getup", "taunt"):
            self.assertIs(getattr(captain, group), getattr(fighter_base, group))


if __name__ == "__main__":
    unittest.main()
