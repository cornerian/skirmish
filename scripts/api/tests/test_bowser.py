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
from fighters.bowser import Bowser, BowserActionState


class _Input:
    stick = (0.0, 0.0)

    def just_pressed(self, button):
        return button is Button.B


class _Fighter:
    def __init__(self, action=None):
        self.action = action
        self.action_frame = 0
        self.grounded = True
        self.facing = 1.0
        self.changes = []
        self.action_state = BowserActionState()

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


def _context(*, resource=True, stick=(0.0, 0.0), ground=True, held=None):
    input_state = SimpleNamespace(stick=stick, just_pressed=lambda button: True)
    if held is not None:
        input_state.held_buttons = held
    return SimpleNamespace(
        input=input_state,
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
        side.finish_hit(fighter, _context(ground=False, held=set()))
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

    def test_bowser_bomb_ground_end_enters_aerial_motion_at_frame_30(self):
        down = Bowser.specials.down
        fighter = _Fighter(down.ground)
        down._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, down.air)
        self.assertEqual(fighter.action_frame, 30)

    def test_bowser_bomb_aerial_end_waits_for_landing_collision(self):
        down = Bowser.specials.down
        fighter = _Fighter(down.air)
        down._transition_animation_end(fighter, SimpleNamespace(grounded=False))
        # ftKp_SpecialAirLw_Anim arms the collision callback at animation end;
        # it does not enter fall until the landing callback runs.
        self.assertEqual(fighter.action, down.air)
        self.assertEqual(fighter.changes, [])

    def test_item_and_capture_effects_are_not_fabricated(self):
        for name, action in self.definition["actions"].items():
            if name.startswith("special."):
                self.assertNotIn("article", action)
                self.assertNotIn("effect", action)

    def test_klaw_hit_and_wait_choose_native_forward_or_back_end(self):
        side = Bowser.specials.side
        fighter = _Fighter(side.ground_wait)
        self.assertTrue(side.choose_throw_direction(
            fighter, _context(stick=(1.0, 0.0))
        ))
        self.assertEqual(fighter.action, side.ground_end_forward)
        self.assertEqual(fighter.changes[-1][1], {
            "preserve_state": True, "keep_frame": True,
        })

        fighter = _Fighter(side.air_hit)
        fighter.facing = -1.0
        self.assertTrue(side.choose_throw_direction(
            fighter, _context(stick=(1.0, 0.0), ground=False)
        ))
        self.assertEqual(fighter.action, side.air_end_back)
        self.assertFalse(side.choose_throw_direction(
            _Fighter(side.ground_start), _context(stick=(1.0, 0.0))
        ))

    def test_klaw_wait_reenters_hold_only_while_b_is_held(self):
        side = Bowser.specials.side
        fighter = _Fighter(side.ground_wait)
        fighter.action_state.klaw_b_held = True
        self.assertTrue(side.hold_capture(
            fighter, _context(held={Button.B})
        ))
        self.assertEqual(fighter.action, side.ground_hold)
        self.assertFalse(fighter.action_state.klaw_b_held)

        fighter = _Fighter(side.air_wait)
        self.assertTrue(side.hold_capture(
            fighter, _context(ground=False, held={Button.B})
        ))
        self.assertEqual(fighter.action, side.air_hold)

        fighter = _Fighter(side.ground_wait)
        self.assertTrue(side.hold_capture(
            fighter, _context(held=set())
        ))
        self.assertEqual(fighter.action, side.ground_wait)

        fighter = _Fighter(side.ground_wait)
        self.assertTrue(side.hold_capture(fighter, _context(held=0x200)))
        self.assertEqual(fighter.action, side.ground_hold)
        fighter = _Fighter(side.ground_wait)
        self.assertTrue(side.hold_capture(fighter, _context(held=0x100)))
        self.assertEqual(fighter.action, side.ground_wait)

    def test_klaw_hit_finishes_into_hold_without_a_second_press(self):
        side = Bowser.specials.side
        # Ground hit has already taken the native no-victim path; only the
        # aerial hit callback branches on the latched B state.
        fighter = _Fighter(side.air_hit)
        side.input_pressed(fighter, _context(ground=False, held=set()))
        side.finish_hit(fighter, _context(ground=False, held=set()))
        self.assertEqual(fighter.action, side.air_hold)
        self.assertFalse(fighter.action_state.klaw_b_held)

        fighter = _Fighter(side.air_hit)
        side.finish_hit(fighter, _context(ground=False, held={Button.B}))
        self.assertEqual(fighter.action, side.air_wait)

        fighter = _Fighter(side.air_hit)
        side.finish_hit(fighter, _context(ground=False, held=0x100))
        self.assertEqual(fighter.action, side.air_wait)


if __name__ == "__main__":
    unittest.main()
