import unittest
from unittest.mock import patch

from fighter import f32, math


class MathSurfaceTests(unittest.TestCase):
    def test_velocity_from_angle_preserves_source_grouping_and_binary32(self):
        # These values make the source grouping differ from the associative
        # rewrite by one binary32 ULP.
        speed, angle, facing = 44126.3125, 0.75, 14338.759765625
        cosine_value, sine_value = f32(0.151302427), f32(0.25)
        with patch.object(math, "cos", return_value=cosine_value) as cosine, \
                patch.object(math, "sin", return_value=sine_value) as sine:
            velocity = math.velocity_from_angle(speed, angle, facing)
        cosine.assert_called_once_with(f32(angle))
        sine.assert_called_once_with(f32(angle))
        self.assertEqual(
            tuple(float(value) for value in velocity),
            (float(f32(facing) * (f32(speed) * cosine_value)),
             float(f32(speed) * sine_value)),
        )
        self.assertTrue(all(type(value).__name__ == "_F32" for value in velocity))

    def test_velocity_from_angle_propagates_nonfinite_ieee_values(self):
        with patch.object(math, "cos", return_value=f32(1.0)), \
                patch.object(math, "sin", return_value=f32(0.0)):
            velocity = math.velocity_from_angle(float("inf"), 0.0, 1.0)
        self.assertTrue(float(velocity[0]) == float("inf"))
        self.assertTrue(float(velocity[1]) != float(velocity[1]))
        with patch.object(math, "cos", return_value=float("inf")), \
                patch.object(math, "sin", return_value=0.0):
            velocity = math.velocity_from_angle(1.0, 0.0, 1.0)
        self.assertTrue(float(velocity[0]) == float("inf"))

    def test_constants_are_binary32_and_have_source_spelling(self):
        self.assertEqual(float(math.PI), 3.1415927410125732)
        self.assertEqual(float(math.HALF_PI), 1.5707963705062866)
        self.assertEqual(float(math.DEG_TO_RAD), 0.01745329238474369)
        self.assertEqual(math.pi, math.PI)
        self.assertEqual(math.half_pi, math.HALF_PI)
        self.assertEqual(math.deg_to_rad, math.DEG_TO_RAD)

    def test_native_functions_fail_closed_outside_pon(self):
        with self.assertRaisesRegex(RuntimeError, "only available inside the Pon host"):
            math.sin(0.0)
        with self.assertRaisesRegex(RuntimeError, "only available inside the Pon host"):
            math.cos(0.0)
        with self.assertRaisesRegex(RuntimeError, "only available inside the Pon host"):
            math.atan2(0.0, 1.0)
        with self.assertRaisesRegex(RuntimeError, "only available inside the Pon host"):
            math.angle_xy((1.0, 0.0, 0.0), (1.0, 0.0))
        with self.assertRaisesRegex(RuntimeError, "only available inside the Pon host"):
            math.facing(1.0)


if __name__ == "__main__":
    unittest.main()
