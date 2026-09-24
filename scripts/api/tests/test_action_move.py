from dataclasses import dataclass
import inspect
import unittest

from fighter import (
    ActionMove,
    AerialMoves,
    DefenseMoves,
    Fighter,
    GetupMoves,
    GrabMoves,
    GroundedMoves,
    LedgeMoves,
    SmashMoves,
    SpecialMoves,
    TauntMoves,
    ThrowMoves,
    TiltMoves,
    action,
    Action,
    export_definition,
    SpecialMove,
    Transition,
)


@dataclass(frozen=True, slots=True)
class Attributes:
    pass


def fighter_with(specials: SpecialMoves) -> type[Fighter]:
    move = specials.neutral
    return type("ActionMoveFighter", (Fighter,), {
        "name": "action_move_fighter",
        "attributes": Attributes,
        "specials": specials,
        "aerials": AerialMoves(move, move, move, move, move),
        "grounded": GroundedMoves(move, move, move),
        "tilts": TiltMoves(move, move, move),
        "smashes": SmashMoves(move, move, move),
        "grabs": GrabMoves(move, move, move),
        "throws": ThrowMoves(move, move, move, move),
        "defense": DefenseMoves(move, move, move, move, move),
        "ledge": LedgeMoves(move, move, move, move, move),
        "getup": GetupMoves(move, move, move, move),
        "taunt": TauntMoves(move),
    })


class ActionMoveTests(unittest.TestCase):
    def test_standard_groups_default_to_fresh_canonical_action_moves(self):
        groups = (
            (AerialMoves(), ("attack_air_n", "attack_air_f", "attack_air_b", "attack_air_hi", "attack_air_lw")),
            (GroundedMoves(), ("jab", "attack100_loop", "attack_dash")),
            (TiltMoves(), ("attack_s3_s", "attack_hi3", "attack_lw3")),
            (SmashMoves(), ("attack_s4_s", "attack_hi4", "attack_lw4")),
            (GrabMoves(), ("catch", "catch_dash", "catch_attack")),
            (ThrowMoves(), ("throw_f", "throw_b", "throw_hi", "throw_lw")),
            (DefenseMoves(), ("guard", "escape_n", "escape_f", "escape_b", "escape_air")),
            (LedgeMoves(), ("cliff_wait", "cliff_climb", "cliff_escape", "cliff_attack", "cliff_jump")),
            (GetupMoves(), ("down_stand", "down_forward", "down_back", "down_attack")),
            (TauntMoves(), ("appeal_sr",)),
        )
        for group, expected in groups:
            moves = tuple(getattr(group, name) for name in group.__dataclass_fields__)
            self.assertEqual(tuple(move.action.value for move in moves), expected)
            self.assertTrue(all(isinstance(move, ActionMove) for move in moves))
        self.assertIsNot(AerialMoves().neutral, AerialMoves().neutral)

    def test_standard_group_defaults_allow_overrides_and_shared_references(self):
        shared = ActionMove(Action.JAB)
        group = GroundedMoves(jab=shared, dash=shared)
        self.assertIs(group.jab, shared)
        self.assertIs(group.dash, shared)

    def test_action_is_required_and_instances_are_frozen(self):
        self.assertEqual(
            list(inspect.signature(ActionMove).parameters), ["action", "resource"]
        )
        with self.assertRaises(TypeError):
            ActionMove()
        move = ActionMove("attack_air_n", resource="fox.attack_air_n")
        with self.assertRaisesRegex(AttributeError, "cannot assign to field"):
            move.resource = "other"

    def test_descriptor_and_resource_metadata_survive_export(self):
        descriptor = action(Action.SPECIAL_N_START, attack="fox.start", animation_loop=True)
        move = ActionMove(descriptor, resource="fox.special_n")
        exported = export_definition(fighter_with(SpecialMoves(move, move, move, move))).as_dict()
        behavior = exported["behaviors"][0]
        self.assertEqual(behavior["entry_action"], "special_n_start")
        self.assertEqual(behavior["resource"], "fox.special_n")
        self.assertEqual(exported["actions"]["special.neutral"], {
            "action": "Action.SPECIAL_N_START",
            "animation_loop": True,
            "attack": "fox.start",
            "source_behavior": "move_0",
        })

    def test_shared_instance_is_one_behavior_and_equal_distinct_instances_are_two(self):
        descriptor = action(Action.SPECIAL_N_START)
        shared = ActionMove(descriptor)
        shared_export = export_definition(
            fighter_with(SpecialMoves(shared, shared, shared, shared))
        ).as_dict()
        self.assertEqual(len(shared_export["behaviors"]), 1)

        first = ActionMove(descriptor)
        second = ActionMove(action(Action.SPECIAL_N_START))
        distinct_export = export_definition(
            fighter_with(SpecialMoves(first, second, first, second))
        ).as_dict()
        self.assertEqual(first.action, second.action)
        self.assertIsNot(first, second)
        self.assertEqual(len(distinct_export["behaviors"]), 2)

    def test_ground_air_transition_uses_fighter_state_when_context_is_missing(self):
        class MoveWithSurfaceTransition(SpecialMove):
            ground = Action.SPECIAL_S_START
            air = Action.SPECIAL_AIR_S_START
            on_ground = {air: Transition(ground)}

        class FighterProxy:
            action = Action.SPECIAL_AIR_S_START
            grounded = True

            def __init__(self):
                self.changes = []

            def change_action(self, action, **kwargs):
                self.changes.append((action, kwargs))

        fighter = FighterProxy()
        MoveWithSurfaceTransition()._transition_ground_air(fighter, None)
        self.assertEqual(fighter.changes, [(Action.SPECIAL_S_START, {
            "preserve_state": False,
            "keep_frame": False,
        })])

    def test_native_source_action_case_normalization_dispatches_bound_transition(self):
        class NativeMember:
            def _value(self):
                return "Source.12:344"

        class SourceMove(SpecialMove):
            source = action("Source.12:344")
            target = Action.SPECIAL_S_START
            on_end = {source: Transition(target)}

        class FighterProxy:
            action = NativeMember()
            grounded = True

            def __init__(self):
                self.changes = []

            def change_action(self, action, **kwargs):
                self.changes.append((action, kwargs))

        fighter = FighterProxy()
        SourceMove()._transition_animation_end(fighter, None)
        self.assertEqual(len(fighter.changes), 1)
        self.assertEqual(fighter.changes[0][0], Action.SPECIAL_S_START)


if __name__ == "__main__":
    unittest.main()
