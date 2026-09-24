import importlib.util
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))

from fighter import Action, export_definition


def _module():
    spec = importlib.util.spec_from_file_location(
        "donkey_kong_giant_punch_test", ROOT / "scripts" / "fighters" / "donkey_kong.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.DonkeyKong.name = "donkey-kong"
    module.DonkeyKong.external_ids = (1,)
    return module


class _Input:
    def just_pressed(self, button):
        return True


class _Fighter:
    grounded = True
    facing = 1.0
    action_frame = 0

    def __init__(self):
        self.action = None
        self.changes = []

    def has_complete_animation(self, state):
        return state in (369, 374)

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


class GiantPunchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.module = _module()
        cls.move = cls.module.GiantPunch()

    def _context(self, *, ground_open=True, air_open=False):
        return SimpleNamespace(
            input=_Input(), ground_open=ground_open, air_open=air_open,
        )

    def test_source_phase_table_matches_ftdonkey(self):
        actions = export_definition(self.module.DonkeyKong).as_dict()["actions"]
        expected = {
            "ground_start": 369, "ground_loop": 370, "ground_cancel": 371,
            "ground_punch": 372, "ground_full": 373,
            "air_start": 374, "air_loop": 375, "air_cancel": 376,
            "air_punch": 377, "air_full": 378,
        }
        for name, state in expected.items():
            self.assertEqual(actions[f"special.neutral.{name}"]["slippi_state"], state)
        self.assertTrue(actions["special.neutral.ground_loop"]["animation_loop"])
        self.assertTrue(actions["special.neutral.air_loop"]["animation_loop"])

    def test_ground_charge_enters_loop_then_b_releases_punch(self):
        fighter = _Fighter()
        self.assertTrue(self.move.input_pressed(fighter, self._context()))
        self.assertEqual(fighter.action, self.move.ground_start)

        self.move._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, self.move.ground_loop)
        self.assertTrue(self.move.input_pressed(fighter, self._context()))
        self.assertEqual(fighter.action, self.move.ground_punch)

    def test_air_charge_l_or_r_cancels_and_punch_ends_to_fall(self):
        fighter = _Fighter()
        self.assertTrue(self.move.input_pressed(fighter, self._context(ground_open=False, air_open=True)))
        self.move._transition_animation_end(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, self.move.air_loop)
        self.assertTrue(self.move.cancel_charge(fighter, self._context(ground_open=False, air_open=True)))
        self.assertEqual(fighter.action, self.move.air_cancel)
        self.move._transition_animation_end(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, Action.FALL)


if __name__ == "__main__":
    unittest.main()
