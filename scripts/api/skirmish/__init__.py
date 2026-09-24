"""Public class based fighter authoring API."""

from fighter import (
    ActionMove, AerialMoves, AnimationEventId, Attributes, DefenseMoves, EventBinding, Fighter, FighterBase, FighterDefinition, FighterPart,
    GetupMoves, GrabMoves, GroundedMoves, Hook, LedgeMoves, Move, MoveContext, MoveWait, MoveError,
    SmashMoves, SpecialMoves, TauntMoves, ThrowMoves, TiltMoves, export_definition, on, register,
    Action, ActionDescriptor, ActionState, Button, CommonParameter, CustomAction, SourceAction, HitContext, MotionBinding, Parameters, SpecialMove, Transition,
    action, clock, motion, parameter, resource, special_attribute, validation, f32,
    registered_fighters, validate_fighter, resolve_identity,
    any_stick_axis_reaches_thresholds, directional_b_input, directional_b_reserved,
    directional_fresh_b, frame_preserving_surface_pairs, fresh_b_input,
    fresh_special_input, resource_attributes, select_facing_phase, special_rules,
    start_action, start_complete_special, start_fresh_open_special, start_open_special,
    stick_axis_reaches_threshold,
    ArticleId, B0ArticleSpecial, b0_source_phases, Roster, DirectionalSpecial, NeutralSpecial, SideSpecial, UpSpecial, DownSpecial, OpenSpecial, SpecialRoot, directional_match, custom_action, source_action, source_phase,
    DonkeyKongAttribute, JigglypuffPoundAttribute, SamusAttribute,
    CaptainNeutralSpecial, CaptainSideSpecial, CaptainUpSpecial, CaptainDownSpecial,
)
from fighter import hook, math
from fighter import velocity_from_angle

__all__ = [
    "ActionMove", "AerialMoves", "AnimationEventId", "Attributes", "DefenseMoves", "EventBinding", "Fighter", "FighterBase", "FighterDefinition", "FighterPart",
    "GetupMoves", "GrabMoves", "GroundedMoves", "LedgeMoves", "SmashMoves", "TiltMoves",
    "Hook", "Move", "MoveContext", "MoveWait", "MoveError", "SpecialMoves", "TauntMoves", "ThrowMoves", "export_definition",
    "Action", "ActionDescriptor", "ActionState", "Button", "CommonParameter", "CustomAction", "SourceAction", "HitContext", "MotionBinding", "Parameters", "SpecialMove", "Transition",
    "action", "clock", "motion", "parameter", "resource", "special_attribute", "validation", "f32", "custom_action", "source_action", "source_phase",
    "on", "hook", "math", "register", "registered_fighters", "validate_fighter", "resolve_identity",
    "velocity_from_angle",
    "ArticleId", "B0ArticleSpecial", "b0_source_phases", "Roster", "DirectionalSpecial", "NeutralSpecial", "SideSpecial", "UpSpecial", "DownSpecial", "OpenSpecial", "SpecialRoot", "directional_match",
    "DonkeyKongAttribute", "JigglypuffPoundAttribute", "SamusAttribute",
    "CaptainNeutralSpecial", "CaptainSideSpecial", "CaptainUpSpecial", "CaptainDownSpecial",
    "any_stick_axis_reaches_thresholds", "directional_b_input", "directional_b_reserved", "directional_fresh_b",
    "fresh_special_input", "resource_attributes", "special_rules", "start_action",
    "fresh_b_input",
    "start_fresh_open_special", "start_open_special", "stick_axis_reaches_threshold",
    "select_facing_phase", "start_complete_special", "frame_preserving_surface_pairs",
]
