import math
import unittest
from enum import IntEnum

import skirmish._native as native
from fighter.compat import (Action, ActionDescriptor, CommonParameter, CustomAction, MotionBinding, Parameters, SourceAction,
                            action, bind_source_action, clock, custom_action, f32, motion, parameter, resource,
                            resolve_source_action, source_action, source_phase, special_attribute, validation)
from fighter import DonkeyKongAttribute, SamusAttribute


class DescriptorTests(unittest.TestCase):
    def test_action_uses_native_decoder_name(self):
        self.assertEqual(Action.SPECIAL_N_START.value, "special_n_start")
        self.assertEqual(Action.SPECIAL_AIR_LW.value, "special_air_lw")
        self.assertEqual(action(Action.SPECIAL_N_START).as_dict()["action"], "Action.SPECIAL_N_START")
        self.assertEqual(action("SPECIAL_N_START").as_dict()["action"], "Action.SPECIAL_N_START")
        self.assertEqual(action("Action.SPECIAL_N_START").as_dict()["action"], "Action.SPECIAL_N_START")

    def test_f32_rounds_and_rejects_non_numbers(self):
        self.assertEqual(f32(1.0 / 3.0), 0.3333333432674408)
        self.assertEqual(f32(16777216) + f32(1) - f32(16777216), 0.0)
        self.assertEqual(math.copysign(1.0, f32(-0.0)), -1.0)
        self.assertTrue(math.isinf(f32(1) / f32(0)))
        self.assertTrue(math.isnan(f32(0) / f32(0)))
        self.assertTrue(math.isinf(f32(1e100)))
        with self.assertRaises(TypeError):
            f32(True)

    def test_f32_scalar_predicates_and_unary_operations(self):
        negative_zero = f32(-0.0)
        self.assertFalse(bool(negative_zero))
        self.assertEqual(-f32(1.5), -1.5)
        self.assertEqual(abs(-f32(1.5)), 1.5)
        self.assertTrue(f32(-1) < 0)
        self.assertTrue(f32(1) >= 1)
        self.assertEqual(f32(0) == negative_zero, True)
        self.assertFalse(f32(float("nan")) == f32(float("nan")))
        self.assertTrue(f32(float("nan")) != f32(float("nan")))
        self.assertEqual(math.copysign(1.0, f32(1) / f32(-0.0)), -1.0)

    def test_action_descriptor_matches_native_action_spelling(self):
        descriptor = action(Action.SPECIAL_N_START)
        self.assertEqual(descriptor, "special_n_start")
        self.assertEqual("special_n_start", descriptor)
        self.assertEqual(descriptor, Action.SPECIAL_N_START)
        self.assertEqual(descriptor, type("Phase", (), {"action": "Action.SPECIAL_N_START"})())

    def test_custom_action_descriptor_exports_qualified_canonical_reference(self):
        custom = custom_action("fighter.training", "special.phase_a")
        self.assertIsInstance(custom, CustomAction)
        self.assertEqual(custom.reference, "Custom.fighter.training:special.phase_a")
        self.assertEqual(
            action(custom, animation_loop=True).as_dict(),
            {
                "action": "Action.Custom.fighter.training:special.phase_a",
                "animation_loop": True,
            },
        )

    def test_source_action_is_numeric_until_concrete_roster_export(self):
        source = source_action(381)
        self.assertIsInstance(source, SourceAction)
        self.assertEqual(source.reference, "Source.381")
        self.assertEqual(resolve_source_action(source.reference, (1,)), "Source.1:381")
        self.assertEqual(resolve_source_action(source.reference, (15,)), "Source.15:381")
        with self.assertRaises(ValueError):
            resolve_source_action(source.reference, ())

    def test_source_phase_reuses_numeric_state_for_action_and_metadata(self):
        phase = source_phase(381, animation=295, attack="neutral.start.ground")
        self.assertEqual(
            phase.as_dict(),
            {
                "action": "Action.Source.381",
                "animation_loop": False,
                "slippi_state": 381,
                "animation": 295,
                "attack": "neutral.start.ground",
            },
        )

    def test_clock_action_references_do_not_double_prefix_wire_names(self):
        self.assertEqual(
            clock(field="frames", actions=(action("Action.SPECIAL_N_START"),)).as_dict(),
            {"field": "frames", "actions": ["Action.SPECIAL_N_START"]},
        )
        self.assertEqual(
            clock(field="frames", actions=(source_phase(381),)).as_dict(),
            {"field": "frames", "actions": ["Source.381"]},
        )

    def test_clock_preserves_prefixed_string_action_references(self):
        self.assertEqual(
            clock(field="frames", actions=(action("Custom.ca:phase"),)).as_dict(),
            {"field": "frames", "actions": ["Custom.ca:phase"]},
        )
        bound = bind_source_action(source_phase(381), (0,))
        self.assertEqual(
            clock(field="frames", actions=(bound,)).as_dict(),
            {"field": "frames", "actions": ["Source.0:381"]},
        )

    def test_native_action_member_comparison_is_symmetric(self):
        previous_get = native._get
        try:
            def get(_token, path):
                if path == "fighter.action":
                    return "SpecialNStart"
                if path == "value.label":
                    return "Special_N_Start"
                raise AssertionError(f"unexpected native lookup: {path}")

            native._get = get
            member = native.NativeObject(1, "fighter", "fighter").action
            same = action(Action.SPECIAL_N_START)
            different = action(Action.SPECIAL_N_END)
            self.assertTrue(member == same)
            self.assertTrue(same == member)
            self.assertFalse(member != same)
            self.assertFalse(same != member)
            self.assertTrue(member != different)
            self.assertTrue(different != member)
            text = native.NativeObject(1, "value", "value").label
            self.assertFalse(text == "special_n_start")
            self.assertTrue(text != "special_n_start")
        finally:
            native._get = previous_get

    def test_complete_animation_query_is_typed_and_numeric(self):
        previous_get = native._get
        previous_call = native._call
        try:
            calls = []

            def call(_token, path, args):
                calls.append((path, args))
                return True

            native._get = lambda _token, _path: None
            native._call = call
            fighter = native.NativeObject(1, "fighter", "fighter")
            self.assertTrue(fighter.has_complete_animation(381))
            self.assertEqual(calls, [("fighter.has_complete_animation", [381])])
        finally:
            native._get = previous_get
            native._call = previous_call

    def test_special_attribute_ids_are_typed_and_layout_scoped(self):
        self.assertEqual(DonkeyKongAttribute.SPECIAL_HI_LANDING_LAG.field_id, 6)
        self.assertEqual(DonkeyKongAttribute.SPECIAL_HI_LANDING_LAG.layout, 3)
        self.assertEqual(int(DonkeyKongAttribute.SPECIAL_HI_LANDING_LAG), 0x30006)
        self.assertEqual(SamusAttribute.SCREW_ATTACK_LANDING_LAG.layout, 4)

    def test_special_attribute_reference_preserves_typed_wire_shape(self):
        self.assertEqual(
            special_attribute(DonkeyKongAttribute.SPECIAL_HI_LANDING_LAG).as_dict(),
            {
                "callee": "special_attribute",
                "args": [{"layout": 3, "field_id": 6}],
                "kwargs": {},
            },
        )

    def test_gravity_multiplier_uses_typed_scalar_reference(self):
        descriptor = motion.gravity_multiplier(
            index=0,
            value=0,
            multiplier=special_attribute(DonkeyKongAttribute.SPECIAL_HI_AERIAL_GRAVITY),
        )
        self.assertEqual(
            descriptor.as_dict()["kwargs"]["multiplier"],
            {
                "callee": "special_attribute",
                "args": [{"layout": 3, "field_id": 1}],
                "kwargs": {},
            },
        )

    def test_parameters_realize_typed_class_defaults(self):
        class P(Parameters):
            speed: float = 2.5
            enabled: bool = True

        value = P(speed=3.0)
        self.assertEqual((value.speed, value.enabled), (3.0, True))

    def test_common_parameter_paths_are_typed_but_wire_compatible(self):
        path = CommonParameter.TERMINAL_VELOCITY
        self.assertEqual(path, "movement.terminal_velocity")
        self.assertEqual(str(path), "movement.terminal_velocity")
        self.assertEqual(
            parameter(path).as_dict(),
            {"callee": "parameter", "args": ["movement.terminal_velocity"], "kwargs": {}},
        )
        self.assertEqual(
            resource(CommonParameter.GRAVITY).as_dict(),
            {"callee": "resource", "args": ["movement.gravity"], "kwargs": {}},
        )
        values = {"movement.gravity": 0.1}
        self.assertEqual(values[CommonParameter.GRAVITY], 0.1)

    def test_motion_constructor_preserves_positional_and_reference_args(self):
        descriptor = motion.gravity(parameter("fighter.accel"), -2.0, resource("move.delay"))
        self.assertEqual(descriptor.as_dict(), {
            "callee": "motion.gravity",
            "args": [parameter("fighter.accel").as_dict(), -2.0, resource("move.delay").as_dict()],
            "kwargs": {},
        })

    def test_command_branch_stringifies_cases_and_preserves_operation_order(self):
        descriptor = motion.command_branch(
            index=1,
            cases={
                0: (motion.gravity(0.25, 9.0, 0.0), motion.friction(0.2)),
                1: (motion.command_velocity_scale(index=1, value=1, multiplier=0.5),),
            },
        )
        self.assertEqual(descriptor.as_dict(), {
            "callee": "motion.command_branch",
            "args": [],
            "kwargs": {
                "index": 1,
                "cases": {
                    "0": [motion.gravity(0.25, 9.0, 0.0).as_dict(), motion.friction(0.2).as_dict()],
                    "1": [motion.command_velocity_scale(index=1, value=1, multiplier=0.5).as_dict()],
                },
            },
        })

    def test_mapping_enum_keys_export_their_values_as_strings(self):
        class Case(IntEnum):
            ZERO = 0
            ONE = 1

        descriptor = motion.command_branch(cases={Case.ZERO: ("zero",), Case.ONE: ("one",)})
        self.assertEqual(descriptor.as_dict()["kwargs"]["cases"], {
            "0": ["zero"],
            "1": ["one"],
        })

    def test_binding_and_clock_are_separate_declarations(self):
        binding = MotionBinding(-1, 0.5, 0.25, 1)
        self.assertEqual(binding.as_dict(), {"facing": -1.0, "cosine": 0.5, "sine": 0.25, "ground_scale": 1.0})
        self.assertEqual(clock(field="frames", actions=(action(Action.WAIT),)).as_dict()["field"], "frames")

    def test_validation_matches_native_numeric_bounds(self):
        self.assertTrue(validation.finite(-1))
        self.assertFalse(validation.finite(math.inf))
        self.assertFalse(validation.finite(math.nan))
        self.assertTrue(validation.number(0.5, True))
        self.assertFalse(validation.number(-0.5, True))
        self.assertFalse(validation.number(1_000_001))

    def test_fields_require_and_check_named_values(self):
        class Values:
            speed = 2.0
            lag = 0.0

        self.assertTrue(validation.fields(Values(), finite=("speed",), nonnegative=("lag",)))
        self.assertFalse(validation.fields(Values(), positive=("lag",)))


if __name__ == "__main__":
    unittest.main()
