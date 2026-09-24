import importlib.util
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))


def _module():
    spec = importlib.util.spec_from_file_location(
        "mewtwo_test", ROOT / "scripts" / "fighters" / "mewtwo.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.Mewtwo.name = "mewtwo"
    module.Mewtwo.external_ids = (10,)
    return module


class _Input:
    def __init__(self, stick=(0.0, 0.0)):
        self.stick = stick

    def just_pressed(self, button):
        return True


class _Fighter:
    action = None
    action_frame = 0

    def __init__(self):
        self.changes = []
        self.action_state = self._module_state()
        self.velocity = (2.0, 3.0)
        self.ground_velocity = 2.0
        self.flags = SimpleNamespace(reflecting=False)

    @staticmethod
    def _module_state():
        return SimpleNamespace(command=(0, 0, 0, 0))

    def has_complete_animation(self, state):
        return 341 <= state <= 360

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def set_velocity(self, x, y):
        self.velocity = (x, y)


class MewtwoScriptTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.module = _module()
        cls.module.Mewtwo.external_ids = (10,)

    def test_source_motion_states_match_ftmewtwo_table(self):
        state = lambda phase: dict(phase.metadata)["slippi_state"]
        self.assertEqual(
            [state(phase) for phase in self.module.ShadowBall._ACTIVE],
            list(range(341, 351)),
        )
        self.assertEqual(
            [state(phase) for phase in self.module.Teleport._ACTIVE],
            list(range(353, 359)),
        )
        self.assertEqual(
            [state(phase) for phase in self.module.Confusion._ACTIVE], [351, 352]
        )
        self.assertEqual(
            [state(phase) for phase in self.module.Disable._ACTIVE], [359, 360]
        )

    def test_special_entry_requires_its_native_resource(self):
        fighter = _Fighter()
        context = SimpleNamespace(
            input=_Input(), ground_open=True, air_open=False, resource=lambda path: None
        )
        self.assertFalse(self.module.ShadowBall().input_pressed(fighter, context))
        self.assertEqual(fighter.changes, [])

    def test_directional_special_uses_shared_dispatch_threshold(self):
        fighter = _Fighter()
        context = SimpleNamespace(
            input=_Input((0.0, 1.0)),
            ground_open=False,
            air_open=True,
            rules=SimpleNamespace(
                specials=SimpleNamespace(vertical_threshold=0.5, side_stick_threshold=0.5)
            ),
            resource=lambda path: object(),
        )
        self.assertTrue(self.module.Teleport().input_pressed(fighter, context))
        self.assertEqual(fighter.changes[0][0], self.module.Teleport.air_start)

    def test_terminal_and_surface_rules_preserve_native_phase_pairs(self):
        move = self.module.Confusion()
        fighter = _Fighter()
        fighter.action = move.air
        move._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.changes[0][0], move.ground)

        fighter.action = move.ground
        move._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.changes[1][0].value, "wait")

    def test_shadow_ball_command_lifecycle_and_lr_cancel_match_source_callbacks(self):
        move = self.module.ShadowBall()
        fighter = _Fighter()
        fighter.action = move.ground_start
        move.enter(fighter, SimpleNamespace(grounded=True))
        move.create_held_shadow(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))
        self.assertTrue(fighter.action_state.shadow_ball_held)

        fighter.action = move.ground_loop
        move.release_input(fighter, SimpleNamespace())
        self.assertEqual(fighter.changes[-1][0], move.ground_end)
        # The source transitions into End first; its animation callback later
        # consumes command 1 and performs the article release.
        self.assertFalse(fighter.action_state.shadow_ball_released)
        self.assertTrue(fighter.action_state.shadow_ball_held)

        fighter.action = move.ground_loop
        move.input_pressed(fighter, SimpleNamespace(input=_Input()))
        self.assertEqual(fighter.changes[-1][0], move.ground_cancel)
        self.assertFalse(fighter.action_state.shadow_ball_held)

    def test_shadow_ball_release_marker_consumes_source_command_in_end_state(self):
        move = self.module.ShadowBall()
        fighter = _Fighter()
        fighter.action = move.ground_end
        fighter.action_state.shadow_ball_held = True

        move.release_shadow(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))

        # The Python boundary records the source command, but an actual launch
        # remains native until the article bridge exists.
        self.assertFalse(fighter.action_state.shadow_ball_held)
        self.assertTrue(fighter.action_state.shadow_ball_released)

    def test_shadow_ball_aerial_entry_halves_vertical_momentum(self):
        move = self.module.ShadowBall()
        fighter = _Fighter()
        fighter.action = move.air_start
        move.enter(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.velocity, (2.0, 1.5))

    def test_teleport_aerial_entry_uses_authored_velocity_divisors(self):
        move = self.module.Teleport()
        fighter = _Fighter()
        fighter.action = move.air_start

        attributes = {
            "up.attributes.teleport_vel_div_x": 4.0,
            "up.attributes.teleport_vel_div_y": 3.0,
        }
        move.enter(
            fighter,
            SimpleNamespace(
                resource=lambda path: attributes.get(path),
            ),
        )

        self.assertEqual(fighter.velocity, (0.5, 1.0))

    def test_teleport_aerial_entry_fails_closed_without_authored_divisors(self):
        move = self.module.Teleport()
        fighter = _Fighter()
        fighter.action = move.air_start

        move.enter(fighter, SimpleNamespace(resource=lambda path: None))

        self.assertEqual(fighter.velocity, (2.0, 3.0))

    def test_teleport_aerial_entry_fails_closed_for_invalid_authored_divisor(self):
        move = self.module.Teleport()
        fighter = _Fighter()
        fighter.action = move.air_start
        attributes = {
            "up.attributes.teleport_vel_div_x": 0.0,
            "up.attributes.teleport_vel_div_y": float("nan"),
        }

        move.enter(
            fighter,
            SimpleNamespace(resource=lambda path: attributes.get(path)),
        )

        self.assertEqual(fighter.velocity, (2.0, 3.0))

    def test_shadow_ball_release_marker_consumes_source_command(self):
        move = self.module.ShadowBall()
        fighter = _Fighter()
        fighter.action = move.ground_end
        fighter.action_state.shadow_ball_held = True
        fighter.action_state.shadow_ball_charge = 2
        fighter.action_state.command = (0, 1, 0, 0)

        move.release_shadow(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))

        self.assertFalse(fighter.action_state.shadow_ball_held)
        self.assertEqual(fighter.action_state.shadow_ball_charge, 0)
        self.assertEqual(fighter.action_state.command[1], 2)

    def test_damage_clears_unfinished_shadow_ball_and_disable(self):
        shadow = self.module.ShadowBall()
        fighter = _Fighter()
        fighter.action_state.shadow_ball_held = True
        fighter.action_state.shadow_ball_charge = 3
        shadow.on_damage(fighter, SimpleNamespace())
        self.assertFalse(fighter.action_state.shadow_ball_held)
        self.assertEqual(fighter.action_state.shadow_ball_charge, 0)

        disable = self.module.Disable()
        fighter.action_state.disable_fired = True
        disable.on_damage(fighter, SimpleNamespace())
        self.assertFalse(fighter.action_state.disable_fired)

    def test_confusion_reflect_commands_toggle_portable_flag(self):
        move = self.module.Confusion()
        fighter = _Fighter()
        fighter.action = move.ground
        fighter.action_state.command = (0, 1, 7, 8)
        move.reflect_command(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))
        self.assertTrue(fighter.flags.reflecting)
        self.assertTrue(fighter.action_state.confusion_reflecting)
        self.assertEqual(fighter.action_state.command, (0, 0, 7, 8))
        fighter.action_state.command = (0, 2, 7, 8)
        move.reflect_command(fighter, SimpleNamespace(event=SimpleNamespace(value=2)))
        self.assertFalse(fighter.flags.reflecting)
        self.assertEqual(fighter.action_state.command, (0, 0, 7, 8))

    def test_confusion_entry_clears_stale_command_and_grab_latches(self):
        move = self.module.Confusion()
        fighter = _Fighter()
        fighter.action = move.air
        fighter.action_state.command = (1, 2, 3, 4)
        fighter.action_state.confusion_grabbed = True
        move.enter(fighter, SimpleNamespace())
        self.assertEqual(fighter.action_state.command, (0, 0, 3, 4))
        self.assertFalse(fighter.action_state.confusion_grabbed)
        self.assertFalse(fighter.action_state.confusion_reflecting)

    def test_confusion_grab_consumes_only_command_zero(self):
        move = self.module.Confusion()
        fighter = _Fighter()
        fighter.action = move.ground
        fighter.victim_gobj = object()
        fighter.action_state.command = (1, 2, 3, 4)

        move.grab_command(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))

        self.assertTrue(fighter.action_state.confusion_grabbed)
        self.assertEqual(fighter.action_state.command, (0, 2, 3, 4))

    def test_confusion_grab_command_requires_and_records_a_victim(self):
        move = self.module.Confusion()
        fighter = _Fighter()
        fighter.action = move.ground

        move.grab_command(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))
        self.assertFalse(getattr(fighter.action_state, "confusion_grabbed", False))

        fighter.victim_gobj = object()
        move.grab_command(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))
        self.assertTrue(fighter.action_state.confusion_grabbed)

    def test_confusion_surface_transition_preserves_reflect_and_air_boost_latch(self):
        move = self.module.Confusion()
        fighter = _Fighter()
        fighter.action = move.air
        fighter.action_state.confusion_active = True
        fighter.action_state.confusion_reflecting = True
        fighter.flags.reflecting = True
        fighter.action_state.confusion_air_boosted = True
        fighter.action_state.command = (1, 2, 3, 4)
        fighter.velocity = (2.0, 7.0)

        # Source AirToGround/GroundToAir handlers preserve these latches;
        # only a fresh SpecialS entry resets them.
        fighter.action = move.ground
        move.enter(fighter, SimpleNamespace())
        self.assertFalse(fighter.action_state.confusion_air_boosted)
        fighter.action = move.air
        move.enter(
            fighter,
            SimpleNamespace(
                resource=lambda path: SimpleNamespace(air_boost=12.0)
                if path == "side.attributes"
                else None,
            ),
        )

        self.assertEqual(fighter.action_state.command, (1, 2, 3, 4))
        self.assertTrue(fighter.flags.reflecting)
        self.assertTrue(fighter.action_state.confusion_reflecting)
        self.assertEqual(fighter.velocity, (2.0, 7.0))
        self.assertFalse(fighter.action_state.confusion_air_boosted)

    def test_confusion_fresh_aerial_entry_applies_authored_boost_once(self):
        move = self.module.Confusion()
        fighter = _Fighter()
        fighter.action = move.air

        move.enter(
            fighter,
            SimpleNamespace(
                resource=lambda path: SimpleNamespace(air_boost=8.0)
                if path == "side.attributes"
                else None,
            ),
        )

        self.assertEqual(fighter.velocity, (2.0, 8.0))
        self.assertTrue(fighter.action_state.confusion_air_boosted)

    def test_disable_and_teleport_keep_source_local_state(self):
        disable = self.module.Disable()
        fighter = _Fighter()
        fighter.action = disable.air
        fighter.action_state.command = (9, 8, 7, 6)
        disable.enter(fighter, SimpleNamespace())
        self.assertEqual(fighter.velocity[1], 0.0)
        self.assertEqual(fighter.action_state.command, (0, 8, 7, 6))
        fighter.action_state.command = (4, 3, 2, 1)
        disable.create_disable(fighter, SimpleNamespace(event=SimpleNamespace(value=1)))
        self.assertTrue(fighter.action_state.disable_fired)
        self.assertEqual(fighter.action_state.command, (0, 3, 2, 1))

        teleport = self.module.Teleport()
        fighter.velocity = (2.0, 3.0)
        fighter.action = teleport.air_start
        fighter.action_state.command = (4, 3, 2, 1)
        teleport.enter(
            fighter,
            SimpleNamespace(
                resource=lambda path: {
                    "up.attributes.teleport_vel_div_x": 2.0,
                    "up.attributes.teleport_vel_div_y": 2.0,
                }.get(path),
            ),
        )
        self.assertEqual(fighter.velocity, (1.0, 1.5))
        self.assertEqual(fighter.action_state.command, (0, 3, 2, 1))
        fighter.action = teleport.air_travel
        teleport.begin_travel(fighter, SimpleNamespace())
        self.assertTrue(fighter.action_state.teleport_active)

        teleport.end_travel(fighter, SimpleNamespace())
        self.assertFalse(fighter.action_state.teleport_active)

        disable = self.module.Disable()
        fighter.action = disable.ground
        fighter.action_state.disable_fired = True
        disable.end_disable(fighter, SimpleNamespace())
        self.assertFalse(fighter.action_state.disable_fired)

    def test_teleport_aerial_landing_honors_timer_and_special_lag(self):
        move = self.module.Teleport()
        fighter = _Fighter()
        fighter.action = move.air_travel

        move.travel_landed(
            fighter, SimpleNamespace(teleport_timer_ready=False)
        )
        self.assertEqual(fighter.changes, [])

        move.travel_landed(
            fighter, SimpleNamespace(teleport_timer_ready=True)
        )
        self.assertEqual(fighter.changes[-1][0], move.ground_travel)
        self.assertEqual(
            fighter.changes[-1][1], {"preserve_state": True, "keep_frame": True}
        )

        fighter.action = move.air_end
        fighter.action_state.teleport_active = True
        move.enter_fall_special(fighter, SimpleNamespace())
        self.assertEqual(fighter.changes[-1][0], self.module.Action.SPECIAL_HI_LANDING)
        self.assertFalse(fighter.action_state.teleport_active)

    def test_teleport_ground_lost_transitions_match_source_collision_callbacks(self):
        move = self.module.Teleport()
        fighter = _Fighter()

        fighter.action = move.ground_travel
        move._transition_ground_air(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.changes[-1][0], move.air_travel)
        self.assertEqual(
            fighter.changes[-1][1], {"preserve_state": True, "keep_frame": True}
        )

        fighter.action = move.ground_end
        move._transition_ground_air(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.changes[-1][0], move.air_end)

    def test_teleport_aerial_end_passes_authored_landing_lag(self):
        move = self.module.Teleport()
        fighter = _Fighter()
        calls = []
        fighter.enter_landing_special = lambda *args: calls.append(args)
        fighter.action = move.air_end

        move.enter_fall_special(
            fighter,
            SimpleNamespace(
                resource=lambda path: {
                    "up.attributes.teleport_landing_lag": 12.5,
                    "escape_air": SimpleNamespace(landing_animation_end=3.5),
                }.get(path),
            ),
        )

        self.assertEqual(calls, [(3.5, 12.5)])
        self.assertFalse(fighter.action_state.teleport_active)
