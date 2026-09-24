"""Regression coverage for the source LR cancel boundary in Giant Punch."""

import importlib.util
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))

from fighter import Button


def _module():
    spec = importlib.util.spec_from_file_location(
        "donkey_kong_cancel_boundary_test",
        ROOT / "scripts" / "fighters" / "donkey_kong.py",
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class _Input:
    def __init__(self, *buttons):
        self.buttons = set(buttons)

    def just_pressed(self, button):
        return button in self.buttons


class _Fighter:
    def __init__(self, action, action_frame):
        self.action = action
        self.action_frame = action_frame
        self.action_state = None
        self.changes = []

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


class GiantPunchCancelBoundaryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.module = _module()
        cls.move = cls.module.GiantPunch()

    def test_lr_latches_until_loop_frame_zero(self):
        fighter = _Fighter(self.move.ground_loop, action_frame=5)
        context = SimpleNamespace(input=_Input(Button.L))

        self.assertTrue(self.move.input_pressed(fighter, context))
        self.assertEqual(fighter.action, self.move.ground_loop)
        self.assertTrue(fighter.action_state.cancel_pending)

        fighter.action_frame = 0
        self.assertTrue(self.move.input_pressed(fighter, context))
        self.assertEqual(fighter.action, self.move.ground_cancel)

    def test_b_release_wins_over_a_pending_lr_cancel(self):
        fighter = _Fighter(self.move.ground_loop, action_frame=5)
        fighter.action_state = self.module.DonkeyKongActionState(cancel_pending=True)
        context = SimpleNamespace(input=_Input(Button.B, Button.L))

        self.assertTrue(self.move.input_pressed(fighter, context))
        self.assertEqual(fighter.action, self.move.ground_punch)


if __name__ == "__main__":
    unittest.main()
