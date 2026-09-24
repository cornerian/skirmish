"""Regression contract for Ganondorf's source state 363 callback binding."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import export_definition
from fighters.ganondorf import Ganondorf


class GanondorfReboundBindingTests(unittest.TestCase):
    def test_rebound_animation_callback_is_scoped_to_source_state_363(self):
        exported = export_definition(Ganondorf).as_dict()
        behavior_id = exported["movesets"]["specials"]["up"]
        behavior = next(item for item in exported["behaviors"] if item["id"] == behavior_id)
        callbacks = [
            callback
            for callback in behavior["callbacks"]
            if callback["callback"].endswith("animation_end_rebound")
        ]
        self.assertEqual(len(callbacks), 1)
        self.assertEqual(callbacks[0]["actions"], ["Source.25:363"])


if __name__ == "__main__":
    unittest.main()
