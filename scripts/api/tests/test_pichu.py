"""Focused source contracts for Pichu's fighter initialization and specials."""

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
from fighters.pichu import Pichu, PichuParameters


class _Fighter:
    def __init__(self, action=None, complete=()):
        self.action = action
        self.action_frame = 0
        self.complete = set(complete)
        self.changes = []

    def has_complete_animation(self, state):
        return state in self.complete

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


def _context(*, grounded=True, resources=("neutral",), stick=(0.0, 0.0)):
    available = set(resources)
    return SimpleNamespace(
        input=SimpleNamespace(
            stick=stick,
            just_pressed=lambda button: button is Button.B,
        ),
        ground_open=grounded,
        air_open=not grounded,
        grounded=grounded,
        rules=SimpleNamespace(
            specials=SimpleNamespace(
                side_stick_threshold=0.5,
                vertical_threshold=0.5,
            )
        ),
        resource=lambda path: object() if path in available else None,
    )


class PichuTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.definition = export_definition(Pichu).as_dict()

    def test_initializer_metadata_matches_ftpichu(self):
        self.assertIs(Pichu.parameters, PichuParameters)
        self.assertTrue(PichuParameters().can_walljump)
        self.assertEqual(PichuParameters().data_file, "PlPc.dat")
        self.assertEqual(PichuParameters().animation_data_file, "PlPcAJ.dat")
        self.assertFalse(PichuParameters().up_special_effects)
        self.assertEqual(PichuParameters().thunder_jolt_sound, 230067)
        self.assertEqual(
            Pichu.costume_files,
            ("PlPcNr.dat", "PlPcRe.dat", "PlPcBu.dat", "PlPcGr.dat"),
        )

    def test_shared_motion_table_declares_every_pichu_special_state(self):
        actions = self.definition["actions"]
        states = sorted({
            value["slippi_state"]
            for name, value in actions.items()
            if name.startswith("special.")
        })
        self.assertEqual(states, list(range(341, 367)))

    def test_special_entry_requires_the_native_resource_and_complete_animation(self):
        move = Pichu.specials.up
        blocked = _Fighter(complete=(356,))
        self.assertFalse(
            move.input_pressed(
                blocked,
                _context(grounded=False, resources=(), stick=(0.0, 1.0)),
            )
        )

        fighter = _Fighter(complete=(356,))
        self.assertTrue(
            move.input_pressed(
                fighter,
                _context(grounded=False, resources=("up",), stick=(0.0, 1.0)),
            )
        )
        self.assertIs(fighter.action, move.air_start)
        self.assertEqual(fighter.action_frame, 1)

    def test_terminal_and_surface_transitions_match_shared_source_table(self):
        move = Pichu.specials.down
        fighter = _Fighter(move.air_start)
        move._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, move.ground_start)

        fighter = _Fighter(move.ground_end)
        move._transition_animation_end(fighter, _context())
        self.assertEqual(fighter.action, Action.WAIT)

        fighter = _Fighter(move.air_end)
        move._transition_animation_end(fighter, _context(grounded=False))
        self.assertEqual(fighter.action, Action.FALL)

    def test_no_unbacked_article_callback_is_exported(self):
        for behavior in self.definition["behaviors"]:
            callbacks = behavior.get("callbacks", [])
            self.assertFalse(any("spawn" in callback["callback"] for callback in callbacks))


if __name__ == "__main__":
    unittest.main()
