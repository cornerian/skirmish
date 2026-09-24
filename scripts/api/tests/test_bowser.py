"""Focused source contracts for Bowser's resource-backed special phases."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action, Button, export_definition
from fighters.bowser import Bowser


class _Input:
    stick = (0.0, 0.0)

    def just_pressed(self, button):
        return button is Button.B


class _Fighter:
    def __init__(self, action=None):
        self.action = action
        self.action_frame = 0
        self.grounded = True
        self.changes = []

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


def _context(*, resource=True, stick=(0.0, 0.0), ground=True):
    return SimpleNamespace(
        input=SimpleNamespace(stick=stick, just_pressed=lambda button: True),
        resource=lambda path: object() if resource else None,
        ground_open=ground,
        air_open=not ground,
        grounded=ground,
        rules=SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5,
            vertical_threshold=0.5,
        )),
    )


class BowserTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.definition = export_definition(Bowser).as_dict()

    def test_source_table_and_canonical_resource_roots(self):
        actions = self.definition["actions"]
        states = sorted(
            value["slippi_state"]
            for name, value in actions.items()
            if name.startswith("special.")
        )
        self.assertEqual(states, list(range(341, 364)))
        self.assertEqual(self.definition["external_ids"], [5])
        for root in ("neutral", "side", "up", "down"):
            move = getattr(Bowser.specials, root)
            self.assertEqual(move.resource, root)
            behavior_id = self.definition["movesets"]["specials"][root]
            behavior = next(item for item in self.definition["behaviors"] if item["id"] == behavior_id)
            self.assertEqual(behavior["resource"], root)

    def test_entries_are_directional_and_resource_gated(self):
        for root, stick in (
            ("neutral", (0.0, 0.0)),
            ("side", (1.0, 0.0)),
            ("up", (0.0, 1.0)),
            ("down", (0.0, -1.0)),
        ):
            move = getattr(Bowser.specials, root)
            fighter = _Fighter()
            self.assertTrue(move.input_pressed(fighter, _context(stick=stick)))
            self.assertEqual(fighter.action, move._ENTRY[0])
            self.assertFalse(move.input_pressed(
                _Fighter(), _context(resource=False, stick=stick)
            ))

        self.assertFalse(
            Bowser.specials.neutral.input_pressed(
                _Fighter(), _context(stick=(1.0, 0.0))
            )
        )

    def test_deterministic_terminal_and_surface_transitions(self):
        neutral = Bowser.specials.neutral
        fighter = _Fighter("Source.5:341")
        neutral._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.5:342")
        fighter = _Fighter("Source.5:344")
        neutral._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.5:341")
        fighter = _Fighter("Source.5:343")
        neutral._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, Action.WAIT)

        side = Bowser.specials.side
        fighter = _Fighter("Source.5:351")
        side._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, Action.WAIT)
        fighter = _Fighter("Source.5:347")
        side._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, Action.WAIT)
        fighter = _Fighter("Source.5:353")
        side._transition_animation_end(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, Action.FALL)
        fighter = _Fighter("Source.5:347")
        side.capture_contact(fighter, SimpleNamespace())
        self.assertEqual(fighter.action, "Source.5:348")
        fighter = _Fighter("Source.5:348")
        side._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.5:350")
        fighter = _Fighter("Source.5:354")
        side._transition_animation_end(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, "Source.5:356")
        fighter = _Fighter("Source.5:353")
        side._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.5:347")

        up = Bowser.specials.up
        fighter = _Fighter("Source.5:360")
        up._transition_animation_end(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, Action.FALL)
        fighter = _Fighter("Source.5:360")
        up._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.5:359")

        down = Bowser.specials.down
        fighter = _Fighter("Source.5:362")
        down._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.5:363")
        fighter = _Fighter("Source.5:363")
        down._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, Action.FALL)

    def test_item_and_capture_effects_are_not_fabricated(self):
        for name, action in self.definition["actions"].items():
            if name.startswith("special."):
                self.assertNotIn("article", action)
                self.assertNotIn("effect", action)


if __name__ == "__main__":
    unittest.main()
