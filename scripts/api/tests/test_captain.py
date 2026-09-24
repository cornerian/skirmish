"""Focused contract tests for the partial Captain Falcon authoring module."""

import math as scalar_math
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

from skirmish import Action, Button, Fighter, FighterBase, action, export_definition, math
from fighters.captain import CaptainFalcon


def _load_captain():
    return CaptainFalcon


class CaptainFalconTests(unittest.TestCase):
    class Input:
        def __init__(self, pressed=(), stick=(0.0, 0.0)):
            self.pressed = set(pressed)
            self.stick = stick

        def just_pressed(self, button):
            return button in self.pressed

    class Fighter:
        def __init__(self, action, *, grounded=False):
            captain = _load_captain()
            self.action = action
            self.action_state = captain.__dict__["action_state"]()
            self.action_frame = 0
            self.grounded = grounded
            self.ground_velocity = 3.0
            self.velocity = [3.0, 4.0]
            self.facing = 1.0
            self.resource_value = None
            self.changes = []
            self.fall_special = []

        def change_action(self, action, **kwargs):
            self.changes.append((action, kwargs))
            self.action = action

        def enter_fall_special(self, **kwargs):
            self.fall_special.append(kwargs)

        def resource(self, path):
            resource = self.resource_value
            if resource is None:
                return None
            return (getattr(resource, "attributes", None)
                    if path.endswith(".attributes") else resource)

    @staticmethod
    def context(*, resource_value=object(), input_value=None, ground_open=False,
                air_open=False, event_value=None, grounded=False):
        resource = resource_value
        return SimpleNamespace(
            input=input_value or CaptainFalconTests.Input(),
            ground_open=ground_open,
            air_open=air_open,
            grounded=grounded,
            event=SimpleNamespace(value=event_value),
            resource=lambda path: (getattr(resource, "attributes", None)
                                    if path.endswith(".attributes") else resource),
        )

    def test_b_entry_requires_resource_and_starts_ground_or_air_action(self):
        captain = _load_captain()
        move = captain.specials.neutral
        ground = self.Fighter(None)
        context = self.context(input_value=self.Input((Button.B,)), ground_open=True)
        self.assertTrue(move.input_pressed(ground, context))
        self.assertEqual(ground.action, move.ground)
        self.assertEqual(ground.action_frame, 1)

        air = self.Fighter(None)
        context = self.context(input_value=self.Input((Button.B,)), air_open=True)
        self.assertTrue(move.input_pressed(air, context))
        self.assertEqual(air.action, move.air)
        self.assertEqual(air.action_frame, 1)

        blocked = self.Fighter(None)
        context = self.context(input_value=self.Input((Button.B,)))
        self.assertFalse(move.input_pressed(blocked, context))
        missing = self.Fighter(None)
        context = self.context(resource_value=None, input_value=self.Input((Button.B,)), ground_open=True)
        self.assertFalse(move.input_pressed(missing, context))

    def test_neutral_punch_reserves_inclusive_directional_threshold(self):
        captain = _load_captain()
        move = captain.specials.neutral
        rules = SimpleNamespace(specials=SimpleNamespace(
            vertical_threshold=0.5,
            horizontal_threshold=0.5,
        ))

        def attempt(stick):
            fighter = self.Fighter(None)
            context = self.context(
                input_value=self.Input((Button.B,), stick),
                ground_open=True,
            )
            context.rules = rules
            return move.input_pressed(fighter, context), fighter

        accepted, fighter = attempt((0.5 - 1e-6, 0.0))
        self.assertTrue(accepted)
        self.assertEqual(fighter.action, move.ground)

        rejected, fighter = attempt((0.5, 0.0))
        self.assertFalse(rejected)
        self.assertIsNone(fighter.action)

    def test_terminal_animation_exits_ground_to_wait_and_air_to_fall(self):
        captain = _load_captain()
        move = captain.specials.neutral
        ground = self.Fighter(move.ground)
        move._transition_animation_end(ground, self.context())
        self.assertEqual(ground.changes, [(
            Action.WAIT, {"preserve_state": False, "keep_frame": False}
        )])
        air = self.Fighter(move.air)
        move._transition_animation_end(air, self.context())
        self.assertEqual(air.changes, [(
            Action.FALL, {"preserve_state": False, "keep_frame": False}
        )])

    def test_surface_transitions_preserve_state_and_frame(self):
        captain = _load_captain()
        move = captain.specials.neutral
        air = self.Fighter(move.air)
        move._transition_ground_air(air, SimpleNamespace(grounded=True))
        self.assertEqual(air.changes, [(move.ground, {"preserve_state": True, "keep_frame": True})])
        ground = self.Fighter(move.ground)
        move._transition_ground_air(ground, SimpleNamespace(grounded=False))
        self.assertEqual(ground.changes, [(move.air, {"preserve_state": True, "keep_frame": True})])

    def test_command_cue_is_air_only_and_ignores_zero(self):
        captain = _load_captain()
        move = captain.specials.neutral
        air = self.Fighter(move.air)
        move.command_changed(air, self.context(event_value=1))
        self.assertTrue(air.action_state.launch_armed)
        ground = self.Fighter(move.ground)
        move.command_changed(ground, self.context(event_value=1))
        self.assertFalse(ground.action_state.launch_armed)
        move.command_changed(air, self.context(event_value=0))
        self.assertTrue(air.action_state.launch_armed)
        self.assertEqual(air.action_state.command[0], 0)

        resource = SimpleNamespace(attributes=SimpleNamespace(
            specialn_stick_range_y_neg=-1.0,
            specialn_stick_range_y_pos=1.0,
            specialn_angle_diff=90.0,
            specialn_vel_x=2.0,
        ))
        cue_context = self.context(
            resource_value=resource,
            input_value=self.Input((), (0.0, 0.5)),
            event_value=1,
        )
        air.velocity = [0.0, 0.0]
        with (
            patch.object(math, "_cos", scalar_math.cos),
            patch.object(math, "_sin", scalar_math.sin),
        ):
            move.command_changed(air, cue_context)
        self.assertAlmostEqual(float(air.velocity[0]), 2.0 * 0.3826834324, places=6)
        self.assertAlmostEqual(float(air.velocity[1]), 2.0 * 0.9238795325, places=6)

    def test_enter_clears_pending_command_and_validation_requires_both_traces(self):
        captain = _load_captain()
        move = captain.specials.neutral
        fighter = self.Fighter(move.ground)
        fighter.action_state.command = (7, 7, 7, 7)
        fighter.action_state.launch_armed = True
        move.enter(fighter, self.context())
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))
        self.assertFalse(fighter.action_state.launch_armed)

        class ValidContext:
            def resource(self, path):
                return SimpleNamespace(
                    cmd_vars=[[0, 0, 0, 0]], allow_interrupt=[False],
                    attributes=SimpleNamespace(
                        specialn_stick_range_y_neg=-1.0,
                        specialn_stick_range_y_pos=1.0,
                        specialn_angle_diff=90.0,
                        specialn_vel_x=2.0,
                        specialn_vel_mul=0.5,
                    ),
                )

            def frames(self, path):
                return 1

            def array_length(self, path, index=None):
                return 4 if index is not None and path.endswith("cmd_vars") else 1

        self.assertTrue(move.validate(ValidContext()))
        self.assertTrue(move.validate(self.context(resource_value=None)))

        class InvalidContext:
            def resource(self, path):
                return object()

        self.assertFalse(move.validate(InvalidContext()))

    def test_punch_air_profile_declares_only_the_source_command_one_scale(self):
        captain = _load_captain()
        exported = export_definition(captain).as_dict()
        action_state = exported["action_state"]
        self.assertEqual(action_state["command"], (0, 0, 0, 0))
        air = exported["actions"]["special.neutral.air"]
        operation = air["motion"]["kwargs"]["air"][0]
        self.assertEqual(operation["callee"], "motion.command_velocity_scale")
        self.assertEqual(operation["kwargs"]["index"], 1)
        self.assertEqual(operation["kwargs"]["value"], 1)
        self.assertEqual(operation["kwargs"]["multiplier"], {
            "callee": "resource",
            "args": ["neutral.attributes.specialn_vel_mul"],
            "kwargs": {},
        })

    def test_punch_entries_bind_real_ground_and_air_attack_resources(self):
        captain = _load_captain()
        move = captain.specials.neutral

        self.assertEqual(
            move.ground.as_dict(),
            {
                "action": "Action.SPECIAL_N_START",
                "slippi_state": 347,
                "animation": 301,
                "animation_loop": False,
                "attack": "neutral.ground",
                "command_trace": "neutral.script.ground",
            },
        )
        self.assertEqual(
            move.air.as_dict(),
            {
                "action": "Action.SPECIAL_AIR_N_START",
                "slippi_state": 348,
                "animation": 302,
                "animation_loop": False,
                "attack": "neutral.air",
                "command_trace": "neutral.script.air",
                "motion": move.air.as_dict()["motion"],
            },
        )

    def test_falcon_dive_uses_upward_dispatch_and_353_354_states(self):
        captain = _load_captain()
        move = captain.specials.up
        rules = SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5,
                                                          horizontal_threshold=0.5))
        ground = self.Fighter(None, grounded=True)
        context = self.context(input_value=self.Input((Button.B,), (-0.6625, 0.7375)),
                               ground_open=True)
        context.rules = rules
        self.assertTrue(move.input_pressed(ground, context))
        self.assertEqual(ground.action, move.ground)
        self.assertEqual(move.ground.as_dict()["slippi_state"], 353)
        self.assertEqual(move.ground.as_dict()["attack"], "up.ground")

        air = self.Fighter(None)
        context = self.context(input_value=self.Input((Button.B,), (-0.6625, 0.7375)),
                               air_open=True)
        context.rules = rules
        self.assertTrue(move.input_pressed(air, context))
        self.assertEqual(air.action, move.air)
        self.assertEqual(move.air.as_dict()["slippi_state"], 354)
        self.assertEqual(move.air.as_dict()["attack"], "up.air")

        punch = captain.specials.neutral
        context = self.context(input_value=self.Input((Button.B,), (-0.6625, 0.7375)),
                               ground_open=True)
        context.rules = rules
        self.assertFalse(punch.input_pressed(self.Fighter(None), context))

    def test_falcon_dive_consumes_fresh_b_once(self):
        class OneShotInput(self.Input):
            def __init__(self, pressed, stick):
                super().__init__(pressed, stick)
                self.calls = 0

            def just_pressed(self, button):
                self.calls += 1
                return self.calls == 1 and super().just_pressed(button)

        captain = _load_captain()
        move = captain.specials.up
        context = self.context(
            input_value=OneShotInput((Button.B,), (0.0, 0.75)),
            air_open=True,
        )
        context.rules = SimpleNamespace(
            specials=SimpleNamespace(vertical_threshold=0.5)
        )
        fighter = self.Fighter(None)

        self.assertTrue(move.input_pressed(fighter, context))
        self.assertEqual(fighter.action, move.air)
        self.assertEqual(context.input.calls, 1)

    def test_falcon_dive_ground_to_air_transition_preserves_state_and_frame(self):
        captain = _load_captain()
        move = captain.specials.up
        ground = self.Fighter(move.ground)
        move._transition_ground_air(ground, SimpleNamespace(grounded=False))
        self.assertEqual(
            ground.changes,
            [(move.air, {"preserve_state": True, "keep_frame": True})],
        )
        self.assertEqual(move.on_ground, {})

    def test_falcon_dive_catch_throw_states_and_attacker_lifecycle(self):
        captain = _load_captain()
        move = captain.specials.up
        self.assertEqual(move.catch.as_dict()["slippi_state"], 355)
        self.assertEqual(move.catch.as_dict()["animation"], 309)
        self.assertEqual(move.throw.as_dict()["slippi_state"], 356)
        self.assertEqual(move.throw.as_dict()["animation"], 310)

        # The before-hit hook models only the attacker's documented state
        # transition. Victim capture and attachment are host-owned.
        fighter = self.Fighter(move.air)
        move.before_hit(fighter, SimpleNamespace())
        self.assertEqual(fighter.changes, [(move.catch, {})])
        move._transition_animation_end(fighter, SimpleNamespace())
        self.assertEqual(
            fighter.changes[-1],
            (Action.SPECIAL_HI_THROW,
             {"preserve_state": False, "keep_frame": False}),
        )
        move._transition_animation_end(fighter, SimpleNamespace())
        self.assertEqual(
            fighter.changes[-1],
            (Action.FALL, {"preserve_state": False, "keep_frame": False}),
        )

    def test_falcon_dive_declares_source_wall_rebound_motion(self):
        """Keep ftCa_MS_SpecialHiThrow1 available to collision routing."""
        captain = _load_captain()
        move = captain.specials.up
        rebound = move.throw_rebound.as_dict()
        self.assertEqual(rebound["action"], "Action.Source.0:363")
        self.assertEqual(rebound["slippi_state"], 363)
        self.assertEqual(rebound["animation"], 317)

    def test_falcon_dive_wall_rebound_animation_enters_fall(self):
        captain = _load_captain()
        move = captain.specials.up
        fighter = self.Fighter(move.throw_rebound)

        move.throw_rebound_animation_end(fighter, SimpleNamespace())

        self.assertEqual(
            fighter.changes,
            [(Action.FALL, {})],
        )

        exported = export_definition(captain).as_dict()
        behavior = next(
            item for item in exported["behaviors"]
            if item["id"] == exported["movesets"]["specials"]["up"]
        )
        rebound_callback = next(
            callback for callback in behavior["callbacks"]
            if callback["callback"].endswith("throw_rebound_animation_end")
        )
        self.assertEqual(rebound_callback["hook"], "animation_ended")
        self.assertEqual(rebound_callback["actions"], ["Source.0:363"])

    def test_falcon_dive_terminal_and_landing_semantics_use_resource_attributes(self):
        captain = _load_captain()
        move = captain.specials.up
        attributes = SimpleNamespace(specialhi_freefall_air_spd_mul=0.8,
                                     specialhi_landing_lag=20)
        resource = SimpleNamespace(attributes=attributes)
        context = self.context(resource_value=resource)
        ground = self.Fighter(move.ground)
        move.animation_end(ground, context)
        self.assertEqual(ground.fall_special, [{"mobility": 0.8, "landing_lag": 20}])

        air = self.Fighter(move.air)
        move.enter(air, context)
        self.assertFalse(move.landed(air, context))
        self.assertEqual(air.changes, [])
        air.action = move.air
        move.command_changed(air, SimpleNamespace(event=SimpleNamespace(value=1)))
        move.landed(air, context)
        self.assertEqual(air.fall_special, [{"mobility": 0.8, "landing_lag": 20}])

    def test_falcon_dive_command_updates_facing_at_strict_resource_threshold(self):
        captain = _load_captain()
        move = captain.specials.up
        fighter = self.Fighter(move.air)
        resource = SimpleNamespace(attributes=SimpleNamespace(
            specialhi_input_var=0.225,
        ))
        context = self.context(
            resource_value=resource,
            input_value=self.Input((), (-0.6625, 0.7375)),
            event_value=1,
        )
        move.command_changed(fighter, context)
        self.assertEqual(fighter.facing, -1.0)
        self.assertTrue(fighter.action_state.dive_released)

        fighter.facing = 1.0
        context.input = self.Input((), (0.225, 0.0))
        move.command_changed(fighter, context)
        self.assertEqual(fighter.facing, 1.0)

        context.input = self.Input((), (0.5, 0.0))
        move.command_changed(fighter, context)
        self.assertEqual(fighter.facing, 1.0)

    def test_falcon_dive_validation_rejects_missing_or_bad_dispatch_threshold(self):
        captain = _load_captain()
        move = captain.specials.up
        self.assertFalse(move.validate(SimpleNamespace(
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.0)))))
        self.assertFalse(move.validate(SimpleNamespace(
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=1.5)))))
        self.assertTrue(move.validate(SimpleNamespace(
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5)))))

        resource = SimpleNamespace(attributes=SimpleNamespace(
            specialhi_input_var=0.225,
        ))
        valid = self.context(resource_value=resource)
        valid.rules = SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5))
        self.assertTrue(move.validate(valid))
        resource.attributes.specialhi_input_var = 0.0
        self.assertFalse(move.validate(valid))

    def test_raptor_boost_dispatch_requires_horizontal_b_and_clears_entry_velocity(self):
        captain = _load_captain()
        move = captain.specials.side
        rules = SimpleNamespace(specials=SimpleNamespace(side_stick_threshold=0.5))
        resource = SimpleNamespace(attributes=SimpleNamespace(
            specials_miss_landing_lag=20.0,
            specials_hit_landing_lag=30.0,
            specials_grav=0.08,
            specials_terminal_vel=2.4,
        ))

        ground = self.Fighter(None)
        context = self.context(
            resource_value=resource,
            input_value=self.Input((Button.B,), (-0.6, 0.0)),
            ground_open=True,
        )
        context.rules = rules
        self.assertTrue(move.input_pressed(ground, context))
        self.assertEqual(ground.action, move.ground_start)
        self.assertEqual(ground.action_frame, 1)
        self.assertEqual(ground.velocity, (0.0, 0.0))
        self.assertEqual(ground.ground_velocity, 0.0)

        air = self.Fighter(None)
        context = self.context(
            resource_value=resource,
            input_value=self.Input((Button.B,), (0.6, 0.0)),
            air_open=True,
        )
        context.rules = rules
        self.assertTrue(move.input_pressed(air, context))
        self.assertEqual(air.action, move.air_start)
        self.assertEqual(air.action_frame, 1)
        self.assertEqual(air.velocity, (0.0, 0.0))

        for stick_x in (0.0, 0.49):
            context = self.context(
                resource_value=resource,
                input_value=self.Input((Button.B,), (stick_x, 0.0)),
                ground_open=True,
            )
            context.rules = rules
            self.assertFalse(move.input_pressed(self.Fighter(None), context))
        for stick_x in (-0.5, 0.5):
            context = self.context(
                resource_value=resource,
                input_value=self.Input((Button.B,), (stick_x, 0.0)),
                ground_open=True,
            )
            context.rules = rules
            self.assertTrue(move.input_pressed(self.Fighter(None), context))
        context = self.context(
            resource_value=None,
            input_value=self.Input((Button.B,), (0.8, 0.0)),
            ground_open=True,
        )
        context.rules = rules
        self.assertFalse(move.input_pressed(self.Fighter(None), context))

    def test_raptor_boost_fighter_contact_enters_follow_through_and_scales_ground_speed(self):
        captain = _load_captain()
        move = captain.specials.side

        attributes = SimpleNamespace(specials_gr_vel_x=0.75)
        ground = self.Fighter(move.ground_start)
        ground.resource_value = SimpleNamespace(attributes=attributes)
        ground.velocity = [2.0, 3.0]
        ground.ground_velocity = 4.0
        move.before_hit(ground, SimpleNamespace())
        self.assertEqual(ground.action, move.ground)
        self.assertEqual(ground.velocity, (2.0, 0.0))
        self.assertEqual(ground.ground_velocity, 3.0)

        air = self.Fighter(move.air_start)
        air.resource_value = SimpleNamespace(attributes=attributes)
        air.velocity = [2.0, 3.0]
        move.before_hit(air, SimpleNamespace())
        self.assertEqual(air.action, move.air)
        self.assertEqual(air.velocity, [2.0, 3.0])

        air_without_attributes = self.Fighter(move.air_start)
        move.before_hit(air_without_attributes, SimpleNamespace())
        self.assertEqual(air_without_attributes.action, move.air)

        for multiplier in (None, float("inf")):
            missing = SimpleNamespace(resource=lambda path, value=multiplier:
                                      SimpleNamespace(attributes=SimpleNamespace(
                                          specials_gr_vel_x=value)))
            untouched = self.Fighter(move.ground_start)
            untouched.resource_value = missing.resource(move.resource)
            move.before_hit(untouched, SimpleNamespace())
        self.assertEqual(untouched.action, move.ground_start)

    def test_raptor_boost_reads_fighter_resource_without_touching_hit_resource(self):
        captain = _load_captain()
        move = captain.specials.side
        attrs = SimpleNamespace(specials_gr_vel_x=0.5)

        class NativeHit:
            def resource(self, path):
                raise AssertionError("Raptor Boost must read attributes from fighter")

        fighter = self.Fighter(move.ground_start)
        fighter.resource_value = SimpleNamespace(attributes=attrs)
        fighter.velocity = [2.0, 3.0]
        fighter.ground_velocity = 4.0
        move.before_hit(fighter, NativeHit())
        self.assertEqual(fighter.action, move.ground)
        self.assertEqual(fighter.ground_velocity, 2.0)

    def test_falcon_kick_facing_wall_enters_state_363_rebound(self):
        captain = _load_captain()
        move = captain.specials.down
        fighter = self.Fighter(move.ground)
        self.assertFalse(move.wall_rebound(fighter, SimpleNamespace(wall=True)))
        fighter.action_state.command = (1, 0, 0, 0)
        self.assertFalse(
            move.wall_rebound(
                fighter,
                SimpleNamespace(wall=SimpleNamespace(normal=(1.0, 0.0, 0.0))),
            )
        )
        self.assertTrue(
            move.wall_rebound(
                fighter,
                SimpleNamespace(wall=SimpleNamespace(normal=(-1.0, 0.0, 0.0))),
            )
        )
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))
        self.assertIn(str(getattr(fighter.action, "reference", fighter.action)), {"Source.363", "Source.0:363"})

    def test_special_entries_consume_stale_command_slots(self):
        captain = _load_captain()
        for move, action in (
            (captain.specials.side, "ground_start"),
            (captain.specials.up, "ground"),
            (captain.specials.down, "ground"),
        ):
            fighter = self.Fighter(getattr(move, action))
            fighter.action_state.command = (7, 6, 5, 4)
            if move is captain.specials.side:
                move.action_enter(fighter, SimpleNamespace())
            elif move is captain.specials.up:
                move.enter(fighter, SimpleNamespace())
            else:
                move.action_enter(fighter, SimpleNamespace())
            expected = (0, 0, 0, 0) if move is captain.specials.side else (0, 0, 0, 4)
            self.assertEqual(fighter.action_state.command, expected)

    def test_raptor_boost_exports_source_action_metadata(self):
        captain = _load_captain()
        exported = export_definition(captain).as_dict()
        side = exported["movesets"]["specials"]["side"]
        behavior = next(item for item in exported["behaviors"] if item["id"] == side)
        self.assertEqual(behavior["resource"], "side")
        self.assertEqual(behavior["actions"]["ground_start"]["slippi_state"], 349)
        self.assertEqual(behavior["actions"]["ground_start"]["animation"], 303)
        self.assertEqual(behavior["actions"]["ground_start"]["attack"], "side.ground_start")
        self.assertEqual(behavior["actions"]["ground"]["slippi_state"], 350)
        self.assertEqual(behavior["actions"]["ground"]["animation"], 304)
        self.assertEqual(behavior["actions"]["ground"]["attack"], "side.ground")
        self.assertEqual(behavior["actions"]["air_start"]["slippi_state"], 351)
        self.assertEqual(behavior["actions"]["air_start"]["animation"], 305)
        self.assertEqual(behavior["actions"]["air_start"]["attack"], "side.air_start")
        self.assertEqual(behavior["actions"]["air"]["slippi_state"], 352)
        self.assertEqual(behavior["actions"]["air"]["animation"], 306)
        self.assertEqual(behavior["actions"]["air"]["attack"], "side.air")
        callbacks = behavior["callbacks"]
        self.assertFalse(any(
            callback["hook"] == "animation_ended"
            and callback.get("callback", "").endswith("_transition_animation_end")
            for callback in callbacks
        ))

    def test_raptor_boost_terminal_and_landing_lags_match_source_callbacks(self):
        captain = _load_captain()
        move = captain.specials.side
        resource = SimpleNamespace(attributes=SimpleNamespace(
            specials_miss_landing_lag=0.0,
            specials_hit_landing_lag=18.0,
            specials_grav=0.08,
            specials_terminal_vel=2.4,
        ))
        context = self.context(resource_value=resource)

        ground = self.Fighter(move.ground_start)
        move.animation_end_ground(ground, context)
        self.assertEqual(ground.changes, [(Action.WAIT, {})])
        ground = self.Fighter(move.ground)
        move.animation_end_ground(ground, context)
        self.assertEqual(ground.changes, [(Action.WAIT, {})])

        air_start = self.Fighter(move.air_start)
        move.animation_end_air(air_start, context)
        self.assertEqual(air_start.changes, [(Action.FALL, {})])
        self.assertEqual(air_start.fall_special, [])

        air = self.Fighter(move.air)
        move.animation_end_air(air, context)
        self.assertEqual(air.fall_special, [{"mobility": 1, "landing_lag": 18.0}])

        air_start = self.Fighter(move.air_start)
        self.assertTrue(move.landed(air_start, context))
        self.assertEqual(air_start.fall_special, [{"mobility": 1, "landing_lag": 0.0}])
        air = self.Fighter(move.air)
        self.assertTrue(move.landed(air, context))
        self.assertEqual(air.fall_special, [{"mobility": 1, "landing_lag": 18.0}])

    def test_raptor_boost_validation_requires_finite_nonnegative_lags(self):
        captain = _load_captain()
        move = captain.specials.side
        resource = SimpleNamespace(attributes=SimpleNamespace(
            specials_miss_landing_lag=0.0,
            specials_hit_landing_lag=18.0,
            specials_grav=0.08,
            specials_terminal_vel=2.4,
        ))
        valid = self.context(resource_value=resource)
        valid.rules = SimpleNamespace(specials=SimpleNamespace(side_stick_threshold=0.5))
        self.assertTrue(move.validate(valid))

        valid.rules.specials.side_stick_threshold = 0.0
        self.assertFalse(move.validate(valid))
        valid.rules.specials.side_stick_threshold = 0.5

        old_schema = self.context(resource_value=resource)
        old_schema.rules = SimpleNamespace(specials=SimpleNamespace(horizontal_threshold=0.5))
        self.assertFalse(move.validate(old_schema))

        valid.resource_value = None
        valid.resource = lambda path: None
        self.assertTrue(move.validate(valid))

        for field, value in (
            ("specials_miss_landing_lag", -1.0),
            ("specials_hit_landing_lag", float("inf")),
        ):
            resource = SimpleNamespace(attributes=SimpleNamespace(
                specials_miss_landing_lag=0.0,
                specials_hit_landing_lag=18.0,
                specials_grav=0.08,
                specials_terminal_vel=2.4,
            ))
            setattr(resource.attributes, field, value)
            invalid = self.context(resource_value=resource)
            invalid.rules = SimpleNamespace(specials=SimpleNamespace(side_stick_threshold=0.5))
            self.assertFalse(move.validate(invalid))

        for field, value in (
            ("specials_grav", -0.01),
            ("specials_terminal_vel", float("inf")),
        ):
            resource = SimpleNamespace(attributes=SimpleNamespace(
                specials_miss_landing_lag=0.0,
                specials_hit_landing_lag=18.0,
                specials_grav=0.08,
                specials_terminal_vel=2.4,
            ))
            setattr(resource.attributes, field, value)
            invalid = self.context(resource_value=resource)
            invalid.rules = SimpleNamespace(specials=SimpleNamespace(side_stick_threshold=0.5))
            self.assertFalse(move.validate(invalid))

    def test_raptor_boost_air_follow_through_binds_source_gravity_profile(self):
        captain = _load_captain()
        exported = export_definition(captain).as_dict()
        side = exported["movesets"]["specials"]["side"]
        behavior = next(item for item in exported["behaviors"] if item["id"] == side)
        air = behavior["actions"]["air"]
        gravity = air["motion"]["kwargs"]["air"][0]
        self.assertEqual(gravity["callee"], "motion.gravity")
        self.assertEqual(gravity["kwargs"]["acceleration"], {
            "callee": "resource",
            "args": ["side.attributes.specials_grav"],
            "kwargs": {},
        })
        self.assertEqual(gravity["kwargs"]["terminal_velocity"], {
            "callee": "resource",
            "args": ["side.attributes.specials_terminal_vel"],
            "kwargs": {},
        })
        self.assertEqual(gravity["kwargs"]["delay"], 0)
        self.assertNotIn("motion", behavior["actions"]["air_start"])

    def test_falcon_kick_dispatches_ground_or_air_and_reaches_recovery(self):
        captain = _load_captain()
        move = captain.specials.down
        rules = SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5,
                                                          horizontal_threshold=0.5))
        context = self.context(input_value=self.Input((Button.B,), (0.0, -0.7)),
                               ground_open=True)
        context.rules = rules
        fighter = self.Fighter(None, grounded=True)
        self.assertTrue(move.input_pressed(fighter, context))
        self.assertEqual(fighter.action, move.ground)
        self.assertEqual(move.ground.as_dict()["slippi_state"], 357)
        self.assertEqual(move.ground_end.as_dict()["slippi_state"], 358)

        move.animation_end(fighter, context)
        self.assertEqual(fighter.action, move.ground_end)
        move._transition_animation_end(fighter, context)
        self.assertEqual(
            fighter.changes[-1],
            (Action.WAIT, {"preserve_state": False, "keep_frame": False}),
        )

        airborne_ground_kick = self.Fighter(move.ground, grounded=False)
        move.animation_end(airborne_ground_kick, context)
        self.assertEqual(airborne_ground_kick.action, move.ground_end_air)
        self.assertEqual(move.ground_end_air.as_dict()["slippi_state"], 362)
        self.assertEqual(move.ground_end_air.as_dict()["animation"], 315)
        move._transition_animation_end(airborne_ground_kick, context)
        self.assertEqual(
            airborne_ground_kick.changes[-1],
            (Action.FALL, {"preserve_state": False, "keep_frame": False}),
        )

        context = self.context(input_value=self.Input((Button.B,), (0.0, -0.7)),
                               air_open=True)
        context.rules = rules
        fighter = self.Fighter(None)
        self.assertTrue(move.input_pressed(fighter, context))
        self.assertEqual(fighter.action, move.air)
        self.assertEqual(move.air.as_dict()["slippi_state"], 359)
        self.assertEqual(move.air_end.as_dict()["slippi_state"], 361)

        move._transition_animation_end(fighter, context)
        self.assertEqual(fighter.action, move.air_end)
        move._transition_animation_end(fighter, context)
        self.assertEqual(
            fighter.changes[-1],
            (Action.FALL, {"preserve_state": False, "keep_frame": False}),
        )

        up_context = self.context(input_value=self.Input((Button.B,), (0.0, 0.7)),
                                  air_open=True)
        up_context.rules = rules
        self.assertFalse(move.input_pressed(self.Fighter(None), up_context))
        neutral_context = self.context(input_value=self.Input((Button.B,), (0.0, 0.0)),
                                       air_open=True)
        neutral_context.rules = rules
        self.assertFalse(move.input_pressed(self.Fighter(None), neutral_context))

    def test_falcon_kick_landing_uses_source_end_motion_then_waits(self):
        captain = _load_captain()
        move = captain.specials.down
        context = self.context()

        self.assertEqual(move.landing.as_dict()["action"],
                         "Action.SPECIAL_AIR_LW_LANDING_END")
        self.assertEqual(move.landing.as_dict()["slippi_state"], 360)
        self.assertEqual(move.landing.as_dict()["animation"], 314)

        for airborne in (move.air, move.air_end):
            fighter = self.Fighter(airborne)
            self.assertTrue(move.landed(fighter, context))
            self.assertEqual(fighter.changes, [(move.landing, {})])
            move._transition_animation_end(fighter, context)
            self.assertEqual(
                fighter.changes[-1],
                (Action.WAIT, {"preserve_state": False, "keep_frame": False}),
            )

    def test_directional_special_dispatch_uses_inclusive_boundaries(self):
        """Keep Captain's B-direction partition stable when helpers are shared."""
        captain = _load_captain()
        rules = SimpleNamespace(specials=SimpleNamespace(
            vertical_threshold=0.5,
            horizontal_threshold=0.5,
            side_stick_threshold=0.5,
        ))

        def context(stick, *, ground_open=True, resource_value=object()):
            value = self.context(
                resource_value=resource_value,
                input_value=self.Input((Button.B,), stick),
                ground_open=ground_open,
            )
            value.rules = rules
            return value

        punch = captain.specials.neutral
        self.assertFalse(punch.input_pressed(self.Fighter(None), context((0.5, 0.0))))
        self.assertFalse(punch.input_pressed(self.Fighter(None), context((0.0, -0.5))))

        partial_rules = SimpleNamespace(specials=SimpleNamespace(
            vertical_threshold=0.5,
            horizontal_threshold=None,
        ))
        partial = context((0.0, 0.0))
        partial.rules = partial_rules
        self.assertTrue(punch.input_pressed(self.Fighter(None), partial))

        side = captain.specials.side
        self.assertTrue(side.input_pressed(self.Fighter(None), context((0.5, 0.0))))

        dive = captain.specials.up
        self.assertTrue(dive.input_pressed(self.Fighter(None), context((0.0, 0.5))))

        kick = captain.specials.down
        self.assertTrue(kick.input_pressed(self.Fighter(None), context((0.0, -0.5))))

        self.assertFalse(kick.input_pressed(
            self.Fighter(None),
            context((0.0, -0.8), resource_value=None),
        ))
        self.assertFalse(kick.input_pressed(
            self.Fighter(None),
            self.context(resource_value=object(), input_value=self.Input(),
                         ground_open=True),
        ))

    def test_landing_descriptor_uses_canonical_api_action(self):
        self.assertEqual(action(Action.LANDING).as_dict()["action"], "Action.LANDING")

    def test_exports_identity_and_resource_backed_punch(self):
        captain = _load_captain()
        exported = export_definition(captain).as_dict()
        self.assertEqual(exported["name"], "captain-falcon")
        self.assertEqual(exported["external_ids"], [0])
        neutral = exported["movesets"]["specials"]["neutral"]
        self.assertEqual(neutral, "move_0")
        behavior = next(item for item in exported["behaviors"] if item["id"] == neutral)
        self.assertEqual(behavior["resource"], "neutral")
        self.assertEqual(exported["actions"]["special.neutral.ground"]["slippi_state"], 347)
        self.assertEqual(exported["actions"]["special.neutral.air"]["slippi_state"], 348)

        kick_end = exported["actions"]["special.down.air_end"]
        self.assertEqual(
            kick_end["motion"]["kwargs"]["air"],
            [
                {
                    "callee": "motion.gravity",
                    "args": [],
                    "kwargs": {
                        "acceleration": {"callee": "parameter", "args": ["movement.gravity"], "kwargs": {}},
                        "terminal_velocity": {"callee": "parameter", "args": ["movement.terminal_velocity"], "kwargs": {}},
                        "delay": 0,
                    },
                },
                {
                    "callee": "motion.air_friction",
                    "args": [],
                    "kwargs": {"amount": {"callee": "parameter", "args": ["movement.aerial_friction"], "kwargs": {}}},
                },
            ],
        )
        self.assertNotIn("motion", exported["actions"]["special.down.air"])

        expected_animation_by_action = {
            "special.neutral.ground": 301,
            "special.neutral.air": 302,
            "special.up.ground": 307,
            "special.up.air": 308,
            "special.up.catch": 309,
            "special.up.throw": 310,
            "special.down.ground": 311,
            "special.down.ground_end": 312,
            "special.down.air": 313,
            "special.down.landing": 314,
            "special.down.air_end": 316,
            "special.down.ground_end_air": 315,
        }
        for action_name, animation in expected_animation_by_action.items():
            self.assertEqual(exported["actions"][action_name]["animation"], animation)

        expected_kick_attacks = {
            "special.down.ground": "down.ground",
            "special.down.ground_end": "down.ground_end",
            "special.down.air": "down.air",
            "special.down.landing": "down.landing",
            "special.down.air_end": "down.air_end",
            "special.down.ground_end_air": "down.ground_end_air",
        }
        for action_name, attack in expected_kick_attacks.items():
            self.assertEqual(exported["actions"][action_name]["attack"], attack)

        expected_dive_attacks = {
            "special.up.ground": "up.ground",
            "special.up.air": "up.air",
        }
        for action_name, attack in expected_dive_attacks.items():
            self.assertEqual(exported["actions"][action_name]["attack"], attack)

    def test_inherits_all_standard_groups_without_duplicate_declarations(self):
        captain = _load_captain()
        self.assertTrue(FighterBase.__abstract__)
        self.assertIs(captain.__bases__[0], Fighter)
        self.assertTrue(issubclass(captain, Fighter))
        self.assertNotIn("shared.common", sys.modules)
        fighter_base = captain.__mro__[1]
        self.assertEqual(fighter_base.__name__, "Fighter")
        for group in ("aerials", "grounded", "tilts", "smashes", "grabs", "throws", "defense", "ledge", "getup", "taunt"):
            self.assertIs(getattr(captain, group), getattr(fighter_base, group))


if __name__ == "__main__":
    unittest.main()
