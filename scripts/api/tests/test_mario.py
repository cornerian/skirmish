"""Focused source contracts for Mario's and Dr. Mario's special motions."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import (
    Action,
    AnimationEventId,
    ArticleId,
    Button,
    Fighter,
    b0_source_phases,
    export_definition,
    source_phase,
)
from fighters.dr_mario import DrMario, Megavitamin
from fighters.mario import Cape, Fireball, Mario, MarioTornado, SuperJumpPunch


class _Input:
    def __init__(self, pressed=(Button.B,)):
        self.pressed = set(pressed)

    def just_pressed(self, button):
        return button in self.pressed


class _Fighter:
    action = Action.WAIT
    facing = -1.0
    grounded = True

    def __init__(self, complete=()):
        self.action_state = SimpleNamespace(command=(7, 2, 3, 4))
        self.complete = set(complete)
        self.changes = []
        self.spawned = []

    def has_complete_animation(self, state):
        return state in self.complete

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def part_position(self, part):
        return (part, 1.0, 2.0)

    def spawn_article(self, *args):
        self.spawned.append(args)


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
            specials=SimpleNamespace(vertical_threshold=0.5, side_stick_threshold=0.5)
        )
    return context


class MarioNeutralTests(unittest.TestCase):
    def test_b0_phase_helper_keeps_native_values_explicit(self):
        ground, air = b0_source_phases(343, 344)
        self.assertEqual(
            (ground, air),
            (
                source_phase(343, animation=295, attack="neutral.start.ground"),
                source_phase(344, animation=296, attack="neutral.start.air"),
            ),
        )
        self.assertEqual((Fireball.ground_state, Fireball.air_state), (343, 344))

    def test_b0_phase_helper_requires_ground_and_air_pairs(self):
        with self.assertRaises(ValueError):
            b0_source_phases(
                343,
                344,
                animation=(295,),
                attack=("neutral.start.ground", "neutral.start.air"),
            )

    def test_source_actions_event_and_inherited_specials(self):
        for cls, move, article in (
            (Mario, Fireball, ArticleId.MARIO_FIRE),
            (DrMario, Megavitamin, ArticleId.DR_MARIO_VITAMIN),
        ):
            definition = export_definition(cls).as_dict()
            neutral = definition["behaviors"][0]
            self.assertEqual(
                [neutral["actions"][name]["slippi_state"] for name in ("ground", "air")],
                [343, 344],
            )
            event = next(item for item in neutral["callbacks"] if item["hook"] == "animation_event")
            self.assertEqual(event["event_id"], AnimationEventId.B0)
            self.assertEqual(event["actions"], [f"Source.{cls.external_ids[0]}:343", f"Source.{cls.external_ids[0]}:344"])
            self.assertEqual(move().article_id, article)
            self.assertIsNot(cls.specials.side, Fighter.specials.side)
            self.assertIsNot(cls.specials.up, Fighter.specials.up)
            self.assertIsNot(cls.specials.down, Fighter.specials.down)
            if cls is Mario:
                self.assertIsInstance(cls.specials.side, Cape)
                self.assertIsInstance(cls.specials.up, SuperJumpPunch)
                self.assertIsInstance(cls.specials.down, MarioTornado)

    def test_source_motion_table_covers_all_mario_special_states(self):
        definition = export_definition(Mario).as_dict()
        states = {
            name: action["slippi_state"]
            for name, action in definition["actions"].items()
            if name.startswith("special.")
        }
        self.assertEqual(
            {states[name] for name in states}, set(range(343, 351))
        )
        self.assertEqual(
            states,
            {
                "special.neutral.ground": 343,
                "special.neutral.air": 344,
                "special.side.ground": 345,
                "special.side.air": 346,
                "special.up.ground": 347,
                "special.up.air": 348,
                "special.down.ground": 349,
                "special.down.air": 350,
            },
        )

    def test_directional_specials_route_b_and_have_native_surface_exits(self):
        for move, stick in (
            (Mario.specials.side, (0.5, 0.0)),
            (Mario.specials.up, (0.0, 0.5)),
            (Mario.specials.down, (0.0, -0.5)),
        ):
            fighter = _Fighter()
            self.assertTrue(move.input_pressed(
                fighter, _context(
                    stick=stick, directional=True, resources=(move.resource,)
                )
            ))
            self.assertIs(fighter.action, move.ground)
            fighter.action = move.ground
            move._transition_animation_end(fighter, _context())
            self.assertIs(fighter.action, Action.WAIT)
            fighter.action = move.air
            move._transition_animation_end(fighter, _context(grounded=False))
            self.assertIs(fighter.action, Action.FALL)

    def test_directional_specials_require_their_native_resource_and_axis(self):
        cases = (
            (Mario.specials.side, (0.5, 0.0), (0.0, 0.5)),
            (Mario.specials.up, (0.0, 0.5), (0.5, 0.0)),
            (Mario.specials.down, (0.0, -0.5), (0.0, 0.5)),
        )
        for move, valid_stick, invalid_stick in cases:
            self.assertFalse(move.input_pressed(
                _Fighter(), _context(
                    stick=valid_stick, directional=True, resources=()
                )
            ))
            fighter = _Fighter()
            self.assertFalse(move.input_pressed(
                fighter, _context(
                    stick=invalid_stick,
                    directional=True,
                    resources=(move.resource,),
                )
            ))
            self.assertEqual(fighter.changes, [])

    def test_cape_reflects_only_eligible_projectiles(self):
        move = Cape()
        fighter = SimpleNamespace(flags=SimpleNamespace(reflecting=True))
        hit = SimpleNamespace(projectile=True, damage=8, max_damage=8, reflect=False)
        move.projectile_contact(fighter, hit)
        self.assertTrue(hit.reflect)

        for values in (
            dict(projectile=False, damage=8, max_damage=8),
            dict(projectile=True, damage=9, max_damage=8),
        ):
            rejected = SimpleNamespace(**values, reflect=False)
            move.projectile_contact(fighter, rejected)
            self.assertFalse(rejected.reflect)

    def test_cape_entry_resets_native_command_window(self):
        move = Cape()
        fighter = _Fighter()
        move.enter(fighter, _context())
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 4))

    def test_super_jump_punch_entry_clears_only_native_command_latch(self):
        move = SuperJumpPunch()
        fighter = _Fighter()
        move.enter(fighter, _context())
        self.assertEqual(fighter.action_state.command, (0, 2, 3, 4))

    def test_tornado_aerial_tap_consumes_native_command_cue(self):
        move = MarioTornado()
        fighter = _Fighter()
        fighter.action = move.air
        move.enter(fighter, _context(grounded=False))
        fighter.action_state.command = (0, 1, 0, 4)
        move.tap_command(
            fighter,
            SimpleNamespace(event=SimpleNamespace(value=1)),
        )
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 4))

    def test_entry_requires_resource_and_complete_animation_and_clears_slot(self):
        move = Fireball()
        fighter = _Fighter(complete=(343,))
        self.assertFalse(move.input_pressed(fighter, _context(resources=())))
        self.assertTrue(move.input_pressed(fighter, _context()))
        self.assertEqual(fighter.action, move.ground)

        move.enter(fighter, _context())
        self.assertEqual(fighter.action_state.command, (0, 2, 3, 4))

    def test_directional_b_is_reserved_by_shared_dispatch_rules(self):
        fighter = _Fighter(complete=(343,))
        self.assertFalse(
            Fireball().input_pressed(
                fighter,
                _context(stick=(0.0, 0.5), directional=True),
            )
        )
        self.assertEqual(fighter.action, Action.WAIT)

    def test_b0_uses_l1st_nb_pose_and_facing_owned_launch(self):
        move = Fireball()
        fighter = _Fighter()
        move.spawn(fighter, SimpleNamespace())
        self.assertEqual(fighter.spawned, [(ArticleId.MARIO_FIRE, (23, 1.0, 2.0), -1.0)])


if __name__ == "__main__":
    unittest.main()
