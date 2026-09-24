"""Source audit for Luigi's aerial Cyclone command timing."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighters.luigi import Cyclone, Fireball


class LuigiSourceAuditTests(unittest.TestCase):
    def test_fireball_entry_clears_command_slot_zero_before_spawn_callback(self):
        """ftLg_SpecialN_Enter clears cmd_vars[0] and throw_flags."""
        move = Fireball()
        fighter = SimpleNamespace(
            action_state=SimpleNamespace(command=(9, 2, 3, 4)),
            throw_flags=7,
        )

        move.enter(fighter, SimpleNamespace())

        self.assertEqual(fighter.action_state.command, (0, 2, 3, 4))
        self.assertEqual(fighter.throw_flags, 0)

    def test_cyclone_charge_command_is_consumed_before_animation_end(self):
        move = Cyclone()
        fighter = SimpleNamespace(
            action=move.air,
            action_state=SimpleNamespace(command=(0, 1, 0, 0)),
        )

        move.charge_command(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))

        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))
        self.assertTrue(fighter.action_state.cyclone_charge)

    def test_cyclone_charge_command_ignores_zero_cue(self):
        move = Cyclone()
        fighter = SimpleNamespace(
            action=move.air,
            action_state=SimpleNamespace(command=(0, 0, 0, 0)),
        )

        move.charge_command(fighter, SimpleNamespace(event=SimpleNamespace(value=0)))

        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))
        self.assertFalse(hasattr(fighter.action_state, "cyclone_charge"))


if __name__ == "__main__":
    unittest.main()
