"""Compatibility contracts for the legacy ``skirmish`` import paths."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))

import fighter  # noqa: E402
import skirmish.api as legacy_api  # noqa: E402
import skirmish.events as legacy_events  # noqa: E402


class SkirmishCompatibilityTests(unittest.TestCase):
    def test_api_reexports_the_complete_canonical_surface(self):
        for name in fighter.__all__:
            self.assertTrue(hasattr(legacy_api, name), name)
            self.assertIs(getattr(legacy_api, name), getattr(fighter, name), name)
        self.assertEqual(set(legacy_api.__all__), set(fighter.__all__))

    def test_events_reexport_shared_singletons_and_types(self):
        for name in legacy_events.__all__:
            self.assertIs(getattr(legacy_events, name), getattr(fighter, name), name)

    def test_legacy_decorator_records_current_event_metadata(self):
        @legacy_events.hook.input_pressed(legacy_api.Button.B)
        def callback(fighter_instance, context):
            return fighter_instance, context

        binding = callback.__fighter_events__[0]
        self.assertIs(binding.hook, legacy_api.Hook.INPUT_PRESSED)
        self.assertEqual(binding.buttons, 0x200)
        self.assertEqual(binding.as_dict(), {
            "hook": "input_pressed",
            "callback": "callback",
            "buttons": 0x200,
        })


if __name__ == "__main__":
    unittest.main()
