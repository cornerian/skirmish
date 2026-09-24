"""Class based fighter authoring surface for the native Pon bridge.

The objects in this module are authoring descriptors.  A :class:`Move` is
shared immutable code and metadata; execution state belongs to the host's
fighter/action instance and is never stored on a move object.
"""

from .api import (
    ActionMove,
    AerialMoves,
    Attributes,
    DefenseMoves,
    Fighter,
    FighterBase,
    FighterDefinition,
    GetupMoves,
    GrabMoves,
    GroundedMoves,
    SmashMoves,
    LedgeMoves,
    Move,
    MoveContext,
    EntityView,
    MoveWait,
    MoveError,
    SpecialMoves,
    TiltMoves,
    TauntMoves,
    ThrowMoves,
    export_definition,
    resolve_identity,
    FighterPart,
)
from .events import AnimationEventId, Hook, EventBinding, on
hook = on
from .transitions import SpecialMove, Transition
from . import math
from .math import velocity_from_angle
try:
    from .compat import (Action, ActionDescriptor, ActionState, Button, CommonParameter, CustomAction, SourceAction, HitContext, MotionBinding,
                          Parameters, SpecialMove, action, clock, motion, parameter, special_attribute,
                          resource, validation, f32, custom_action, source_action, source_phase)
except ImportError:  # compat descriptors are supplied by the native facade build.
    pass
from .registry import register, registered_fighters, validate_fighter
from .articles import ArticleId, B0ArticleSpecial, b0_source_phases
from .roster import Roster
from .special_attributes import (
    DonkeyKongAttribute,
    JigglypuffPoundAttribute,
    JigglypuffRolloutAttribute,
    KirbyAttribute,
    SamusAttribute,
)
from .standard import (
    DirectionalSpecial,
    DownSpecial,
    NeutralSpecial,
    OpenSpecial,
    SideSpecial,
    SpecialRoot,
    UpSpecial,
    directional_match,
)
from .captain_family import (
    CaptainDownSpecial,
    CaptainNeutralSpecial,
    CaptainSideSpecial,
    CaptainUpSpecial,
)
from .helpers import (any_stick_axis_reaches_thresholds, directional_b_input,
                      directional_b_reserved, directional_fresh_b,
                      frame_preserving_surface_pairs, fresh_b_input,
                      fresh_special_input, resource_attributes, select_facing_phase,
                      special_rules, start_action, start_complete_special,
                      start_fresh_open_special, start_open_special,
                      stick_axis_reaches_threshold)

__all__ = [
    "ActionMove", "AerialMoves", "AnimationEventId", "Attributes", "DefenseMoves", "EventBinding", "Fighter", "FighterBase", "FighterDefinition", "FighterPart",
    "GetupMoves", "GrabMoves", "GroundedMoves", "LedgeMoves", "SmashMoves", "TiltMoves",
    "Hook", "Move", "MoveContext", "EntityView", "MoveWait", "MoveError", "SpecialMoves", "TauntMoves", "ThrowMoves", "export_definition",
    "Action", "ActionDescriptor", "ActionState", "Button", "CommonParameter", "CustomAction", "SourceAction", "HitContext", "MotionBinding", "Parameters", "SpecialMove", "Transition",
    "action", "clock", "motion", "parameter", "resource", "special_attribute", "validation", "f32",
    "on", "hook", "math", "register", "registered_fighters", "validate_fighter", "resolve_identity", "custom_action", "source_action", "source_phase",
    "velocity_from_angle",
    "ArticleId", "B0ArticleSpecial", "b0_source_phases", "Roster", "NeutralSpecial", "SideSpecial", "UpSpecial", "DownSpecial", "OpenSpecial", "SpecialRoot",
    "DirectionalSpecial", "directional_match",
    "DonkeyKongAttribute", "JigglypuffPoundAttribute", "JigglypuffRolloutAttribute", "KirbyAttribute", "SamusAttribute",
    "CaptainNeutralSpecial", "CaptainSideSpecial", "CaptainUpSpecial", "CaptainDownSpecial",
    "any_stick_axis_reaches_thresholds", "directional_b_input", "directional_b_reserved", "directional_fresh_b",
    "fresh_special_input", "resource_attributes", "special_rules", "start_action",
    "fresh_b_input",
    "start_fresh_open_special", "start_open_special", "stick_axis_reaches_threshold",
    "select_facing_phase", "start_complete_special", "frame_preserving_surface_pairs",
]
