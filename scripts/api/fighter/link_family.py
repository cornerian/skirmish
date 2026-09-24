"""Shared source phases for Link and Young Link's specials.

The two fighters use the same ``ftLink`` motion-state table and fighter-side
special callbacks.  Article ownership (arrows, boomerangs, and bombs) is
intentionally not modeled here: those item archives are not part of the
authoring package.  The family therefore declares the source states,
resource-gated entry, and transitions which do not depend on article state.
"""

from __future__ import annotations

from typing import Any

from .actions import Action
from .compat import ActionDescriptor, Button, source_phase
from .events import on
from .helpers import (
    directional_b_input,
    directional_b_reserved,
    fresh_special_input,
    start_action,
)
from .standard import DownSpecial, NeutralSpecial, SideSpecial, SpecialRoot, UpSpecial
from .transitions import SpecialMove, Transition


def _phase(state: int, *, loop: bool = False):
    """Keep the native numeric state and optional loop bit together."""
    return source_phase(state, animation_loop=loop)


class _LinkSpecial(SpecialMove):
    """The common B-input and surface policy for the four special roots."""

    _ACTIVE_NAMES: tuple[str, ...] = ()
    _ENTRY_NAMES: tuple[str, str]

    def __init_subclass__(cls, **kwargs: Any) -> None:
        # Source identity is bound on the concrete fighter move class.  Give
        # each roster member its own class attributes so exporting Link cannot
        # permanently bind Young Link's otherwise shared numeric states.
        for base in cls.__mro__[1:]:
            for name, value in base.__dict__.items():
                if isinstance(value, ActionDescriptor) and name not in cls.__dict__:
                    setattr(cls, name, value)
        super().__init_subclass__(**kwargs)

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        # Native IASA consumes repeated B while a special is active.  Do this
        # before consulting the optional resource, matching the standard
        # special contract and avoiding a restart of the current state.
        if fighter.action in self._active_actions():
            return True
        if not fresh_special_input(ctx, self.resource):
            return False
        if not self._direction_matches(ctx):
            return False
        if not (ctx.ground_open or ctx.air_open):
            return False
        start_action(fighter, getattr(self, self._ENTRY_NAMES[0 if ctx.ground_open else 1]))
        return True

    def _active_actions(self) -> tuple[Any, ...]:
        return tuple(getattr(self, name) for name in self._ACTIVE_NAMES)

    def _direction_matches(self, ctx: Any) -> bool:
        if self.root is SpecialRoot.NEUTRAL:
            return not directional_b_reserved(ctx)
        if self.root is SpecialRoot.SIDE:
            return directional_b_input(ctx, self.resource, 0, "side_stick_threshold") is True
        if self.root is SpecialRoot.UP:
            return directional_b_input(
                ctx, self.resource, 1, "vertical_threshold", direction=1
            ) is True
        return directional_b_input(
            ctx, self.resource, 1, "vertical_threshold", direction=-1
        ) is True


class LinkNeutralSpecial(NeutralSpecial, _LinkSpecial):
    ground_start = _phase(344)
    ground_loop = _phase(345, loop=True)
    ground_end = _phase(346)
    air_start = _phase(347)
    air_loop = _phase(348, loop=True)
    air_end = _phase(349)
    _ENTRY_NAMES = ("ground_start", "air_start")
    _ACTIVE_NAMES = ("ground_start", "ground_loop", "air_start", "air_loop")

    on_end = {
        ground_start: Transition(ground_loop),
        air_start: Transition(air_loop),
        ground_end: Transition(Action.WAIT),
        air_end: Transition(Action.FALL),
    }

    @on.release(Button.B)
    def release(self, fighter: Any, ctx: Any) -> None:
        if fighter.action not in self._active_actions():
            return
        destination = {
            self.ground_start: self.ground_end,
            self.ground_loop: self.ground_end,
            self.air_start: self.air_end,
            self.air_loop: self.air_end,
        }.get(fighter.action)
        if destination is not None:
            fighter.change_action(destination)

    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_loop: Transition(ground_loop, preserve_state=True, keep_frame=True),
        air_end: Transition(ground_end, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_loop: Transition(air_loop, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_end, preserve_state=True, keep_frame=True),
    }


class LinkSideSpecial(SideSpecial, _LinkSpecial):
    ground_start = _phase(350)
    ground = _phase(351)
    ground_empty = _phase(352)
    air_start = _phase(353)
    air = _phase(354)
    air_empty = _phase(355)
    _ENTRY_NAMES = ("ground_start", "air_start")
    _ACTIVE_NAMES = (
        "ground_start", "ground", "ground_empty", "air_start", "air", "air_empty"
    )

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        """Enter the native empty throw phase when the boomerang is held."""
        if fighter.action in self._active_actions():
            return True
        if not fresh_special_input(ctx, self.resource) or not self._direction_matches(ctx):
            return False
        if not (ctx.ground_open or ctx.air_open):
            return False
        held = bool(
            getattr(fighter, "used_boomerang", False)
            or getattr(fighter, "boomerang_active", False)
        )
        prefix = "ground_" if ctx.ground_open else "air_"
        phase = prefix + ("empty" if held else "start")
        start_action(fighter, getattr(self, phase))
        return True

    @on.command_changed(0)
    def boomerang_release(self, fighter: Any, ctx: Any) -> None:
        """Forward native boomerang release to an article-aware host."""
        if not getattr(getattr(ctx, "event", None), "value", False):
            return
        update = getattr(fighter, "update_boomerang_trajectory", None)
        if callable(update):
            update(ctx)

    # The phase-2 and empty variants are source-declared for article-aware
    # hosts, but entry selection is deliberately left to the unavailable
    # boomerang state.  Their terminal and surface behavior is still native.
    on_end = {
        ground_start: Transition(Action.WAIT),
        ground: Transition(Action.WAIT),
        ground_empty: Transition(Action.WAIT),
        air_start: Transition(Action.FALL),
        air: Transition(Action.FALL),
        air_empty: Transition(Action.FALL),
    }
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air: Transition(ground, preserve_state=True, keep_frame=True),
        air_empty: Transition(ground_empty, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground: Transition(air, preserve_state=True, keep_frame=True),
        ground_empty: Transition(air_empty, preserve_state=True, keep_frame=True),
    }


class LinkUpSpecial(UpSpecial, _LinkSpecial):
    ground = _phase(356)
    air = _phase(357)
    _ENTRY_NAMES = ("ground", "air")
    _ACTIVE_NAMES = ("ground", "air")

    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}


class LinkDownSpecial(DownSpecial, _LinkSpecial):
    ground = _phase(358)
    air = _phase(359)
    _ENTRY_NAMES = ("ground", "air")
    _ACTIVE_NAMES = ("ground", "air")

    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}


__all__ = [
    "LinkNeutralSpecial",
    "LinkSideSpecial",
    "LinkUpSpecial",
    "LinkDownSpecial",
]
