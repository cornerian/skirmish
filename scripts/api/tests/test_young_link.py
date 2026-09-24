"""Focused source contracts for Young Link's fighter-side specials."""

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
from fighters.young_link import (
    YoungLink,
    YoungLinkDownSpecial,
    YoungLinkNeutralSpecial,
    YoungLinkSideSpecial,
    YoungLinkUpSpecial,
)


class _Input:
    def __init__(self, stick=(0.0, 0.0), pressed=True):
        self.stick = stick
        self.pressed = pressed

    def just_pressed(self, button):
        return self.pressed and button is Button.B


class _Fighter:
    def __init__(self, action=Action.WAIT, grounded=True):
        self.action = action
        self.action_frame = 0
        self.grounded = grounded
        self.changes = []

    def change_action(self, action, **kwargs):
        self.action = action
        self.changes.append((action, kwargs))


def _context(*, resource=True, stick=(0.0, 0.0), grounded=True, pressed=True):
    return SimpleNamespace(
        input=_Input(stick, pressed),
        resource=lambda path: object() if resource else None,
        ground_open=grounded,
        air_open=not grounded,
        grounded=grounded,
        rules=SimpleNamespace(
            specials=SimpleNamespace(
                side_stick_threshold=0.5, vertical_threshold=0.5
            )
        ),
    )


class YoungLinkTests(unittest.TestCase):
    def test_exports_c_link_identity_and_complete_ftlink_special_table(self):
        exported = export_definition(YoungLink).as_dict()
        self.assertEqual((exported["name"], exported["external_ids"]), ("young-link", [21]))
        states = sorted(
            value["slippi_state"]
            for name, value in exported["actions"].items()
            if name.startswith("special.")
        )
        self.assertEqual(states, list(range(344, 360)))
        self.assertTrue(exported["actions"]["special.neutral.ground_loop"]["animation_loop"])
        self.assertTrue(exported["actions"]["special.neutral.air_loop"]["animation_loop"])

    def test_source_move_classes_keep_article_boundary_and_resources(self):
        for root, move_type in (
            ("neutral", YoungLinkNeutralSpecial),
            ("side", YoungLinkSideSpecial),
            ("up", YoungLinkUpSpecial),
            ("down", YoungLinkDownSpecial),
        ):
            move = getattr(YoungLink.specials, root)
            self.assertIsInstance(move, move_type)
            self.assertEqual(move.resource, root)
            self.assertFalse(any("article" in event.callback for event in move.events()))

    def test_directional_entry_is_resource_gated_and_routes_ground_or_air(self):
        cases = (
            (YoungLinkNeutralSpecial(), (0.0, 0.0)),
            (YoungLinkSideSpecial(), (1.0, 0.0)),
            (YoungLinkUpSpecial(), (0.0, 1.0)),
            (YoungLinkDownSpecial(), (0.0, -1.0)),
        )
        for move, stick in cases:
            blocked = _Fighter()
            self.assertFalse(move.input_pressed(blocked, _context(resource=False, stick=stick)))
            self.assertFalse(blocked.changes)

            grounded = _Fighter()
            self.assertTrue(move.input_pressed(grounded, _context(stick=stick)))
            self.assertEqual(grounded.action, move.ground_start if hasattr(move, "ground_start") else move.ground)

            airborne = _Fighter(grounded=False)
            self.assertTrue(move.input_pressed(airborne, _context(stick=stick, grounded=False)))
            self.assertEqual(airborne.action, move.air_start if hasattr(move, "air_start") else move.air)

    def test_source_terminal_and_surface_transitions_match_ftlink(self):
        moves = (
            YoungLinkNeutralSpecial(),
            YoungLinkSideSpecial(),
            YoungLinkUpSpecial(),
            YoungLinkDownSpecial(),
        )
        for move in moves:
            ground = move.ground_start if hasattr(move, "ground_start") else move.ground
            air = move.air_start if hasattr(move, "air_start") else move.air
            grounded = _Fighter(ground)
            move._transition_animation_end(grounded, _context())
            self.assertNotEqual(grounded.action, ground)
            airborne = _Fighter(air)
            move._transition_animation_end(airborne, _context(grounded=False))
            self.assertNotEqual(airborne.action, air)

        move = YoungLinkNeutralSpecial()
        fighter = _Fighter(move.ground_loop)
        move.release(fighter, _context(pressed=False))
        self.assertEqual(fighter.action, move.ground_end)

        move = YoungLinkSideSpecial()
        fighter = _Fighter(move.air)
        move._transition_ground_air(fighter, _context(grounded=True))
        self.assertEqual(fighter.action, move.ground)

    def test_boomerang_empty_branch_and_native_release_forwarding(self):
        move = YoungLinkSideSpecial()
        fighter = _Fighter()
        fighter.used_boomerang = True
        self.assertTrue(move.input_pressed(fighter, _context(stick=(1.0, 0.0))))
        self.assertEqual(fighter.action, move.ground_empty)

        updates = []
        fighter.update_boomerang_trajectory = updates.append
        move.boomerang_release(
            fighter,
            SimpleNamespace(event=SimpleNamespace(value=True)),
        )
        self.assertEqual(len(updates), 1)

    def test_held_bomb_branch_is_optional_and_reports_airborne_state(self):
        move = YoungLinkDownSpecial()
        fighter = _Fighter(move.air, grounded=False)
        fighter.link_bomb_held = True
        branches = []
        fighter.reuse_held_bomb_special = lambda **kwargs: branches.append(kwargs)

        move.reuse_held_bomb(fighter, SimpleNamespace())
        self.assertEqual(branches, [{"airborne": True}])


if __name__ == "__main__":
    unittest.main()
