"""Focused source-state and transition tests for Peach's specials."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import (  # noqa: E402
    Action,
    Button,
    DownSpecial,
    NeutralSpecial,
    SideSpecial,
    UpSpecial,
    export_definition,
)
from fighters.peach import (  # noqa: E402
    Peach,
    PeachDownSpecial,
    PeachNeutralSpecial,
    PeachSideSpecial,
    PeachUpSpecial,
)


class _Input:
    def __init__(self, stick):
        self.stick = stick

    def just_pressed(self, button):
        return button is Button.B


def _context(*, stick=(0.0, 0.0), ground_open=True, air_open=False, resource_value=object()):
    return SimpleNamespace(
        input=_Input(stick),
        resource=lambda path: resource_value,
        ground_open=ground_open,
        air_open=air_open,
        rules=SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5,
            vertical_threshold=0.5,
        )),
    )


class _Fighter:
    action = None
    grounded = True

    def __init__(self):
        self.changes = []
        self.action_state = SimpleNamespace(side_blocked=False)

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action


class PeachSpecialTests(unittest.TestCase):
    def test_source_states_and_identity_are_native(self):
        exported = export_definition(Peach).as_dict()
        self.assertEqual(exported["name"], "peach")
        self.assertEqual(exported["external_ids"], [12])
        states = {
            name: phase["slippi_state"]
            for name, phase in exported["actions"].items()
            if name.startswith("special.")
        }
        self.assertEqual(
            set(states.values()), set(range(352, 369))
        )
        self.assertTrue(all(
            phase["action"] == f"Action.Source.12:{phase['slippi_state']}"
            for phase in exported["actions"].values()
            if phase.get("slippi_state") is not None
        ))

    def test_special_roots_use_typed_directional_bases(self):
        self.assertIsInstance(Peach.specials.neutral, NeutralSpecial)
        self.assertIsInstance(Peach.specials.side, SideSpecial)
        self.assertIsInstance(Peach.specials.up, UpSpecial)
        self.assertIsInstance(Peach.specials.down, DownSpecial)
        self.assertEqual(
            [move.resource for move in (
                Peach.specials.neutral,
                Peach.specials.side,
                Peach.specials.up,
                Peach.specials.down,
            )],
            ["neutral", "side", "up", "down"],
        )

    def test_entries_are_resource_gated_and_select_ground_or_air_phase(self):
        sticks = ((0.0, 0.0), (0.5, 0.0), (0.0, 0.5), (0.0, -0.5))
        for move, stick in zip((
            Peach.specials.neutral,
            Peach.specials.side,
            Peach.specials.up,
            Peach.specials.down,
        ), sticks):
            fighter = _Fighter()
            self.assertTrue(move.input_pressed(fighter, _context(stick=stick)))
            self.assertIs(fighter.action, move.ground)

            fighter = _Fighter()
            self.assertTrue(move.input_pressed(
                fighter, _context(stick=stick, ground_open=False, air_open=True)
            ))
            self.assertIs(fighter.action, move.air)

            fighter = _Fighter()
            self.assertFalse(move.input_pressed(
                fighter, _context(stick=stick, resource_value=None)
            ))
            self.assertEqual(fighter.changes, [])

    def test_source_phases_have_safe_terminals_and_surface_pairs(self):
        neutral = Peach.specials.neutral
        fighter = _Fighter()
        fighter.action = neutral.ground
        neutral._transition_animation_end(fighter, _context())
        self.assertIs(fighter.action, Action.WAIT)

        down = Peach.specials.down
        fighter = _Fighter()
        fighter.action = down.air
        down._transition_animation_end(fighter, _context(ground_open=False, air_open=True))
        self.assertIs(fighter.action, Action.FALL)

        side = Peach.specials.side
        fighter = _Fighter()
        fighter.action = side.air_start
        side._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, side.ground_start)
        self.assertEqual(fighter.changes[-1][1], {
            "preserve_state": True,
            "keep_frame": True,
        })

        up = Peach.specials.up
        fighter = _Fighter()
        fighter.action = up.ground
        up._transition_animation_end(fighter, _context())
        # ftPe_SpecialHiStart_Anim (361) enters common FallSpecial directly;
        # state 362 is reached by the landing collision callback instead.
        self.assertIs(fighter.action, Action.FALL)

        fighter = _Fighter()
        fighter.action = up.ground_end
        up._transition_animation_end(fighter, _context())
        # ftPe_SpecialHiEnd_Anim calls ftCo_80096900, entering FallSpecial
        # after the grounded end motion completes.
        self.assertIs(fighter.action, Action.SPECIAL_HI_FALL)

    def test_native_command_branches_only_select_wall_end_phase(self):
        neutral = Peach.specials.neutral
        fighter = _Fighter()
        fighter.action = neutral.ground
        # Source cmd_vars[1] arms the native shield callback. The hit phase
        # is entered only when that callback reports contact.
        self.assertFalse(any(
            action.callback == "accessory_hit"
            for action in neutral.events()
        ))

        side = Peach.specials.side
        fighter = _Fighter()
        fighter.action = side.air_jump
        side.wall_end(
            fighter, SimpleNamespace(event=SimpleNamespace(value=1))
        )
        self.assertIs(fighter.action, side.air_end1)


    def test_side_air_jump_ends_on_animation_completion(self):
        side = Peach.specials.side
        fighter = _Fighter()
        fighter.action = side.air_jump

        side._transition_animation_end(fighter, SimpleNamespace())

        self.assertEqual(fighter.action, side.air_end0.action)

    def test_special_entries_clear_native_command_windows(self):
        for move, action, expected in (
            (Peach.specials.neutral, Peach.specials.neutral.ground, (0, 0, 0, 0)),
            (Peach.specials.side, Peach.specials.side.ground_start, (0, 0, 0, 0)),
            (Peach.specials.up, Peach.specials.up.ground, (0, 0, 0, 8)),
        ):
            fighter = _Fighter()
            fighter.action_state.command = (7, 6, 5, 8)
            fighter.action = action

            move.reset_command_window(fighter, SimpleNamespace())

            self.assertEqual(fighter.action_state.command, expected)

    def test_side_start_waits_for_animation_end_then_selects_block_branch(self):
        side = Peach.specials.side

        fighter = _Fighter()
        fighter.action = side.ground_start
        side.enter_start(fighter, SimpleNamespace())
        side.blocked_start(fighter, SimpleNamespace(event=SimpleNamespace(value=0)))
        side.finish_start(fighter, SimpleNamespace())
        self.assertIs(fighter.action, side.air_jump)

        fighter = _Fighter()
        fighter.action = side.ground_start
        side.enter_start(fighter, SimpleNamespace())
        side.blocked_start(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))
        side.finish_start(fighter, SimpleNamespace())
        self.assertIs(fighter.action, side.ground_end)

        fighter = _Fighter()
        fighter.action = side.air_start
        side.enter_start(fighter, SimpleNamespace())
        side.blocked_start(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))
        side.finish_start(fighter, SimpleNamespace())
        self.assertIs(fighter.action, side.air_end1)


if __name__ == "__main__":
    unittest.main()
