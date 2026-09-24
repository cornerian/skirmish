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
        self.changes = []
        self.action_frame = 1

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


class PeachNativeHitTests(unittest.TestCase):
    def test_hit_phase_transition_does_not_fabricate_native_frame_offset(self):
        move = Peach.specials.neutral
        fighter = _Fighter(move.ground)

        move.hit_contact(fighter, object())

        self.assertEqual(fighter.changes, [(move.ground_hit, {})])
        self.assertEqual(fighter.action_frame, 1)

    def test_ordinary_hit_does_not_restart_air_hit_at_frame_nine(self):
        move = Peach.specials.neutral
        fighter = _Fighter(move.air)

        move.hit_contact(fighter, object())

        self.assertEqual(fighter.changes, [(move.air_hit, {})])
        self.assertEqual(fighter.action_frame, 1)

    def test_unrelated_phase_does_not_enter_toad_hit(self):
        move = Peach.specials.neutral
        fighter = _Fighter(move.ground_hit)

        move.hit_contact(fighter, object())

        self.assertEqual(fighter.changes, [])


if __name__ == "__main__":
    unittest.main()
