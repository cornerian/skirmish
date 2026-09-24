"""Focused source/resource contract tests for Ganondorf's special script."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action, Button, DownSpecial, NeutralSpecial, SideSpecial, UpSpecial, export_definition
from fighters.captain import CaptainFalcon
from fighters.ganondorf import Ganondorf


class _Input:
    def __init__(self, stick):
        self.stick = stick

    def just_pressed(self, button):
        return button is Button.B


class _Fighter:
    def __init__(self, action=None):
        self.action = action
        self.action_state = Ganondorf.action_state()
        self.action_frame = 0
        self.grounded = False
        self.ground_velocity = 2.0
        self.velocity = [2.0, 3.0]
        self.facing = 1.0
        self.changes = []
        self.fall_special = []
        self.resource_value = object()

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def enter_fall_special(self, **kwargs):
        self.fall_special.append(kwargs)

    def resource(self, path):
        return self.resource_value


def _context(stick=(0.0, 0.0), *, resource_value=object(), ground_open=True, air_open=False):
    return SimpleNamespace(
        input=_Input(stick),
        resource=lambda path: resource_value,
        ground_open=ground_open,
        air_open=air_open,
        grounded=ground_open,
        rules=SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5,
            vertical_threshold=0.5,
        )),
    )


class GanondorfTests(unittest.TestCase):
    def test_export_keeps_ganon_identity_and_all_source_special_states(self):
        exported = export_definition(Ganondorf).as_dict()
        self.assertEqual(exported["name"], "ganondorf")
        self.assertEqual(exported["external_ids"], [25])
        expected = {
            "special.neutral.ground": (347, 301),
            "special.neutral.air": (348, 302),
            "special.side.ground_start": (349, 303),
            "special.side.ground": (350, 304),
            "special.side.air_start": (351, 305),
            "special.side.air": (352, 306),
            "special.up.ground": (353, 307),
            "special.up.air": (354, 308),
            "special.up.catch": (355, 309),
            "special.up.throw": (356, 310),
            "special.up.throw_rebound": (363, 317),
            "special.down.ground": (357, 311),
            "special.down.ground_end": (358, 312),
            "special.down.air": (359, 313),
            "special.down.landing": (360, 314),
            "special.down.air_end": (361, 316),
            "special.down.ground_end_air": (362, 315),
        }
        for action_name, (state, animation) in expected.items():
            action = exported["actions"][action_name]
            self.assertEqual(action["slippi_state"], state)
            self.assertEqual(action["animation"], animation)
            self.assertEqual(action["action"], f"Action.Source.25:{state}")

    def test_motion_table_matches_ftganon_source_order(self):
        exported = export_definition(Ganondorf).as_dict()
        states = sorted([
            phase["slippi_state"]
            for phase in exported["actions"].values()
            if isinstance(phase, dict) and 347 <= phase.get("slippi_state", -1) <= 363
        ])
        self.assertEqual(states, list(range(347, 363)) + [363])

    def test_each_special_has_ganon_resource_owner_and_source_attack_paths(self):
        exported = export_definition(Ganondorf).as_dict()
        specials = exported["movesets"]["specials"]
        for root in ("neutral", "side", "up", "down"):
            behavior = next(item for item in exported["behaviors"] if item["id"] == specials[root])
            self.assertEqual(behavior["resource"], root)
            self.assertTrue(all(
                "attack" in phase
                for phase in behavior["actions"].values()
                if phase["slippi_state"] in set(range(347, 355)) | set(range(357, 363))
            ))
        up = next(item for item in exported["behaviors"] if item["id"] == specials["up"])
        rebound = next(
            phase for phase in up["actions"].values() if phase["slippi_state"] == 363
        )
        self.assertNotIn("attack", rebound)
        for state in (355, 356):
            phase = next(
                item for item in up["actions"].values()
                if item["slippi_state"] == state
            )
            self.assertNotIn("attack", phase)

    def test_directional_resource_defaults_export_for_both_family_members(self):
        """Directional bases own the canonical resource roots for both fighters."""
        captain = export_definition(CaptainFalcon).as_dict()
        ganon = export_definition(Ganondorf).as_dict()
        for root in ("neutral", "side", "up", "down"):
            self.assertNotIn("resource", type(getattr(CaptainFalcon.specials, root)).__dict__)
            self.assertNotIn("resource", type(getattr(Ganondorf.specials, root)).__dict__)
            self.assertEqual(getattr(CaptainFalcon.specials, root).resource, root)
            self.assertEqual(getattr(Ganondorf.specials, root).resource, root)
            captain_id = captain["movesets"]["specials"][root]
            ganon_id = ganon["movesets"]["specials"][root]
            captain_behavior = next(item for item in captain["behaviors"] if item["id"] == captain_id)
            ganon_behavior = next(item for item in ganon["behaviors"] if item["id"] == ganon_id)
            self.assertEqual(captain_behavior["resource"], root)
            self.assertEqual(ganon_behavior["resource"], root)

    def test_directional_entries_are_resource_gated_and_partitioned(self):
        for root, stick, phase in (
            ("side", (0.5, 0.0), "ground_start"),
            ("up", (0.0, 0.5), "ground"),
            ("down", (0.0, -0.5), "ground"),
        ):
            move = getattr(Ganondorf.specials, root)
            fighter = _Fighter()
            self.assertTrue(move.input_pressed(fighter, _context(stick)))
            self.assertIs(fighter.action, getattr(move, phase))

            missing = _Fighter()
            self.assertFalse(move.input_pressed(
                missing, _context(stick, resource_value=None)
            ))

    def test_source_family_does_not_alias_captain_move_classes(self):
        from fighters.captain import FalconDive, FalconKick, FalconPunch, RaptorBoost
        from fighters.ganondorf import DarkDive, GerudoDragon, WarlockPunch, WizardFoot

        for ganon, captain in (
            (WarlockPunch, FalconPunch),
            (GerudoDragon, RaptorBoost),
            (DarkDive, FalconDive),
            (WizardFoot, FalconKick),
        ):
            self.assertIsNot(ganon, captain)
            self.assertFalse(issubclass(ganon, captain))

    def test_specials_use_directional_api_bases(self):
        self.assertIsInstance(Ganondorf.specials.neutral, NeutralSpecial)
        self.assertIsInstance(Ganondorf.specials.side, SideSpecial)
        self.assertIsInstance(Ganondorf.specials.up, UpSpecial)
        self.assertIsInstance(Ganondorf.specials.down, DownSpecial)

    def test_resolved_source_states_drive_terminal_and_surface_callbacks(self):
        # Export binds numeric authoring descriptors to the concrete Ganon
        # identity.  Runtime actions arrive in that qualified form, while the
        # shared family rules continue to compare by numeric source state.
        export_definition(Ganondorf)
        neutral = Ganondorf.specials.neutral
        fighter = _Fighter("Source.25:347")
        neutral._transition_animation_end(fighter, _context(ground_open=True))
        self.assertEqual(fighter.action, Action.WAIT)

        fighter = _Fighter("Source.25:348")
        neutral._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.25:347")
        for invalid in ("Source.24:347", "Source.347", "Action.Source.25:347"):
            fighter = _Fighter(invalid)
            neutral._transition_animation_end(fighter, _context())
            self.assertEqual(fighter.action, invalid)
            self.assertEqual(fighter.changes, [])

        side = Ganondorf.specials.side
        fighter = _Fighter("Source.25:349")
        side.animation_end_ground(fighter, _context(ground_open=True))
        self.assertEqual(fighter.action, Action.WAIT)
        fighter = _Fighter("Source.25:351")
        fighter.resource_value = SimpleNamespace(attributes=SimpleNamespace(
            specials_miss_landing_lag=3,
            specials_hit_landing_lag=4,
        ))
        self.assertTrue(side.landed(fighter, _context(resource_value=fighter.resource_value)))
        self.assertEqual(fighter.fall_special, [{"mobility": 1, "landing_lag": 3}])

        up = Ganondorf.specials.up
        fighter = _Fighter("Source.25:355")
        up._transition_animation_end(fighter, _context())
        self.assertEqual(fighter.action, "Source.25:356")
        fighter = _Fighter("Source.25:353")
        up._transition_ground_air(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, "Source.25:354")

        down = Ganondorf.specials.down
        fighter = _Fighter("Source.25:358")
        down._transition_animation_end(fighter, _context())
        self.assertEqual(fighter.action, Action.WAIT)
        fighter = _Fighter("Source.25:359")
        down._transition_animation_end(fighter, _context())
        self.assertEqual(fighter.action, "Source.25:361")

        # Canonical Captain actions remain unaffected by source binding.
        from fighters.captain import CaptainFalcon
        export_definition(CaptainFalcon)
        captain = CaptainFalcon.specials.neutral
        fighter = _Fighter(Action.SPECIAL_N_START)
        captain._transition_animation_end(fighter, _context())
        self.assertEqual(fighter.action, Action.WAIT)

    def test_warlock_punch_entry_clears_source_throw_latch(self):
        move = Ganondorf.specials.neutral
        fighter = _Fighter(move.ground)
        fighter.throw_flags = 7
        fighter.action_state.command = (9, 8, 7, 6)
        fighter.action_state.launch_armed = True
        move.enter(fighter, _context())
        self.assertEqual(fighter.throw_flags, 0)
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))
        self.assertFalse(fighter.action_state.launch_armed)

    def test_dark_dive_wall_rebound_finishes_into_fall(self):
        move = Ganondorf.specials.up
        fighter = _Fighter(move.throw_rebound)
        move.animation_end_rebound(fighter, _context())
        self.assertEqual(fighter.action, Action.FALL)
        self.assertEqual(fighter.changes, [(Action.FALL, {})])

    def test_dark_dive_air_landing_requires_release_cue(self):
        move = Ganondorf.specials.up
        attributes = SimpleNamespace(
            specialhi_freefall_air_spd_mul=0.8,
            specialhi_landing_lag=20,
        )
        fighter = _Fighter(move.air)
        context = _context(resource_value=SimpleNamespace(attributes=attributes))
        self.assertFalse(move.landed(fighter, context))
        self.assertEqual(fighter.fall_special, [])

        fighter.action_state.dive_released = True
        self.assertTrue(move.landed(fighter, context))
        self.assertEqual(
            fighter.fall_special,
            [{"mobility": 0.8, "landing_lag": 20}],
        )

    def test_wizard_foot_wall_rebound_requires_source_command_cue(self):
        move = Ganondorf.specials.down
        fighter = _Fighter(move.ground)
        self.assertFalse(move.wall_rebound(fighter, SimpleNamespace(wall=True)))
        self.assertEqual(fighter.changes, [])

        fighter.action_state.command = (1, 0, 0, 0)
        self.assertTrue(move.wall_rebound(fighter, SimpleNamespace(wall=True)))
        self.assertTrue(getattr(fighter.action, "reference", fighter.action).endswith(":363"))

    def test_wizard_foot_wall_rebound_requires_opposite_facing_wall(self):
        move = Ganondorf.specials.down

        same_side = _Fighter(move.ground)
        same_side.action_state.command = (1, 0, 0, 0)
        same_side.facing = 1.0
        self.assertFalse(move.wall_rebound(
            same_side,
            SimpleNamespace(wall=SimpleNamespace(normal=(1.0, 0.0))),
        ))
        self.assertEqual(same_side.changes, [])

        opposite_side = _Fighter(move.ground)
        opposite_side.action_state.command = (1, 0, 0, 0)
        opposite_side.facing = 1.0
        self.assertTrue(move.wall_rebound(
            opposite_side,
            SimpleNamespace(wall=SimpleNamespace(normal=(-1.0, 0.0))),
        ))
        self.assertTrue(getattr(opposite_side.action, "reference", opposite_side.action).endswith(":363"))


if __name__ == "__main__":
    unittest.main()
