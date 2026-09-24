"""Source contracts for Falco's shared ftFox special implementation."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter import Action, ArticleId, export_definition  # noqa: E402
from fighters.falco import Falco, FalcoActionState, FalcoParameters  # noqa: E402
from fighters.fox import Blaster, Fox, FoxActionState  # noqa: E402


class FalcoSourceTests(unittest.TestCase):
    def test_falco_uses_its_own_state_and_article_identity(self):
        self.assertIs(Falco.action_state, FalcoActionState)
        self.assertTrue(issubclass(FalcoActionState, FoxActionState))
        self.assertEqual(FalcoParameters().article_id, ArticleId.FALCO_LASER)
        self.assertNotEqual(FalcoParameters().article_id, Fox.parameters().article_id)

    def test_ftfalco_motion_table_is_shared_special_state_range(self):
        exported = export_definition(Falco).as_dict()
        states = {
            action["slippi_state"]
            for action in exported["actions"].values()
            if action.get("slippi_state") is not None
        }
        self.assertTrue(set(range(341, 370)).issubset(states))
        self.assertEqual(
            [
                dict(Falco.specials.neutral.ground_start.metadata)["slippi_state"],
                dict(Falco.specials.neutral.ground_loop.metadata)["slippi_state"],
                dict(Falco.specials.neutral.ground_end.metadata)["slippi_state"],
                dict(Falco.specials.neutral.air_start.metadata)["slippi_state"],
                dict(Falco.specials.neutral.air_loop.metadata)["slippi_state"],
                dict(Falco.specials.neutral.air_end.metadata)["slippi_state"],
            ],
            [341, 342, 343, 344, 345, 346],
        )

    def test_shared_special_policies_still_bind_to_falco_source_id(self):
        self.assertIs(Falco.specials, Fox.specials)
        exported = export_definition(Falco).as_dict()
        neutral = next(
            behavior for behavior in exported["behaviors"]
            if behavior["resource"] == "neutral"
        )
        command = next(
            callback for callback in neutral["callbacks"]
            if callback["hook"] == "command_trace_changed"
        )
        self.assertEqual(command["actions"], [
            Action.SPECIAL_N_LOOP, Action.SPECIAL_AIR_N_LOOP,
        ])

    def test_shared_illusion_callback_selects_falco_article_once(self):
        move = Falco.specials.side
        spawned = []
        fighter = SimpleNamespace(
            action=move.ground_dash,
            action_state=SimpleNamespace(command=(4, 5, 1, 7)),
            position=(12.0, 4.0, 0.0),
            facing=-1,
            spawn_article=lambda *args: spawned.append(args),
        )
        context = SimpleNamespace(
            event=SimpleNamespace(value=1),
            parameters=FalcoParameters(),
        )

        move.command_changed(fighter, context)

        self.assertEqual(
            spawned,
            [(ArticleId.FALCO_PHANTASM, (12.0, 4.0, 0.0), -1)],
        )
        self.assertEqual(fighter.action_state.command, (4, 5, 0, 7))

    def test_shared_blaster_dispatches_falco_laser_article(self):
        move = Blaster()
        spawned = []
        fighter = SimpleNamespace(
            action=move.ground_loop,
            action_state=SimpleNamespace(command=(0, 0, 1, 0)),
            ecb=SimpleNamespace(current=SimpleNamespace(top=(0.0, 2.0), bottom=(0.0, 0.0))),
            facing=1,
            position=(12.0, 4.0),
            depth=0.0,
            spawn_article=lambda *args: spawned.append(args),
        )
        resource = SimpleNamespace(
            attributes=SimpleNamespace(angle=0.0, speed=5.0, landing_lag=0.0),
        )
        context = SimpleNamespace(
            event=SimpleNamespace(value=1),
            parameters=FalcoParameters(),
            resource=lambda path=None: resource,
        )

        move.command_changed(fighter, context)

        self.assertEqual(len(spawned), 1)
        self.assertEqual(spawned[0][0], ArticleId.FALCO_LASER)
        self.assertEqual(spawned[0][1], (12.0, 5.0, 0.0))
        self.assertEqual(spawned[0][2:], (0.0, 5.0))


if __name__ == "__main__":
    unittest.main()
