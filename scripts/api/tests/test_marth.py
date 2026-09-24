"""Focused contracts for Marth's ftEmblem special declaration."""

import importlib.util
import sys
import unittest
from types import SimpleNamespace
from pathlib import Path


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))

from fighter import Button


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

    def test_dolphin_slash_animation_end_enters_source_fall_special(self):
        move = self.marth.specials.up
        calls = []
        fighter = SimpleNamespace(
            action=move.ground,
            enter_fall_special=lambda **kwargs: calls.append(kwargs),
        )
        attributes = SimpleNamespace(
            specialhi_freefall_air_spd_mul=0.8,
            specialhi_landing_lag=12.0,
        )
        context = SimpleNamespace(
            resource=lambda path: SimpleNamespace(attributes=attributes)
        )

        self.assertTrue(move.enter_fall_special(fighter, context))
        self.assertEqual(
            calls,
            [{"mobility": 0.8, "landing_lag": 12.0}],
        )

    def test_dancing_blade_arms_then_consumes_source_command_variables(self):
        move = self.marth.specials.side

        class Input:
            stick = (0.0, 0.0)

            def __init__(self, buttons):
                self.buttons = set(buttons)

            def just_pressed(self, button):
                return button in self.buttons

        fighter = SimpleNamespace(
            action=move.ground_start,
            action_state=SimpleNamespace(command=(0, 0, 0, 0)),
            changes=[],
        )
        fighter.change_action = lambda action: (fighter.changes.append(action), setattr(fighter, "action", action))
        context = SimpleNamespace(input=Input((Button.A, Button.B)))

        self.assertTrue(move.choose_phase(fighter, context))
        self.assertEqual(fighter.action_state.command, (0, 1, 0, 0))
        self.assertEqual(fighter.changes, [])

        fighter.action_state.command = (1, 0, 0, 0)
        Input.stick = (0.0, 0.8)
        self.assertTrue(move.choose_phase(fighter, context))
        self.assertIs(fighter.action, move.ground_2_up)
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))

    def test_dancing_blade_rejects_single_button_and_exports_atomic_chord(self):
        move = self.marth.specials.side

        class Input:
            stick = (0.0, 0.0)

            def __init__(self, buttons):
                self.buttons = set(buttons)

            def just_pressed(self, button):
                return button in self.buttons

        for buttons in ((Button.A,), (Button.B,)):
            fighter = SimpleNamespace(
                action=move.ground_start,
                action_state=SimpleNamespace(command=(0, 0, 0, 0)),
            )
            context = SimpleNamespace(input=Input(buttons))
            self.assertFalse(move.input_pressed(fighter, context))
            self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))

        binding = next(
            event for event in move.events()
            if event.hook.value == "input_pressed"
        )
        self.assertEqual(binding.buttons, 0x300)
        self.assertTrue(binding.buttons_all)

    def test_dancing_blade_accepts_native_button_mask_fallback(self):
        move = self.marth.specials.side
        fighter = SimpleNamespace(
            action=move.ground_start,
            action_state=SimpleNamespace(command=(0, 0, 0, 0)),
        )
        context = SimpleNamespace(
            input=SimpleNamespace(pressed_buttons=0x300, stick=(0.0, 0.0))
        )
        self.assertTrue(move.choose_phase(fighter, context))
        self.assertEqual(fighter.action_state.command, (0, 1, 0, 0))

if __name__ == "__main__":
    unittest.main()
