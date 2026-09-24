"""Focused source and article ownership contracts for Link."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action, Button, export_definition  # noqa: E402
from fighters.link import Link  # noqa: E402


class _Input:
    def __init__(self, stick):
        self.stick = stick

    def just_pressed(self, button):
        return button is Button.B


class _Fighter:
    def __init__(self, *, used_boomerang=False):
        self.action = None
        self.action_frame = 0
        self.used_boomerang = used_boomerang
        self.boomerang_active = False
        self.link_bomb_held = False
        self.changes = []
        self.trajectory_events = []
        self.bomb_reuse = []
        self.fall_special = []
        self.arrow_releases = []

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def update_boomerang_trajectory(self, ctx):
        self.trajectory_events.append(ctx)

    def reuse_held_bomb_special(self, *, airborne):
        self.bomb_reuse.append(airborne)

    def enter_fall_special(self, **kwargs):
        self.fall_special.append(kwargs)

    def release_arrow(self, ctx):
        self.arrow_releases.append(ctx)


def _context(*, ground=True, resource=True, stick=(0.0, 0.0)):
    return SimpleNamespace(
        input=_Input(stick),
        resource=lambda _: object() if resource else None,
        ground_open=ground,
        air_open=not ground,
        grounded=ground,
        rules=SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5, vertical_threshold=0.5,
        )),
    )


class LinkTests(unittest.TestCase):
    def test_source_actions_bind_attack_paths_and_identity(self):
        exported = export_definition(Link).as_dict()
        self.assertEqual((exported["name"], exported["external_ids"]), ("link", [6]))
        for name, state in {
            "special.neutral.ground_start": 344,
            "special.neutral.air_end": 349,
            "special.side.ground_empty": 352,
            "special.side.air_empty": 355,
            "special.up.ground": 356,
            "special.down.air": 359,
        }.items():
            self.assertEqual(exported["actions"][name]["slippi_state"], state)
            self.assertIn("attack", exported["actions"][name])

    def test_side_special_selects_empty_phase_when_boomerang_is_owned(self):
        move = Link.specials.side
        fighter = _Fighter(used_boomerang=True)
        self.assertTrue(move.input_pressed(fighter, _context(stick=(1.0, 0.0))))
        self.assertIs(fighter.action, move.ground_empty)

        fighter = _Fighter()
        self.assertTrue(move.input_pressed(fighter, _context(ground=False, stick=(1.0, 0.0))))
        self.assertIs(fighter.action, move.air_start)

    def test_side_entry_stays_resource_gated(self):
        move = Link.specials.side
        fighter = _Fighter()
        self.assertFalse(move.input_pressed(
            fighter, _context(resource=False, stick=(1.0, 0.0)),
        ))
        self.assertIsNone(fighter.action)

    def test_neutral_release_forwards_arrow_launch_and_enters_end_phase(self):
        move = Link.specials.neutral
        fighter = _Fighter()
        fighter.action = move.ground_loop
        context = SimpleNamespace()

        self.assertTrue(move.release(fighter, context))
        self.assertEqual(fighter.arrow_releases, [context])
        self.assertIs(fighter.action, move.ground_end)

        fighter.action = move.air_start
        self.assertTrue(move.release(fighter, context))
        self.assertEqual(fighter.arrow_releases, [context, context])
        self.assertIs(fighter.action, move.air_end)

    def test_neutral_release_ignores_non_charge_actions(self):
        move = Link.specials.neutral
        fighter = _Fighter()
        fighter.action = Action.WAIT
        self.assertFalse(move.release(fighter, SimpleNamespace()))
        self.assertEqual(fighter.arrow_releases, [])

    def test_side_command_forwards_boomerang_release_to_article_host(self):
        move = Link.specials.side
        fighter = _Fighter()
        fighter.action = move.ground
        context = SimpleNamespace(event=SimpleNamespace(value=True))
        move.boomerang_release(fighter, context)
        self.assertEqual(fighter.trajectory_events, [context])
        move.boomerang_release(fighter, SimpleNamespace(event=SimpleNamespace(value=False)))
        self.assertEqual(fighter.trajectory_events, [context])

    def test_boomerang_release_obeys_source_stick_and_window_gate(self):
        move = Link.specials.side
        fighter = _Fighter()
        context = SimpleNamespace(
            event=SimpleNamespace(value=True),
            input=SimpleNamespace(stick=(0.25, 0.0)),
            rules=SimpleNamespace(specials=SimpleNamespace(
                dash_smash_stick_threshold=0.5, dash_smash_window=3,
            )),
        )
        move.boomerang_release(fighter, context)
        self.assertEqual(fighter.trajectory_events, [])
        context.input.stick = (1.0, 0.0)
        fighter.action_frame = 4
        move.boomerang_release(fighter, context)
        self.assertEqual(fighter.trajectory_events, [])
        fighter.action_frame = 3
        move.boomerang_release(fighter, context)
        self.assertEqual(fighter.trajectory_events, [context])

    def test_down_entry_forwards_held_bomb_branch_to_article_host(self):
        move = Link.specials.down
        fighter = _Fighter()
        fighter.link_bomb_held = True
        fighter.action = move.ground
        move.reuse_held_bomb(fighter, SimpleNamespace())
        self.assertEqual(fighter.bomb_reuse, [False])

        fighter.action = move.air
        move.reuse_held_bomb(fighter, SimpleNamespace())
        self.assertEqual(fighter.bomb_reuse, [False, True])

    def test_spin_attack_air_end_enters_fall_special(self):
        move = Link.specials.up
        fighter = _Fighter()
        fighter.action = move.air
        self.assertTrue(move.enter_fall_special(fighter, SimpleNamespace()))
        self.assertEqual(fighter.fall_special, [{"mobility": 1}])

    def test_terminal_phases_return_to_native_wait_or_fall(self):
        for move in (Link.specials.neutral, Link.specials.side,
                     Link.specials.up, Link.specials.down):
            for phase in (getattr(move, "ground_end", None),
                          getattr(move, "ground_empty", None),
                          getattr(move, "ground", None)):
                if phase is None:
                    continue
                fighter = _Fighter()
                fighter.action = phase
                move._transition_animation_end(fighter, _context())
                self.assertEqual(fighter.action, Action.WAIT)


if __name__ == "__main__":
    unittest.main()
