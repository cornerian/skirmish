"""Focused source contracts for Yoshi's four special state families."""

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
from fighters.yoshi import Yoshi


class _Input:
    def __init__(self, stick=(0.0, 0.0), pressed=True):
        self.stick = stick
        self.pressed = pressed

    def just_pressed(self, button):
        return button is Button.B and self.pressed


class _Fighter:
    def __init__(self, action=None):
        self.action = action
        self.action_frame = 0
        self.facing = 1.0
        self.changes = []

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


def _context(*, stick=(0.0, 0.0), ground=True, resource=True, pressed=True):
    return SimpleNamespace(
        input=_Input(stick, pressed),
        resource=lambda path: object() if resource else None,
        ground_open=ground,
        air_open=not ground,
        grounded=ground,
        rules=SimpleNamespace(
            specials=SimpleNamespace(side_stick_threshold=0.5, vertical_threshold=0.5)
        ),
    )


class YoshiSpecialTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.definition = export_definition(Yoshi).as_dict()

    def test_native_state_spans_are_declared_without_article_strings(self):
        actions = self.definition["actions"]
        self.assertEqual(
            sorted({
                value["slippi_state"]
                for name, value in actions.items()
                if name.startswith("special.")
            }),
            list(range(346, 369)),
        )
        for root in ("neutral", "side", "up", "down"):
            move = getattr(Yoshi.specials, root)
            self.assertEqual(move.resource, root)
            behavior = next(
                item
                for item in self.definition["behaviors"]
                if item["id"] == self.definition["movesets"]["specials"][root]
            )
            self.assertEqual(behavior["resource"], root)
            self.assertFalse(any("article" in cb["callback"] for cb in behavior["callbacks"]))

    def test_each_entry_is_resource_and_direction_gated(self):
        for move, stick in (
            (Yoshi.specials.neutral, (0.0, 0.0)),
            (Yoshi.specials.side, (1.0, 0.0)),
            (Yoshi.specials.up, (0.0, 1.0)),
            (Yoshi.specials.down, (0.0, -1.0)),
        ):
            fighter = _Fighter()
            self.assertFalse(move.input_pressed(fighter, _context(stick=stick, resource=False)))
            self.assertTrue(move.input_pressed(fighter, _context(stick=stick)))
            self.assertIs(fighter.action, move.ground)

        # ftYs_SpecialS_Enter and ftYs_SpecialAirS_Enter both enter native
        # motion 360; 356 is the post-landing continuation selected by the
        # aerial start collision callback.
        self.assertEqual(Yoshi.specials.side.ground, "Source.17:360")
        self.assertEqual(Yoshi.specials.side.air, "Source.17:360")

    def test_directional_specials_reject_wrong_axis(self):
        for move, stick in (
            (Yoshi.specials.side, (0.0, 1.0)),
            (Yoshi.specials.up, (0.0, -1.0)),
            (Yoshi.specials.down, (0.0, 1.0)),
        ):
            self.assertFalse(move.input_pressed(_Fighter(), _context(stick=stick)))
        self.assertFalse(
            Yoshi.specials.neutral.input_pressed(_Fighter(), _context(stick=(1.0, 0.0)))
        )

    def test_terminal_and_surface_transitions_use_native_phases(self):
        move = Yoshi.specials.neutral
        fighter = _Fighter("Source.17:351")
        move._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.17:346")
        fighter = _Fighter("Source.17:355")
        move._transition_animation_end(fighter, _context(ground=False))
        self.assertEqual(fighter.action, Action.FALL)

        move = Yoshi.specials.side
        fighter = _Fighter("Source.17:356")
        move._transition_animation_end(fighter, _context())
        self.assertEqual(fighter.action, "Source.17:357")
        fighter = _Fighter(move.air)
        move._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.17:356")
        fighter = _Fighter(move.air_landing)
        move._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        # ftYs_SpecialAirSLanding_Coll is guarded by a non-ground collision
        # result, so grounded contact leaves phase 363 active.
        self.assertEqual(fighter.action, move.air_landing)
        # ftYs_SpecialAirSEnd_Coll (motion 359) enters motion 363 only on a
        # ground collision.  ftYs_SpecialAirSLanding_Coll intentionally
        # leaves motion 363 active when it reports ground contact.
        fighter = _Fighter(move.ground_end)
        move._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, move.air_landing)
        fighter = _Fighter(move.air_landing)
        move._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, move.air_landing)
        fighter = _Fighter(move.ground_end)
        move._transition_ground_air(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, move.ground_end)

        move = Yoshi.specials.down
        fighter = _Fighter("Source.17:368")
        move._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.17:367")

        # The aerial pound's animation end arms descent in ftYoshi; it does
        # not enter FALL before the collision callback resolves the move.
        fighter = _Fighter("Source.17:368")
        move._transition_animation_end(fighter, _context(ground=False))
        self.assertEqual(fighter.action, "Source.17:368")

    def test_egg_lay_capture_phases_wait_for_native_article_commands(self):
        move = Yoshi.specials.neutral
        # N1_1/N1_0 and their aerial variants only leave through the source
        # command-variable/article or victim branches.  No generic animation
        # end transition may turn an unarmed phase into Wait/Fall.
        for phase in (
            move.ground_tongue,
            move.ground_swallow,
            move.air_tongue,
            move.air_swallow,
        ):
            fighter = _Fighter(phase)
            move._transition_animation_end(
                fighter,
                _context(
                    ground=phase in (move.ground_tongue, move.ground_swallow)
                ),
            )
            self.assertIs(fighter.action, phase)

    def test_egg_roll_release_requires_fresh_b_edge(self):
        move = Yoshi.specials.side
        fighter = _Fighter(move.ground_loop)
        self.assertFalse(
            move.input_pressed(
                fighter,
                _context(stick=(1.0, 0.0), pressed=False),
            )
        )
        self.assertIs(fighter.action, move.ground_loop)

        fighter = _Fighter(move.ground_loop)
        self.assertTrue(move.input_pressed(fighter, _context(stick=(1.0, 0.0))))
        self.assertIs(fighter.action, move.ground_end)

    def test_egg_roll_entry_faces_with_native_stick_direction(self):
        move = Yoshi.specials.side
        for stick, expected in (((1.0, 0.0), 1.0), ((-1.0, 0.0), -1.0)):
            fighter = _Fighter(move.ground)
            move.set_entry_facing(fighter, _context(stick=stick))
            self.assertEqual(fighter.facing, expected)

    def test_neutral_preserves_each_source_phase_across_surface_changes(self):
        move = Yoshi.specials.neutral
        for ground, air in (
            (move.ground, move.air),
            (move.ground_tongue, move.air_tongue),
            (move.ground_egg, move.air_egg),
            (move.ground_swallow, move.air_swallow),
            (move.ground_end, move.air_end),
        ):
            airborne = _Fighter(air)
            move._transition_ground_air(airborne, SimpleNamespace(grounded=True))
            self.assertEqual(airborne.action, ground)
            grounded = _Fighter(ground)
            move._transition_ground_air(grounded, SimpleNamespace(grounded=False))
            self.assertEqual(grounded.action, air)

    def test_egg_roll_b_press_releases_ground_and_air_loops(self):
        move = Yoshi.specials.side
        grounded = _Fighter(move.ground_loop)
        self.assertTrue(move.input_pressed(grounded, _context(stick=(1.0, 0.0))))
        self.assertEqual(grounded.action, move.ground_end)
        self.assertEqual(
            grounded.changes,
            [(move.ground_end, {"preserve_state": True, "keep_frame": True})],
        )

        airborne = _Fighter(move.air_loop)
        self.assertTrue(
            move.input_pressed(
                airborne,
                _context(stick=(1.0, 0.0), ground=False),
            )
        )
        self.assertEqual(airborne.action, move.air_landing)


if __name__ == "__main__":
    unittest.main()
