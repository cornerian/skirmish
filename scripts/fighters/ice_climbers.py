"""Popo's source-defined special motion graph.

The native Ice Climbers implementation routes several branches through Nana
and through the ice, blizzard, and belay articles. This declaration keeps the
lead fighter's source states and ordinary ground/air lifecycle visible while
using the generic secondary-entity projection for Nana branches whenever its
source position, lifecycle, and authored radius facts are present.
"""

from __future__ import annotations

import math
from typing import Any

from skirmish import (
    Action,
    DirectionalSpecial,
    DownSpecial,
    Fighter,
    NeutralSpecial,
    SideSpecial,
    Transition,
    UpSpecial,
    MoveContext,
    frame_preserving_surface_pairs,
    on,
    source_phase,
)


_MISSING = object()
_NO_RESOLVER = object()


def _partner_projection(ctx: Any) -> Any | None:
    """Resolve the same-port partner projection, failing closed on host gaps.

    The native source resolves Nana through ``Player_GetEntityAtIndex``.  The
    current callback context does not promise that resolver, so the script
    probes only a generic ``entity_at_index(1)`` capability.  Missing,
    malformed, and foreign objects remain unavailable to source branches.
    """
    resolve = getattr(ctx, "entity_at_index", None)
    if not callable(resolve):
        return _NO_RESOLVER
    try:
        partner = resolve(1)
    except Exception:
        return None
    return partner


def _partner_available(ctx: Any) -> bool | None:
    """Read partner availability from the generic projection.

    ``partner_available`` remains an explicit legacy shim for older hosts;
    current hosts should expose ``EntityProjection.available`` instead.
    """
    partner = _partner_projection(ctx)
    if partner is _NO_RESOLVER:
        legacy = getattr(ctx, "partner_available", _MISSING)
        return legacy if isinstance(legacy, bool) else None
    if partner is not None:
        value = getattr(partner, "available", _MISSING)
        return value if isinstance(value, bool) else None
    return None


def _partner_launching(ctx: Any) -> bool | None:
    """Derive Belay launch readiness from Nana's source motion state.

    The decomp tests Nana's active Belay rows (362 through 366), not a named
    ``launching`` flag.  ``partner_launching`` is accepted only as a legacy
    compatibility shim when no generic projection exists.
    """
    partner = _partner_projection(ctx)
    if partner is _NO_RESOLVER:
        legacy = getattr(ctx, "partner_launching", _MISSING)
        return legacy if isinstance(legacy, bool) else None
    if partner is not None:
        available = getattr(partner, "available", _MISSING)
        lifecycle = getattr(partner, "lifecycle", _MISSING)
        if not isinstance(available, bool) or not isinstance(lifecycle, str):
            return None
        if lifecycle in ("", "unavailable", "dead", "destroyed"):
            return None
        motion_state = getattr(partner, "motion_state", _MISSING)
        if isinstance(motion_state, bool) or not isinstance(motion_state, int):
            return None
        return 362 <= motion_state <= 366
    return None


def _resource_attribute(ctx: Any, path: str, name: str) -> float | None:
    """Read one finite raw Ice Climber attribute, failing closed."""
    lookup = getattr(ctx, "resource", None)
    resource = lookup(path) if callable(lookup) else None
    attrs = getattr(resource, "attributes", resource)
    value = getattr(attrs, name, _MISSING)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    value = float(value)
    return value if math.isfinite(value) and value >= 0.0 else None


def _partner_in_range(fighter: Any, ctx: Any, radius_attr: str) -> bool | None:
    """Mirror the source Nana range test from generic entity facts."""
    partner = _partner_projection(ctx)
    if partner is _NO_RESOLVER or partner is None:
        return None
    available = getattr(partner, "available", _MISSING)
    lifecycle = getattr(partner, "lifecycle", _MISSING)
    if not isinstance(available, bool) or not isinstance(lifecycle, str):
        return None
    if lifecycle in ("", "unavailable", "dead", "destroyed"):
        return None
    if not available:
        return False
    position = getattr(fighter, "position", _MISSING)
    partner_position = getattr(partner, "position", _MISSING)
    if not all(
        isinstance(value, (tuple, list)) and len(value) >= 2
        for value in (position, partner_position)
    ):
        return None
    try:
        dx = float(position[0]) - float(partner_position[0])
        dy = float(position[1]) - float(partner_position[1])
    except (TypeError, ValueError):
        return None
    if not math.isfinite(dx) or not math.isfinite(dy):
        return None
    scale_y = getattr(fighter, "scale_y", _MISSING)
    if scale_y is _MISSING:
        scale = getattr(fighter, "scale", _MISSING)
        if isinstance(scale, (tuple, list)) and len(scale) >= 2:
            scale_y = scale[1]
        elif isinstance(scale, (int, float)) and not isinstance(scale, bool):
            scale_y = scale
    radius = _resource_attribute(ctx, "side.attributes", radius_attr)
    if radius is None or scale_y is _MISSING:
        return None
    try:
        radius = radius * float(scale_y)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(radius):
        return None
    return dx * dx + dy * dy < radius * radius


class IceShot(NeutralSpecial, DirectionalSpecial):
    """Ice Shot's fighter phases; the ice article remains native-owned."""

    ground = source_phase(341)
    air = source_phase(342)
    _ACTIVE = (ground, air)

    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground, on_air = frame_preserving_surface_pairs(ground, ground, air, air)

    @on.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Clear the article command latch on both Ice Shot entries.

        ``ftPp_SpecialN_Enter`` and ``ftPp_SpecialAirN_Enter`` clear
        ``cmd_vars[0]`` before installing the ice article callback.  Preserve
        the other command slots because this fighter callback owns only slot 0.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and command:
            state.command = (0, *command[1:])


class SquallHammer(SideSpecial, DirectionalSpecial):
    """Squall Hammer's four Popo motion rows (ftPp special S1/S2).

    The source collision callbacks rebound wall velocity and synchronize
    Nana's attached pose. The generic API has no partner action mutation,
    bone lookup, collision normal, or article hitlag state, so those follower
    effects remain explicit host capability gaps; the source S1/S2 entry
    choice is handled here when the generic Nana projection supplies its
    range facts.
    """

    ground_start = source_phase(343)
    ground_partner = source_phase(344)
    air_start = source_phase(345)
    air_partner = source_phase(346)
    # Public roots select the first source row; the script can enter the
    # partner rows when the generic Nana projection proves the source range.
    ground, air = ground_start, air_start
    _ACTIVE = (ground_start, ground_partner, air_start, air_partner)

    on_end = {
        ground_start: Transition(Action.WAIT),
        ground_partner: Transition(Action.WAIT),
        air_start: Transition(Action.FALL),
        air_partner: Transition(Action.FALL),
    }
    on_ground, on_air = frame_preserving_surface_pairs(
        ground_start, ground_partner, air_start, air_partner
    )

    @on.action_enter(ground_start, air_start)
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Clear Squall's four source command latches on entry.

        ``ftPp_SpecialS_Enter`` and ``ftPp_SpecialAirS_Enter`` reset
        ``cmd_vars[0..3]`` before choosing the Popo/Nana row.  Article
        creation and collision response require generic article facts that
        this callback context does not expose.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)):
            values = [0] * min(len(command), 4)
            values.extend(command[4:])
            state.command = type(command)(values) if isinstance(command, tuple) else values
        in_range = _partner_in_range(fighter, ctx, "xD0")
        if in_range is True:
            target = self.ground_partner if fighter.action is self.ground_start else self.air_partner
            fighter.change_action(target)

class Belay(UpSpecial, DirectionalSpecial):
    """Belay's ten Popo rows with source Nana range/lifecycle selection.

    The ``*_start_1`` rows are the source fallback branch used when Nana is
    unavailable or out of range. Selection is applied only when generic
    entity position/lifecycle facts and the authored ``x7C`` radius exist.
    Rope creation, launch velocity, wall/ceiling collision, and Nana's
    teleport/throw callbacks still require generic primitives not in this API,
    including partner action/position mutation and collision/article handles.
    """

    ground_start_0 = source_phase(347)
    ground_throw_0 = source_phase(348)
    ground_throw_2 = source_phase(349)
    ground_start_1 = source_phase(350)
    ground_throw_1 = source_phase(351)
    air_start_0 = source_phase(352)
    air_throw_0 = source_phase(353)
    air_throw_2 = source_phase(354)
    air_start_1 = source_phase(355)
    air_throw_1 = source_phase(356)
    ground, air = ground_start_0, air_start_0
    # Compatibility aliases for the earlier generic names.
    ground_throw = ground_throw_0
    ground_launch = ground_throw_2
    ground_fallback = ground_start_1
    ground_fallback_throw = ground_throw_1
    air_throw = air_throw_0
    air_launch = air_throw_2
    air_fallback = air_start_1
    air_fallback_throw = air_throw_1
    _ACTIVE = (
        ground_start_0, ground_throw_0, ground_throw_2, ground_start_1,
        ground_throw_1, air_start_0, air_throw_0, air_throw_2, air_start_1,
        air_throw_1,
    )

    on_end = {
        ground_start_0: Transition(ground_throw_0),
        ground_throw_0: Transition(Action.WAIT),
        ground_throw_2: Transition(Action.WAIT),
        ground_start_1: Transition(ground_throw_1),
        ground_throw_1: Transition(Action.WAIT),
        air_start_0: Transition(air_throw_0),
        # ftPp_SpecialAirHiThrow{0,1,2}_Anim calls the source fall-special
        # helper when the aerial animation ends.
        air_throw_0: Transition(Action.SPECIAL_HI_FALL),
        air_throw_2: Transition(Action.SPECIAL_HI_FALL),
        air_start_1: Transition(air_throw_1),
        air_throw_1: Transition(Action.SPECIAL_HI_FALL),
    }
    on_ground, on_air = frame_preserving_surface_pairs(
        ground_start_0, ground_throw_0, air_start_0, air_throw_0
    )
    fallback_ground, fallback_air = frame_preserving_surface_pairs(
        ground_throw_2, ground_start_1, air_throw_2, air_start_1
    )
    on_ground.update(fallback_ground)
    on_air.update(fallback_air)
    final_ground, final_air = frame_preserving_surface_pairs(
        ground_throw_1, ground_throw_1, air_throw_1, air_throw_1
    )
    on_ground.update(final_ground)
    on_air.update(final_air)
    # AirThrow1/AirThrow2 call ftCo_LandingFallSpecial_Enter on ground
    # contact; they do not become the matching grounded source rows.
    on_ground[air_throw_1] = Transition(Action.SPECIAL_HI_LANDING)
    on_ground[air_throw_2] = Transition(Action.SPECIAL_HI_LANDING)

    @on.action_enter(ground_start_0, air_start_0)
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Clear Belay's source command latches on ground and air entry.

        ``ftPp_SpecialHi_Enter`` and its aerial counterpart clear
        ``cmd_vars[0..2]``.  Command slot 3 belongs to another source path
        and is preserved here.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)):
            values = list(command)
            for index in range(min(3, len(values))):
                values[index] = 0
            state.command = type(command)(values) if isinstance(command, tuple) else values

    @on.command_changed(2, actions=(ground_start_0, air_start_0))
    def partner_fallback(self, fighter: Any, ctx: MoveContext) -> None:
        """Enter the source's no-Nana start branch from projection facts.

        ``ftPp_SpecialHiStart_{0,Air}_Anim`` checks command 2 and then calls
        the fallback motion only when Nana is out of range.  Partner range is
        Missing partner position, lifecycle, scale, or ``x7C`` leaves this
        callback inert rather than guessing from the command value alone.
        """
        event = getattr(ctx, "event", None)
        if not getattr(event, "value", 0):
            return
        in_range = _partner_in_range(fighter, ctx, "x7C")
        if in_range is None and _partner_projection(ctx) is _NO_RESOLVER:
            in_range = _partner_available(ctx)
        if in_range is not False:
            return
        current = getattr(fighter.action, "action", fighter.action)
        ground_start = getattr(self.ground_start_0, "action", self.ground_start_0)
        target = self.ground_start_1 if current == ground_start else self.air_start_1
        fighter.change_action(target, preserve_state=True, keep_frame=True)

    @on.command_changed(1, actions=(ground_throw_0, air_throw_0))
    def partner_launch(self, fighter: Any, ctx: MoveContext) -> None:
        """Enter AirHiThrow2 when the projected Nana launch state is active."""
        event = getattr(ctx, "event", None)
        if not getattr(event, "value", 0) or _partner_launching(ctx) is not True:
            return
        # ftPp_SpecialHi_8012280C always selects motion state 354, including
        # when the command arrived on the ground throw row.
        fighter.change_action(self.air_throw_2)


class Blizzard(DownSpecial, DirectionalSpecial):
    """Blizzard's ground/air fighter phases; the blizzard article is native."""

    ground = source_phase(357)
    air = source_phase(358)
    _ACTIVE = (ground, air)

    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground, on_air = frame_preserving_surface_pairs(ground, ground, air, air)

    @on.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Clear the source command latches on both Blizzard entries.

        ``ftPp_SpecialLw_Enter`` and ``ftPp_SpecialAirLw_Enter`` clear
        ``cmd_vars[0]`` and ``cmd_vars[3]`` before the article callback is
        installed. Preserve the other command slots owned by the host.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and command:
            values = list(command)
            for index in (0, 3):
                if index < len(values):
                    values[index] = 0
            state.command = type(command)(values) if isinstance(command, tuple) else values


class IceClimbers(Fighter):
    specials = Fighter.specials.replace(
        neutral=IceShot(),
        side=SquallHammer(),
        up=Belay(),
        down=Blizzard(),
    )


__all__ = ["IceClimbers", "IceShot", "SquallHammer", "Belay", "Blizzard"]
