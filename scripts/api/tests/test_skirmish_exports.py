import sys
import unittest
from pathlib import Path


API = Path(__file__).parents[1]
if str(API) not in sys.path:
    sys.path.insert(0, str(API))

import fighter
import skirmish


class SkirmishExportTests(unittest.TestCase):
    def test_facade_reexports_every_canonical_fighter_public_name(self):
        for name in fighter.__all__:
            self.assertTrue(hasattr(skirmish, name), name)
            self.assertIs(getattr(skirmish, name), getattr(fighter, name), name)

    def test_facade_all_is_unique_and_resolves(self):
        self.assertEqual(len(skirmish.__all__), len(set(skirmish.__all__)))
        self.assertTrue(all(isinstance(name, str) and hasattr(skirmish, name)
                            for name in skirmish.__all__))

    def test_native_wait_token_is_available_from_public_facade(self):
        self.assertIs(skirmish.MoveWait, fighter.MoveWait)
        self.assertEqual(skirmish.MoveContext(None, None).wait(3).as_dict(), {
            "kind": "scheduled_deadline", "frames": 3,
        })


if __name__ == "__main__":
    unittest.main()
