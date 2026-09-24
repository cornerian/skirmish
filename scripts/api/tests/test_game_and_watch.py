"""Focused source contracts for Game & Watch's fighter-side specials."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
sys.path.insert(0, str(ROOT / "scripts" / "api"))
sys.path.insert(0, str(ROOT / "scripts"))

from fighter import Action, Button, export_definition
from fighters.game_and_watch import (
    Chef, Fire, GameAndWatch, GameAndWatchActionState, Judge, OilPanic,
)


class _Input:
    def __init__(self, stick=(0.0, 0.0), pressed=True):
        self.stick = stick
        self.pressed = pressed

    def just_pressed(self, button):
        return self.pressed and button is Button.B


class _Fighter:
    def __init__(self, action=Action.WAIT, grounded=True):
        self.action = action
        self.grounded = grounded
        self.action_frame = 0
        self.fall_special = []

    def change_action(self, action, **kwargs):
        self.action = action

    def enter_fall_special(self, **kwargs):
        self.fall_special.append(kwargs)


def _context(*, grounded=True, resource=True, stick=(0.0, 0.0), pressed=True,
             judge_weights=None, judge_roll=None, judge_previous=(),
             rescue_landing=None, panic_damage_mul=None, panic_damage_add=None,
             panic_damage=None, panic_charge=None, chef_b_held=None):
    resource_value = SimpleNamespace(
        attributes=SimpleNamespace(
            judge_roll=judge_weights, rescue_landing=rescue_landing,
            panic_damage_mul=panic_damage_mul, panic_damage_add=panic_damage_add,
        )
    )

    def lookup(path):
        if not resource:
            return None
        return resource_value if path.endswith(".attributes") else object()

    return SimpleNamespace(
        ground_open=grounded,
        air_open=not grounded,
        grounded=grounded,
        input=_Input(stick, pressed),
        resource=lookup,
        judge_roll=judge_roll,
        judge_previous=judge_previous,
        panic_damage=panic_damage,
        panic_charge=panic_charge,
        chef_b_held=chef_b_held,
        rules=SimpleNamespace(
            specials=SimpleNamespace(side_stick_threshold=0.5, vertical_threshold=0.5)
        ),
    )


class GameAndWatchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.definition = export_definition(GameAndWatch).as_dict()

    def test_source_special_states_and_roots_are_complete(self):
        states = sorted(
            value["slippi_state"]
            for name, value in self.definition["actions"].items()
            if name.startswith("special.")
        )
        self.assertEqual(states, list(range(353, 381)))
        self.assertEqual(
            [GameAndWatch.specials.neutral.resource,
             GameAndWatch.specials.side.resource,
             GameAndWatch.specials.up.resource,
             GameAndWatch.specials.down.resource],
            ["neutral", "side", "up", "down"],
        )

    def test_pair_specials_gate_resources_and_preserve_surface(self):
        move = Chef()
        self.assertFalse(move.input_pressed(_Fighter(), _context(resource=False)))
        fighter = _Fighter()
        self.assertTrue(move.input_pressed(fighter, _context()))
        self.assertEqual(fighter.action, move.ground)

        fighter.action = move.ground
        move._transition_ground_air(fighter, _context(grounded=False))
        self.assertEqual(fighter.action, move.air)
        move._transition_animation_end(fighter, _context(grounded=False))
        self.assertEqual(fighter.action, Action.FALL)

    def test_directional_pair_specials_use_their_root_gate(self):
        move = Fire()
        self.assertFalse(move.input_pressed(_Fighter(), _context(stick=(1.0, 0.0))))
        fighter = _Fighter()
        self.assertTrue(move.input_pressed(fighter, _context(stick=(0.0, 1.0))))
        self.assertEqual(fighter.action, move.ground)

    def test_fire_rescue_uses_native_landing_attribute_when_available(self):
        move = Fire()
        fighter = _Fighter(move.air)
        move._transition_animation_end(
            fighter, _context(grounded=False, rescue_landing=12.0)
        )
        self.assertEqual(fighter.fall_special, [{"mobility": 1, "landing_lag": 12.0}])

        no_lag = _Fighter(move.air)
        move._transition_animation_end(
            no_lag, _context(grounded=False, rescue_landing=0)
        )
        self.assertEqual(no_lag.action, Action.FALL)

    def test_judge_declares_all_phases_without_random_or_article_callbacks(self):
        move = Judge()
        self.assertEqual(
            [getattr(move, f"ground_{index}").as_dict()["slippi_state"] for index in range(1, 10)],
            list(range(355, 364)),
        )
        self.assertEqual(
            [getattr(move, f"air_{index}").as_dict()["slippi_state"] for index in range(1, 10)],
            list(range(364, 373)),
        )
        self.assertFalse(any("article" in event.callback for event in move.events()))
        self.assertFalse(move.input_pressed(_Fighter(), _context(resource=False, stick=(1.0, 0.0))))
        self.assertFalse(move.input_pressed(_Fighter(), _context(stick=(0.0, 1.0))))
        fighter = _Fighter()
        self.assertTrue(move.input_pressed(fighter, _context(stick=(1.0, 0.0))))
        self.assertEqual(fighter.action, move.ground_1)

        # Native code supplies the roll after excluding its previous two rows;
        # the script applies those weights without creating its own RNG.
        weighted = _Fighter()
        self.assertTrue(move.input_pressed(
            weighted,
            _context(stick=(1.0, 0.0), judge_weights=(0, 0, 3, 0, 0, 0, 0, 0, 0), judge_roll=0),
        ))
        self.assertEqual(weighted.action, move.ground_3)

        excluded = _Fighter()
        self.assertTrue(move.input_pressed(
            excluded,
            _context(stick=(1.0, 0.0), judge_weights=(3, 0, 3, 0, 0, 0, 0, 0, 0), judge_roll=0, judge_previous=(0,)),
        ))
        self.assertEqual(excluded.action, move.ground_3)

        # Repeated B while a Judge row is active is consumed without
        # restarting the selected source motion.
        fighter.action_frame = 7
        self.assertTrue(move.input_pressed(fighter, _context(stick=(1.0, 0.0))))
        self.assertEqual(fighter.action_frame, 7)

    def test_oil_panic_keeps_source_catch_and_shoot_states(self):
        move = OilPanic()
        self.assertEqual(
            [move.ground.as_dict()["slippi_state"],
             move.ground_catch.as_dict()["slippi_state"],
             move.ground_shoot.as_dict()["slippi_state"],
             move.air.as_dict()["slippi_state"],
             move.air_catch.as_dict()["slippi_state"],
             move.air_shoot.as_dict()["slippi_state"]],
            list(range(375, 381)),
        )
        self.assertEqual(move._ACTIVE, (
            move.ground, move.ground_catch, move.ground_shoot,
            move.air, move.air_catch, move.air_shoot,
        ))
        fighter = _Fighter(move.ground_shoot)
        move._transition_ground_air(fighter, _context(grounded=False))
        self.assertEqual(fighter.action, move.air_shoot)

    def test_judge_records_the_source_previous_row_shift(self):
        move = Judge()
        fighter = _Fighter()
        self.assertTrue(move.input_pressed(
            fighter, _context(stick=(1.0, 0.0), judge_weights=(0, 0, 3, 0, 0, 0, 0, 0, 0), judge_roll=0)
        ))
        self.assertEqual(fighter.action_state.judge_current, 2)
        self.assertEqual(fighter.action_state.judge_previous, -1)

    def test_chef_release_disables_future_source_loops(self):
        move = Chef()
        fighter = _Fighter(move.ground)
        self.assertTrue(move.release(fighter, _context()))
        self.assertTrue(fighter.action_state.chef_loop_disabled)
        self.assertTrue(move.input_pressed(
            fighter, _context(chef_b_held=True)
        ))
        self.assertEqual(fighter.action_frame, 0)

    def test_full_oil_panic_caches_source_release_damage(self):
        move = OilPanic()
        fighter = _Fighter()
        self.assertTrue(move.input_pressed(
            fighter,
            _context(
                stick=(0.0, -1.0),
                panic_charge=3,
                panic_damage=12.0,
                panic_damage_mul=1.5,
                panic_damage_add=4.0,
            ),
        ))
        self.assertEqual(fighter.action_state.panic_release_damage, 22.0)
        self.assertEqual(fighter.action_state.panic_damage, 0.0)

    def test_oil_panic_uses_retained_full_charge_when_context_omits_charge(self):
        move = OilPanic()
        fighter = _Fighter()
        fighter.action_state = GameAndWatchActionState()
        fighter.action_state.panic_charge = 3
        fighter.action_state.panic_damage = 8.0

        self.assertTrue(move.input_pressed(
            fighter,
            _context(
                stick=(0.0, -1.0), panic_damage_mul=2.0,
                panic_damage_add=1.0,
            ),
        ))
        self.assertIs(fighter.action, move.ground_shoot)
        self.assertEqual(fighter.action_state.panic_release_damage, 17.0)
        self.assertEqual(fighter.action_state.panic_charge, 0)


if __name__ == "__main__":
    unittest.main()
