"""Focused source-contract tests for Jigglypuff's special phases."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))

from fighter import (
    Action,
    Button,
    JigglypuffPoundAttribute,
    JigglypuffRolloutAttribute,
    export_definition,
    f32,
)
from fighters.jigglypuff import Jigglypuff, Pound, Rest, Roll, Sing


class _Input:
    def __init__(self, pressed=()):
        self.pressed = set(pressed)

    def just_pressed(self, button):
        return button in self.pressed


class _Fighter:
    def __init__(self, action=Action.WAIT, *, facing=1.0, complete=()):
        self.action = action
        self.action_frame = 0
        self.facing = facing
        self.grounded = True
        self.complete = set(complete)
        self.changes = []

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def has_complete_animation(self, state):
        return state in self.complete


class _PoundFighter(_Fighter):
    def __init__(self, action=Action.WAIT, *, facing=1.0, complete=(), attributes=None):
        super().__init__(action, facing=facing, complete=complete)
        self.action_state = SimpleNamespace(command=(1, 7, 8, 9))
        self.velocity = None
        self.attributes = attributes or {}

    def special_attribute(self, attribute):
        return self.attributes.get(attribute)

    def set_velocity(self, x, y):
        self.velocity = (x, y)


def _context(*, grounded=True, pressed=(Button.B,)):
    return SimpleNamespace(
        ground_open=grounded,
        air_open=not grounded,
        input=_Input(pressed),
    )


def _pound_context(*, grounded=True, stick=(1.0, 0.0), resources=()):
    available = set(resources)
    return SimpleNamespace(
        ground_open=grounded,
        air_open=not grounded,
        input=SimpleNamespace(
            stick=stick,
            just_pressed=lambda button: button is Button.B,
        ),
        rules=SimpleNamespace(
            specials=SimpleNamespace(side_stick_threshold=0.5),
        ),
        resource=lambda path: object() if path in available else None,
    )


class JigglypuffTests(unittest.TestCase):
    def test_roll_declares_complete_native_phase_family(self):
        exported = export_definition(Jigglypuff).as_dict()
        states = {
            record["slippi_state"]
            for key, record in exported["actions"].items()
            if key.startswith("special.neutral.")
        }
        self.assertEqual(states, set(range(346, 363)))
        for state in (348, 349, 356, 357):
            phase = next(
                record for record in exported["actions"].values()
                if record.get("slippi_state") == state
            )
            self.assertTrue(phase["animation_loop"])
        neutral = next(item for item in exported["behaviors"] if item["resource"] == "neutral")
        wall = next(item for item in neutral["callbacks"] if item["callback"].endswith("wall_bounce"))
        self.assertEqual(wall["hook"], "surface_contact")
        self.assertEqual(wall["actions"], ["Source.15:350", "Source.15:358"])

    def test_roll_requires_neutral_b_and_selected_complete_start(self):
        roll = Roll()
        rules = SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5, vertical_threshold=0.5,
        ))
        ctx = SimpleNamespace(
            ground_open=True,
            air_open=False,
            input=SimpleNamespace(
                stick=(0.0, 0.0),
                just_pressed=lambda button: button is Button.B,
            ),
            rules=rules,
        )
        blocked = _Fighter(facing=-1.0)
        self.assertFalse(roll.input_pressed(blocked, ctx))
        fighter = _Fighter(facing=-1.0, complete=(346,))
        self.assertTrue(roll.input_pressed(fighter, ctx))
        self.assertEqual(fighter.action, Roll.ground_start_left)

        ctx.input.stick = (1.0, 0.0)
        self.assertFalse(roll.input_pressed(_Fighter(complete=(347,)), ctx))

    def test_roll_entry_clears_all_native_command_slots(self):
        roll = Roll()
        fighter = _PoundFighter(facing=-1.0, complete=(346,))
        ctx = SimpleNamespace(
            ground_open=True,
            air_open=False,
            input=SimpleNamespace(
                stick=(0.0, 0.0),
                just_pressed=lambda button: button is Button.B,
            ),
            rules=SimpleNamespace(
                specials=SimpleNamespace(side_stick_threshold=0.5, vertical_threshold=0.5),
            ),
        )
        self.assertTrue(roll.input_pressed(fighter, ctx))
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))

    def test_roll_release_and_animation_callbacks_follow_native_exits(self):
        roll = Roll()
        fighter = _Fighter(Roll.ground_loop)
        roll.release(fighter, None)
        self.assertEqual(fighter.action, Roll.ground_release)
        self.assertEqual(
            fighter.changes[-1][1], {"preserve_state": True, "keep_frame": True}
        )

        fighter.facing = -1.0
        roll.rolling_end(fighter, None)
        self.assertEqual(fighter.action, Roll.ground_end_left)
        roll.terminal_end(fighter, None)
        self.assertEqual(fighter.action, Action.WAIT)

        fighter = _Fighter(Roll.air_start_right)
        roll.start_end(fighter, None)
        self.assertEqual(fighter.action, Roll.air_loop)

        fighter = _Fighter(Roll.air_release)
        roll.after_roll_hit(fighter, None)
        self.assertEqual(fighter.action, Roll.hit)
        fighter.grounded = False
        roll.hit_end(fighter, None)
        self.assertEqual(fighter.action, Action.FALL)

        fighter = _Fighter(Roll.ground_release, facing=1.0)
        fighter.velocity = (3.0, 0.5)
        fighter.ground_velocity = 3.0
        fighter.set_velocity = lambda x, y: setattr(fighter, "velocity", (x, y))
        self.assertTrue(roll.wall_bounce(fighter, SimpleNamespace(wall=object())))
        self.assertEqual(fighter.velocity, (-3.0, 0.5))
        self.assertEqual(fighter.ground_velocity, -3.0)
        self.assertEqual(fighter.facing, -1.0)

    def test_roll_wall_callback_is_exported_and_tolerates_unmodeled_charge(self):
        roll = Roll()
        exported = export_definition(Jigglypuff).as_dict()
        neutral = next(item for item in exported["behaviors"] if item["resource"] == "neutral")
        wall = next(item for item in neutral["callbacks"] if item["callback"].endswith("wall_bounce"))
        self.assertEqual(wall["hook"], "surface_contact")
        fighter = _Fighter(Roll.ground_release)
        fighter.velocity = (2.0, 0.0)
        fighter.ground_velocity = 2.0
        fighter.action_state = SimpleNamespace()
        fighter.set_velocity = lambda x, y: setattr(fighter, "velocity", (x, y))
        self.assertTrue(roll.wall_bounce(fighter, SimpleNamespace(wall=object())))
        self.assertEqual(fighter.velocity, (-2.0, 0.0))
        self.assertFalse(hasattr(fighter.action_state, "charge"))

        landed = _Fighter(Roll.hit)
        self.assertTrue(roll.hit_landed(landed, SimpleNamespace(grounded=True)))
        self.assertEqual(landed.action, Action.WAIT)

        landed = _Fighter(Roll.hit)
        landed.grounded = False
        self.assertTrue(roll.hit_landed(landed, SimpleNamespace(grounded=False)))
        self.assertEqual(landed.action, Action.FALL)

    def test_roll_ground_release_reverses_only_at_opposite_stick_threshold(self):
        roll = Roll()
        ctx = SimpleNamespace(
            input=SimpleNamespace(stick=(-0.49, 0.0)),
            rules=SimpleNamespace(
                specials=SimpleNamespace(side_stick_threshold=0.5),
            ),
        )
        fighter = _PoundFighter(
            Roll.ground_release,
            facing=1.0,
            attributes={JigglypuffRolloutAttribute.TURN_STICK_THRESHOLD: f32(0.5)},
        )
        self.assertFalse(roll.reverse(fighter, ctx))
        self.assertEqual(fighter.action, Roll.ground_release)

        ctx.input.stick = (-0.5, 0.0)
        self.assertFalse(roll.reverse(fighter, ctx))
        self.assertEqual(fighter.action, Roll.ground_release)

        ctx.input.stick = (-0.51, 0.0)
        self.assertTrue(roll.reverse(fighter, ctx))
        self.assertEqual(fighter.action, Roll.ground_turn)
        self.assertEqual(fighter.facing, -1.0)
        self.assertEqual(
            fighter.changes[-1][1], {"preserve_state": True, "keep_frame": True}
        )

    def test_roll_turn_uses_native_attribute_instead_of_generic_rule_threshold(self):
        roll = Roll()
        ctx = SimpleNamespace(input=SimpleNamespace(stick=(-0.4, 0.0)))
        fighter = _PoundFighter(
            Roll.ground_release,
            attributes={JigglypuffRolloutAttribute.TURN_STICK_THRESHOLD: f32(0.3)},
        )
        self.assertTrue(roll.reverse(fighter, ctx))
        self.assertEqual(fighter.action, Roll.ground_turn)

    def test_pound_declares_source_states_traces_and_command_branches(self):
        exported = export_definition(Jigglypuff).as_dict()
        ground = exported["actions"]["special.side.ground"]
        air = exported["actions"]["special.side.air"]
        self.assertEqual(ground["slippi_state"], 363)
        self.assertEqual(air["slippi_state"], 364)
        self.assertEqual(ground["command_trace"], "side.script.ground")
        self.assertEqual(air["command_trace"], "side.script.air")
        ground_motion = ground["motion"]["kwargs"]["ground"][0]
        self.assertEqual(ground_motion, {
            "callee": "motion.ground_friction_above_walk",
            "args": [],
            "kwargs": {},
        })

        branch = air["motion"]["kwargs"]["air"][0]
        self.assertEqual(branch["callee"], "motion.command_branch")
        self.assertEqual(branch["kwargs"]["index"], 1)
        cases = branch["kwargs"]["cases"]
        self.assertEqual(set(cases), {"0", "1", "2"})
        self.assertEqual(cases["1"][0]["callee"], "motion.command_velocity_scale")
        self.assertEqual(
            cases["1"][0]["kwargs"]["multiplier"]["args"][0],
            {"layout": JigglypuffPoundAttribute.VELOCITY_MULTIPLIER.layout,
             "field_id": JigglypuffPoundAttribute.VELOCITY_MULTIPLIER.field_id},
        )
        self.assertEqual(cases["2"][1]["callee"], "motion.drift_or_friction")

    def test_pound_input_requires_side_direction_and_complete_native_phase(self):
        pound = Pound()
        for grounded, state, phase in (
            (True, 363, Pound.ground),
            (False, 364, Pound.air),
        ):
            blocked = _PoundFighter(facing=-1.0)
            self.assertFalse(pound.input_pressed(blocked, _pound_context(grounded=grounded)))
            self.assertFalse(blocked.changes)

            wrong_direction = _PoundFighter(complete=(state,))
            self.assertFalse(
                pound.input_pressed(
                    wrong_direction,
                    _pound_context(grounded=grounded, stick=(0.0, 1.0)),
                )
            )

            fighter = _PoundFighter(complete=(state,))
            self.assertTrue(pound.input_pressed(fighter, _pound_context(grounded=grounded)))
            self.assertEqual(fighter.action, phase)
            self.assertEqual(fighter.action_frame, 1)

    def test_pound_command_zero_launch_matches_source_grouping_and_clears_slot(self):
        attributes = {
            JigglypuffPoundAttribute.STICK_ANGLE_MIN: f32(0.25),
            JigglypuffPoundAttribute.STICK_ANGLE_MAX: f32(0.75),
            JigglypuffPoundAttribute.MAX_LAUNCH_ANGLE: f32(60.0),
            JigglypuffPoundAttribute.LAUNCH_SPEED: f32(3.1),
        }
        fighter = _PoundFighter(Pound.air, facing=-1.0, attributes=attributes)
        ctx = _pound_context(grounded=False, stick=(0.0, f32(-0.5)))
        ctx.event = SimpleNamespace(value=1)
        with patch("fighters.jigglypuff.velocity_from_angle", return_value=(2.0, -3.0)) as launch:
            Pound().launch(fighter, ctx)

        speed, angle, facing = launch.call_args.args
        expected_angle = f32(0.01745329238474369) * (
            f32(-0.25) * f32(60.0) / (f32(0.75) - f32(0.25))
        )
        self.assertEqual((speed, angle, facing), (f32(3.1), expected_angle, -1.0))
        self.assertEqual(fighter.velocity, (2.0, -3.0))
        self.assertEqual(fighter.action_state.command, (0, 7, 8, 9))

    def test_pound_entry_clears_all_native_command_slots(self):
        pound = Pound()
        fighter = _PoundFighter(complete=(363,))
        self.assertTrue(pound.input_pressed(fighter, _pound_context()))
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))

    def test_pound_command_zero_ignores_zero_event_or_incomplete_attributes(self):
        fighter = _PoundFighter(Pound.air)
        ctx = _pound_context(grounded=False)
        ctx.event = SimpleNamespace(value=0)
        with patch("fighters.jigglypuff.velocity_from_angle") as launch:
            Pound().launch(fighter, ctx)
        launch.assert_not_called()
        self.assertEqual(fighter.action_state.command, (1, 7, 8, 9))

    def test_pound_surface_transitions_preserve_phase_and_terminal_destinations(self):
        pound = Pound()
        for source, grounded, target in (
            (Pound.air, True, Pound.ground),
            (Pound.ground, False, Pound.air),
        ):
            fighter = _PoundFighter(source)
            pound._transition_ground_air(fighter, SimpleNamespace(grounded=grounded))
            self.assertEqual(
                fighter.changes,
                [(target, {"preserve_state": True, "keep_frame": True})],
            )
        for source, target in ((Pound.ground, Action.WAIT), (Pound.air, Action.FALL)):
            fighter = _PoundFighter(source)
            pound._transition_animation_end(fighter, None)
            self.assertEqual(fighter.changes[0][0], target)

    def test_pound_validation_requires_both_command_traces(self):
        pound = Pound()
        self.assertFalse(pound.validate(_pound_context(resources=("side.script.ground",))))
        self.assertTrue(
            pound.validate(
                _pound_context(resources=("side.script.ground", "side.script.air"))
            )
        )

    def test_rest_declares_exact_native_states_without_guessed_resources(self):
        phases = (Rest.ground_left, Rest.air_left, Rest.ground_right, Rest.air_right)
        self.assertEqual(
            [phase.as_dict()["slippi_state"] for phase in phases],
            [369, 370, 371, 372],
        )
        self.assertTrue(all("animation" not in phase.as_dict() for phase in phases))
        self.assertTrue(all("attack" not in phase.as_dict() for phase in phases))

    def test_rest_enters_only_with_the_selected_complete_animation(self):
        rest = Rest()
        for grounded, facing, state, expected in (
            (True, -1.0, 369, Rest.ground_left),
            (True, 1.0, 371, Rest.ground_right),
            (False, -1.0, 370, Rest.air_left),
            (False, 1.0, 372, Rest.air_right),
        ):
            blocked = _Fighter(facing=facing)
            self.assertFalse(rest.input_pressed(blocked, _context(grounded=grounded)))
            self.assertFalse(blocked.changes)

            fighter = _Fighter(facing=facing, complete=(state,))
            self.assertTrue(rest.input_pressed(fighter, _context(grounded=grounded)))
            self.assertEqual(fighter.action, expected)
            self.assertEqual(fighter.action_frame, 1)

    def test_rest_entry_clears_native_command_slot_zero(self):
        fighter = _Fighter(facing=-1.0, complete=(369,))
        fighter.action_state = SimpleNamespace(command=(7, 8, 9, 10))
        self.assertTrue(Rest().input_pressed(fighter, _context()))
        self.assertEqual(fighter.action_state.command, (0, 8, 9, 10))

    def test_rest_ground_air_and_terminal_callbacks_reach_native_destinations(self):
        rest = Rest()

        for source, grounded, target in (
            (Rest.ground_left, False, Rest.air_left),
            (Rest.ground_right, False, Rest.air_right),
            (Rest.air_left, True, Rest.ground_left),
            (Rest.air_right, True, Rest.ground_right),
        ):
            fighter = _Fighter(source)
            rest._transition_ground_air(fighter, SimpleNamespace(grounded=grounded))
            self.assertEqual(
                fighter.changes,
                [(target, {"preserve_state": True, "keep_frame": True})],
            )

        for phase, target in (
            (Rest.ground_left, Action.WAIT),
            (Rest.ground_right, Action.WAIT),
            (Rest.air_left, Action.FALL),
            (Rest.air_right, Action.FALL),
        ):
            fighter = _Fighter(phase)
            rest._transition_animation_end(fighter, None)
            self.assertEqual(fighter.changes[0][0], target)

    def test_registered_definition_contains_all_rest_states(self):
        exported = export_definition(Jigglypuff).as_dict()
        states = {
            record["slippi_state"]
            for key, record in exported["actions"].items()
            if key.startswith("special.down.")
        }
        self.assertEqual(states, {369, 370, 371, 372})

    def test_sing_declares_exact_native_states_without_hit_capsule_data(self):
        phases = (Sing.ground_left, Sing.air_left, Sing.ground_right, Sing.air_right)
        self.assertEqual(
            [phase.as_dict()["slippi_state"] for phase in phases],
            [365, 366, 367, 368],
        )
        self.assertTrue(all("attack" not in phase.as_dict() for phase in phases))

    def test_sing_requires_upward_input_and_selected_complete_animation(self):
        sing = Sing()
        rules = SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5))
        context = SimpleNamespace(
            ground_open=True,
            air_open=False,
            input=SimpleNamespace(
                stick=(0.0, 1.0),
                just_pressed=lambda button: button is Button.B,
            ),
            rules=rules,
        )

        blocked = _Fighter(facing=-1.0)
        self.assertFalse(sing.input_pressed(blocked, context))
        self.assertFalse(blocked.changes)

        fighter = _Fighter(facing=-1.0, complete=(365,))
        self.assertTrue(sing.input_pressed(fighter, context))
        self.assertEqual(fighter.action, Sing.ground_left)

        context.input.stick = (0.0, -1.0)
        fighter = _Fighter(complete=(365,))
        self.assertFalse(sing.input_pressed(fighter, context))

    def test_sing_entry_clears_native_command_slot_zero(self):
        fighter = _Fighter(facing=-1.0, complete=(365,))
        fighter.action_state = SimpleNamespace(command=(7, 8, 9, 10))
        context = SimpleNamespace(
            ground_open=True,
            air_open=False,
            input=SimpleNamespace(
                stick=(0.0, 1.0),
                just_pressed=lambda button: button is Button.B,
            ),
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5)),
        )
        self.assertTrue(Sing().input_pressed(fighter, context))
        self.assertEqual(fighter.action_state.command, (0, 8, 9, 10))

    def test_sing_ground_air_and_terminal_callbacks_match_source(self):
        sing = Sing()
        for source, grounded, target in (
            (Sing.ground_left, False, Sing.air_left),
            (Sing.ground_right, False, Sing.air_right),
            (Sing.air_left, True, Sing.ground_left),
            (Sing.air_right, True, Sing.ground_right),
        ):
            fighter = _Fighter(source)
            sing._transition_ground_air(fighter, SimpleNamespace(grounded=grounded))
            self.assertEqual(
                fighter.changes,
                [(target, {"preserve_state": True, "keep_frame": True})],
            )

        for phase, target in (
            (Sing.ground_left, Action.WAIT),
            (Sing.ground_right, Action.WAIT),
            (Sing.air_left, Action.FALL),
            (Sing.air_right, Action.FALL),
        ):
            fighter = _Fighter(phase)
            sing._transition_animation_end(fighter, None)
            self.assertEqual(fighter.changes[0][0], target)

    def test_registered_definition_contains_all_sing_states(self):
        exported = export_definition(Jigglypuff).as_dict()
        states = {
            record["slippi_state"]
            for record in exported["actions"].values()
            if "slippi_state" in record
        }
        self.assertTrue({365, 366, 367, 368}.issubset(states))


if __name__ == "__main__":
    unittest.main()
