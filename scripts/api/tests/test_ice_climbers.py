"""Source/resource contracts for Popo's Ice Climbers special script."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action, Button, export_definition
from fighters.ice_climbers import Belay, Blizzard, IceClimbers, IceShot, SquallHammer


class _Input:
    def __init__(self, stick):
        self.stick = stick

    def just_pressed(self, button):
        return button is Button.B


class _Fighter:
    def __init__(self, action=None):
        self.action = action
        self.action_frame = 0
        self.changes = []
        self.action_state = SimpleNamespace(command=(7, 6, 5, 4))

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


class _Event:
    def __init__(self, value):
        self.value = value


def _context(stick=(0.0, 0.0), *, resource_value=object(), grounded=True):
    return SimpleNamespace(
        input=_Input(stick),
        resource=lambda path: resource_value,
        ground_open=grounded,
        air_open=not grounded,
        grounded=grounded,
        rules=SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5,
            vertical_threshold=0.5,
        )),
    )


class IceClimbersTests(unittest.TestCase):
    def test_export_binds_popo_source_states_and_keeps_nana_states_out(self):
        exported = export_definition(IceClimbers).as_dict()
        self.assertEqual(exported["name"], "ice-climbers")
        self.assertEqual(exported["external_ids"], [14])

        actions = exported["actions"]
        states = {
            action["slippi_state"]
            for name, action in actions.items()
            if name.startswith("special.")
        }
        self.assertEqual(states, set(range(341, 359)))
        self.assertFalse(states & set(range(359, 367)))
        for name, action in actions.items():
            if name.startswith("special."):
                self.assertEqual(action["action"], f"Action.Source.14:{action['slippi_state']}")

    def test_special_roots_own_resources_and_source_phase_ranges(self):
        exported = export_definition(IceClimbers).as_dict()
        specials = exported["movesets"]["specials"]
        expected = {
            "neutral": (IceShot, {341, 342}),
            "side": (SquallHammer, {343, 344, 345, 346}),
            "up": (Belay, set(range(347, 357))),
            "down": (Blizzard, {357, 358}),
        }
        for root, (move_type, states) in expected.items():
            move = getattr(IceClimbers.specials, root)
            self.assertIsInstance(move, move_type)
            self.assertEqual(move.resource, root)
            behavior = next(item for item in exported["behaviors"] if item["id"] == specials[root])
            self.assertEqual(
                {phase["slippi_state"] for phase in behavior["actions"].values()},
                states,
            )

    def test_special_input_is_resource_gated_and_directional(self):
        for root, stick in (
            ("neutral", (0.0, 0.0)),
            ("side", (0.5, 0.0)),
            ("up", (0.0, 0.5)),
            ("down", (0.0, -0.5)),
        ):
            move = getattr(IceClimbers.specials, root)
            fighter = _Fighter()
            self.assertTrue(move.input_pressed(fighter, _context(stick)), root)
            self.assertIs(fighter.action, move.ground, root)

            missing = _Fighter()
            self.assertFalse(
                move.input_pressed(missing, _context(stick, resource_value=None)),
                root,
            )

    def test_source_lifecycle_transitions_preserve_surface_phase(self):
        export_definition(IceClimbers)
        neutral = IceClimbers.specials.neutral
        fighter = _Fighter("Source.14:341")
        neutral._transition_animation_end(fighter, _context())
        self.assertEqual(fighter.action, Action.WAIT)

        side = IceClimbers.specials.side
        fighter = _Fighter("Source.14:346")
        side._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.14:344")

        up = IceClimbers.specials.up
        fighter = _Fighter("Source.14:347")
        up._transition_animation_end(fighter, _context())
        self.assertEqual(fighter.action, "Source.14:348")
        for state, expected in ((354, 349), (356, 351)):
            fighter = _Fighter(f"Source.14:{state}")
            up._transition_ground_air(fighter, SimpleNamespace(grounded=True))
            self.assertEqual(fighter.action, f"Source.14:{expected}")

    def test_belay_source_rows_follow_ground_air_pairs(self):
        export_definition(IceClimbers)
        up = IceClimbers.specials.up
        for ground_state, air_state in (
            (347, 352), (348, 353), (349, 354), (350, 355), (351, 356)
        ):
            fighter = _Fighter(f"Source.14:{air_state}")
            up._transition_ground_air(fighter, SimpleNamespace(grounded=True))
            self.assertEqual(fighter.action, f"Source.14:{ground_state}")
            fighter = _Fighter(f"Source.14:{ground_state}")
            up._transition_ground_air(fighter, SimpleNamespace(grounded=False))
            self.assertEqual(fighter.action, f"Source.14:{air_state}")

        down = IceClimbers.specials.down
        fighter = _Fighter("Source.14:358")
        down._transition_animation_end(fighter, _context(grounded=False))
        self.assertEqual(fighter.action, Action.FALL)

    def test_ice_shot_entry_clears_only_source_command_slot(self):
        export_definition(IceClimbers)
        move = IceClimbers.specials.neutral
        for phase in (move.ground, move.air):
            fighter = _Fighter(phase)
            move.enter(fighter, _context())
            self.assertEqual(fighter.action_state.command, (0, 6, 5, 4))

    def test_belay_partner_command_branches_require_native_partner_facts(self):
        export_definition(IceClimbers)
        up = IceClimbers.specials.up

        # Command 2 alone is insufficient: the source checks Nana's range
        # before selecting the alternate start row.
        fighter = _Fighter("Source.14:347")
        up.partner_fallback(fighter, SimpleNamespace(event=_Event(1)))
        self.assertEqual(fighter.action, "Source.14:347")

        fighter = _Fighter("Source.14:347")
        up.partner_fallback(
            fighter, SimpleNamespace(event=_Event(1), partner_available=True)
        )
        self.assertEqual(fighter.action, "Source.14:347")

        fighter = _Fighter("Source.14:347")
        up.partner_fallback(
            fighter, SimpleNamespace(event=_Event(1), partner_available=False)
        )
        self.assertEqual(fighter.action.action, "Source.14:350")
        self.assertEqual(fighter.changes[-1][1], {"preserve_state": True, "keep_frame": True})

        # ftPp_SpecialHi_8012280C selects state 354 for either throw row when
        # the native partner launch condition is observed.
        fighter = _Fighter("Source.14:348")
        up.partner_launch(fighter, SimpleNamespace(event=_Event(1)))
        self.assertEqual(fighter.action, "Source.14:348")

        fighter = _Fighter("Source.14:348")
        up.partner_launch(
            fighter, SimpleNamespace(event=_Event(1), partner_launching=False)
        )
        self.assertEqual(fighter.action, "Source.14:348")

        fighter = _Fighter("Source.14:348")
        up.partner_launch(
            fighter, SimpleNamespace(event=_Event(1), partner_launching=True)
        )
        self.assertEqual(fighter.action.action, "Source.14:354")

    def test_belay_exports_native_command_branch_hooks(self):
        definition = export_definition(IceClimbers).as_dict()
        behavior_id = definition["movesets"]["specials"]["up"]
        behavior = next(item for item in definition["behaviors"] if item["id"] == behavior_id)
        command_hooks = {
            (callback["command_index"], tuple(callback["actions"]))
            for callback in behavior["callbacks"]
            if callback["hook"] == "command_trace_changed"
        }
        self.assertIn((2, ("Source.14:347", "Source.14:352")), command_hooks)
        self.assertIn((1, ("Source.14:348", "Source.14:353")), command_hooks)

    def test_squall_wall_contact_stays_in_the_active_source_row(self):
        """The source reverses velocity on walls; it does not change S1/S2.

        ``ftPp_SpecialS1_Coll`` and ``ftPp_SpecialS2_Coll`` both keep their
        current motion state.  The native collision host owns the velocity
        rebound and Nana synchronization, so the Python declaration must not
        invent a wall-driven action transition.
        """
        export_definition(IceClimbers)
        side = IceClimbers.specials.side
        definition = export_definition(IceClimbers).as_dict()
        behavior_id = definition["movesets"]["specials"]["side"]
        behavior = next(
            item for item in definition["behaviors"] if item["id"] == behavior_id
        )
        self.assertFalse(any(
            callback["hook"] == "surface_contact"
            for callback in behavior["callbacks"]
        ))
        for source in (343, 344, 345, 346):
            fighter = _Fighter(f"Source.14:{source}")
            self.assertEqual(fighter.action, f"Source.14:{source}")


if __name__ == "__main__":
    unittest.main()
