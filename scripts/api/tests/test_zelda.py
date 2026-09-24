"""Source contracts for Zelda's four special state machines.

The state numbers and terminal behavior are taken from the ftZelda special
modules in the pinned melee decomp.  Article spawning and transformation are
native responsibilities, so the Python declaration only owns the phase graph.
"""

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
from fighters.zelda import TransformOutcome, Zelda  # noqa: E402


class _Input:
    def __init__(self, stick=(0.0, 0.0), pressed=True):
        self.stick = stick
        self.pressed = pressed

    def just_pressed(self, button):
        return self.pressed and button is Button.B


class _Fighter:
    grounded = True

    def __init__(self, action=None):
        self.action = action
        self.changes = []
        self.action_state = SimpleNamespace(command=(7, 2, 3, 4))
        self.position = (1.0, 2.0, 0.0)
        self.facing = -1.0
        self.spawned = []

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def spawn_article(self, *args):
        self.spawned.append(args)


def _context(*, stick=(0.0, 0.0), grounded=True, resource=True, pressed=True):
    return SimpleNamespace(
        input=_Input(stick, pressed),
        resource=lambda path: object() if resource else None,
        ground_open=grounded,
        air_open=not grounded,
        grounded=grounded,
        rules=SimpleNamespace(
            specials=SimpleNamespace(side_stick_threshold=0.5, vertical_threshold=0.5)
        ),
    )


class ZeldaSpecialTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.definition = export_definition(Zelda).as_dict()

    def test_decomp_state_table_and_loop_metadata(self):
        self.assertEqual((self.definition["name"], self.definition["external_ids"]),
                         ("zelda", [18]))
        actions = self.definition["actions"]
        self.assertEqual(
            sorted({value["slippi_state"] for value in actions.values()
                    if value.get("slippi_state") is not None}),
            list(range(341, 359)),
        )
        self.assertTrue(actions["special.side.ground_loop"]["animation_loop"])
        self.assertTrue(actions["special.side.air_loop"]["animation_loop"])

    def test_entries_are_resource_and_axis_gated(self):
        for move, stick in (
            (Zelda.specials.neutral, (0.0, 0.0)),
            (Zelda.specials.side, (1.0, 0.0)),
            (Zelda.specials.up, (0.0, 1.0)),
            (Zelda.specials.down, (0.0, -1.0)),
        ):
            self.assertFalse(move.input_pressed(_Fighter(), _context(
                stick=stick, resource=False)))
            fighter = _Fighter()
            self.assertTrue(move.input_pressed(fighter, _context(stick=stick)))
            self.assertIs(fighter.action, move.ground)

        self.assertFalse(Zelda.specials.side.input_pressed(
            _Fighter(), _context(stick=(0.0, 1.0))))
        self.assertFalse(Zelda.specials.up.input_pressed(
            _Fighter(), _context(stick=(0.0, -1.0))))
        self.assertFalse(Zelda.specials.down.input_pressed(
            _Fighter(), _context(stick=(0.0, 1.0))))

    def test_decomp_terminal_and_surface_transitions(self):
        cases = (
            (Zelda.specials.neutral, "Source.18:342", Action.FALL),
            (Zelda.specials.side, "Source.18:348", Action.FALL),
            (Zelda.specials.up, "Source.18:354", Action.FALL),
            (Zelda.specials.down, "Source.18:358", Action.FALL),
        )
        for move, terminal, expected in cases:
            fighter = _Fighter(terminal)
            move._transition_animation_end(fighter, _context(grounded=False))
            self.assertEqual(fighter.action, expected)

        for move in (Zelda.specials.neutral, Zelda.specials.side,
                     Zelda.specials.up, Zelda.specials.down):
            fighter = _Fighter(move.air)
            move._transition_ground_air(fighter, SimpleNamespace(grounded=True))
            self.assertEqual(fighter.action, move.ground)
            self.assertEqual(fighter.changes[-1][1], {
                "preserve_state": True, "keep_frame": True,
            })

    def test_din_release_routes_each_loop_to_its_source_end_phase(self):
        move = Zelda.specials.side
        for loop, end in ((move.ground_loop, move.ground_end),
                          (move.air_loop, move.air_end)):
            fighter = _Fighter(loop)
            self.assertTrue(move.release(fighter, _context(pressed=False)))
            self.assertIs(fighter.action, end)

        fighter = _Fighter(move.ground_start)
        self.assertFalse(move.release(fighter, _context(pressed=False)))

        behavior_id = self.definition["movesets"]["specials"]["side"]
        behavior = next(item for item in self.definition["behaviors"]
                        if item["id"] == behavior_id)
        release = next(item for item in behavior["callbacks"]
                       if item["hook"] == "input_released")
        self.assertEqual(release["actions"], ["Source.18:344", "Source.18:347"])

    def test_din_command_cue_spawns_native_article_and_clears_slot(self):
        move = Zelda.specials.side
        fighter = _Fighter(move.ground_loop)
        ctx = _context()
        ctx.event = SimpleNamespace(value=1)

        move.spawn(fighter, ctx)

        self.assertEqual(
            fighter.spawned,
            [(108, fighter.position, fighter.facing)],
        )
        self.assertEqual(fighter.action_state.command, (0, 2, 3, 4))

    def test_din_command_callback_ignores_zero_cue(self):
        move = Zelda.specials.side
        fighter = _Fighter(move.ground_start)
        ctx = _context()
        ctx.event = SimpleNamespace(value=0)

        move.spawn(fighter, ctx)

        self.assertEqual(fighter.spawned, [])
        self.assertEqual(fighter.action_state.command, (7, 2, 3, 4))

    def test_transform_entry_resets_command_and_completion_is_typed_unsupported(self):
        move = Zelda.specials.down
        fighter = _Fighter(move.ground)

        move.enter(fighter, SimpleNamespace())

        self.assertEqual(fighter.action_state.command, (0, 2, 3, 4))
        self.assertIs(move.native_completion(fighter, SimpleNamespace()),
                      TransformOutcome.UNSUPPORTED)


if __name__ == "__main__":
    unittest.main()
