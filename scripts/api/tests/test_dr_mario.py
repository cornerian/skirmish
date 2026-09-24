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
        self.velocity = (4.0, 3.0)
        self.action_state = type("State", (), {"command": (7, 6, 5, 4)})()
        self.flags = type("Flags", (), {"reflecting": True})()

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def set_velocity(self, horizontal, vertical):
        self.velocity = (horizontal, vertical)

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

    def test_cape_entry_applies_source_ground_and_air_velocity_rules(self):
        move = DrMario.specials.side
        attributes = type("Attributes", (), {"vel_x_decay": 2.0})()
        context = type("Context", (), {
            "resource": lambda self, path: type(
                "Resource", (), {"attributes": attributes}
            )(),
        })()

        ground = _Fighter()
        ground.action = move.ground
        move.enter(ground, context)
        self.assertEqual(ground.velocity, (4.0, 0.0))

        air = _Fighter()
        air.action = move.air
        move.enter(air, context)
        self.assertEqual(air.velocity, (2.0, 3.0))

    def test_cape_command_one_controls_reflection_window(self):
        move = DrMario.specials.side
        fighter = _Fighter()
        fighter.flags.reflecting = False

        class Event:
            value = 1

        Event.value = 2
        move.reflect_command(fighter, type("Context", (), {"event": Event()})())
        self.assertFalse(fighter.flags.reflecting)
        Event.value = 1
        move.reflect_command(fighter, type("Context", (), {"event": Event()})())
        self.assertTrue(fighter.flags.reflecting)
        Event.value = 2
        move.reflect_command(fighter, type("Context", (), {"event": Event()})())
        self.assertTrue(fighter.flags.reflecting)
        Event.value = 0
        move.reflect_command(fighter, type("Context", (), {"event": Event()})())
        self.assertFalse(fighter.flags.reflecting)

    def test_cape_exit_clears_stale_reflection_but_surface_transfer_preserves_it(self):
        move = DrMario.specials.side
        fighter = _Fighter()
        fighter.flags.reflecting = True
        fighter.action = move.air
        move.exit(fighter, object())
        self.assertTrue(fighter.flags.reflecting)

        fighter.action = Action.WAIT
        move.exit(fighter, object())
        self.assertFalse(fighter.flags.reflecting)

    def test_up_entry_clears_command_zero_and_throw_flags(self):
        fighter = _Fighter()
        fighter.throw_flags = 7
        move = DrMario.specials.up
        move.enter(fighter, object())
        self.assertEqual(fighter.action_state.command, (0, 6, 5, 4))
        self.assertEqual(fighter.throw_flags, 0)

    def test_up_air_entry_applies_source_horizontal_scale_and_clears_vertical_velocity(self):
        fighter = _Fighter()
        fighter.action = DrMario.specials.up.air
        attributes = type("Attributes", (), {"specialhi_vel_x": 0.5})()
        context = type("Context", (), {
            "resource": lambda self, path: type(
                "Resource", (), {"attributes": attributes}
            )(),
        })()

        DrMario.specials.up.enter(fighter, context)

        self.assertEqual(fighter.velocity, (2.0, 0.0))

    def test_up_animation_end_enters_source_fall_special(self):
        move = DrMario.specials.up
        calls = []
        fighter = type("FallSpecialFighter", (), {
            "enter_fall_special": lambda self, **kwargs: calls.append(kwargs),
        })()
        attributes = type("Attributes", (), {
            "specialhi_freefall_air_spd_mul": 0.8,
            "specialhi_landing_lag": 12.0,
        })()
        context = type("Context", (), {
            "resource": lambda self, path: type(
                "Resource", (), {"attributes": attributes}
            )(),
        })()

        self.assertTrue(move.enter_fall_special(fighter, context))
        self.assertEqual(calls, [{"mobility": 0.8, "landing_lag": 12.0}])

    def test_down_entry_clears_tornado_commands_and_aerial_tap_latches_charge(self):
        fighter = _Fighter()
        move = DrMario.specials.down
        move.enter(fighter, object())
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 4))

        class Event:
            value = 1

        ctx = type("Context", (), {"event": Event()})()
        fighter.action_state.command = (0, 1, 5, 4)
        move.tap_command(fighter, ctx)
        self.assertEqual(fighter.action_state.command, (0, 0, 5, 4))
        self.assertTrue(fighter.action_state.tornado_charge)

    def test_down_animation_end_consumes_late_aerial_tap(self):
        fighter = _Fighter()
        move = DrMario.specials.down
        fighter.action = move.air
        fighter.action_state.command = (0, 1, 5, 4)

        self.assertFalse(move.finish_air(fighter, object()))
        self.assertEqual(fighter.action_state.command, (0, 0, 5, 4))
        self.assertTrue(fighter.action_state.tornado_charge)

    def test_down_aerial_completion_with_tap_enters_authored_fall_special(self):
        move = DrMario.specials.down
        calls = []
        fighter = _Fighter()
        fighter.action = move.air
        fighter.action_state.command = (0, 1, 5, 4)
        fighter.enter_fall_special = lambda **kwargs: calls.append(kwargs)
        attributes = type("Attributes", (), {"speciallw_landing_lag": 18})()
        context = type("Context", (), {
            "resource": lambda self, path: type(
                "Resource", (), {"attributes": attributes}
            )(),
        })()

        self.assertTrue(move.finish_air(fighter, context))
        self.assertEqual(fighter.action_state.command, (0, 0, 5, 4))
        self.assertTrue(fighter.action_state.tornado_charge)
        self.assertEqual(calls, [{"mobility": 1, "landing_lag": 18}])

    def test_down_aerial_completion_with_tap_keeps_plain_fall_at_zero_lag(self):
        move = DrMario.specials.down
        fighter = _Fighter()
        fighter.action = move.air
        fighter.action_state.command = (0, 1, 5, 4)
        fighter.enter_fall_special = lambda **kwargs: self.fail(
            "zero landing lag must use ordinary Fall"
        )
        attributes = type("Attributes", (), {"speciallw_landing_lag": 0})()
        context = type("Context", (), {
            "resource": lambda self, path: type(
                "Resource", (), {"attributes": attributes}
            )(),
        })()

        self.assertFalse(move.finish_air(fighter, context))
        self.assertEqual(fighter.action_state.command, (0, 0, 5, 4))
        self.assertTrue(fighter.action_state.tornado_charge)

    def test_down_aerial_completion_enters_authored_fall_special(self):
        move = DrMario.specials.down
        calls = []
        fighter = _Fighter()
        fighter.action = move.air
        fighter.enter_fall_special = lambda **kwargs: calls.append(kwargs)
        attributes = type("Attributes", (), {"speciallw_landing_lag": 18})()
        context = type("Context", (), {
            "resource": lambda self, path: type(
                "Resource", (), {"attributes": attributes}
            )(),
        })()

        self.assertTrue(move.finish_air(fighter, context))
        self.assertEqual(calls, [{"mobility": 1, "landing_lag": 18}])

    def test_down_aerial_completion_keeps_plain_fall_when_lag_is_zero(self):
        move = DrMario.specials.down
        fighter = _Fighter()
        fighter.action = move.air
        fighter.action_state.command = (0, 0, 5, 4)
        fighter.enter_fall_special = lambda **kwargs: self.fail(
            "zero landing lag must use ordinary Fall"
        )
        attributes = type("Attributes", (), {"speciallw_landing_lag": 0})()
        context = type("Context", (), {
            "resource": lambda self, path: type(
                "Resource", (), {"attributes": attributes}
            )(),
        })()

        self.assertFalse(move.finish_air(fighter, context))


if __name__ == "__main__":
    unittest.main()
