"""Focused contract tests for Fox's special-input behavior."""

import importlib.util
import sys
import types
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))

from fighter import Action, Button


def _load_fox():
    if "fighters.fox" in sys.modules:
        return sys.modules["fighters.fox"].Fox
    shared = types.ModuleType("shared")
    spec = importlib.util.spec_from_file_location(
        "shared.common", ROOT / "scripts" / "fighters" / "common.py"
    )
    common = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules["shared"] = shared
    sys.modules["shared.common"] = common
    spec.loader.exec_module(common)
    shared.common = common
    from fighters.fox import Fox

    return Fox


class FoxBlasterTests(unittest.TestCase):
    def test_neutral_threshold_boundary_is_rejected_but_inner_stick_starts_blaster(self):
        fox = _load_fox()
        move = fox.specials.neutral

        def attempt(stick):
            fighter = SimpleNamespace(
                action=Action.WAIT,
                action_frame=7,
                action_state=SimpleNamespace(),
                ground_velocity=3.0,
                velocity=[2.0, 0.0],
            )
            fighter.change_action = lambda action: setattr(fighter, "action", action)
            context = SimpleNamespace(
                ground_open=True,
                air_open=False,
                input=SimpleNamespace(
                    stick=stick,
                    just_pressed=lambda button: button == Button.B,
                ),
                resource=lambda path: SimpleNamespace(neutral_thresholds=(0.5, 0.5)),
            )
            return move.press(fighter, context), fighter

        accepted, fighter = attempt((0.5 - 1e-6, 0.0))
        self.assertTrue(accepted)
        self.assertEqual(fighter.action, move.ground_start)

        rejected, fighter = attempt((0.5, 0.0))
        self.assertFalse(rejected)
        self.assertEqual(fighter.action, Action.WAIT)

    def test_rejected_neutral_input_does_not_start_blaster(self):
        fox = _load_fox()
        move = fox.specials.neutral
        fighter = SimpleNamespace(
            action=Action.WAIT,
            action_frame=7,
            action_state=SimpleNamespace(),
            ground_velocity=3.0,
            velocity=[2.0, 0.0],
        )
        context = SimpleNamespace(
            ground_open=True,
            air_open=False,
            input=SimpleNamespace(
                stick=(1.0, 0.0),
                just_pressed=lambda button: button == Button.B,
            ),
            resource=lambda path: SimpleNamespace(neutral_thresholds=(0.5, 0.5)),
        )

        self.assertFalse(move.press(fighter, context))
        self.assertEqual(fighter.action, Action.WAIT)
        self.assertEqual(fighter.action_frame, 7)


if __name__ == "__main__":
    unittest.main()
