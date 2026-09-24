"""Regression coverage for Zelda Din Fire entry command reset."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import export_definition  # noqa: E402
from fighters.zelda import Zelda  # noqa: E402


class _Fighter:
    def __init__(self):
        self.action_state = SimpleNamespace(command=(9, 8, 7, 6))


class ZeldaDinEntryTests(unittest.TestCase):
    def test_ground_entry_clears_all_native_command_slots(self):
        fighter = _Fighter()
        Zelda.specials.side.enter(fighter, SimpleNamespace())
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))

    def test_air_entry_clears_all_native_command_slots(self):
        fighter = _Fighter()
        Zelda.specials.side.enter(fighter, SimpleNamespace())
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))

    def test_entry_callback_is_bound_to_both_source_start_states(self):
        definition = export_definition(Zelda).as_dict()
        behavior_id = definition["movesets"]["specials"]["side"]
        behavior = next(item for item in definition["behaviors"]
                        if item["id"] == behavior_id)
        entries = [item for item in behavior["callbacks"]
                   if item["hook"] == "action_entered"]
        self.assertEqual(entries[0]["actions"], ["Source.18:343", "Source.18:346"])


if __name__ == "__main__":
    unittest.main()
