"""Shared behavior for fighters with resource-backed open specials.

``OpenSpecial`` deliberately stops at the entry action.  Character-specific
animation phases, transitions, and resource attributes belong to a fighter's
own move implementation; this class only defines the common B-input policy.
"""

from __future__ import annotations

from enum import Enum
from typing import Any

from .actions import Action
from .compat import Button
from .events import on
from .helpers import (
    fresh_b_input,
    fresh_special_input,
    directional_match,
    start_action,
    start_open_special,
)
from .transitions import SpecialMove


class SpecialRoot(str, Enum):
    """The four shared resource roots used by standard open specials."""

    NEUTRAL = "neutral"
    SIDE = "side"
    UP = "up"
    DOWN = "down"


class NeutralSpecial(SpecialMove):
    root = SpecialRoot.NEUTRAL
    resource = SpecialRoot.NEUTRAL.value


class SideSpecial(SpecialMove):
    root = SpecialRoot.SIDE
    resource = SpecialRoot.SIDE.value


class UpSpecial(SpecialMove):
    root = SpecialRoot.UP
    resource = SpecialRoot.UP.value


class DownSpecial(SpecialMove):
    root = SpecialRoot.DOWN
    resource = SpecialRoot.DOWN.value


_ENTRY_ACTIONS: dict[SpecialRoot, tuple[Action, Action]] = {
    SpecialRoot.NEUTRAL: (Action.SPECIAL_N_START, Action.SPECIAL_AIR_N_START),
    SpecialRoot.SIDE: (Action.SPECIAL_S_START, Action.SPECIAL_AIR_S_START),
    SpecialRoot.UP: (Action.SPECIAL_HI, Action.SPECIAL_AIR_HI),
    SpecialRoot.DOWN: (Action.SPECIAL_LW, Action.SPECIAL_AIR_LW),
}


class DirectionalSpecial:
    """Shared resource-gated B entry for a typed special root.

    Concrete moves provide ``ground``, ``air``, and ``_ACTIVE``.  The root is
    inherited from ``NeutralSpecial``/``SideSpecial``/``UpSpecial``/
    ``DownSpecial``; keeping the policy here avoids four subtly diverging
    copies of the same callback in fighter scripts.
    """

    _ACTIVE: tuple[Any, ...] = ()

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        if fighter.action in self._ACTIVE:
            return True
        if not fresh_special_input(ctx, self.resource):
            return False
        if not directional_match(ctx, self.root):
            return False
        ground_open = bool(getattr(ctx, "ground_open", False))
        air_open = bool(getattr(ctx, "air_open", False))
        if not (ground_open or air_open):
            return False
        start_action(fighter, self.ground if ground_open else self.air)
        return True


class OpenSpecial(SpecialMove):
    """Behavioral standard special that enters one ground/air action.

    The move accepts a fresh B only when its shared resource exists.  A
    directional root checks the corresponding one-sided threshold; neutral
    accepts only when neither side nor vertical directional threshold is met.
    Repeated B input during either entry action is consumed without restarting
    the action.  Surface availability is checked only for a new entry.
    """

    __slots__ = ("root", "resource", "ground", "air")

    def __init__(self, root: SpecialRoot) -> None:
        root = root if isinstance(root, SpecialRoot) else SpecialRoot(root)
        ground, air = _ENTRY_ACTIONS[root]
        self.root = root
        self.resource = root.value
        self.ground = ground
        self.air = air

    def __setattr__(self, name: str, value: Any) -> None:
        if hasattr(self, name):
            raise AttributeError(f"{type(self).__name__} instances are immutable")
        object.__setattr__(self, name, value)

    @property
    def active_actions(self) -> tuple[Action, Action]:
        """The two entry actions that consume repeated B input."""
        return self.ground, self.air

    @on.input_pressed("B")
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        # Read B exactly once.  Repeated B while this move's entry action is
        # active is consumed before resource/directional gates are consulted;
        # this mirrors the host's input callback contract without restarting
        # the action.  A nonfresh input never gets consumed.
        if not fresh_b_input(ctx):
            return False
        if getattr(fighter, "action", None) in self.active_actions:
            return True
        resource_lookup = getattr(ctx, "resource", None)
        if resource_lookup is None or resource_lookup(self.resource) is None:
            return False
        if not directional_match(ctx, self.root):
            return False
        return start_open_special(fighter, ctx, self.ground, self.air)


__all__ = [
    "NeutralSpecial", "SideSpecial", "UpSpecial", "DownSpecial",
    "DirectionalSpecial", "OpenSpecial",
    "directional_match",
    "SpecialRoot",
]
