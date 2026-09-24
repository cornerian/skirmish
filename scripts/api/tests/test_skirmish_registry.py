"""Scoped registry contracts for class based fighter source bundles."""

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import (
    AerialMoves, Attributes, DefenseMoves, Fighter, GetupMoves, GrabMoves,
    GroundedMoves, LedgeMoves, Move, MoveError, SmashMoves, SpecialMoves,
    TauntMoves, ThrowMoves, TiltMoves,
)
from skirmish.registry import FighterRegistry


class _Attributes(Attributes):
    pass


class _Move(Move):
    pass


def _fighter(name: str, external_id: int) -> type[Fighter]:
    move = _Move()
    return type(
        name.title().replace(" ", ""),
        (Fighter,),
        {
            "name": name,
            "external_ids": (external_id,),
            "attributes": _Attributes,
            "specials": SpecialMoves(move, move, move, move),
            "aerials": AerialMoves(move, move, move, move, move),
            "grounded": GroundedMoves(move, move, move),
            "tilts": TiltMoves(move, move, move),
            "smashes": SmashMoves(move, move, move),
            "grabs": GrabMoves(move, move, move),
            "throws": ThrowMoves(move, move, move, move),
            "defense": DefenseMoves(move, move, move, move, move),
            "ledge": LedgeMoves(move, move, move, move, move),
            "getup": GetupMoves(move, move, move, move),
            "taunt": TauntMoves(move),
        },
    )


class FighterRegistryTests(unittest.TestCase):
    def test_lookup_and_idempotent_registration(self):
        registry = FighterRegistry()
        mario = _fighter("mario", 8)
        self.assertIs(registry.register(mario), mario)
        self.assertIs(registry.register(mario), mario)
        self.assertIs(registry.lookup("mario"), mario)
        self.assertIs(registry.lookup(8), mario)
        self.assertEqual(registry.fighters(), (mario,))

    def test_name_only_declarations_are_supported_before_identity_binding(self):
        registry = FighterRegistry()
        mario = _fighter("mario", 8)
        del mario.external_ids
        registry.register(mario)
        self.assertIs(registry.lookup("mario"), mario)

    def test_conflicting_identity_is_rejected(self):
        registry = FighterRegistry()
        registry.register(_fighter("mario", 8))
        with self.assertRaises(MoveError):
            registry.register(_fighter("mario", 9))
        with self.assertRaises(MoveError):
            registry.register(_fighter("luigi", 8))

    def test_registries_are_isolated_between_source_bundles(self):
        first = FighterRegistry()
        second = FighterRegistry()
        mario = _fighter("mario", 8)
        first.register(mario)
        self.assertIsNone(second.lookup("mario"))
        self.assertEqual(second.fighters(), ())

    def test_lookup_rejects_ambiguous_key_types(self):
        with self.assertRaises(TypeError):
            FighterRegistry().lookup(8.0)
        with self.assertRaises(TypeError):
            FighterRegistry().lookup(True)


if __name__ == "__main__":
    unittest.main()
