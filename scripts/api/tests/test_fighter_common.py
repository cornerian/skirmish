"""Contract tests for shared fighter authoring helpers."""

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


from fighter import (
    Action,
    AerialMoves,
    DefenseMoves,
    Fighter,
    FighterBase,
    GetupMoves,
    GrabMoves,
    GroundedMoves,
    LedgeMoves,
    Move,
    MoveError,
    MotionBinding,
    SmashMoves,
    SpecialMoves,
    TauntMoves,
    ThrowMoves,
    TiltMoves,
    any_stick_axis_reaches_thresholds,
    directional_b_input,
    directional_b_reserved,
    directional_fresh_b,
    frame_preserving_surface_pairs,
    fresh_special_input,
    resource_attributes,
    select_facing_phase,
    special_rules,
    start_action,
    start_fresh_open_special,
    start_open_special,
    stick_axis_reaches_threshold,
)
from fighter import math as fighter_math


class FighterCommonTests(unittest.TestCase):
    def test_source_phase_helpers_keep_numeric_state_data_together(self):
        left = Action.ATTACK_AIR_N
        right = Action.ATTACK_AIR_F
        air_left = Action.ATTACK_AIR_B
        air_right = Action.ATTACK_AIR_HI
        self.assertEqual(
            select_facing_phase(True, -1.0, left, right, air_left, air_right, 1, 2, 3, 4),
            (left, 1),
        )
        self.assertEqual(
            select_facing_phase(False, 1.0, left, right, air_left, air_right, 1, 2, 3, 4),
            (air_right, 4),
        )
        on_ground, on_air = frame_preserving_surface_pairs(left, right, air_left, air_right)
        self.assertEqual(on_ground[air_left].target, left)
        self.assertTrue(on_ground[air_left].preserve_state)
        self.assertTrue(on_ground[air_left].keep_frame)
        self.assertEqual(on_air[right].target, air_right)

    def test_special_moves_replace_preserves_unmodified_slots(self):
        original = SpecialMoves(*(Move() for _ in range(4)))
        neutral = Move()
        updated = original.replace(neutral=neutral)
        self.assertIs(updated.neutral, neutral)
        self.assertIs(updated.side, original.side)
        self.assertIs(updated.up, original.up)
        self.assertIs(updated.down, original.down)

        with self.assertRaises(TypeError):
            original.replace(missing=neutral)
        with self.assertRaises(TypeError):
            original.replace(neutral=object())

    def test_directional_fresh_b_is_permissive_without_rules_and_thresholded_with_rules(self):
        context = SimpleNamespace(
            input=SimpleNamespace(stick=(0.0, 1.0), just_pressed=lambda button: True)
        )
        self.assertTrue(directional_fresh_b(context, 1))
        context.rules = SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5))
        self.assertTrue(directional_fresh_b(context, 1))
        context.input.stick = (0.0, -1.0)
        self.assertFalse(directional_fresh_b(context, 1))

    def test_motion_binding_from_angle_preserves_wire_shape(self):
        with patch.object(fighter_math, "cos", return_value=0.25) as cosine, \
                patch.object(fighter_math, "sin", return_value=0.75) as sine:
            binding = MotionBinding.from_angle(0.5, facing=-1.0, ground_scale=0.75)
        cosine.assert_called_once_with(0.5)
        sine.assert_called_once_with(0.5)
        self.assertEqual(
            binding.as_dict(),
            {"facing": -1.0, "cosine": 0.25, "sine": 0.75, "ground_scale": 0.75},
        )

    def test_fighter_provides_standard_groups_and_fighter_base_is_strict(self):
        class Attributes:
            pass

        move = Move()
        class Defaulted(Fighter):
            name = "defaulted"
            attributes = Attributes
            specials = SpecialMoves(move, move, move, move)

        for group, expected in (
            ("aerials", AerialMoves), ("grounded", GroundedMoves),
            ("tilts", TiltMoves), ("smashes", SmashMoves),
            ("grabs", GrabMoves), ("throws", ThrowMoves),
            ("defense", DefenseMoves), ("ledge", LedgeMoves),
            ("getup", GetupMoves), ("taunt", TauntMoves),
        ):
            self.assertIsInstance(getattr(Defaulted, group), expected)

        with self.assertRaisesRegex(MoveError, "aerials"):
            class Incomplete(FighterBase):
                name = "incomplete"
                attributes = Attributes
                specials = SpecialMoves(move, move, move, move)

    def test_directional_b_input_gates_fresh_input_and_preserves_boundary(self):
        rules = SimpleNamespace(specials=SimpleNamespace(side_stick_threshold=0.5))

        def context(stick=(0.0, 0.0), pressed=True, resource_value=object()):
            return SimpleNamespace(
                rules=rules,
                input=SimpleNamespace(
                    stick=stick,
                    just_pressed=lambda button: pressed,
                ),
                resource=lambda path: resource_value,
            )

        self.assertIsNone(directional_b_input(
            context(pressed=False), "side", 0, "side_stick_threshold"
        ))
        self.assertFalse(directional_b_input(
            context((0.5 - 1e-6, 0.0)), "side", 0, "side_stick_threshold"
        ))
        self.assertTrue(directional_b_input(
            context((-0.5, 0.0)), "side", 0, "side_stick_threshold"
        ))
        self.assertTrue(directional_b_input(
            context((0.0, 0.5)), "side", 1, "side_stick_threshold", direction=1
        ))

    def test_stick_axis_threshold_is_inclusive_and_uses_magnitude(self):
        self.assertFalse(stick_axis_reaches_threshold(0.49, 0.5))
        self.assertTrue(stick_axis_reaches_threshold(0.5, 0.5))
        self.assertTrue(stick_axis_reaches_threshold(-0.5, 0.5))
        self.assertTrue(any_stick_axis_reaches_thresholds((0.1, -0.5), ((0, 0.5), (1, 0.5))))

    def test_stick_dispatch_treats_missing_axes_as_neutral(self):
        rules = SimpleNamespace(specials=SimpleNamespace(
            side_stick_threshold=0.5,
            vertical_threshold=0.5,
        ))
        context = SimpleNamespace(
            rules=rules,
            input=SimpleNamespace(stick=(), just_pressed=lambda button: True),
            resource=lambda path: object(),
        )
        self.assertFalse(any_stick_axis_reaches_thresholds((), ((0, 0.5),)))
        self.assertFalse(directional_b_reserved(context))
        self.assertFalse(directional_b_input(
            context, "side", 0, "side_stick_threshold"
        ))
        context.input.stick = (0.5,)
        self.assertTrue(directional_b_input(
            context, "side", 0, "side_stick_threshold"
        ))

    def test_special_rules_returns_optional_dispatch_table(self):
        specials = object()
        self.assertIs(
            special_rules(SimpleNamespace(rules=SimpleNamespace(specials=specials))),
            specials,
        )
        self.assertIsNone(special_rules(SimpleNamespace()))
        self.assertIsNone(special_rules(SimpleNamespace(rules=SimpleNamespace())))
        self.assertIsNone(special_rules(SimpleNamespace(rules=None)))

    def test_resource_attributes_support_resource_objects_and_named_paths(self):
        attributes = object()
        resource = SimpleNamespace(attributes=attributes)
        looked_up = []

        def lookup(path=None):
            looked_up.append(path)
            if path == "special.attributes":
                return attributes
            return resource

        context = SimpleNamespace(resource=lookup)
        self.assertIs(resource_attributes(context, resource), attributes)
        self.assertIs(resource_attributes(context, "special"), attributes)
        self.assertEqual(looked_up, ["special"])

    def test_resource_attributes_uses_owned_resource_when_omitted(self):
        attributes = object()
        owned = SimpleNamespace(attributes=attributes)
        calls = []

        def lookup(path=None):
            calls.append(path)
            return owned

        self.assertIs(resource_attributes(SimpleNamespace(resource=lookup)), attributes)
        self.assertEqual(calls, [None])

    def test_resource_attributes_returns_none_for_missing_or_malformed_optional_data(self):
        self.assertIsNone(resource_attributes(SimpleNamespace()))
        self.assertIsNone(resource_attributes(SimpleNamespace(resource=lambda: None)))
        self.assertIsNone(
            resource_attributes(SimpleNamespace(resource=lambda path: object()), "special")
        )
        self.assertIsNone(
            resource_attributes(SimpleNamespace(resource=lambda path: None), "special.attributes")
        )

    def test_start_action_changes_action_and_resets_animation_frame(self):
        class Fighter:
            action_frame = 27

            def __init__(self):
                self.changes = []

            def change_action(self, action):
                self.changes.append(action)

        fighter = Fighter()
        action = object()
        self.assertIsNone(start_action(fighter, action))
        self.assertEqual(fighter.changes, [action])
        self.assertEqual(fighter.action_frame, 1)

    def test_fresh_special_helpers_gate_input_and_select_open_surface(self):
        class Input:
            def just_pressed(self, button):
                return button.name == "B"

        class Fighter:
            action_frame = 0

            def __init__(self):
                self.actions = []

            def change_action(self, action):
                self.actions.append(action)

        context = SimpleNamespace(
            input=Input(),
            ground_open=False,
            air_open=True,
            resource=lambda path: object(),
        )
        self.assertTrue(fresh_special_input(context, "neutral"))
        fighter = Fighter()
        air = object()
        ground = object()
        self.assertTrue(start_open_special(fighter, context, ground, air))
        self.assertEqual(fighter.actions, [air])
        self.assertEqual(fighter.action_frame, 1)

        context.ground_open = False
        context.air_open = False
        self.assertFalse(start_open_special(fighter, context, ground, air))

    def test_fresh_special_checks_button_before_resource_proxy(self):
        calls = []
        context = SimpleNamespace(
            input=SimpleNamespace(just_pressed=lambda button: False),
            resource=lambda path: calls.append(path),
        )
        self.assertFalse(fresh_special_input(context, "neutral"))
        self.assertEqual(calls, [])

    def test_open_surface_defaults_closed_for_partial_native_context(self):
        class Fighter:
            action_frame = 0

            def change_action(self, action):
                self.action = action

        fighter = Fighter()
        self.assertFalse(start_open_special(fighter, SimpleNamespace(), object(), object()))

    def test_directional_b_reserved_uses_configured_inclusive_axes(self):
        context = SimpleNamespace(
            rules=SimpleNamespace(specials=SimpleNamespace(
                vertical_threshold=0.5,
                horizontal_threshold=0.6,
            )),
            input=SimpleNamespace(stick=(0.0, 0.0)),
        )
        self.assertFalse(directional_b_reserved(context))
        context.input.stick = (0.6, 0.0)
        self.assertTrue(directional_b_reserved(context))
        context.input.stick = (0.0, -0.5)
        self.assertTrue(directional_b_reserved(context))

    def test_start_fresh_open_special_requires_resource_and_b(self):
        class Fighter:
            action_frame = 0
            action = None
            def change_action(self, action):
                self.action = action
        context = SimpleNamespace(
            ground_open=True,
            air_open=False,
            input=SimpleNamespace(
                just_pressed=lambda button: button.name == "B",
            ),
            resource=lambda path: object(),
        )
        fighter = Fighter()
        action = object()
        self.assertTrue(start_fresh_open_special(
            fighter, context, "neutral", action, object()))
        self.assertIs(fighter.action, action)
        self.assertEqual(fighter.action_frame, 1)
        self.assertTrue(start_fresh_open_special(
            fighter, context, "neutral", action, object(),
            active_actions=(action,)))
        context.input.just_pressed = lambda button: False
        self.assertFalse(start_fresh_open_special(
            fighter, context, "neutral", action, object(),
            active_actions=(action,)))


if __name__ == "__main__":
    unittest.main()
