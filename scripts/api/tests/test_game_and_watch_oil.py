"""Focused source contract for Game & Watch's full Oil Panic bucket."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
sys.path.insert(0, str(ROOT / "scripts" / "api"))
sys.path.insert(0, str(ROOT / "scripts"))

from fighter import Action, Button
from fighters.game_and_watch import Chef, OilPanic


class _Input:
    def __init__(self, stick):
        self.stick = stick

    def just_pressed(self, button):
        return button is Button.B


class _Fighter:
    def __init__(self):
        self.action = Action.WAIT

    def change_action(self, action, **kwargs):
        self.action = action


def _context(charge, *, stick=(0.0, -1.0), chef_loop_open=False,
             chef_sausages=0, chef_maximum=5):
    return SimpleNamespace(
        ground_open=True,
        air_open=False,
        stick=(0.0, -1.0),
        input=_Input(stick),
        panic_charge=charge,
        chef_loop_open=chef_loop_open,
        chef_sausages=chef_sausages,
        chef_maximum=chef_maximum,
        resource=lambda path: object(),
        rules=SimpleNamespace(
            specials=SimpleNamespace(side_stick_threshold=0.5, vertical_threshold=0.5)
        ),
    )


class OilPanicTests(unittest.TestCase):
    def test_full_bucket_enters_shoot_state_on_source_entry(self):
        move = OilPanic()
        fighter = _Fighter()

        self.assertTrue(move.input_pressed(fighter, _context(3)))
        self.assertIs(fighter.action, move.ground_shoot)

    def test_partial_bucket_keeps_absorb_entry(self):
        move = OilPanic()
        fighter = _Fighter()

        self.assertTrue(move.input_pressed(fighter, _context(2)))
        self.assertIs(fighter.action, move.ground)

    def test_partial_catch_returns_to_absorb_loop_on_animation_end(self):
        move = OilPanic()
        fighter = _Fighter()
        fighter.action = move.ground_catch

        move.catch_animation_end(fighter, _context(2))

        self.assertIs(fighter.action, move.ground)

    def test_full_catch_exits_to_wait_or_fall_on_animation_end(self):
        move = OilPanic()
        grounded = _Fighter()
        grounded.action = move.ground_catch
        move.catch_animation_end(grounded, _context(3))
        self.assertIs(grounded.action, Action.WAIT)

        aerial = _Fighter()
        aerial.action = move.air_catch
        move.catch_animation_end(aerial, _context(3))
        self.assertIs(aerial.action, Action.FALL)

    def test_chef_command_frame_restarts_motion_when_loop_is_available(self):
        move = Chef()
        fighter = _Fighter()
        self.assertTrue(move.input_pressed(fighter, _context(
            None, stick=(0.0, 0.0), chef_loop_open=True,
            chef_sausages=2, chef_maximum=5,
        )))
        self.assertIs(fighter.action, move.ground)

        fighter.action = move.ground
        fighter.action_frame = 9
        self.assertTrue(move.input_pressed(fighter, _context(
            None, stick=(0.0, 0.0), chef_loop_open=True,
            chef_sausages=5, chef_maximum=5,
        )))
        self.assertEqual(fighter.action_frame, 9)


if __name__ == "__main__":
    unittest.main()
