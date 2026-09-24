"""Regression contract for Samus Charge Shot source entry state."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action, Button
from fighters.samus import ChargeShot


class _Fighter:
    def __init__(self):
        self.action = Action.WAIT
        self.action_state = SimpleNamespace(command=(7, 6, 5, 4))

    def change_action(self, action, **kwargs):
        self.action = action


class SamusChargeEntryTests(unittest.TestCase):
    def test_source_entry_clears_all_command_latches(self):
        move = ChargeShot()
        for action in (move.ground_start, move.air_start):
            fighter = _Fighter()
            fighter.action = action
            move.enter_start(fighter, SimpleNamespace())
            self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))

    def test_source_entry_does_not_require_command_storage(self):
        move = ChargeShot()
        fighter = SimpleNamespace(action=move.ground_start)
        move.enter_start(fighter, SimpleNamespace())


if __name__ == "__main__":
    unittest.main()
