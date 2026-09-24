"""Focused contracts for Luigi's source-backed neutral special."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action, ArticleId, Button, Fighter, export_definition
from fighters.luigi import Cyclone, Fireball, GreenMissile, Luigi, SuperJumpPunch


class _Fighter:
    action = Action.WAIT
    facing = -1.0

    def __init__(self, complete=()):
        self.action_state = SimpleNamespace(command=(7, 2, 3, 4))
        self.complete = set(complete)
        self.changes = []

    def has_complete_animation(self, state):
        return state in self.complete

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


def _context(*, grounded=True, resources=("neutral",), stick=(0.0, 0.0), directional=False):
    available = set(resources)
    context = SimpleNamespace(
        ground_open=grounded,
        air_open=not grounded,
        input=SimpleNamespace(
            stick=stick,
            just_pressed=lambda button: button is Button.B,
        ),
        resource=lambda path: object() if path in available else None,
    )
    if directional:
        context.rules = SimpleNamespace(
            specials=SimpleNamespace(vertical_threshold=0.5, side_stick_threshold=0.5),
        )
    return context


class LuigiNeutralTests(unittest.TestCase):
    def test_source_actions_and_typed_article_identity(self):
        definition = export_definition(Luigi).as_dict()
        neutral = definition["behaviors"][0]
        self.assertEqual(
            [neutral["actions"][name]["slippi_state"] for name in ("ground", "air")],
            [341, 342],
        )
        self.assertEqual(Fireball().article_id, ArticleId.LUIGI_FIRE)
        self.assertNotEqual(Fireball().article_id, ArticleId.MARIO_FIRE)
        self.assertIsInstance(Luigi.specials.side, GreenMissile)

        actions = definition["actions"]
        self.assertEqual(
            sorted(
                value["slippi_state"]
                for name, value in actions.items()
                if name.startswith("special.")
            ),
            list(range(341, 359)),
        )
        self.assertIsInstance(Luigi.specials.up, SuperJumpPunch)
        self.assertIsInstance(Luigi.specials.down, Cyclone)

    def test_directional_specials_route_and_resource_gate(self):
        for name, stick in (("side", (1.0, 0.0)), ("up", (0.0, 1.0)), ("down", (0.0, -1.0))):
            move = getattr(Luigi.specials, name)
            fighter = _Fighter()
            self.assertTrue(move.input_pressed(fighter, _context(resources=(name,), stick=stick, directional=True)))
            self.assertEqual(fighter.action, move.ground)
            self.assertFalse(move.input_pressed(_Fighter(), _context(resources=(), stick=stick, directional=True)))

    def test_green_missile_and_cyclone_end_on_correct_surface(self):
        move = Luigi.specials.side
        fighter = _Fighter()
        fighter.action = move.ground_start
        move._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, move.ground_hold)
        fighter.action = move.ground_end
        move._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, Action.WAIT)

        move = Luigi.specials.down
        fighter = _Fighter()
        fighter.action = move.air
        move._transition_animation_end(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, Action.FALL)

    def test_green_missile_aerial_fly_landing_enters_fresh_ground_end(self):
        move = Luigi.specials.side
        for phase in (move.air_s2, move.air_end):
            fighter = _Fighter()
            fighter.action = phase
            move._transition_ground_air(fighter, SimpleNamespace(grounded=True))
            self.assertEqual(fighter.action, move.ground_end)
        self.assertFalse(move.on_ground[move.air_s2].preserve_state)
        self.assertFalse(move.on_ground[move.air_end].keep_frame)

    def test_green_missile_release_launches_from_charge(self):
        move = Luigi.specials.side
        fighter = _Fighter()
        fighter.action = move.ground_hold
        self.assertTrue(move.release_charge(fighter, _context(resources=("side",), stick=(1.0, 0.0), directional=True)))
        self.assertEqual(fighter.action, move.ground)
        move._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, move.air_s2)

        fighter = _Fighter()
        fighter.action = move.air_hold
        self.assertTrue(move.release_charge(fighter, _context(grounded=False, resources=("side",), stick=(1.0, 0.0), directional=True)))
        self.assertEqual(fighter.action, move.air)

    def test_cyclone_tap_uses_armed_command_window(self):
        move = Luigi.specials.down
        fighter = _Fighter()
        fighter.action = move.ground
        fighter.action_state.command = (0, 0, 1, 0)
        self.assertTrue(move.input_pressed(fighter, _context(resources=("down",), stick=(0.0, -1.0), directional=True)))
        self.assertEqual(fighter.action, move.air)

        fighter = _Fighter()
        fighter.action = move.ground
        fighter.action_state.command = (0, 0, 0, 0)
        self.assertTrue(move.input_pressed(fighter, _context(resources=("down",), stick=(0.0, -1.0), directional=True)))
        self.assertEqual(fighter.action, move.ground)

    def test_source_entry_resets_only_luigi_command_slots(self):
        missile = Luigi.specials.side
        fighter = _Fighter()
        fighter.action_state.command = (9, 8, 7, 6)
        missile.enter_start(fighter, _context())
        self.assertEqual(fighter.action_state.command, (0, 8, 7, 6))

        cyclone = Luigi.specials.down
        fighter = _Fighter()
        fighter.action_state.command = (9, 8, 7, 6)
        cyclone.enter(fighter, _context())
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 6))

    def test_cyclone_air_animation_latches_charge_command(self):
        move = Luigi.specials.down
        fighter = _Fighter()
        fighter.action = move.air
        fighter.action_state.command = (0, 1, 0, 0)
        move.finish_air(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))
        self.assertTrue(fighter.action_state.cyclone_charge)

        fighter = _Fighter()
        fighter.action = move.air
        fighter.action_state.command = (0, 0, 0, 0)
        move.finish_air(fighter, SimpleNamespace(grounded=False))
        self.assertFalse(hasattr(fighter.action_state, "cyclone_charge"))

    def test_neutral_is_reserved_only_when_b_is_not_directional(self):
        move = Fireball()
        fighter = _Fighter(complete=(341,))
        self.assertTrue(move.input_pressed(fighter, _context()))
        self.assertEqual(fighter.action, move.ground)

        fighter = _Fighter(complete=(341,))
        self.assertFalse(move.input_pressed(fighter, _context(stick=(0.0, 0.5), directional=True)))
        self.assertEqual(fighter.action, Action.WAIT)

    def test_fireball_waits_for_spawn_command_before_terminal_transition(self):
        move = Fireball()
        fighter = _Fighter()
        fighter.action = move.ground
        fighter.action_state.command = (0, 0, 0, 0)
        move._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, move.ground)

        fighter.action_state.command = (1, 0, 0, 0)
        move._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, Action.WAIT)

        fighter = _Fighter()
        fighter.action = move.air
        fighter.action_state.command = (0, 0, 0, 0)
        move._transition_animation_end(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, move.air)


if __name__ == "__main__":
    unittest.main()
