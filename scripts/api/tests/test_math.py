import unittest

from fighter import math


class MathSurfaceTests(unittest.TestCase):
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
