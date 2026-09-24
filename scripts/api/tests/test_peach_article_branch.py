"""Focused regression for Peach's source command-3 end branch."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighters.peach import Peach  # noqa: E402


class _Fighter:
    def __init__(self, action):
        self.action = action
        self.changes = []
        self.peach_turnip_held = False
        self.throw_calls = []

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def throw_held_turnip_special(self, **kwargs):
        self.throw_calls.append(kwargs)


class PeachArticleBranchTests(unittest.TestCase):
    def test_air_jump_command_three_enters_end_phase(self):
        move = Peach.specials.side
        fighter = _Fighter(move.air_jump)

        move.jump_end(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))

        self.assertIs(fighter.action, move.air_end0)
        self.assertEqual(fighter.changes, [(move.air_end0, {})])

    def test_air_jump_command_three_ignores_cleared_cue(self):
        move = Peach.specials.side
        fighter = _Fighter(move.air_jump)

        move.jump_end(fighter, SimpleNamespace(event=SimpleNamespace(value=0)))

        self.assertEqual(fighter.changes, [])

    def test_command_three_is_exported_for_air_jump_only(self):
        side = next(
            action
            for action in Peach.specials.side.events()
            if action.callback == "jump_end"
        )

        self.assertEqual(side.command_index, 3)
        self.assertEqual(side.actions, ("Source.360",))

    def test_down_entry_forwards_held_turnip_to_common_throw(self):
        move = Peach.specials.down
        fighter = _Fighter(move.air)
        fighter.peach_turnip_held = True

        move.throw_held_turnip(fighter, SimpleNamespace())

        self.assertEqual(fighter.throw_calls, [{"airborne": True}])

    def test_down_entry_without_turnip_keeps_native_article_gate(self):
        move = Peach.specials.down
        fighter = _Fighter(move.ground)

        move.throw_held_turnip(fighter, SimpleNamespace())

        self.assertEqual(fighter.throw_calls, [])


if __name__ == "__main__":
    unittest.main()
