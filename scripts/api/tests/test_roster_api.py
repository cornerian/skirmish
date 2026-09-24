import unittest
import sys
from pathlib import Path
from types import SimpleNamespace

API = Path(__file__).parents[1]
if str(API) not in sys.path:
    sys.path.insert(0, str(API))

from fighter import (
    Action,
    Fighter,
    FighterBase,
    MoveError,
    OpenSpecial,
    Roster,
    SpecialRoot,
    resolve_identity,
    directional_b_input,
    directional_b_reserved,
)


class _Input:
    def __init__(self, pressed=True, stick=(0.0, 0.0)):
        self.pressed = pressed
        self.stick = stick
        self.calls = 0

    def just_pressed(self, button):
        self.calls += 1
        return self.pressed


class _Fighter:
    def __init__(self, action=None):
        self.action = action
        self.action_frame = 19
        self.changes = []

    def change_action(self, action):
        self.changes.append(action)
        self.action = action


def _context(*, resource=True, pressed=True, stick=(0.0, 0.0), ground=True, air=False,
             rules=None):
    input_state = _Input(pressed=pressed, stick=stick)
    return SimpleNamespace(
        input=input_state,
        ground_open=ground,
        air_open=air,
        rules=rules,
        resource=lambda path: object() if resource else None,
    )


class RosterApiTests(unittest.TestCase):
    def test_roster_is_the_unique_26_identity_table(self):
        self.assertEqual(len(Roster), 26)
        self.assertEqual(len({item.slug for item in Roster}), 26)
        self.assertEqual(len({item.external_id for item in Roster}), 26)
        self.assertEqual(Roster.MARIO.slug, "mario")
        self.assertEqual(Roster.MARIO.external_id, 8)

    def test_roster_derives_identity_and_rejects_mismatch(self):
        Mario = type("Mario", (Fighter,), {"__module__": "mario"})
        resolve_identity(Mario)
        self.assertEqual(Mario.name, "mario")
        self.assertEqual(Mario.external_ids, (8,))

        with self.assertRaises(MoveError):
            resolve_identity(type("WrongName", (Fighter,), {"__module__": "mario", "name": "luigi"}))
        with self.assertRaises(MoveError):
            resolve_identity(type("WrongId", (Fighter,), {"__module__": "mario", "external_ids": (7,)}))

        Fox = type("Fox", (Fighter,), {"__module__": "fox"})
        resolve_identity(Fox)
        Falco = type("Falco", (Fox,), {"__module__": "falco"})
        resolve_identity(Falco)
        self.assertEqual(Falco.name, "falco")
        self.assertEqual(Falco.external_ids, (20,))

    def test_trusted_loader_module_overrides_embedded_runtime_module_name(self):
        Captain = type("CaptainFalcon", (Fighter,), {"__module__": "fighter"})
        resolve_identity(Captain, "captain")
        self.assertEqual(Captain.name, "captain-falcon")
        self.assertEqual(Captain.external_ids, (0,))

    def test_external_ids_are_integer_identity_values(self):
        declarations = {
            "name": "custom",
            "external_ids": (1, "bad"),
        }
        with self.assertRaises(MoveError):
            resolve_identity(type("Invalid", (Fighter,), declarations))

        declarations["external_ids"] = (1, 1)
        with self.assertRaises(MoveError):
            resolve_identity(type("Duplicate", (Fighter,), declarations))

    def test_fighter_has_behavioral_defaults(self):
        self.assertIsInstance(Fighter.attributes, type(Fighter.attributes))
        special_names = ("neutral", "side", "up", "down")
        self.assertTrue(all(
            isinstance(getattr(Fighter.specials, name), OpenSpecial)
            for name in special_names
        ))
        self.assertEqual(Fighter.specials.neutral.ground, Action.SPECIAL_N_START)
        self.assertEqual(Fighter.specials.neutral.air, Action.SPECIAL_AIR_N_START)

    def test_open_special_gates_resource_input_surface_and_resets_frame(self):
        move = OpenSpecial(SpecialRoot.SIDE)
        rules = SimpleNamespace(specials=SimpleNamespace(side_stick_threshold=0.5))

        missing = _Fighter()
        self.assertFalse(move.input_pressed(missing, _context(resource=False, rules=rules)))
        self.assertFalse(move.input_pressed(_Fighter(), _context(pressed=False, rules=rules)))
        closed = _Fighter()
        self.assertFalse(move.input_pressed(closed, _context(rules=rules, ground=False, air=False,
                                                            stick=(0.5, 0.0))))

        ground = _Fighter()
        self.assertTrue(move.input_pressed(ground, _context(rules=rules, stick=(0.5, 0.0))))
        self.assertEqual(ground.changes, [Action.SPECIAL_S_START])
        self.assertEqual(ground.action_frame, 1)

        air = _Fighter()
        self.assertTrue(move.input_pressed(air, _context(rules=rules, ground=False, air=True,
                                                         stick=(-0.5, 0.0))))
        self.assertEqual(air.changes, [Action.SPECIAL_AIR_S_START])

    def test_directional_boundaries_are_inclusive_and_one_sided(self):
        rules = SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5, vertical_threshold=0.5,
        ))
        def ctx(stick):
            return _context(rules=rules, stick=stick)

        up = OpenSpecial(SpecialRoot.UP)
        down = OpenSpecial(SpecialRoot.DOWN)
        self.assertTrue(up.input_pressed(_Fighter(), ctx((0.0, 0.5))))
        self.assertFalse(up.input_pressed(_Fighter(), ctx((0.0, -0.5))))
        self.assertTrue(down.input_pressed(_Fighter(), ctx((0.0, -0.5))))
        self.assertFalse(down.input_pressed(_Fighter(), ctx((0.0, 0.5))))
        self.assertTrue(directional_b_input(ctx((0.0, 0.5)), "up", 1,
                                            "vertical_threshold", direction=1))
        self.assertFalse(directional_b_input(ctx((0.0, -0.5)), "up", 1,
                                             "vertical_threshold", direction=1))

    def test_repeated_b_is_consumed_without_restarting_active_action(self):
        move = OpenSpecial(SpecialRoot.NEUTRAL)
        fighter = _Fighter()
        context = _context()
        self.assertTrue(move.input_pressed(fighter, context))
        self.assertTrue(move.input_pressed(fighter, context))
        self.assertEqual(fighter.changes, [Action.SPECIAL_N_START])
        self.assertEqual(context.input.calls, 2)

    def test_active_repeat_is_consumed_before_resource_and_direction_gates(self):
        move = OpenSpecial(SpecialRoot.SIDE)
        fighter = _Fighter(move.ground)
        context = _context(resource=False, stick=(0.0, 0.0), rules=None)
        self.assertTrue(move.input_pressed(fighter, context))
        self.assertEqual(context.input.calls, 1)

        context = _context(resource=False, pressed=False, stick=(0.0, 0.0), rules=None)
        self.assertFalse(move.input_pressed(fighter, context))
        self.assertEqual(context.input.calls, 1)

    def test_reserved_direction_prefers_side_threshold_with_legacy_fallback(self):
        context = _context(rules=SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5, horizontal_threshold=0.9,
            vertical_threshold=0.8,
        )), stick=(0.5, 0.0))
        self.assertTrue(directional_b_reserved(context))

        legacy = _context(rules=SimpleNamespace(specials=SimpleNamespace(
            horizontal_threshold=0.5,
        )), stick=(0.5, 0.0))
        self.assertTrue(directional_b_reserved(legacy))


if __name__ == "__main__":
    unittest.main()
