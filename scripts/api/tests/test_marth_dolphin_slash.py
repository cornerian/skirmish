"""Source-backed Dolphin Slash steering contracts for Marth."""

import importlib.util
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))


def _load_marth():
    path = ROOT / "scripts" / "fighters" / "marth.py"
    spec = importlib.util.spec_from_file_location("marth_dolphin_under_test", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class DolphinSlashTests(unittest.TestCase):
    def test_horizontal_stick_selects_stronger_source_angle(self):
        move = _load_marth().Marth.specials.up
        fighter = SimpleNamespace(
            action=move.air,
            lstick_angle=0.0,
            action_state=SimpleNamespace(command=(0, 0, 0, 0)),
        )
        attrs = SimpleNamespace(x34=0.3, x38=45.0)
        context = SimpleNamespace(
            input=SimpleNamespace(stick=(0.8, 0.0)),
            resource=lambda _path: SimpleNamespace(attributes=attrs),
        )

        self.assertTrue(move.steer(fighter, context))
        self.assertAlmostEqual(fighter.lstick_angle, -0.560998688, places=6)

    def test_source_angle_is_monotonic_and_command_zero_gate_is_preserved(self):
        move = _load_marth().Marth.specials.up
        attrs = SimpleNamespace(specialhi_facing_threshold=0.3, specialhi_angle_limit=45.0)
        context = SimpleNamespace(
            input=SimpleNamespace(stick=(-0.8, 0.0)),
            resource=lambda _path: SimpleNamespace(attributes=attrs),
        )
        fighter = SimpleNamespace(
            action=move.ground,
            lstick_angle=0.9,
            action_state=SimpleNamespace(command=(0, 0, 0, 0)),
        )
        self.assertFalse(move.steer(fighter, context))
        self.assertEqual(fighter.lstick_angle, 0.9)

        fighter.action_state.command = (1, 0, 0, 0)
        self.assertFalse(move.steer(fighter, context))
        self.assertEqual(fighter.lstick_angle, 0.9)

    def test_source_angle_uses_strict_x34_threshold(self):
        move = _load_marth().Marth.specials.up
        fighter = SimpleNamespace(
            action=move.ground,
            lstick_angle=0.0,
            action_state=SimpleNamespace(command=(0, 0, 0, 0)),
        )
        attrs = SimpleNamespace(x30=0.1, x34=0.3, x38=45.0)
        context = SimpleNamespace(
            input=SimpleNamespace(stick=(0.3, 0.0)),
            resource=lambda _path: SimpleNamespace(attributes=attrs),
        )

        self.assertFalse(move.steer(fighter, context))
        context.input.stick = (0.31, 0.0)
        self.assertTrue(move.steer(fighter, context))


if __name__ == "__main__":
    unittest.main()
