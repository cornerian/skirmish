import math
import unittest

import skirmish._native as native
from fighter.compat import (Action, ActionDescriptor, MotionBinding, Parameters, action, clock, f32,
                            motion, parameter, resource, validation)


class DescriptorTests(unittest.TestCase):
    def test_action_uses_native_decoder_name(self):
        self.assertEqual(Action.SPECIAL_N_START.value, "special_n_start")
        self.assertEqual(Action.SPECIAL_AIR_LW.value, "special_air_lw")
        self.assertEqual(action(Action.SPECIAL_N_START).as_dict()["action"], "Action.SPECIAL_N_START")

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

    def test_parameters_realize_typed_class_defaults(self):
        class P(Parameters):
            speed: float = 2.5
            enabled: bool = True

        value = P(speed=3.0)
        self.assertEqual((value.speed, value.enabled), (3.0, True))

    def test_motion_constructor_preserves_positional_and_reference_args(self):
        descriptor = motion.gravity(parameter("fighter.accel"), -2.0, resource("move.delay"))
        self.assertEqual(descriptor.as_dict(), {
            "callee": "motion.gravity",
            "args": [parameter("fighter.accel").as_dict(), -2.0, resource("move.delay").as_dict()],
            "kwargs": {},
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
