"""Source contracts for Link and Young Link's shared special state table."""

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
from fighters.link import Link
from fighters.young_link import YoungLink


class _Input:
    def __init__(self, stick=(0.0, 0.0), pressed=True):
        self.stick = stick
        self.pressed = pressed

    def just_pressed(self, button):
        return self.pressed and button is Button.B


class _Fighter:
    def __init__(self, action=None):
        self.action = action
        self.action_frame = 0
        self.grounded = True
        self.changes = []
        self.used_boomerang = False
        self.trajectory_updates = 0

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def update_boomerang_trajectory(self, ctx):
        self.trajectory_updates += 1


def _context(*, resource=True, stick=(0.0, 0.0), ground=True, pressed=True):
    rules = SimpleNamespace(
        specials=SimpleNamespace(side_stick_threshold=0.5, vertical_threshold=0.5)
    )
    return SimpleNamespace(
        input=_Input(stick, pressed),
        resource=lambda path: object() if resource else None,
        ground_open=ground,
        air_open=not ground,
        grounded=ground,
        rules=rules,
    )


class LinkFamilyTests(unittest.TestCase):
    def test_both_fighters_derive_identity_and_share_state_numbers(self):
        link = export_definition(Link).as_dict()
        young = export_definition(YoungLink).as_dict()
        self.assertEqual((link["name"], link["external_ids"]), ("link", [6]))
        self.assertEqual((young["name"], young["external_ids"]), ("young-link", [21]))
        self.assertEqual(
            link["actions"]["special.neutral.ground_start"]["action"],
            "Action.Source.6:344",
        )
        self.assertEqual(
            young["actions"]["special.neutral.ground_start"]["action"],
            "Action.Source.21:344",
        )
        for definition in (link, young):
            actions = definition["actions"]
            states = sorted(
                value["slippi_state"]
                for name, value in actions.items()
                if name.startswith("special.")
            )
            self.assertEqual(states, list(range(344, 360)))
            self.assertTrue(actions["special.neutral.ground_loop"]["animation_loop"])

    def test_special_roots_keep_canonical_resources_and_no_article_callbacks(self):
        for fighter in (Link, YoungLink):
            exported = export_definition(fighter).as_dict()
            for root in ("neutral", "side", "up", "down"):
                move = getattr(fighter.specials, root)
                self.assertEqual(move.resource, root)
                behavior_id = exported["movesets"]["specials"][root]
                behavior = next(item for item in exported["behaviors"] if item["id"] == behavior_id)
                self.assertEqual(behavior["resource"], root)
                self.assertFalse(any("article" in callback["callback"] for callback in behavior["callbacks"]))

    def test_entry_is_resource_gated_and_uses_directional_root(self):
        neutral = Link.specials.neutral
        fighter = _Fighter()
        self.assertFalse(neutral.input_pressed(fighter, _context(resource=False)))
        self.assertTrue(neutral.input_pressed(fighter, _context()))
        self.assertEqual(fighter.action, neutral.ground_start)

        side = Link.specials.side
        fighter = _Fighter()
        self.assertFalse(side.input_pressed(fighter, _context(stick=(0.0, 1.0))))
        self.assertTrue(side.input_pressed(fighter, _context(stick=(1.0, 0.0))))
        self.assertEqual(fighter.action, side.ground_start)

    def test_side_entry_uses_empty_phase_for_held_boomerang(self):
        side = YoungLink.specials.side
        fighter = _Fighter()
        fighter.used_boomerang = True
        self.assertTrue(side.input_pressed(fighter, _context(stick=(1.0, 0.0))))
        self.assertEqual(fighter.action, side.ground_empty)

        fighter = _Fighter()
        fighter.used_boomerang = True
        side.boomerang_release(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))
        self.assertEqual(fighter.trajectory_updates, 1)

    def test_source_transitions_cover_charge_terminal_and_surface_states(self):
        neutral = Link.specials.neutral
        fighter = _Fighter("Source.6:344")
        neutral._transition_animation_end(fighter, _context())
        self.assertEqual(fighter.action, "Source.6:345")

        fighter = _Fighter("Source.6:347")
        neutral._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.6:344")

        fighter = _Fighter("Source.6:349")
        neutral._transition_animation_end(fighter, _context(ground=False))
        self.assertEqual(fighter.action, Action.FALL)

        down = Link.specials.down
        fighter = _Fighter("Source.6:359")
        down._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, "Source.6:358")

    def test_neutral_release_enters_source_end_phase(self):
        neutral = Link.specials.neutral
        fighter = _Fighter(neutral.ground_loop)
        neutral.release(fighter, _context(pressed=False))
        self.assertEqual(fighter.action, neutral.ground_end)


if __name__ == "__main__":
    unittest.main()
