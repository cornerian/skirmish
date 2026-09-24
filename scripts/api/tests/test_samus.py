"""Focused source-contract tests for Samus's Screw Attack."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[3]
sys.path.insert(0, str(ROOT / "scripts" / "api"))
sys.path.insert(0, str(ROOT / "scripts"))

from fighter import Action, Button, SamusAttribute, export_definition
from fighters.samus import Bomb, ChargeShot, Missile, Samus, ScrewAttack


class _Input:
    def __init__(self, stick=(0.0, 1.0), pressed=(Button.B,)):
        self.stick = stick
        self.pressed = set(pressed)

    def just_pressed(self, button):
        return button in self.pressed


class _Fighter:
    def __init__(self, *, grounded=True, complete=(353, 354), values=None):
        self.action = Action.WAIT
        self.action_frame = 0
        self.facing = 1.0
        self.grounded = grounded
        self.ground_velocity = 2.0
        self.velocity = (2.0, -1.0)
        self.complete = set(complete)
        self.values = values or {attribute: 1.0 for attribute in SamusAttribute}
        self.action_state = SimpleNamespace(command=(0, 0, 0, 0))
        self.changes = []
        self.fall_special = None
        self.max_jumps_calls = 0

    def special_attribute(self, attribute):
        return self.values.get(attribute)

    def has_complete_animation(self, state):
        return state in self.complete

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def set_velocity(self, x, y):
        self.velocity = (x, y)

    def enter_fall_special(self, **kwargs):
        self.fall_special = kwargs

    def max_jumps(self):
        self.max_jumps_calls += 1


def _context(*, grounded=True, resource=True, stick=(0.0, 1.0), pressed=(Button.B,)):
    return SimpleNamespace(
        ground_open=grounded,
        air_open=not grounded,
        input=_Input(stick, pressed),
        resource=lambda path: object() if resource else None,
    )


class SamusTests(unittest.TestCase):
    def test_all_specials_expose_source_motion_graphs(self):
        self.assertEqual(ChargeShot.ground_start.as_dict()["slippi_state"], 343)
        self.assertEqual(ChargeShot.ground_hold.as_dict()["slippi_state"], 344)
        self.assertEqual(ChargeShot.air_fire.as_dict()["slippi_state"], 348)
        self.assertEqual(Missile.ground.as_dict()["slippi_state"], 349)
        self.assertEqual(Missile.air_smash.as_dict()["slippi_state"], 352)
        self.assertEqual(Bomb.ground.as_dict()["slippi_state"], 341)
        self.assertEqual(Bomb.air_bomb.as_dict()["slippi_state"], 356)
        self.assertIsInstance(Samus.specials.neutral, ChargeShot)
        self.assertIsInstance(Samus.specials.side, Missile)
        self.assertIsInstance(Samus.specials.down, Bomb)

    def test_charge_shot_repress_b_releases_held_charge(self):
        move = ChargeShot()
        fighter = _Fighter()
        fighter.action = move.ground_hold
        self.assertTrue(move.input_pressed(fighter, _context()))
        self.assertEqual(fighter.action, move.ground_fire)

    def test_charge_shot_lr_cancels_held_charge(self):
        move = ChargeShot()
        for button in (Button.L, Button.R):
            fighter = _Fighter()
            fighter.action = move.ground_hold
            self.assertTrue(move.input_pressed(fighter, _context(pressed=(button,))))
            self.assertEqual(fighter.action, move.ground_cancel)

    def test_definition_uses_source_states_and_typed_motion(self):
        self.assertEqual(ScrewAttack.ground.as_dict()["slippi_state"], 353)
        self.assertEqual(ScrewAttack.air.as_dict()["slippi_state"], 354)
        self.assertEqual(ScrewAttack.ground.as_dict()["motion"]["kwargs"]["ground"][0]["callee"],
                         "motion.ground_friction_above_walk")
        air = ScrewAttack.air.as_dict()["motion"]["kwargs"]["air"]
        self.assertEqual(air[0]["callee"], "motion.gravity")
        self.assertEqual(air[1]["callee"], "motion.stick_steering")

    def test_fresh_upward_b_requires_resource_and_complete_animation(self):
        move = ScrewAttack()
        fighter = _Fighter(grounded=True, complete=())
        self.assertFalse(move.input_pressed(fighter, _context(resource=True)))
        fighter.complete = {353}
        self.assertTrue(move.input_pressed(fighter, _context(resource=True)))
        self.assertEqual(fighter.action, move.ground)
        self.assertFalse(move.input_pressed(_Fighter(), _context(resource=False)))

    def test_ground_entry_clamps_velocity_and_air_entry_uses_typed_values(self):
        move = ScrewAttack()
        ground = _Fighter(grounded=True)
        move.input_pressed(ground, _context(grounded=True))
        self.assertEqual(ground.ground_velocity, 1.0)
        self.assertEqual(ground.velocity, (1.0, 0.0))

        air = _Fighter(grounded=False)
        air.values[SamusAttribute.SCREW_ATTACK_AERIAL_LAUNCH_VELOCITY] = 3.0
        air.values[SamusAttribute.SCREW_ATTACK_HORIZONTAL_CLAMP] = 1.0
        move.input_pressed(air, _context(grounded=False))
        self.assertEqual(air.velocity, (1.0, 3.0))

    def test_ground_entry_resets_source_commands_without_exhausting_jumps(self):
        move = ScrewAttack()
        fighter = _Fighter()
        fighter.action_state.command = (3, 2, 1, 4)
        self.assertTrue(move.input_pressed(fighter, _context()))
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))
        self.assertEqual(fighter.max_jumps_calls, 0)

    def test_aerial_entry_exhausts_jumps_like_source(self):
        move = ScrewAttack()
        fighter = _Fighter(grounded=False)
        self.assertTrue(move.input_pressed(fighter, _context(grounded=False)))
        self.assertEqual(fighter.max_jumps_calls, 1)

    def test_ground_command_launches_airborne_and_consumes_once(self):
        move = ScrewAttack()
        fighter = _Fighter()
        move.input_pressed(fighter, _context())
        fighter.action_frame = 7
        fighter.values[SamusAttribute.SCREW_ATTACK_LAUNCH_HORIZONTAL_VELOCITY] = 2.5
        fighter.action_state.command = (1, 0, 0, 0)
        move.launch(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))
        self.assertEqual(fighter.action, move.air)
        self.assertEqual(fighter.action_frame, 7)
        self.assertEqual(fighter.velocity[0], 2.5)
        self.assertEqual(fighter.action_state.command[0], 0)

    def test_turn_is_one_time_and_completion_selects_fall_special(self):
        move = ScrewAttack()
        fighter = _Fighter()
        move.input_pressed(fighter, _context())
        fighter.values[SamusAttribute.SCREW_ATTACK_TURNAROUND_STICK_THRESHOLD] = 0.5
        move.turn(fighter, _context(stick=(-1.0, 1.0)))
        self.assertEqual(fighter.facing, -1.0)
        self.assertEqual(fighter.action_state.command[1], 1)
        move.turn(fighter, _context(stick=(1.0, 1.0)))
        self.assertEqual(fighter.facing, -1.0)

        fighter.values[SamusAttribute.SCREW_ATTACK_LANDING_TRANSITION_MULTIPLIER] = 0.75
        fighter.values[SamusAttribute.SCREW_ATTACK_LANDING_LAG] = 12.0
        move.animation_end(fighter, _context())
        self.assertEqual(fighter.fall_special, {"mobility": 0.75, "landing_lag": 12.0})


if __name__ == "__main__":
    unittest.main()
