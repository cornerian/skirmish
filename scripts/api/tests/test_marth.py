"""Focused contracts for Marth's ftEmblem special declaration."""

import importlib.util
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))


def _load_marth():
    path = ROOT / "scripts" / "fighters" / "marth.py"
    spec = importlib.util.spec_from_file_location("marth_under_test", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class MarthSpecialTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.marth = _load_marth().Marth

    def test_specials_use_the_ftemblem_motion_state_ranges(self):
        neutral = self.marth.specials.neutral
        side = self.marth.specials.side
        up = self.marth.specials.up
        down = self.marth.specials.down

        self.assertEqual(neutral.ground_start.action.reference, "Source.341")
        self.assertEqual(neutral.air_end1.action.reference, "Source.348")
        self.assertEqual(side.ground_start.action.reference, "Source.349")
        self.assertEqual(side.air_4_down.action.reference, "Source.366")
        self.assertEqual(up.ground.action.reference, "Source.367")
        self.assertEqual(up.air.action.reference, "Source.368")
        self.assertEqual(down.ground.action.reference, "Source.369")
        self.assertEqual(down.air_hit.action.reference, "Source.372")
        self.assertTrue(neutral.ground_loop.animation_loop)
        self.assertTrue(neutral.air_loop.animation_loop)

    def test_source_terminal_rules_match_ftmars_animation_callbacks(self):
        from fighter.actions import Action

        specials = self.marth.specials
        neutral_end = {
            rule.source: rule.transition.target
            for rule in specials.neutral.__transition_rules__
            if rule.event == "on_end"
        }
        down_end = {
            rule.source: rule.transition.target
            for rule in specials.down.__transition_rules__
            if rule.event == "on_end"
        }
        self.assertIs(neutral_end["Source.341"], specials.neutral.ground_loop)
        self.assertEqual(neutral_end["Source.343"], Action.WAIT)
        self.assertEqual(neutral_end["Source.348"], Action.FALL)
        self.assertEqual(down_end["Source.369"], Action.WAIT)
        self.assertEqual(down_end["Source.372"], Action.FALL)

    def test_surface_transitions_preserve_state_and_frame(self):
        side = self.marth.specials.side
        rules = side.__transition_rules__
        ground = next(rule for rule in rules if rule.event == "on_ground" and rule.source == "Source.358")
        air = next(rule for rule in rules if rule.event == "on_air" and rule.source == "Source.349")
        self.assertTrue(ground.transition.preserve_state)
        self.assertTrue(ground.transition.keep_frame)
        self.assertTrue(air.transition.preserve_state)
        self.assertTrue(air.transition.keep_frame)

    def test_neutral_release_ends_the_charge_on_the_matching_surface(self):
        from types import SimpleNamespace

        move = self.marth.specials.neutral
        fighter = SimpleNamespace(action=move.ground_loop)
        fighter.change_action = lambda action: setattr(fighter, "action", action)
        context = SimpleNamespace()

        self.assertTrue(move.release(fighter, context))
        self.assertIs(fighter.action, move.ground_end0)


if __name__ == "__main__":
    unittest.main()
