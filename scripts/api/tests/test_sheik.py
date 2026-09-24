"""Source-backed behavior checks for Sheik's special motion families."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action, Button, export_definition  # noqa: E402
from fighters.sheik import Chain, Needles, Sheik, Transform, Vanish  # noqa: E402


class _Input:
    def __init__(self, *, pressed: bool = True, buttons=(Button.B,), held=None):
        self.pressed = pressed
        self.buttons = set(buttons)
        self._held = None if held is None else set(held)
        self.stick = (0.0, 0.0)

    def just_pressed(self, button):
        return self.pressed and button in self.buttons

    def held(self, button):
        return self._held is None or button in self._held


class _Fighter:
    grounded = True

    def __init__(self, action=None):
        self.action = action
        self.action_frame = 0
        self.changes = []
        self.action_state = SimpleNamespace(chain_release_latched=False)

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


def _context(*, ground: bool = True, resource: bool = True, buttons=(Button.B,),
             held=None, attributes=None):
    resource_value = (
        SimpleNamespace(attributes=attributes)
        if attributes is not None else object()
    )
    return SimpleNamespace(
        input=_Input(buttons=buttons, held=held),
        resource=lambda path: resource_value if resource else None,
        ground_open=ground,
        air_open=not ground,
        grounded=ground,
        rules=SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5, vertical_threshold=0.5,
        )),
    )


class SheikTests(unittest.TestCase):
    def test_export_matches_native_motion_table(self):
        definition = export_definition(Sheik).as_dict()
        states = {
            value["slippi_state"]
            for value in definition["actions"].values()
            if value.get("slippi_state") is not None
        }
        self.assertEqual(definition["name"], "sheik")
        self.assertEqual(definition["external_ids"], [19])
        self.assertEqual(states, set(range(341, 365)))

    def test_each_special_is_resource_gated_and_surface_aware(self):
        moves = (Sheik.specials.neutral, Sheik.specials.side,
                 Sheik.specials.up, Sheik.specials.down)
        sticks = ((0.0, 0.0), (0.5, 0.0), (0.0, 0.5), (0.0, -0.5))
        for move, stick in zip(moves, sticks):
            instance = _Fighter()
            ctx = _context()
            ctx.input.stick = stick
            self.assertTrue(move.input_pressed(instance, ctx))
            self.assertIs(instance.action, move.ground)

            missing = _Fighter()
            self.assertFalse(move.input_pressed(missing, _context(resource=False)))
            self.assertIsNone(missing.action)

            airborne = _Fighter()
            air_ctx = _context(ground=False)
            air_ctx.input.stick = stick
            self.assertTrue(move.input_pressed(airborne, air_ctx))
            self.assertIs(airborne.action, move.air)

    def test_needles_release_ends_each_charge_phase(self):
        move = Needles()
        for source, target in (
            (move.ground_loop, move.ground_end),
            (move.air_loop, move.air_end),
        ):
            fighter = _Fighter(source)
            self.assertTrue(move.release(fighter, _context()))
            self.assertIs(fighter.action, target)

        for source in (move.ground_start, move.air_start):
            fighter = _Fighter(source)
            self.assertFalse(move.release(fighter, _context()))
            self.assertIs(fighter.action, source)

        fighter = _Fighter(Chain.ground_start)
        self.assertFalse(move.release(fighter, _context()))
        self.assertIs(fighter.action, Chain.ground_start)

    def test_needles_loop_shoulder_input_cancels_charge(self):
        move = Needles()
        for source, target in (
            (move.ground_loop, move.ground_cancel),
            (move.air_loop, move.air_cancel),
        ):
            fighter = _Fighter(source)
            self.assertTrue(move.input_pressed(
                fighter, _context(buttons=(Button.L,)),
            ))
            self.assertIs(fighter.action, target)

        fighter = _Fighter(move.ground_start)
        self.assertFalse(move.input_pressed(
            fighter, _context(buttons=(Button.L,)),
        ))
        self.assertIs(fighter.action, move.ground_start)

    def test_needles_release_wins_over_same_frame_shoulder_cancel(self):
        """Native IASA checks !held B before its LR cancel branch."""
        move = Needles()
        fighter = _Fighter(move.ground_loop)
        self.assertTrue(move.input_pressed(
            fighter,
            _context(buttons=(Button.L,), held=(Button.L,)),
        ))
        self.assertIs(fighter.action, move.ground_end)

    def test_vanish_travel_uses_source_fall_special_attributes(self):
        """State 360 calls FallSpecial with Sheik x58 and x5C."""
        move = Vanish()
        fighter = _Fighter(move.air_move)
        fighter.fall_special = []
        fighter.enter_fall_special = lambda **kwargs: fighter.fall_special.append(kwargs)
        context = _context(
            ground=False,
            attributes=SimpleNamespace(x58=0.75, x5C=12.0),
        )

        self.assertTrue(move.enter_fall_special(fighter, context))
        self.assertEqual(
            fighter.fall_special,
            [{"mobility": 0.75, "landing_lag": 12.0}],
        )

    def test_vanish_countdown_travel_states_do_not_advance_on_animation_end(self):
        """States 356/359 use source countdown/collision callbacks."""
        move = Vanish()
        for state in (move.ground_start_1, move.air_start_1):
            fighter = _Fighter(state)
            move._transition_animation_end(fighter, _context())
            self.assertIs(fighter.action, state)

    def test_chain_release_latches_without_bypassing_native_minimum_frame(self):
        move = Chain()
        for source in (move.ground_loop, move.air_loop):
            fighter = _Fighter(source)
            self.assertFalse(move.release(fighter, _context()))
            self.assertIs(fighter.action, source)
            self.assertTrue(fighter.action_state.chain_release_latched)

        for source in (move.ground_start, move.air_start):
            fighter = _Fighter(source)
            self.assertFalse(move.release(fighter, _context()))
            self.assertIs(fighter.action, source)

    def test_chain_loop_entry_clears_a_stale_release_latch(self):
        move = Chain()
        fighter = _Fighter(move.ground_loop)
        fighter.action_state.chain_release_latched = True
        move.enter_loop(fighter, _context())
        self.assertFalse(fighter.action_state.chain_release_latched)

    def test_native_terminal_and_surface_transitions(self):
        for move in (Needles(), Chain(), Vanish(), Transform()):
            for source, transition in move.on_end.items():
                fighter = _Fighter(source)
                move._transition_animation_end(fighter, _context())
                expected = getattr(transition.target, "action", transition.target)
                self.assertIs(fighter.action, expected)

            for source, transition in move.on_ground.items():
                fighter = _Fighter(source)
                move._transition_ground_air(fighter, _context(ground=True))
                expected = getattr(transition.target, "action", transition.target)
                self.assertIs(fighter.action, expected)
                self.assertEqual(fighter.changes[-1][1], {
                    "preserve_state": True, "keep_frame": True,
                })

            for source, transition in move.on_air.items():
                fighter = _Fighter(source)
                move._transition_ground_air(fighter, _context(ground=False))
                expected = getattr(transition.target, "action", transition.target)
                self.assertIs(fighter.action, expected)


if __name__ == "__main__":
    unittest.main()
