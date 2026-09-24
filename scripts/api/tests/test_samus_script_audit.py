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
        self.max_jumps_calls = 0

    def change_action(self, action, **kwargs):
        self.action = action

    def max_jumps(self):
        self.max_jumps_calls += 1


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


class SamusScrewAttackEntryTests(unittest.TestCase):
    def test_only_aerial_source_entry_exhausts_jump_budget(self):
        from fighters.samus import ScrewAttack

        move = ScrewAttack()
        grounded = _Fighter()
        grounded.action = move.ground
        move.action_enter(grounded, SimpleNamespace())
        self.assertEqual(grounded.max_jumps_calls, 0)

        aerial = _Fighter()
        aerial.action = move.air
        move.action_enter(aerial, SimpleNamespace())
        self.assertEqual(aerial.max_jumps_calls, 1)


if __name__ == "__main__":
    unittest.main()
