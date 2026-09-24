"""Focused contracts for source article identities and B0 lifecycle policy."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action, ArticleId, Button, B0ArticleSpecial, source_phase
from fighter.special_attributes import SamusAttribute, _Attribute


class _Move(B0ArticleSpecial):
    article_id = ArticleId.MARIO_FIRE
    ground = source_phase(343)
    air = source_phase(344)


class _Fighter:
    action = Action.WAIT
    facing = -1.0

    def __init__(self, *, complete=(343, 344)):
        self.action_state = SimpleNamespace(command=(9, 2, 3, 4))
        self.complete = set(complete)
        self.spawned = []
        self.changes = []

    def has_complete_animation(self, state):
        return state in self.complete

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def part_position(self, part):
        return (part, 1.0, 2.0)

    def spawn_article(self, *args):
        self.spawned.append(args)


def _context(*, pressed=True, resource=True, grounded=True, stick=(0.0, 0.0)):
    calls = []

    def lookup(path):
        calls.append(path)
        return object() if resource else None

    return SimpleNamespace(
        calls=calls,
        ground_open=grounded,
        air_open=not grounded,
        input=SimpleNamespace(
            stick=stick,
            just_pressed=lambda button: pressed and button is Button.B,
        ),
        resource=lookup,
        rules=SimpleNamespace(
            specials=SimpleNamespace(vertical_threshold=0.5, side_stick_threshold=0.5)
        ),
    )


class ArticleTests(unittest.TestCase):
    def test_source_article_ids_match_item_kind_enum(self):
        ids = {member.name: int(member) for member in ArticleId}
        self.assertEqual(len(ids), 70)
        self.assertEqual(
            {name: ids[name] for name in (
                "MARIO_FIRE", "DR_MARIO_VITAMIN", "FOX_LASER", "FALCO_LASER",
                "LINK_BOMB", "YOUNG_LINK_BOMB", "NESS_PK_FLASH_EXPLOSION",
                "SHEIK_NEEDLE_THROW", "SHEIK_NEEDLE_HELD",
                "SAMUS_BOMB", "SAMUS_CHARGE", "SAMUS_MISSILE",
                "PEACH_TOAD_SPORE", "LUIGI_FIRE",
            )},
            {
                "MARIO_FIRE": 48, "DR_MARIO_VITAMIN": 49, "FOX_LASER": 54,
                "FALCO_LASER": 55, "LINK_BOMB": 58, "YOUNG_LINK_BOMB": 59,
                "NESS_PK_FLASH_EXPLOSION": 78, "SAMUS_BOMB": 93,
                "SAMUS_CHARGE": 94, "SAMUS_MISSILE": 95,
                "SHEIK_NEEDLE_THROW": 79, "SHEIK_NEEDLE_HELD": 80,
                "PEACH_TOAD_SPORE": 111, "LUIGI_FIRE": 105,
            },
        )

    def test_declared_state_metadata_cannot_diverge_from_source_phases(self):
        with self.assertRaises(ValueError):
            class InvalidMove(B0ArticleSpecial):
                article_id = ArticleId.MARIO_FIRE
                ground = source_phase(343)
                air = source_phase(344)
                ground_state = 999
                air_state = 344

    def test_held_b_is_not_consumed_while_article_action_is_active(self):
        move = _Move()
        fighter = _Fighter()
        fighter.action = move.ground
        self.assertFalse(move.input_pressed(fighter, _context(pressed=False)))

    def test_entry_is_resource_and_animation_gated_once(self):
        move = _Move()
        fighter = _Fighter(complete=())
        context = _context()
        self.assertFalse(move.input_pressed(fighter, context))
        self.assertEqual(context.calls, ["neutral"])

        fighter.complete = {343}
        context = _context()
        self.assertTrue(move.input_pressed(fighter, context))
        self.assertEqual(fighter.action, move.ground)
        self.assertEqual(fighter.changes, [(move.ground, {})])
        self.assertEqual(context.calls, ["neutral"])

    def test_animation_event_spawns_at_native_part_and_facing(self):
        fighter = _Fighter()
        _Move().spawn(fighter, SimpleNamespace())
        self.assertEqual(fighter.spawned, [(ArticleId.MARIO_FIRE, (23, 1.0, 2.0), -1.0)])

    def test_attribute_wire_components_reject_out_of_range_values(self):
        with self.assertRaises(ValueError):
            class InvalidLayout(_Attribute):
                VALUE = (0, 256)

        with self.assertRaises(ValueError):
            class InvalidField(_Attribute):
                VALUE = (0x10000, 1)

    def test_samus_attribute_surface_covers_all_special_roots(self):
        roots = ("SPECIAL_N_", "SPECIAL_S_", "SCREW_ATTACK_", "SPECIAL_LW_")
        names = {member.name for member in SamusAttribute}
        for root in roots:
            self.assertTrue(any(name.startswith(root) for name in names), root)
        self.assertEqual(SamusAttribute.SPECIAL_N_CHARGE_RATE.layout, 4)
        self.assertEqual(SamusAttribute.SPECIAL_LW_STICK_THRESHOLD.field_id, 26)


if __name__ == "__main__":
    unittest.main()
