"""Roy's ftMars/ftEmblem source-state declarations."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
FIGHTERS = ROOT / "scripts" / "fighters"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))


def _load_roy():
    spec = importlib.util.spec_from_file_location("roy", FIGHTERS / "roy.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class RoyTests(unittest.TestCase):
    class _Input:
        def __init__(self, buttons, stick=(0.0, 0.8)):
            self.buttons = set(buttons)
            self.stick = stick

        def just_pressed(self, button):
            return button in self.buttons

    class _Fighter:
        def __init__(self, action):
            self.action = action
            self.action_state = SimpleNamespace(command=(1, 0, 0, 0))
            self.changes = []

        def change_action(self, action, **kwargs):
            self.changes.append((action, kwargs))
            self.action = action

    def test_motion_state_table_matches_ftmars(self):
        roy = _load_roy()
        moves = (
            (roy.FlareBlade, (341, 342, 343, 344, 345, 346, 347, 348)),
            (roy.DoubleEdgeDance, tuple(range(349, 367))),
            (roy.Blazer, (367, 368)),
            (roy.Counter, (369, 370, 371, 372)),
        )
        for move, states in moves:
            actual = [
                int(phase.action.reference.removeprefix("Source."))
                for phase in move.__dict__.values()
                if hasattr(phase, "action")
                and phase.action.reference.startswith("Source.")
            ]
            self.assertEqual(actual, list(states), move.__name__)

    def test_flare_blade_loop_is_animation_looping_and_release_exits(self):
        roy = _load_roy()
        move = roy.Roy.specials.neutral
        self.assertTrue(move.ground_loop.animation_loop)
        self.assertTrue(move.air_loop.animation_loop)
        release_targets = {
            rule.source_name: rule.transition.target.action.reference
            for rule in move.__transition_rules__
            if rule.event == "on_end"
            and rule.source_name in {"Source.341", "Source.345"}
        }
        self.assertEqual(release_targets["Source.341"], "Source.342")
        self.assertEqual(release_targets["Source.345"], "Source.346")

    def test_counter_has_ground_and_air_terminal_exits(self):
        roy = _load_roy()
        move = roy.Roy.specials.down
        terminal = {
            rule.source_name: rule.transition.target
            for rule in move.__transition_rules__
            if rule.event == "on_end"
        }
        self.assertEqual(terminal["Source.369"].name, "WAIT")
        self.assertEqual(terminal["Source.370"].name, "WAIT")
        self.assertEqual(terminal["Source.371"].name, "FALL")
        self.assertEqual(terminal["Source.372"].name, "FALL")

    def test_dancing_blade_accepts_either_button_for_phase_choice(self):
        roy = _load_roy()
        move = roy.Roy.specials.side
        for ctx in (
            SimpleNamespace(input=self._Input({roy.Button.A})),
            SimpleNamespace(input=self._Input({roy.Button.B})),
        ):
            fighter = self._Fighter(move.ground_start)
            fighter.action_state.command = (0, 0, 0, 0)
            self.assertTrue(move.choose_phase(fighter, ctx))
            self.assertEqual(fighter.action_state.command[1], 1)

        fighter = self._Fighter(move.ground_start)
        none = SimpleNamespace(input=self._Input(set()))
        fighter.action_state.command = (0, 0, 0, 0)
        self.assertFalse(move.choose_phase(fighter, none))
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))

    def test_dancing_blade_terminal_phases_ignore_button_input(self):
        roy = _load_roy()
        move = roy.Roy.specials.side
        for action in (
            move.ground_4_up, move.ground_4_neutral, move.ground_4_down,
            move.air_4_up, move.air_4_neutral, move.air_4_down,
        ):
            fighter = self._Fighter(action)
            fighter.action_state.command = (0, 0, 0, 0)
            context = SimpleNamespace(
                input=SimpleNamespace(pressed_buttons=0x100, stick=(0.0, 0.0))
            )
            self.assertFalse(move.input_pressed(fighter, context))
            self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))

    def test_counter_command_callback_does_not_claim_contact(self):
        move = _load_roy().Roy.specials.down
        hooks = {event.hook.value for event in move.events()}
        self.assertIn("command_trace_changed", hooks)
        self.assertNotIn("before_hit", hooks)
        fighter = SimpleNamespace(action=move.ground)
        self.assertFalse(
            move.command_changed(
                fighter, SimpleNamespace(event=SimpleNamespace(value=1))
            )
        )

    def test_blazer_steering_requires_native_throw_projection(self):
        move = _load_roy().Roy.specials.up
        fighter = SimpleNamespace(
            action=move.air,
            lstick_angle=0.0,
            action_state=SimpleNamespace(command=(0, 0, 0, 0)),
        )
        attributes = SimpleNamespace(x34=0.3, x38=45.0, x30=0.5)
        context = SimpleNamespace(
            input=SimpleNamespace(stick=(0.8, 0.0)),
            resource=lambda _path: SimpleNamespace(attributes=attributes),
        )

        self.assertFalse(move.steer(fighter, context))
        self.assertEqual(fighter.lstick_angle, 0.0)

        fighter.throw_flags_b3 = 0
        self.assertTrue(move.steer(fighter, context))
        self.assertNotEqual(fighter.lstick_angle, 0.0)

        fighter.facing = -1.0
        fighter.throw_flags_b3 = 1
        self.assertTrue(move.steer(fighter, context))
        self.assertEqual(fighter.facing, 1.0)

    def test_blazer_facing_turn_uses_x30_independently_of_angle_gate(self):
        move = _load_roy().Roy.specials.up
        fighter = SimpleNamespace(
            action=move.air,
            facing=-1.0,
            throw_flags_b3=1,
            lstick_angle=0.0,
            action_state=SimpleNamespace(command=(1, 0, 0, 0)),
        )
        attributes = SimpleNamespace(x34=0.5, x38=45.0, x30=0.2)
        context = SimpleNamespace(
            input=SimpleNamespace(stick=(0.3, 0.0)),
            resource=lambda _path: SimpleNamespace(attributes=attributes),
        )

        # 0.3 is below the source angle threshold x34 but above facing x30;
        # command 0 suppresses only angle sampling, not the facing branch.
        self.assertTrue(move.steer(fighter, context))
        self.assertEqual(fighter.facing, 1.0)
        self.assertEqual(fighter.lstick_angle, 0.0)


if __name__ == "__main__":
    unittest.main()
