"""Focused source parity checks for Fox's command driven article callback."""

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))

from skirmish import ArticleId
from fighters.fox import Fox


class FoxSourceAuditTests(unittest.TestCase):
    def test_illusion_command_callback_consumes_only_source_marker_one(self):
        move = Fox.specials.side
        for marker in (0, 2, -1):
            spawned = []
            fighter = SimpleNamespace(
                action=move.ground_dash,
                action_state=SimpleNamespace(command=(0, 0, marker, 0)),
                position=(0.0, 0.0, 0.0),
                facing=1.0,
                spawn_article=lambda *args: spawned.append(args),
            )
            context = SimpleNamespace(
                event=SimpleNamespace(value=marker),
                parameters=Fox.parameters(),
            )

            move.command_changed(fighter, context)

            self.assertEqual(spawned, [])
            self.assertEqual(fighter.action_state.command[2], marker)

    def test_illusion_command_callback_spawns_fox_article_for_source_marker_one(self):
        move = Fox.specials.side
        spawned = []
        fighter = SimpleNamespace(
            action=move.ground_dash,
            action_state=SimpleNamespace(command=(0, 0, 1, 0)),
            position=(1.0, 2.0, 3.0),
            facing=1.0,
            spawn_article=lambda *args: spawned.append(args),
        )
        context = SimpleNamespace(
            event=SimpleNamespace(value=1),
            parameters=Fox.parameters(),
        )

        move.command_changed(fighter, context)

        self.assertEqual(spawned[0][0], ArticleId.FOX_ILLUSION)
        self.assertEqual(fighter.action_state.command[2], 0)


if __name__ == "__main__":
    unittest.main()
