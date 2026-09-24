"""Source-contract tests for Kirby's native special motion graph."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action, Button, export_definition  # noqa: E402
from fighters.kirby import (  # noqa: E402
    FinalCutter,
    Hammer,
    Inhale,
    Kirby,
    Stone,
)


class _Input:
    def __init__(self, stick):
        self.stick = stick

    def just_pressed(self, button):
        return button is Button.B


class _Fighter:
    action = Action.WAIT

    def __init__(self):
        self.changes = []
        self.action_frame = 0
        self.facing = 1.0
        self.action_state = SimpleNamespace(command=(0, 0, 0, 0))

    def enter_fall_special(self, **kwargs):
        self.fall_special = kwargs

    def change_action(self, action, **kwargs):
        self.action = action
        self.changes.append((action, kwargs))


def _context(*, resource="neutral", stick=(0.0, 0.0), grounded=True):
    return SimpleNamespace(
        input=_Input(stick),
        ground_open=grounded,
        air_open=not grounded,
        resource=lambda path: object() if path == resource else None,
        rules=SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5,
            vertical_threshold=0.5,
        )),
    )


class KirbySpecialTests(unittest.TestCase):
    def test_native_state_range_and_typed_special_roots(self):
        exported = export_definition(Kirby).as_dict()
        states = {
            phase["slippi_state"]
            for phase in exported["actions"].values()
            if phase.get("slippi_state") is not None
        }
        self.assertEqual(states, set(range(353, 399)))
        self.assertEqual(exported["external_ids"], [4])
        self.assertEqual(
            [move.resource for move in (
                Kirby.specials.neutral,
                Kirby.specials.side,
                Kirby.specials.up,
                Kirby.specials.down,
            )],
            ["neutral", "side", "up", "down"],
        )

    def test_entries_are_resource_gated_and_directional(self):
        for move, stick in (
            (Inhale(), (0.0, 0.0)),
            (Hammer(), (1.0, 0.0)),
            (FinalCutter(), (0.0, 1.0)),
            (Stone(), (0.0, -1.0)),
        ):
            blocked = _Fighter()
            self.assertFalse(move.input_pressed(
                blocked, _context(resource=None, stick=stick)
            ))
            self.assertFalse(blocked.changes)

            fighter = _Fighter()
            self.assertTrue(move.input_pressed(
                fighter, _context(resource=move.resource, stick=stick)
            ))
            self.assertIs(fighter.action, move.ground)
            self.assertEqual(fighter.action_frame, 1)

        wrong = _Fighter()
        self.assertFalse(
            Hammer().input_pressed(
                wrong, _context(resource="side", stick=(0.0, 1.0))
            )
        )

    def test_native_terminals_and_surface_pairs(self):
        for move, ground, air in (
            (Inhale(), Inhale.ground_end, Inhale.air_end),
            (Hammer(), Hammer.ground, Hammer.air),
            (FinalCutter(), FinalCutter.ground_end, FinalCutter.air_end),
            (Stone(), Stone.ground_end, Stone.air_end),
        ):
            grounded = _Fighter()
            grounded.action = ground
            move._transition_animation_end(grounded, SimpleNamespace(grounded=True))
            self.assertIs(grounded.action, Action.WAIT)

            airborne = _Fighter()
            airborne.action = air
            move._transition_animation_end(airborne, SimpleNamespace(grounded=False))
            self.assertIs(airborne.action, Action.FALL)

        fighter = _Fighter()
        fighter.action = FinalCutter.air_rise
        FinalCutter()._transition_ground_air(
            fighter, SimpleNamespace(grounded=True)
        )
        self.assertEqual(fighter.action, FinalCutter.ground_rise.action)
        self.assertEqual(fighter.changes[-1][1], {
            "preserve_state": True,
            "keep_frame": True,
        })

        fighter = _Fighter()
        fighter.action = Stone.ground
        Stone()._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, Stone.ground_hold.action)

    def test_inhale_release_matches_source_loop_iASA(self):
        inhale = Inhale()
        for loop, end in ((inhale.ground_loop, inhale.ground_end),
                          (inhale.air_loop, inhale.air_end)):
            fighter = _Fighter()
            fighter.action = loop
            self.assertTrue(inhale.release(fighter, SimpleNamespace()))
            self.assertIs(fighter.action, end)

        fighter = _Fighter()
        fighter.action = inhale.ground
        self.assertFalse(inhale.release(fighter, SimpleNamespace()))

    def test_inhale_capture_and_article_phases_preserve_surface_frames(self):
        inhale = Inhale()
        pairs = (
            (inhale.ground_capture, inhale.air_capture_wait),
            (inhale.ground_capture_wait, inhale.air_capture),
            (inhale.ground_eat, inhale.air_eat),
            (inhale.ground_eat_wait, inhale.air_eat_fall),
            (inhale.ground_drink, inhale.air_drink_end),
            (inhale.ground_drink_end, inhale.air_drink),
            (inhale.ground_spit, inhale.air_spit_end),
            (inhale.ground_spit_end, inhale.air_spit),
            (inhale.ground_eat_turn, inhale.air_eat_turn),
        )
        for ground, air in pairs:
            fighter = _Fighter()
            fighter.action = air
            inhale._transition_ground_air(fighter, SimpleNamespace(grounded=True))
            self.assertIs(fighter.action, ground.action)
            self.assertEqual(fighter.changes[-1][1], {
                "preserve_state": True,
                "keep_frame": True,
            })

    def test_final_cutter_reverse_is_one_shot_and_thresholded(self):
        move = FinalCutter()
        fighter = _Fighter()
        fighter.action = move.ground_start.action
        ctx = _context(stick=(-0.75, 0.0))
        ctx.rules.specials.reverse_upb_stick_range = 0.5
        self.assertTrue(move.reverse(fighter, ctx))
        self.assertEqual(fighter.facing, -1.0)
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 1))
        self.assertFalse(move.reverse(fighter, ctx))

        fighter = _Fighter()
        fighter.action = move.ground_start.action
        ctx.input.stick = (-0.5, 0.0)
        self.assertFalse(move.reverse(fighter, ctx))

    def test_stone_hold_phases_are_animation_loops(self):
        self.assertTrue(Stone.ground_hold.animation_loop)
        self.assertTrue(Stone.air_hold.animation_loop)
        exported = export_definition(Kirby).as_dict()["actions"]
        self.assertTrue(exported["special.down.ground_hold"]["animation_loop"])
        self.assertTrue(exported["special.down.air_hold"]["animation_loop"])

    def test_stone_release_waits_for_source_minimum_hold(self):
        move = Stone()
        fighter = _Fighter()
        fighter.action = move.ground_hold.action
        fighter.action_frame = 4
        ctx = SimpleNamespace(
            rules=SimpleNamespace(specials=SimpleNamespace(stone_min_hold_frames=5))
        )
        self.assertFalse(move.release(fighter, ctx))
        self.assertIs(fighter.action, move.ground_hold.action)
        fighter.action_frame = 5
        self.assertTrue(move.release(fighter, ctx))
        self.assertIs(fighter.action, move.ground_end)

    def test_hammer_air_landing_enters_fall_special_lag(self):
        move = Hammer()
        fighter = _Fighter()
        fighter.action = move.air.action
        ctx = SimpleNamespace(
            rules=SimpleNamespace(specials=SimpleNamespace(landing_lag=12))
        )
        self.assertTrue(move.landing(fighter, ctx))
        self.assertEqual(fighter.fall_special, {"mobility": 0, "landing_lag": 12})


if __name__ == "__main__":
    unittest.main()
