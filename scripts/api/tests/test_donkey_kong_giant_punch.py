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
        self.fall_special = []

    def has_complete_animation(self, state):
        return state in (369, 373, 374, 378)

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def enter_fall_special(self, **kwargs):
        self.fall_special.append(kwargs)


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

    def test_source_cap_selects_full_entry_from_persisted_arm_swings(self):
        fighter = _Fighter()
        fighter.action_state = self.module.DonkeyKongActionState(arm_swings=4)
        context = self._context()
        context.resource = lambda path: {"max_arm_swings": 4}
        self.assertTrue(self.move.input_pressed(fighter, context))
        self.assertEqual(fighter.action, self.move.ground_full)
        self.assertEqual(fighter.action_state.release_swings, 4)
        self.assertEqual(fighter.action_state.arm_swings, 0)

    def test_release_copies_source_arm_swing_count_and_clears_counter(self):
        fighter = _Fighter()
        fighter.action = self.move.ground_loop
        fighter.action_state = self.module.DonkeyKongActionState(arm_swings=3)
        self.assertTrue(self.move.input_pressed(fighter, self._context()))
        self.assertEqual(fighter.action, self.move.ground_punch)
        self.assertEqual(fighter.action_state.release_swings, 3)
        self.assertEqual(fighter.action_state.arm_swings, 0)

    def test_air_release_uses_neutral_landing_lag_for_fall_special(self):
        fighter = _Fighter()
        fighter.action = self.move.air_punch
        context = SimpleNamespace(
            resource=lambda path: {"special_n_landing_lag": 7.0},
        )

        self.move.finish_air_release(fighter, context)

        self.assertEqual(fighter.fall_special, [{"mobility": 1, "landing_lag": 7.0}])
        self.assertEqual(fighter.changes, [])

    def test_air_release_with_zero_landing_lag_falls_normally(self):
        fighter = _Fighter()
        fighter.action = self.move.air_full
        context = SimpleNamespace(
            resource=lambda path: {"specialn_landing_lag": 0.0},
        )

        self.move.finish_air_release(fighter, context)

        self.assertEqual(fighter.changes, [(Action.FALL, {})])
        self.assertEqual(fighter.fall_special, [])

    def test_grounded_punch_entry_clears_vertical_velocity_but_air_entry_preserves_it(self):
        grounded = _Fighter()
        grounded.velocity = (2.0, 3.0)
        self.assertTrue(self.move.input_pressed(grounded, self._context()))
        self.move.enter(grounded, self._context())
        self.assertEqual(grounded.velocity, (2.0, 0.0))

        aerial = _Fighter()
        aerial.velocity = (2.0, 3.0)
        self.assertTrue(self.move.input_pressed(
            aerial, self._context(ground_open=False, air_open=True)
        ))
        self.move.enter(aerial, self._context(ground_open=False, air_open=True))
        self.assertEqual(aerial.velocity, (2.0, 3.0))


if __name__ == "__main__":
    unittest.main()
