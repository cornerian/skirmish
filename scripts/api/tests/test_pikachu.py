"""Regression checks for Pikachu's native ``ftPikachu`` fighter contract."""

import re
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))

from fighter import export_definition
from fighters.pikachu import (
    Pikachu,
    QuickAttack,
    SOURCE_MOTION_STATES,
    advance_quick_attack_animation,
    advance_quick_attack_hold,
    quick_attack_effect_offset,
    skull_bash_command_transition,
    thunder_jolt_spawn_position,
)


def _pinned_melee_root() -> Path:
    """Locate the pinned decomp beside either the old or nested checkout."""
    for ancestor in (ROOT, *ROOT.parents):
        candidate = ancestor / "External" / "melee"
        if (candidate / "src" / "melee").is_dir():
            return candidate
    raise FileNotFoundError("could not locate pinned External/melee checkout")


class PikachuScriptTests(unittest.TestCase):
    def test_source_index_covers_every_ftpikachu_special_state(self):
        states = [state for _, state in SOURCE_MOTION_STATES]
        self.assertEqual(states, list(range(341, 367)))

        source = (
            _pinned_melee_root()
            / "src"
            / "melee"
            / "ft"
            / "kinds"
            / "ftPikachu"
            / "ftpikachu.c"
        ).read_text()
        table_states = [
            int(value)
            for value in re.findall(r"ftPk_MS_[^=]+\s*=\s*(\d+)", source)
            if 341 <= int(value) <= 366
        ]
        self.assertEqual(table_states, states)

    def test_export_preserves_native_phase_identity_and_lifecycle(self):
        Pikachu.external_ids = (13,)
        definition = export_definition(Pikachu).as_dict()
        actions = definition["actions"]

        expected = {
            "special.neutral.ground": 341,
            "special.neutral.air": 342,
            "special.side.ground_start": 343,
            "special.side.ground_hold": 344,
            "special.side.ground_travel": 345,
            "special.side.ground_end": 346,
            "special.side.ground_dash": 347,
            "special.side.air_start": 348,
            "special.side.air_hold": 349,
            "special.side.air_travel": 350,
            "special.side.air_end": 351,
            "special.side.air_dash": 352,
            "special.up.ground_start": 353,
            "special.up.ground_move": 354,
            "special.up.ground_end": 355,
            "special.up.air_start": 356,
            "special.up.air_move": 357,
            "special.up.air_end": 358,
            "special.down.ground_start": 359,
            "special.down.ground_loop": 360,
            "special.down.ground_hit": 361,
            "special.down.ground_end": 362,
            "special.down.air_start": 363,
            "special.down.air_loop": 364,
            "special.down.air_hit": 365,
            "special.down.air_end": 366,
        }
        self.assertEqual(
            {name: actions[name]["slippi_state"] for name in expected}, expected
        )

        callbacks = {
            callback["callback"]
            for behavior in definition["behaviors"]
            for callback in behavior["callbacks"]
        }
        self.assertTrue(any(name.endswith("._transition_animation_end") for name in callbacks))
        self.assertTrue(any(name.endswith("._transition_ground_air") for name in callbacks))

    def test_quick_attack_animation_and_hold_branches_match_source(self):
        class Fighter:
            def __init__(self, action):
                self.action = action
                self.changes = []

            def change_action(self, action):
                self.changes.append(action)
                self.action = action

        for source, destination in (
            (QuickAttack.ground_start, QuickAttack.ground_hold),
            (QuickAttack.air_start, QuickAttack.air_hold),
            (QuickAttack.air_travel, QuickAttack.air_end),
        ):
            fighter = Fighter(source)
            self.assertTrue(advance_quick_attack_animation(fighter))
            self.assertEqual(fighter.changes, [destination])

        fighter = Fighter(QuickAttack.ground_dash)
        self.assertFalse(advance_quick_attack_animation(fighter))
        self.assertEqual(fighter.changes, [])

        fighter = Fighter(QuickAttack.ground_hold)
        self.assertFalse(advance_quick_attack_hold(fighter, frames_held=4, hold_limit=4))
        self.assertTrue(advance_quick_attack_hold(fighter, frames_held=5, hold_limit=4))
        self.assertEqual(fighter.changes, [QuickAttack.ground_dash])

        fighter = Fighter(QuickAttack.air_hold)
        self.assertTrue(advance_quick_attack_hold(fighter, frames_held=6, hold_limit=5))
        self.assertEqual(fighter.changes, [QuickAttack.air_dash])

    def test_skull_bash_command_ends_ground_and_air_loops(self):
        from fighters.pikachu import Thunder

        for source, destination in (
            (Thunder.ground_loop, Thunder.ground_end),
            (Thunder.ground_hit, Thunder.ground_end),
            (Thunder.air_loop, Thunder.air_end),
            (Thunder.air_hit, Thunder.air_end),
        ):
            self.assertIsNone(skull_bash_command_transition(source, 0))
            self.assertEqual(skull_bash_command_transition(source, 1), destination)
        self.assertIsNone(skull_bash_command_transition(Thunder.ground_end, 1))

    def test_quick_attack_effect_matches_source_jitter_and_pichu_gate(self):
        self.assertEqual(
            quick_attack_effect_offset(0.0, 1.0),
            (-3.0, 3.0),
        )
        self.assertEqual(
            quick_attack_effect_offset(0.0, 1.0, aerial=True),
            (-5.0, 5.0),
        )
        self.assertEqual(
            quick_attack_effect_offset(0.9, 0.1, terminal=True),
            (0.0, 0.0),
        )
        self.assertIsNone(quick_attack_effect_offset(0.5, 0.5, pichu=True))

    def test_thunder_jolt_spawn_position_matches_source_scaling(self):
        self.assertEqual(
            thunder_jolt_spawn_position((10, 5, 9), (3, 2), -1, 2),
            (4.0, 9.0, 0.0),
        )

if __name__ == "__main__":
    unittest.main()
