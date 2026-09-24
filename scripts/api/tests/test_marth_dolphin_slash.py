"""Host capability guard for Marth's Dolphin Slash IASA callback."""

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
    def test_steering_requires_native_throw_flags_projection(self):
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

        self.assertFalse(move.steer(fighter, context))
        self.assertEqual(fighter.lstick_angle, 0.0)

    def test_source_angle_uses_strict_x34_threshold(self):
        move = _load_marth().Marth.specials.up
        fighter = SimpleNamespace(
            action=move.ground,
            lstick_angle=0.0,
            throw_flags_b3=0,
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

    def test_throw_projection_turns_facing_at_source_x30_threshold(self):
        move = _load_marth().Marth.specials.up
        fighter = SimpleNamespace(
            action=move.air,
            facing=1.0,
            lstick_angle=0.0,
            throw_flags_b3=1,
            action_state=SimpleNamespace(command=(0, 0, 0, 0)),
        )
        attrs = SimpleNamespace(x30=0.2, x34=0.8, x38=45.0)
        context = SimpleNamespace(
            input=SimpleNamespace(stick=(-0.3, 0.0)),
            resource=lambda _path: SimpleNamespace(attributes=attrs),
        )

        self.assertTrue(move.steer(fighter, context))
        self.assertEqual(fighter.facing, -1.0)
        self.assertEqual(fighter.lstick_angle, 0.0)

    def test_facing_turn_is_independent_of_angle_command_gate(self):
        move = _load_marth().Marth.specials.up
        fighter = SimpleNamespace(
            action=move.air,
            facing=-1.0,
            lstick_angle=0.0,
            throw_flags_b3=1,
            action_state=SimpleNamespace(command=(1, 0, 0, 0)),
        )
        attrs = SimpleNamespace(x30=0.2, x34=0.5, x38=45.0)
        context = SimpleNamespace(
            input=SimpleNamespace(stick=(0.3, 0.0)),
            resource=lambda _path: SimpleNamespace(attributes=attrs),
        )

        # The source command gate suppresses only angle sampling.  x30 still
        # controls the independent ftCheckThrowB3 facing branch.
        self.assertTrue(move.steer(fighter, context))
        self.assertEqual(fighter.facing, 1.0)
        self.assertEqual(fighter.lstick_angle, 0.0)


if __name__ == "__main__":
    unittest.main()
