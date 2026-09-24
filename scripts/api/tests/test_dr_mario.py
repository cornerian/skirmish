"""Source contracts for Dr. Mario's Mario-family special motion table."""

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action, Button, Fighter, HitContext, export_definition
from fighters.dr_mario import DrMario, DrTornado, SuperJumpPunch, SuperSheet


class _Input:
    def __init__(self, stick=(0.0, 0.0)):
        self.stick = stick

    def just_pressed(self, button):
        return button is Button.B


class _Fighter:
    action = Action.WAIT
    facing = -1.0

    def __init__(self):
        self.changes = []
        self.spawned = []
        self.action_state = type("State", (), {"command": (7, 6, 5, 4)})()
        self.flags = type("Flags", (), {"reflecting": True})()

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def part_position(self, part):
        return (part, 1.0, 2.0)

    def spawn_article(self, *args):
        self.spawned.append(args)


def _context(root, stick):
    return type("Context", (), {
        "ground_open": True,
        "air_open": False,
        "input": _Input(stick),
        "resource": lambda self, path: object() if path == root else None,
        "rules": type("Rules", (), {
            "specials": type("Specials", (), {
                "side_stick_threshold": 0.5,
                "vertical_threshold": 0.5,
            })()
        })(),
    })()


class DrMarioSpecialTests(unittest.TestCase):
    def test_native_special_states_are_declared(self):
        definition = export_definition(DrMario).as_dict()
        expected = {
            "neutral": (343, 344),
            "side": (345, 346),
            "up": (347, 348),
            "down": (349, 350),
        }
        for root, states in expected.items():
            behavior_id = definition["movesets"]["specials"][root]
            behavior = next(
                item for item in definition["behaviors"] if item["id"] == behavior_id
            )
            self.assertEqual(
                [behavior["actions"][name]["slippi_state"] for name in ("ground", "air")],
                list(states),
            )

    def test_directional_specials_route_to_ground_phase(self):
        for move, stick in (
            (DrMario.specials.side, (1.0, 0.0)),
            (DrMario.specials.up, (0.0, 1.0)),
            (DrMario.specials.down, (0.0, -1.0)),
        ):
            fighter = _Fighter()
            self.assertTrue(move.input_pressed(fighter, _context(move.root.value, stick)))
            self.assertEqual(fighter.action, move.ground)

    def test_specials_preserve_surface_and_terminal_behavior(self):
        for move in (DrMario.specials.side, DrMario.specials.up, DrMario.specials.down):
            self.assertEqual(move.on_ground[move.air].target, move.ground)
            self.assertEqual(move.on_air[move.ground].target, move.air)
            self.assertEqual(move.on_end[move.ground].target, Action.WAIT)
            self.assertEqual(move.on_end[move.air].target, Action.FALL)

    def test_vitamin_spawn_uses_typed_article_and_l1st_nb_pose(self):
        fighter = _Fighter()
        DrMario.specials.neutral.spawn(fighter, object())
        from fighter import ArticleId, FighterPart

        self.assertEqual(
            fighter.spawned,
            [(ArticleId.DR_MARIO_VITAMIN, (FighterPart.L1ST_NB, 1.0, 2.0), -1.0)],
        )

    def test_vitamin_entry_clears_command_zero_and_throw_flags(self):
        fighter = _Fighter()
        fighter.throw_flags = 7
        move = DrMario.specials.neutral
        fighter.action = move.ground
        move.enter(fighter, object())
        self.assertEqual(fighter.action_state.command, (0, 6, 5, 4))
        self.assertEqual(fighter.throw_flags, 0)

    def test_cape_entry_resets_commands_and_reflects_eligible_projectiles(self):
        move = DrMario.specials.side
        fighter = _Fighter()
        fighter.action = move.ground
        move.enter(fighter, object())
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 4))
        self.assertFalse(fighter.flags.reflecting)

        hit = HitContext(projectile=True, damage=3.0, max_damage=4.0, reflect=False)
        fighter.flags.reflecting = True
        move.projectile_contact(fighter, hit)
        self.assertTrue(hit.reflect)

        blocked = HitContext(projectile=True, damage=5.0, max_damage=4.0, reflect=False)
        move.projectile_contact(fighter, blocked)
        self.assertFalse(blocked.reflect)


if __name__ == "__main__":
    unittest.main()
