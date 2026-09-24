"""Source-backed tests for Pikachu's native Thunder chain boundary."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[3]
for path in (ROOT / "scripts" / "api", ROOT / "scripts"):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from fighter.articles import ArticleId
from fighter.pikachu_thunder import (
    ThunderAttributes,
    ThunderChainPlan,
    build_thunder_chain,
    spawn_thunder_chain,
    thunder_jolt_ground_angle,
)


class _Fighter:
    position = (10.0, 5.0, 0.0)

    def __init__(self):
        self.plans = []

    def spawn_pikachu_thunder_chain(self, plan):
        self.plans.append(plan)


class PikachuThunderTests(unittest.TestCase):
    def test_plan_matches_it_802b1df8_chain_initialization(self):
        attrs = ThunderAttributes(
            velocity_y=2.5,
            effect_offset_y=1.25,
            spawn_offset_y=3.0,
            segment_count=3,
            segment_delay=7,
        )
        plan = build_thunder_chain((10, 5, 0), attrs)
        self.assertIsInstance(plan, ThunderChainPlan)
        self.assertEqual(plan.article_id, ArticleId.PIKACHU_THUNDER)
        self.assertEqual(plan.position, (10.0, 8.0, 0.0))
        self.assertEqual(plan.effect_position, (10.0, 9.25, 0.0))
        self.assertEqual([segment.delay for segment in plan.segments], [0, 7, 14])
        self.assertEqual(
            [segment.velocity for segment in plan.segments],
            [(0.0, 2.5, 0.0)] * 3,
        )
        self.assertEqual(
            [segment.article_id for segment in plan.segments],
            [ArticleId.PIKACHU_THUNDER] * 3,
        )

        pichu = build_thunder_chain(
            (0, 0, 0), ThunderAttributes(1.0, 0.0, 0.0, 1, 0, ArticleId.PICHU_THUNDER)
        )
        self.assertEqual(pichu.article_id, ArticleId.PICHU_THUNDER)
        self.assertEqual(pichu.segments[0].article_id, ArticleId.PICHU_THUNDER)

    def test_plan_rejects_wrong_kind_and_invalid_count(self):
        with self.assertRaises(ValueError):
            ThunderAttributes(1.0, 0.0, 0.0, 1, 0, ArticleId.MARIO_FIRE)
        with self.assertRaises(ValueError):
            ThunderAttributes(1.0, 0.0, 0.0, -1, 0)

    def test_native_sink_submits_one_immutable_chain_plan(self):
        fighter = _Fighter()
        attrs = ThunderAttributes(1.0, 0.0, 2.0, 2, 4)
        plan = build_thunder_chain(fighter.position, attrs)
        self.assertFalse(spawn_thunder_chain(fighter, plan))
        self.assertTrue(spawn_thunder_chain(fighter, plan, resource_available=True))
        self.assertFalse(spawn_thunder_chain(object(), plan, resource_available=True))

        self.assertEqual(len(fighter.plans), 1)
        self.assertEqual(fighter.plans[0].position, (10.0, 7.0, 0.0))

    def test_native_resource_gate_rejects_empty_chain_without_fallback(self):
        plan = build_thunder_chain(
            (0, 0, 0), ThunderAttributes(1.0, 0.0, 0.0, 0, 2)
        )
        fighter = _Fighter()
        self.assertFalse(spawn_thunder_chain(fighter, plan, resource_available=True))
        self.assertEqual(fighter.plans, [])

    def test_ground_jolt_angle_matches_it_802b3554(self):
        self.assertEqual(thunder_jolt_ground_angle(1, 0.25), 0.25)
        self.assertAlmostEqual(thunder_jolt_ground_angle(-1, 0.25), 3.141592653589793 + 0.25)
        self.assertAlmostEqual(thunder_jolt_ground_angle(0, -0.25), 3.141592653589793 + 0.25)


if __name__ == "__main__":
    unittest.main()
