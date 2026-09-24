"""Authoring metadata for exact multi-button input selectors."""

from __future__ import annotations

import sys
from pathlib import Path

import unittest

ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))

from fighter.compat import Button
from fighter.events import on


class ButtonChordTests(unittest.TestCase):
    def test_exact_mode(self):
        @on.input_pressed(Button.A, Button.B, require_all_buttons=True)
        def callback(_fighter, _ctx):
            return None

        binding = callback.__fighter_events__[0]
        self.assertEqual(binding.buttons, 0x300)
        self.assertTrue(binding.buttons_all)
        self.assertTrue(binding.as_dict()["buttons_all"])

    def test_default_mode(self):
        @on.input_pressed(Button.A, Button.B)
        def callback(_fighter, _ctx):
            return None

        binding = callback.__fighter_events__[0]
        self.assertEqual(binding.buttons, 0x300)
        self.assertFalse(binding.buttons_all)
        self.assertNotIn("buttons_all", binding.as_dict())

    def test_exact_mode_requires_a_button_set(self):
        with self.assertRaisesRegex(ValueError, "at least one button"):
            on.input_pressed(require_all_buttons=True)(lambda _fighter, _ctx: None)


if __name__ == "__main__":
    unittest.main()
