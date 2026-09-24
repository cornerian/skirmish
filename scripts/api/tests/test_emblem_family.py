"""Focused checks for the shared Marth/Roy ftEmblem declarations."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from types import SimpleNamespace
from pathlib import Path


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
FIGHTERS = ROOT / "scripts" / "fighters"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))

from fighter import Action, Button, export_definition, resolve_identity


def _load(name: str):
    spec = importlib.util.spec_from_file_location(name, FIGHTERS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class EmblemFamilyTests(unittest.TestCase):
    class _Input:
        def __init__(self, stick=(0.0, 0.0), buttons=(Button.A, Button.B)):
            self.stick = stick
            self.buttons = set(buttons)

        def just_pressed(self, button):
            return button in self.buttons

    class _Fighter:
        def __init__(self, action):
            self.action = action
            self.action_state = SimpleNamespace(command=(0, 0, 0, 0))
            self.changes = []

        def change_action(self, action, **kwargs):
            self.changes.append((action, kwargs))
            self.action = action

    def test_both_rosters_share_native_motion_state_layout(self):
        marth = _load("marth")
        roy = _load("roy")
        for fighter in (marth.Marth, roy.Roy):
            neutral = fighter.specials.neutral
            side = fighter.specials.side
            up = fighter.specials.up
            down = fighter.specials.down
            self.assertEqual(neutral.ground_start.action.reference, "Source.341")
            self.assertEqual(neutral.air_end1.action.reference, "Source.348")
            self.assertEqual(side.ground_4_down.action.reference, "Source.357")
            self.assertEqual(side.air_4_down.action.reference, "Source.366")
            self.assertEqual(up.ground.action.reference, "Source.367")
            self.assertEqual(down.air_hit.action.reference, "Source.372")

    def test_side_family_does_not_invent_dancing_blade_phase_transitions(self):
        marth = _load("marth")
        events = marth.Marth.specials.side.events()
        self.assertFalse(any(event.hook.value == "animation_ended" for event in events))
        self.assertEqual(marth.Marth.specials.side.resource, "side")
        surface = [event for event in events if event.hook.value == "ground_air_changed"]
        self.assertEqual(len(surface), 1)
        self.assertEqual(len(surface[0].actions), 18)

    def test_dancing_blade_input_route_requires_atomic_a_and_b_mask(self):
        marth = _load("marth")
        event = next(
            event for event in marth.Marth.specials.side.events()
            if event.hook.value == "input_pressed"
        )
        self.assertEqual(event.buttons, 0x300)
        self.assertTrue(event.buttons_all)

    def test_dancing_blade_ab_selects_source_phase_tree(self):
        marth = _load("marth")
        move = marth.Marth.specials.side
        fighter = self._Fighter(move.ground_start)
        ctx = SimpleNamespace(input=self._Input(stick=(0.0, 0.8)))

        # ftMars first arms cmd_vars[1], then consumes A+B after cmd_vars[0].
        self.assertTrue(move.choose_phase(fighter, ctx))
        self.assertEqual(fighter.action_state.command[1], 1)
        # The first callback arms cmd_vars[1]; the animation later sets
        # cmd_vars[0], clearing cmd_vars[1] before the phase chooser runs.
        fighter.action_state.command = (1, 0, 0, 0)
        self.assertTrue(move.choose_phase(fighter, ctx))
        self.assertEqual(fighter.action, move.ground_2_up)

        fighter = self._Fighter(move.ground_3_up)
        fighter.action_state.command = (1, 0, 0, 0)
        neutral = SimpleNamespace(input=self._Input(stick=(0.0, 0.0)))
        self.assertTrue(move.choose_phase(fighter, neutral))
        self.assertEqual(fighter.action, move.ground_4_neutral)

    def test_counter_command_one_enters_shared_hit_phase(self):
        for module_name, fighter_name in (("marth", "Marth"), ("roy", "Roy")):
            module = _load(module_name)
            move = getattr(module, fighter_name).specials.down
            ctx = SimpleNamespace(event=SimpleNamespace(value=1))
            for start, hit in ((move.ground, move.ground_hit), (move.air, move.air_hit)):
                fighter = self._Fighter(start)
                self.assertTrue(move.command_changed(fighter, ctx))
                self.assertEqual(fighter.action, hit)
                self.assertEqual(fighter.changes, [(hit, {})])

            # The native callback only reacts to the animation command's
            # value one, and only while the entry phase is active.
            fighter = self._Fighter(move.ground)
            ctx.event.value = 0
            self.assertFalse(move.command_changed(fighter, ctx))
            self.assertEqual(fighter.action, move.ground)
            fighter = self._Fighter(move.ground_hit)
            ctx.event.value = 1
            self.assertFalse(move.command_changed(fighter, ctx))
            self.assertEqual(fighter.action, move.ground_hit)

    def test_neutral_command_zero_enters_fully_charged_end_phase(self):
        for module_name, fighter_name in (("marth", "Marth"), ("roy", "Roy")):
            module = _load(module_name)
            move = getattr(module, fighter_name).specials.neutral
            ctx = SimpleNamespace(event=SimpleNamespace(value=1))
            for start, end in ((move.ground_loop, move.ground_end1),
                               (move.air_loop, move.air_end1)):
                fighter = self._Fighter(start)
                self.assertTrue(move.fully_charged(fighter, ctx))
                self.assertEqual(fighter.action, end)
                self.assertEqual(fighter.changes, [(end, {})])

            fighter = self._Fighter(move.ground_loop)
            ctx.event.value = 0
            self.assertFalse(move.fully_charged(fighter, ctx))
            self.assertEqual(fighter.action, move.ground_loop)

    def test_neutral_and_counter_have_only_source_supported_terminal_rules(self):
        marth = _load("marth")
        neutral_end = {rule.source for rule in marth.Marth.specials.neutral.__transition_rules__ if rule.event == "on_end"}
        down_end = {rule.source for rule in marth.Marth.specials.down.__transition_rules__ if rule.event == "on_end"}
        self.assertEqual(neutral_end, {"Source.341", "Source.345", "Source.347", "Source.348", "Source.343", "Source.344"})
        self.assertEqual(down_end, {"Source.369", "Source.370", "Source.371", "Source.372"})

    def test_marth_and_roy_exports_are_source_equivalent(self):
        marth = _load("marth")
        roy = _load("roy")
        resolve_identity(marth.Marth)
        resolve_identity(roy.Roy)

        def normalize(value):
            if isinstance(value, dict):
                return {
                    key: normalize(item)
                    for key, item in value.items()
                    if key != "buttons_all"
                }
            if isinstance(value, list):
                return [normalize(item) for item in value]
            if isinstance(value, str) and "Source." in value and ":" in value:
                prefix, state = value.split("Source.", 1)
                return prefix + "Source:" + state.split(":", 1)[1]
            return value

        marth_export = export_definition(marth.Marth).as_dict()
        roy_export = export_definition(roy.Roy).as_dict()
        for root in ("neutral", "side", "up", "down"):
            marth_id = marth_export["movesets"]["specials"][root]
            roy_id = roy_export["movesets"]["specials"][root]
            marth_move = next(item for item in marth_export["behaviors"] if item["id"] == marth_id)
            roy_move = next(item for item in roy_export["behaviors"] if item["id"] == roy_id)
            self.assertEqual(normalize(marth_move), normalize(roy_move), root)


if __name__ == "__main__":
    unittest.main()
