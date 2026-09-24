"""Focused source-state coverage for Sheik and Zelda specials."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import (  # noqa: E402
    Action, Button, DownSpecial, NeutralSpecial, SideSpecial, UpSpecial,
    export_definition,
)
from fighters.sheik import Sheik  # noqa: E402
from fighters.zelda import Zelda  # noqa: E402


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
        self.action_frame = 0
        self.changes = []

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


def _context(*, stick=(0.0, 0.0), ground=True, resource=True, pressed=True,
             attributes=None):
    resource_value = (
        object() if attributes is None else SimpleNamespace(attributes=attributes)
    )
    return SimpleNamespace(
        input=_Input(stick, pressed),
        resource=lambda path: resource_value if resource else None,
        ground_open=ground,
        air_open=not ground,
        grounded=ground,
        rules=SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5, vertical_threshold=0.5,
        )),
    )


class SheikZeldaSpecialTests(unittest.TestCase):
    def test_source_state_ranges_and_directional_roots(self):
        for fighter, expected, external in ((Sheik, range(341, 365), 19),
                                            (Zelda, range(341, 359), 18)):
            definition = export_definition(fighter).as_dict()
            self.assertEqual((definition["name"], definition["external_ids"]),
                             (fighter.__name__.lower(), [external]))
            states = {value["slippi_state"] for value in definition["actions"].values()
                      if value.get("slippi_state") is not None}
            self.assertEqual(states, set(expected))
            moves = [getattr(fighter.specials, root)
                     for root in ("neutral", "side", "up", "down")]
            self.assertIsInstance(moves[0], NeutralSpecial)
            self.assertIsInstance(moves[1], SideSpecial)
            self.assertIsInstance(moves[2], UpSpecial)
            self.assertIsInstance(moves[3], DownSpecial)

    def test_entries_are_resource_gated_and_directional(self):
        for fighter in (Sheik, Zelda):
            roots = (fighter.specials.neutral, fighter.specials.side,
                     fighter.specials.up, fighter.specials.down)
            for move, stick in zip(roots, ((0.0, 0.0), (0.5, 0.0),
                                           (0.0, 0.5), (0.0, -0.5))):
                instance = _Fighter()
                self.assertTrue(move.input_pressed(instance, _context(stick=stick)))
                self.assertIs(instance.action, move.ground)
                self.assertFalse(move.input_pressed(_Fighter(), _context(
                    stick=stick, resource=False,
                )))

    def test_terminal_and_surface_transitions(self):
        for fighter in (Sheik, Zelda):
            for move in (fighter.specials.neutral, fighter.specials.side,
                         fighter.specials.up, fighter.specials.down):
                instance = _Fighter(move.air)
                move._transition_ground_air(instance, SimpleNamespace(grounded=True))
                self.assertTrue(instance.changes)
                self.assertEqual(instance.changes[-1][1], {
                    "preserve_state": True, "keep_frame": True,
                })
                terminal = getattr(move, "air_end", getattr(move, "air_move", move.air))
                instance = _Fighter(terminal)
                if hasattr(move, "aerial_move_end"):
                    # Farore's source callback enters FallSpecial with Zelda
                    # attributes; its dedicated test covers that handoff.
                    continue
                terminal_context = _context(ground=False)
                if fighter is Sheik and move is Sheik.specials.neutral:
                    # States 347/348 call ordinary Fall when x10 is zero;
                    # nonzero x10 is covered by the focused Sheik test.
                    terminal_context = _context(
                        ground=False, attributes=SimpleNamespace(x10=0.0),
                    )
                if fighter is Sheik and move is Sheik.specials.neutral:
                    # The source callback owns this terminal branch; invoke
                    # it directly because the lightweight transition helper
                    # only exercises declarative on_end mappings.
                    move.enter_air_fall(instance, terminal_context)
                else:
                    move._transition_animation_end(instance, terminal_context)
                if ((fighter is Sheik and move is Sheik.specials.up)
                        or (fighter is Zelda and move in (Zelda.specials.up, Zelda.specials.down))):
                    # Sheik state 360 and Zelda states 354/357 enter
                    # FallSpecial through native callbacks when attributes
                    # are available; without that resource, the callbacks
                    # intentionally leave the terminal source action intact.
                    self.assertEqual(instance.action, terminal)
                else:
                    self.assertEqual(instance.action, Action.FALL)


if __name__ == "__main__":
    unittest.main()
