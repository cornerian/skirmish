"""Focused contract tests for Fox's special-input behavior."""

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))

from skirmish import Action, ArticleId, Button, Fighter
from fighters.fox import FireFox, Fox, Shine
from fighters.falco import Falco


class FoxBlasterTests(unittest.TestCase):
    def test_blaster_entry_clears_source_ground_velocity_but_preserves_air_momentum(self):
        move = Fox.specials.neutral

        ground = SimpleNamespace(
            action=move.ground_start,
            action_state=SimpleNamespace(
                command=(7, 6, 5, 4), repeat_armed=True, fire_pending=True
            ),
            ground_velocity=3.5,
            velocity=[2.0, -1.0],
        )
        ground.set_velocity = lambda x, y: setattr(ground, "velocity", [x, y])
        move.enter(ground, SimpleNamespace())
        self.assertEqual(ground.ground_velocity, 0.0)
        self.assertEqual(ground.velocity, [0.0, 0.0])

        air = SimpleNamespace(
            action=move.air_start,
            action_state=SimpleNamespace(
                command=(7, 6, 5, 4), repeat_armed=True, fire_pending=True
            ),
            ground_velocity=3.5,
            velocity=[2.0, -1.0],
        )
        air.set_velocity = lambda x, y: setattr(air, "velocity", [x, y])
        move.enter(air, SimpleNamespace())
        self.assertEqual(air.ground_velocity, 3.5)
        self.assertEqual(air.velocity, [2.0, -1.0])

        for fighter in (ground, air):
            self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))
            self.assertFalse(fighter.action_state.repeat_armed)
            self.assertFalse(fighter.action_state.fire_pending)

    def test_falco_directly_uses_fighter_and_shares_fox_moves(self):
        self.assertEqual(Falco.__bases__, (Fighter,))
        self.assertFalse(issubclass(Falco, Fox))
        # Falco shares the Fox callback objects for neutral, up, and down
        # specials.  Side special keeps the shared implementation through a
        # Falco-only wrapper that restores the grounded Phantasm end transition.
        self.assertIsNot(Falco.specials, Fox.specials)
        self.assertIs(Falco.specials.neutral, Fox.specials.neutral)
        self.assertIs(Falco.specials.up, Fox.specials.up)
        self.assertIs(Falco.specials.down, Fox.specials.down)
        self.assertIsNot(Falco.specials.side, Fox.specials.side)
        self.assertIs(Falco.aerials, Fox.aerials)
        self.assertIs(Falco.grounded, Fox.grounded)
        self.assertEqual(Falco.parameters().article_id, ArticleId.FALCO_LASER)
        self.assertEqual(Fox.parameters().article_id, ArticleId.FOX_LASER)

    def test_neutral_threshold_boundary_is_rejected_but_inner_stick_starts_blaster(self):
        fox = Fox
        move = fox.specials.neutral

        def attempt(stick):
            fighter = SimpleNamespace(
                action=Action.WAIT,
                action_frame=7,
                action_state=SimpleNamespace(),
                ground_velocity=3.0,
                velocity=[2.0, 0.0],
            )
            fighter.change_action = lambda action: setattr(fighter, "action", action)
            fighter.set_velocity = lambda x, y: setattr(fighter, "velocity", [x, y])
            context = SimpleNamespace(
                ground_open=True,
                air_open=False,
                input=SimpleNamespace(
                    stick=stick,
                    just_pressed=lambda button: button == Button.B,
                ),
                resource=lambda path: SimpleNamespace(neutral_thresholds=(0.5, 0.5)),
            )
            return move.press(fighter, context), fighter

        accepted, fighter = attempt((0.5 - 1e-6, 0.0))
        self.assertTrue(accepted)
        self.assertEqual(fighter.action, move.ground_start)

        rejected, fighter = attempt((0.5, 0.0))
        self.assertFalse(rejected)
        self.assertEqual(fighter.action, Action.WAIT)

    def test_rejected_neutral_input_does_not_start_blaster(self):
        fox = Fox
        move = fox.specials.neutral
        fighter = SimpleNamespace(
            action=Action.WAIT,
            action_frame=7,
            action_state=SimpleNamespace(),
            ground_velocity=3.0,
            velocity=[2.0, 0.0],
        )
        context = SimpleNamespace(
            ground_open=True,
            air_open=False,
            input=SimpleNamespace(
                stick=(1.0, 0.0),
                just_pressed=lambda button: button == Button.B,
            ),
            resource=lambda path: SimpleNamespace(neutral_thresholds=(0.5, 0.5)),
        )

        self.assertFalse(move.press(fighter, context))
        self.assertEqual(fighter.action, Action.WAIT)
        self.assertEqual(fighter.action_frame, 7)


class FoxShineTests(unittest.TestCase):
    @staticmethod
    def _validation_context(exit_flags):
        attributes = SimpleNamespace(
            gravity_delay=0.0,
            entry_speed_div=1.0,
            hold_air_friction=0.0,
            hold_fall_accel=1.0,
            direction_stick_min=0.0,
            duration=1.0,
            duration_end=0.0,
            speed=1.0,
            reverse_accel=0.0,
            landing_friction=0.0,
            bound_speed_mul=1.0,
            facing_stick_min=0.0,
            freefall_mobility=0.0,
            landing_lag=1.0,
            bound_angle_degrees=0.0,
            air_drift_clamp_accel=0.0,
            bounce_frames=0.0,
        )
        bound = SimpleNamespace(transn_y=[0.0], exit_flags=exit_flags)
        return SimpleNamespace(
            resource=lambda path=None: (
                SimpleNamespace(attributes=attributes)
                if path is None
                else bound
            ),
            frames=lambda path: 1,
        )

    def test_surface_destinations_cover_each_reflector_phase_in_both_directions(self):
        destinations = Shine._SURFACE_DESTINATIONS
        for ground, air in Shine._SURFACE_PAIRS:
            self.assertIs(destinations[ground], air)
            self.assertIs(destinations[air], ground)

    def test_validate_requires_native_booleans_for_bound_exit_flags(self):
        self.assertTrue(FireFox().validate(self._validation_context([True])))
        self.assertFalse(FireFox().validate(self._validation_context([1])))
        self.assertFalse(FireFox().validate(self._validation_context([0])))

    def test_fresh_reflector_entry_initializes_command_var_one(self):
        move = Fox.specials.down
        attributes = SimpleNamespace(
            release_lag=3.0,
            gravity_delay=2.0,
            air_momentum_div=2.0,
        )
        fighter = SimpleNamespace(
            action=Action.WAIT,
            grounded=True,
            action_state=SimpleNamespace(command=(9, 9, 9, 9)),
            velocity=[0.0, 0.0],
        )
        fighter.change_action = lambda action, **kwargs: setattr(
            fighter, "action", action
        )
        context = SimpleNamespace(
            ground_open=True,
            air_open=False,
            input=SimpleNamespace(
                stick=(0.0, -1.0),
                just_pressed=lambda button: button == Button.B,
            ),
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5)),
            resource=lambda path=None: SimpleNamespace(attributes=attributes),
        )

        self.assertTrue(move.press_b(fighter, context))
        self.assertEqual(fighter.action, move.ground_start)
        self.assertEqual(fighter.action_state.command, (0, 4, 0, 0))

    def test_firefox_bound_entry_clears_command_var_zero(self):
        move = Fox.specials.up
        attributes = SimpleNamespace(bound_speed_mul=0.5)
        fighter = SimpleNamespace(
            action=move.bound,
            action_state=SimpleNamespace(command=(7, 8, 9, 10)),
            velocity=[4.0, -2.0],
        )
        fighter.set_velocity = lambda x, y: setattr(fighter, "velocity", [x, y])
        context = SimpleNamespace(
            resource=lambda path=None: SimpleNamespace(attributes=attributes),
        )

        move.action_entered(fighter, context)

        self.assertEqual(fighter.velocity, [2.0, -2.0])
        self.assertEqual(fighter.action_state.command, (0, 8, 9, 10))

    def test_firefox_bound_exit_restores_jumps_before_fallspecial(self):
        move = Fox.specials.up
        attributes = SimpleNamespace(freefall_mobility=0.5, landing_lag=3.0)
        restored = []
        entered = []
        fighter = SimpleNamespace(
            action=move.bound,
            grounded=False,
            max_jumps=lambda: restored.append(True),
            enter_fall_special=lambda **kwargs: entered.append(kwargs),
        )
        context = SimpleNamespace(
            resource=lambda path=None: SimpleNamespace(attributes=attributes),
        )

        move.bound_exit_marker(fighter, context)

        self.assertEqual(restored, [True])
        self.assertEqual(
            entered,
            [{"mobility": 0.5, "landing_lag": 3.0}],
        )

    def test_firefox_bound_clip_end_also_restores_jumps(self):
        move = Fox.specials.up
        restored = []
        entered = []
        fighter = SimpleNamespace(
            action=move.bound,
            grounded=False,
            max_jumps=lambda: restored.append(True),
            enter_fall_special=lambda **kwargs: entered.append(kwargs),
        )
        context = SimpleNamespace(
            resource=lambda path=None: SimpleNamespace(
                attributes=SimpleNamespace(freefall_mobility=0.25, landing_lag=2.0)
            ),
        )

        move.terminal_animation_end(fighter, context)

        self.assertEqual(restored, [True])
        self.assertEqual(
            entered,
            [{"mobility": 0.25, "landing_lag": 2.0}],
        )

    def test_illusion_entry_clears_ghost_command_var_two(self):
        move = Fox.specials.side
        attributes = SimpleNamespace(
            entry_speed_div=1.0,
            gravity_delay=2.0,
        )

        for phase in (move.ground_start, move.ground_dash):
            fighter = SimpleNamespace(
                action=phase,
                action_state=SimpleNamespace(command=(1, 2, 9, 4)),
                ground_velocity=3.0,
            )
            context = SimpleNamespace(
                resource=lambda path=None: SimpleNamespace(
                    attributes=attributes,
                    ground_speed_retention=1.0,
                ),
            )

            move.action_enter(fighter, context)

            self.assertEqual(fighter.action_state.command, (1, 2, 0, 4))

    def test_illusion_command_trace_spawns_the_source_article_once(self):
        move = Fox.specials.side
        spawned = []
        fighter = SimpleNamespace(
            action=move.ground_dash,
            action_state=SimpleNamespace(command=(1, 2, 1, 4)),
            position=(10.0, 20.0, 0.0),
            facing=-1.0,
            spawn_article=lambda *args: spawned.append(args),
        )
        context = SimpleNamespace(
            event=SimpleNamespace(value=1),
            parameters=Fox.parameters(),
        )

        move.command_changed(fighter, context)

        self.assertEqual(
            spawned,
            [(ArticleId.FOX_ILLUSION, (10.0, 20.0, 0.0), -1.0)],
        )
        self.assertEqual(fighter.action_state.command, (1, 2, 0, 4))

    def test_illusion_command_trace_uses_falco_phantasm_identity(self):
        move = Fox.specials.side
        spawned = []
        fighter = SimpleNamespace(
            action=move.air_dash,
            action_state=SimpleNamespace(command=(0, 0, 0, 0)),
            position=(1.0, 2.0, 3.0),
            facing=1.0,
            spawn_article=lambda *args: spawned.append(args),
        )
        context = SimpleNamespace(
            event=SimpleNamespace(value=1),
            parameters=Falco.parameters(),
        )

        move.command_changed(fighter, context)

        self.assertEqual(
            spawned,
            [(ArticleId.FALCO_PHANTASM, (1.0, 2.0, 3.0), 1.0)],
        )

    def test_reflector_projectile_contact_enters_hit_phase(self):
        move = Fox.specials.down
        fighter = SimpleNamespace(
            grounded=True,
            flags=SimpleNamespace(reflecting=True),
            action=move.ground_loop,
        )
        fighter.change_action = lambda action: setattr(fighter, "action", action)
        hit = SimpleNamespace(projectile=True, damage=8, max_damage=8, reflect=False)

        move.projectile_contact(fighter, hit)

        self.assertTrue(hit.reflect)
        self.assertIs(fighter.action, move.ground_hit)

    def test_firefox_shallow_wall_redirects_but_steep_wall_does_not(self):
        move = Fox.specials.up
        attributes = SimpleNamespace(bound_angle_degrees=0.0)

        def attempt(normal):
            fighter = SimpleNamespace(
                action=move.travel_air,
                facing=1.0,
                velocity=[1.0, 1.0],
                action_state=SimpleNamespace(travel_angle=0.0),
            )
            fighter.set_motion_angle = lambda angle: setattr(
                fighter.action_state, "travel_angle", angle
            )
            context = SimpleNamespace(
                ceiling=None,
                wall=SimpleNamespace(normal=normal),
                resource=lambda path=None: SimpleNamespace(attributes=attributes),
            )
            with patch(
                "fighters.fox.math.angle_xy",
                side_effect=lambda candidate, velocity: (
                    0.785398 if candidate[0] else 2.356194
                ),
            ), patch("fighters.fox.math.facing", return_value=1.0), patch(
                "fighters.fox.math.atan2", return_value=0.785398
            ):
                return move.surface_contact(fighter, context), fighter

        redirected, fighter = attempt([1.0, 0.0, 0.0])
        self.assertTrue(redirected)
        self.assertAlmostEqual(fighter.action_state.travel_angle, 0.785398, places=5)

        rejected, fighter = attempt([0.0, -1.0, 0.0])
        self.assertFalse(rejected)
        self.assertEqual(fighter.action_state.travel_angle, 0.0)


if __name__ == "__main__":
    unittest.main()
