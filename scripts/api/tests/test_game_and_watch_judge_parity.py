"""Regression for the two-row Judge exclusion in the pinned decomp."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
sys.path.insert(0, str(ROOT / "scripts" / "api"))
sys.path.insert(0, str(ROOT / "scripts"))

from fighter import Action, Button
from fighters.game_and_watch import Judge


class _Input:
    pressed = True
    stick = (1.0, 0.0)

    def just_pressed(self, button):
        return button is Button.B


class _Fighter:
    def __init__(self):
        self.action = Action.WAIT
        self.grounded = True
        self.action_frame = 0

    def change_action(self, action, **kwargs):
        self.action = action


def _context(weights, roll):
    resource_value = SimpleNamespace(attributes=SimpleNamespace(judge_roll=weights))

    def resource(path):
        return resource_value if path.endswith(".attributes") else object()

    return SimpleNamespace(
        ground_open=True,
        air_open=False,
        grounded=True,
        stick=(1.0, 0.0),
        input=_Input(),
        resource=resource,
        judge_roll=roll,
        judge_previous=(),
        rules=SimpleNamespace(
            specials=SimpleNamespace(side_stick_threshold=0.5, vertical_threshold=0.5)
        ),
    )


class JudgeParityTests(unittest.TestCase):
    def test_selection_excludes_current_and_previous_rows(self):
        move = Judge()
        fighter = _Fighter()
        weights = (1, 0, 1, 0, 0, 0, 0, 0, 0)

        self.assertTrue(move.input_pressed(fighter, _context(weights, 0)))
        self.assertEqual(fighter.action, move.ground_1)

        fighter.action = Action.WAIT
        # Row one is the current result, and row three is the only remaining
        # weighted row. A stale implementation would select row one again.
        self.assertTrue(move.input_pressed(fighter, _context(weights, 0)))
        self.assertEqual(fighter.action, move.ground_3)


if __name__ == "__main__":
    unittest.main()
