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
    FighterDefinition,
    GetupMoves,
    GrabMoves,
    GroundedMoves,
    SmashMoves,
    LedgeMoves,
    Move,
    MoveContext,
    MoveWait,
    MoveError,
    SpecialMoves,
    TiltMoves,
    TauntMoves,
    ThrowMoves,
    export_definition,
)
from .events import Hook, EventBinding, on
hook = on
from .transitions import SpecialMove, Transition
from . import math
try:
    from .compat import (Action, ActionDescriptor, ActionState, Button, HitContext, MotionBinding,
                          Parameters, SpecialMove, action, clock, motion, parameter,
                          resource, validation, f32)
except ImportError:  # compat descriptors are supplied by the native facade build.
    pass
from .registry import register, registered_fighters, validate_fighter

__all__ = [
    "ActionMove", "AerialMoves", "Attributes", "DefenseMoves", "EventBinding", "Fighter", "FighterDefinition",
    "GetupMoves", "GrabMoves", "GroundedMoves", "LedgeMoves", "SmashMoves", "TiltMoves",
    "Hook", "Move", "MoveContext", "MoveWait", "MoveError", "SpecialMoves", "TauntMoves", "ThrowMoves", "export_definition",
    "Action", "ActionDescriptor", "ActionState", "Button", "HitContext", "MotionBinding", "Parameters", "SpecialMove", "Transition",
    "action", "clock", "motion", "parameter", "resource", "validation", "f32",
    "on", "hook", "math", "register", "registered_fighters", "validate_fighter",
]
