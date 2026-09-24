"""Focused source contract for Mario's cape reflect command window."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action
from fighters.mario import Cape, Fireball


class MarioSpecialAuditTests(unittest.TestCase):
    def test_fireball_entry_clears_source_throw_flags(self):
        fighter = SimpleNamespace(
            action=Action.WAIT,
            action_state=SimpleNamespace(command=(4, 5, 6, 7)),
            throw_flags=9,
        )
        Fireball().enter(fighter, SimpleNamespace())
        self.assertEqual(fighter.action_state.command, (0, 5, 6, 7))
        self.assertEqual(fighter.throw_flags, 0)

    def test_entry_clears_fighter_reflect_latch(self):
        fighter = SimpleNamespace(
            action=Action.WAIT,
            action_state=SimpleNamespace(command=(4, 5, 6, 7)),
            flags=SimpleNamespace(reflecting=True),
        )
        Cape().enter(fighter, SimpleNamespace())
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 7))
        self.assertFalse(fighter.flags.reflecting)

    def test_command_one_opens_and_closes_reflect_latch(self):
        move = Cape()
        fighter = SimpleNamespace(
            action=move.ground,
            action_state=SimpleNamespace(command=(0, 1, 0, 0)),
            flags=SimpleNamespace(reflecting=False),
        )
        move.reflect_command(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))
        self.assertTrue(fighter.flags.reflecting)
        move.reflect_command(fighter, SimpleNamespace(event=SimpleNamespace(value=0)))
        self.assertFalse(fighter.flags.reflecting)


if __name__ == "__main__":
    unittest.main()
