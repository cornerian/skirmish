from dataclasses import dataclass, fields

import unittest

from fighter import (AerialMoves, DefenseMoves, Fighter, GetupMoves, GrabMoves,
                     GroundedMoves, LedgeMoves, Move, MoveContext, SmashMoves, SpecialMoves,
                     TauntMoves, ThrowMoves, TiltMoves, export_definition, on, register,
                     validate_fighter, Action, ActionDescriptor, SpecialMove, Transition, action)
from fighter.api import MoveError


@dataclass(frozen=True, slots=True)
class FoxAttributes:
    projectile: str = "fox_laser"


class Blaster(Move):
    @on.press("B")
    async def run(self, action: MoveContext) -> None:
        action.fighter.action_state("special.neutral")["fired"] = True


class Illusion(Move):
    async def run(self, action: MoveContext) -> None:
        return None


class FireFox(Move):
    async def run(self, action: MoveContext) -> None:
        return None


class Shine(Move):
    async def run(self, action: MoveContext) -> None:
        return None


class Aerial(Move):
    async def run(self, action: MoveContext) -> None:
        return None


@register
class Fox(Fighter):
    name = "fox"
    attributes = FoxAttributes
    blaster = Blaster()
    specials = SpecialMoves(blaster, Illusion(), FireFox(), Shine())
    aerials = AerialMoves(blaster, Aerial(), Aerial(), Aerial(), Aerial())
    grounded = GroundedMoves(*([Aerial()] * 3))
    tilts = TiltMoves(Aerial(), Aerial(), Aerial())
    smashes = SmashMoves(Aerial(), Aerial(), Aerial())
    grabs = GrabMoves(Aerial(), Aerial(), Aerial())
    throws = ThrowMoves(Aerial(), Aerial(), Aerial(), Aerial())
    defense = DefenseMoves(*([Aerial()] * 5))
    ledge = LedgeMoves(*([Aerial()] * 5))
    getup = GetupMoves(*([Aerial()] * 4))
    taunt = TauntMoves(Aerial())


class AuthoringTests(unittest.TestCase):
  def test_transition_rules_are_frozen_inherited_and_context_filtered(self):
    class Parent(SpecialMove):
      ground = action(Action.SPECIAL_S_START)
      air = action(Action.SPECIAL_AIR_S_START)
      on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}

    class Child(Parent):
      dash = action(Action.SPECIAL_S)
      on_ground = {Parent.air: Transition(dash)}
      on_air = {Parent.ground: Transition(Parent.air)}

    self.assertEqual(len(Parent.__transition_rules__), 1)
    self.assertEqual(len(Child.__transition_rules__), 2)
    self.assertIsNot(Parent.__transition_rules__, Child.__transition_rules__)
    landed = [event.as_dict() for event in Child.events() if event.hook.value == "landed"]
    self.assertEqual(landed[0]["actions"], ["special_air_s_start"])

    class Context:
      grounded = True

    class Host:
      action = Action.SPECIAL_AIR_S_START
      grounded = False
      def __init__(self): self.calls = []
      def change_action(self, *args, **kwargs): self.calls.append((args, kwargs))

    host = Host()
    Child()._transition_ground_air(host, Context())
    self.assertEqual(host.calls[0][0], (Action.SPECIAL_S,))
    self.assertEqual(host.calls[0][1], {"preserve_state": False, "keep_frame": False})

  def test_transition_target_descriptor_does_not_change_action_wire_shape(self):
    target = action(Action.SPECIAL_S, attack="side.dash")
    self.assertIsInstance(Transition(target).target, ActionDescriptor)
    self.assertEqual(target.as_dict(), {
      "action": "Action.SPECIAL_S", "animation_loop": False, "attack": "side.dash",
    })

  def test_combat_hooks_support_bare_parenthesized_and_filtered_forms(self):
    class CombatHooks(Move):
      @on.before_hit
      def bare(self, value):
        return value

      @on.after_hit()
      def parenthesized(self, value):
        return value

      @on.before_receive_hit(actions=(action(Action.SPECIAL_N_START),))
      def filtered(self, value):
        return value

    exported = [event.as_dict() for event in CombatHooks.events()]
    self.assertEqual({event["hook"] for event in exported},
                     {"before_hit", "after_hit", "before_receive_hit"})
    filtered = next(event for event in exported if event["callback"] == "filtered")
    self.assertEqual(filtered["actions"], ["special_n_start"])

  def test_extended_group_fields_are_exported(self):
    @dataclass(frozen=True, slots=True)
    class ExtendedSpecialMoves(SpecialMoves):
      extra: Move
    declarations = {name: getattr(Fox, name) for name in
                    ("attributes", "aerials", "grounded", "tilts", "smashes", "grabs",
                     "throws", "defense", "ledge", "getup", "taunt")}
    declarations.update(name="extended", specials=ExtendedSpecialMoves(*(getattr(Fox.specials, field.name) for field in fields(Fox.specials)), Aerial()))
    extended = type("Extended", (Fighter,), declarations)
    exported = export_definition(extended).as_dict()
    self.assertIn("extra", exported["movesets"]["specials"])

  def test_registration_exports_callbacks_and_reuses_moves(self):
    definition = export_definition(Fox)
    exported = definition.as_dict()
    self.assertEqual(exported["name"], "fox")
    self.assertEqual(exported["parameters"], {"projectile": "fox_laser"})
    self.assertEqual(exported["movesets"]["specials"]["neutral"], "move_0")
    self.assertEqual(exported["behaviors"][0]["callbacks"][0]["hook"], "input_pressed")
    self.assertEqual(len(definition.moves), 40)

  def test_action_records_keep_authored_move_slot_names(self):
    exported = export_definition(Fox).as_dict()
    keys = set(exported["actions"])
    self.assertFalse(any(key.startswith(("run.", "validate.")) for key in keys))
    if keys:
      self.assertTrue(any(key.startswith("special.") for key in keys))
      self.assertTrue(any(key.startswith("aerial.") for key in keys))
      self.assertTrue(any(key.startswith("grounded.") for key in keys))

  def test_shared_move_behavior_exposes_entry_action_once(self):
    class EntryMove(Move):
      action = "special_n_start"

      async def run(self, context):
        return None

    shared = EntryMove()
    declarations = {name: getattr(Fox, name) for name in
                    ("attributes", "grounded", "tilts", "smashes", "grabs", "throws",
                     "defense", "ledge", "getup", "taunt")}
    declarations.update(name="entry", specials=SpecialMoves(shared, Fox.specials.side,
                       Fox.specials.up, Fox.specials.down),
                        aerials=AerialMoves(shared, Fox.aerials.forward, Fox.aerials.back,
                                            Fox.aerials.up, Fox.aerials.down))
    entry = type("EntryFighter", (Fighter,), declarations)
    exported = export_definition(entry).as_dict()
    self.assertEqual(exported["movesets"]["specials"]["neutral"], "move_0")
    self.assertEqual(exported["movesets"]["aerials"]["neutral"], "move_0")
    behavior = next(item for item in exported["behaviors"] if item["id"] == "move_0")
    self.assertEqual(behavior["entry_action"], "special_n_start")

  def test_declarative_action_metadata_is_serialized_per_shared_owner(self):
    class Declarative(Move):
      action = action(Action.SPECIAL_N_START, attack="fox.start",
                      command_trace="fox.start.commands")

    shared = Declarative()
    declarations = {name: getattr(Fox, name) for name in
                    ("attributes", "tilts", "smashes", "grabs", "throws",
                     "defense", "ledge", "getup", "taunt")}
    declarations.update(name="declarative", specials=SpecialMoves(
                        shared, Fox.specials.side, Fox.specials.up, Fox.specials.down),
                        aerials=AerialMoves(shared, Fox.aerials.forward, Fox.aerials.back,
                                            Fox.aerials.up, Fox.aerials.down),
                        grounded=GroundedMoves(Fox.grounded.jab, Fox.grounded.rapid_jab,
                                               Fox.grounded.dash))
    exported = export_definition(type("DeclarativeFighter", (Fighter,), declarations)).as_dict()
    behavior = next(item for item in exported["behaviors"] if item["id"] == "move_0")
    self.assertEqual(behavior["entry_action"], "special_n_start")
    self.assertIsNone(behavior["run"])
    self.assertEqual(exported["actions"]["special.neutral"], {
      "action": "Action.SPECIAL_N_START",
      "animation_loop": False,
      "attack": "fox.start",
      "command_trace": "fox.start.commands",
      "source_behavior": "move_0",
    })
    self.assertEqual(exported["actions"]["aerial.neutral"]["source_behavior"], "move_0")


  def test_missing_groups_are_rejected(self):
    with self.assertRaisesRegex(MoveError, "specials"):
      class Missing(Fighter):
          name = "missing"
          attributes = FoxAttributes


  def test_fighter_is_declaration_only(self):
    self.assertFalse(hasattr(Fox(), "action_state"))

  def test_registration_is_module_scoped_marker(self):
    declarations = {name: getattr(Fox, name) for name in
                    ("attributes", "specials", "aerials", "grounded", "tilts", "smashes",
                     "grabs", "throws", "defense", "ledge", "getup", "taunt")}
    declarations["name"] = "same"
    first = type("First", (Fighter,), declarations)
    second = type("Second", (Fighter,), declarations)
    self.assertIs(register(first), first)
    self.assertIs(register(second), second)
