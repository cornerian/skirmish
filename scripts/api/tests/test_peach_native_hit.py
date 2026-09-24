"""Source-backed regression for Peach's Toad contact transition."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighters.peach import Peach  # noqa: E402


class _Fighter:
    def __init__(self, action):
        self.action = action
        self.action_frame = 0
        self.changes = []

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


class PeachNativeHitTests(unittest.TestCase):
    def test_toad_contact_restarts_ground_hit_at_source_frame(self):
        move = Peach.specials.neutral
        fighter = _Fighter(move.ground)

        move.hit_contact(fighter, object())

        self.assertEqual(fighter.changes, [(move.ground_hit, {})])
        self.assertEqual(fighter.action_frame, 9)

    def test_toad_contact_restarts_air_hit_at_source_frame(self):
        move = Peach.specials.neutral
        fighter = _Fighter(move.air)

        move.hit_contact(fighter, object())

        self.assertEqual(fighter.changes, [(move.air_hit, {})])
        self.assertEqual(fighter.action_frame, 9)

    def test_unrelated_phase_does_not_enter_toad_hit(self):
        move = Peach.specials.neutral
        fighter = _Fighter(move.ground_hit)

        move.hit_contact(fighter, object())

        self.assertEqual(fighter.changes, [])


if __name__ == "__main__":
    unittest.main()
