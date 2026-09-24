"""Source-contract tests for the shared Pikachu/Pichu special family."""

import importlib
import re
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))

from fighter import Action, Button, export_definition


class _Input:
    def __init__(self, stick=(1.0, 0.0)):
        self.stick = stick

    def just_pressed(self, button):
        return button is Button.B


class _Fighter:
    def __init__(self, complete=()):
        self.action = Action.WAIT
        self.action_frame = 0
        self.facing = 1.0
        self.complete = set(complete)
        self.changes = []

    def has_complete_animation(self, state):
        return state in self.complete

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


def _context(*, stick=(1.0, 0.0), resources=(), grounded=True):
    available = set(resources)
    return SimpleNamespace(
        input=_Input(stick),
        ground_open=grounded,
        air_open=not grounded,
        rules=SimpleNamespace(
            specials=SimpleNamespace(side_stick_threshold=0.5, vertical_threshold=0.5)
        ),
        resource=lambda path: object() if path in available else None,
    )


class ElectricFamilyTests(unittest.TestCase):
    @staticmethod
    def _semantic_export(value):
        """Drop only roster identity and normalize qualified source states."""
        if isinstance(value, dict):
            return {
                key: ElectricFamilyTests._semantic_export(item)
                for key, item in value.items()
                if key not in {"name", "external_ids"}
            }
        if isinstance(value, list):
            return [ElectricFamilyTests._semantic_export(item) for item in value]
        if isinstance(value, str):
            return re.sub(r"(?<=Source\.)\d+:", "", value)
        return value

    def test_fighters_have_semantically_identical_special_exports(self):
        exports = []
        for module_name, fighter_name, external_id in (
            ("pikachu", "Pikachu", 13), ("pichu", "Pichu", 24)
        ):
            module = importlib.import_module("fighters." + module_name)
            fighter = getattr(module, fighter_name)
            fighter.external_ids = (external_id,)
            exports.append(self._semantic_export(export_definition(fighter).as_dict()))
        self.assertEqual(exports[0], exports[1])

    def test_both_fighters_use_the_same_native_phase_table(self):
        for module_name, fighter_name, external_id in (
            ("pikachu", "Pikachu", 13), ("pichu", "Pichu", 24)
        ):
            module = importlib.import_module("fighters." + module_name)
            fighter = getattr(module, fighter_name)
            fighter.external_ids = (external_id,)
            exported = export_definition(fighter).as_dict()
            self.assertEqual(
                [exported["actions"][f"special.side.{phase}"]["slippi_state"]
                 for phase in ("ground_start", "ground_hold", "ground_dash", "ground_end")],
                [343, 344, 347, 346],
            )
            self.assertEqual(
                [exported["actions"][f"special.up.{phase}"]["slippi_state"]
                 for phase in ("ground_start", "ground_move", "ground_end")],
                [353, 354, 355],
            )
            self.assertEqual(
                [exported["actions"][f"special.down.{phase}"]["slippi_state"]
                 for phase in ("ground_start", "ground_loop", "ground_hit", "ground_end")],
                [359, 360, 361, 362],
            )

    def test_directional_entry_is_resource_gated_and_uses_complete_native_phase(self):
        from fighters.pikachu import QuickAttack

        move = QuickAttack()
        blocked = _Fighter(complete=(343,))
        self.assertFalse(move.input_pressed(blocked, _context()))
        self.assertFalse(blocked.changes)

        fighter = _Fighter(complete=(343,))
        self.assertTrue(move.input_pressed(fighter, _context(resources=("side",))))
        self.assertEqual(fighter.action, move.ground_start)
        self.assertEqual(fighter.action_frame, 1)

        wrong = _Fighter(complete=(343,))
        self.assertFalse(
            move.input_pressed(wrong, _context(stick=(0.0, 1.0), resources=("side",)))
        )

    def test_source_callbacks_cover_hold_release_and_terminal_phases(self):
        from fighters.pikachu import QuickAttack

        move = QuickAttack()
        fighter = _Fighter()
        fighter.action = move.ground_hold
        self.assertTrue(move.release(fighter, SimpleNamespace()))
        self.assertEqual(fighter.action, move.ground_dash)

        fighter = _Fighter()
        fighter.action = move.ground_end
        move._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, Action.WAIT)

        fighter = _Fighter()
        fighter.action = move.air_end
        move._transition_animation_end(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, Action.FALL)

    def test_neutral_declares_phases_but_no_article_callback(self):
        from fighters.pikachu import ThunderJolt

        move = ThunderJolt()
        self.assertEqual(move.ground.as_dict()["slippi_state"], 341)
        self.assertEqual(move.air.as_dict()["slippi_state"], 342)
        self.assertFalse(any(binding.callback == "spawn" for binding in move.events()))

    def test_neutral_entry_is_resource_gated_and_rejects_directional_b(self):
        from fighters.pikachu import ThunderJolt

        move = ThunderJolt()
        blocked = _Fighter(complete=(341,))
        self.assertFalse(blocked.action == move.ground)
        self.assertFalse(move.input_pressed(blocked, _context()))

        fighter = _Fighter(complete=(341,))
        self.assertTrue(
            move.input_pressed(
                fighter,
                _context(stick=(0.0, 0.0), resources=("neutral",)),
            )
        )
        self.assertEqual(fighter.action, move.ground)

        directional = _Fighter(complete=(341,))
        self.assertFalse(
            move.input_pressed(
                directional,
                _context(stick=(0.0, 1.0), resources=("neutral",)),
            )
        )

    def test_thunder_contact_enters_matching_hit_phase_for_ground_and_air(self):
        for module_name in ("pikachu", "pichu"):
            module = importlib.import_module("fighters." + module_name)
            move = module.Thunder()
            for loop, hit in (
                (move.ground_loop, move.ground_hit),
                (move.air_loop, move.air_hit),
            ):
                fighter = _Fighter()
                fighter.action = loop
                self.assertTrue(move.projectile_contact(fighter, SimpleNamespace()))
                self.assertEqual(fighter.action, hit)

    def test_thunder_contact_ignores_non_loop_phases_and_exports_hook(self):
        from fighters.pikachu import Thunder

        move = Thunder()
        fighter = _Fighter()
        fighter.action = move.ground_hit
        self.assertFalse(move.projectile_contact(fighter, SimpleNamespace()))
        self.assertEqual(fighter.action, move.ground_hit)
        self.assertTrue(
            any(binding.callback == "projectile_contact" for binding in move.events())
        )


if __name__ == "__main__":
    unittest.main()
