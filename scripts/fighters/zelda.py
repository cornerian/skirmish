"""Zelda's source-defined special phases.

Din Fire is article-backed.  Its Python declaration intentionally stops at
the native phase graph and resource gate; article ownership stays in the host.
Transformation likewise exposes no cross-character replacement policy.
"""

from __future__ import annotations

from enum import Enum
from typing import Any

from skirmish import (
    Action, ArticleId, Button, DirectionalSpecial, DownSpecial, Fighter,
    NeutralSpecial, SideSpecial, Transition, UpSpecial, on, source_phase,
    resource_attributes,
)
from fighter.helpers import resource_attributes


class Nayru(NeutralSpecial, DirectionalSpecial):
    """Nayru's Love phases and native command reflection window."""

    ground = source_phase(341)
    air = source_phase(342)
    _ACTIVE = (ground, air)

    @on.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx: object) -> None:
        """Clear the source command slot and stale reflection latch."""
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and command:
            state.command = (0, *command[1:])
        flags = getattr(fighter, "flags", None)
        if flags is not None and hasattr(flags, "reflecting"):
            flags.reflecting = False

    @on.command_changed(0, actions=(ground, air))
    def reflect_command(self, fighter: Fighter, ctx: object) -> None:
        """Mirror ``ftZd_SpecialN_Anim``'s command-0 reflect latch.

        The native callback consumes command value 1 by rewriting it to 2,
        creates the reflect volume, and clears the fighter's reflecting state
        when command 0 is observed. The volume and hit routing remain native
        host responsibilities.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        flags = getattr(fighter, "flags", None)
        value = getattr(getattr(ctx, "event", None), "value", 0)
        if value == 1:
            if isinstance(command, (tuple, list)) and command:
                state.command = (2, *command[1:])
            if flags is None or not hasattr(flags, "reflecting"):
                return
            flags.reflecting = True
        elif value == 0:
            if flags is None or not hasattr(flags, "reflecting"):
                return
            flags.reflecting = False

    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}


class Din(SideSpecial, DirectionalSpecial):
    """Din Fire phases; the fire article and its owner remain native-owned."""

    ground_start = source_phase(343)
    ground_loop = source_phase(344, animation_loop=True)
    ground_end = source_phase(345)
    air_start = source_phase(346)
    air_loop = source_phase(347, animation_loop=True)
    air_end = source_phase(348)
    ground = ground_start
    air = air_start
    _ACTIVE = (ground_start, ground_loop, ground_end, air_start, air_loop, air_end)

    @on.action_enter(ground_start, air_start)
    def enter(self, fighter: Fighter, ctx: object) -> None:
        """Reset all four Din Fire command slots on source entry.

        ``ftZd_SpecialS_Enter`` and its aerial counterpart explicitly clear
        ``cmd_vars[0..3]`` before starting the motion.  Keeping the complete
        command tuple reset prevents a stale animation cue from spawning an
        article in a newly entered Din Fire action.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, 0)

    @on.release(Button.B, actions=(ground_loop, air_loop))
    def release(self, fighter: Fighter, ctx: object) -> bool:
        """End the loop once the source's hold timer permits release.

        The native IASA decrements its release timer every frame and changes
        to the matching end state when B is no longer held.  The host exposes
        the release edge, so routing that edge to the source end phase keeps
        the declarative move responsive while preserving the native terminal
        state and surface transitions.
        """
        destination = {
            self.ground_loop: self.ground_end,
            self.air_loop: self.air_end,
        }.get(fighter.action)
        if destination is None:
            return False
        fighter.change_action(destination)
        return True

    @on.command_changed(0, actions=(ground_start, ground_loop, air_start, air_loop))
    def spawn(self, fighter: Fighter, ctx: object) -> None:
        """Forward the source command cue that creates Din Fire.

        ``ftZd_Special*S*_Anim`` consumes command variable 0 and creates the
        article once per cue.  The native article host must provide the exact
        joint-89 position and owner-handle state; article motion, ownership,
        and contact callbacks remain native responsibilities.
        """
        event = getattr(ctx, "event", None)
        if not getattr(event, "value", False):
            return

        # The source checks the native owner handle before spawning.  Require
        # an explicit host answer so a missing owner query cannot duplicate an
        # article, and require the host-computed joint-89 position rather than
        # fabricating a root-position fallback.
        active = getattr(fighter, "din_fire_active", None)
        if active is None:
            has_article = getattr(fighter, "has_active_article", None)
            if callable(has_article):
                active = has_article(ArticleId.ZELDA_DIN_FIRE)
        if active is None or active:
            return

        spawn = getattr(fighter, "spawn_article", None)
        if not callable(spawn):
            return
        position_provider = getattr(fighter, "din_fire_spawn_position", None)
        if not callable(position_provider):
            return
        position = position_provider()
        if position is None:
            return

        spawn(
            ArticleId.ZELDA_DIN_FIRE,
            position,
            getattr(fighter, "facing", 1.0),
        )
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, command[1], command[2], command[3])

    on_end = {
        ground_start: Transition(ground_loop), ground_end: Transition(Action.WAIT),
        air_start: Transition(air_loop), air_end: Transition(Action.FALL),
    }
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


class Farore(UpSpecial, DirectionalSpecial):
    """Farore's Wind start, travel, and terminal phases."""

    ground_start = source_phase(349)
    ground_start_1 = source_phase(350)
    ground_move = source_phase(351)
    air_start = source_phase(352)
    air_start_1 = source_phase(353)
    air_move = source_phase(354)
    ground = ground_start
    air = air_start
    _ACTIVE = (ground_start, ground_start_1, ground_move, air_start, air_start_1, air_move)

    @on.animation_end(air_move)
    def enter_fall_special(self, fighter: Fighter, ctx: Any) -> bool:
        """Enter FallSpecial with the source mobility and landing lag.

        ``ftZd_SpecialAirHi_Anim`` calls ``ftCo_80096900`` when state 354
        ends, passing Zelda's ``ftZelda_DatAttrs::x68`` and ``x6C``.  A plain
        ``Action.FALL`` loses both values and makes the aerial recovery
        immediately interruptible.  Resource exports may use descriptive
        names; the ``x68``/``x6C`` fallbacks preserve the source layout.
        """
        attributes = resource_attributes(ctx, self.resource)
        mobility = getattr(
            attributes,
            "specialhi_freefall_air_spd_mul",
            getattr(attributes, "specialhi_freefall_mobility", getattr(attributes, "x68", None)),
        )
        landing_lag = getattr(
            attributes,
            "specialhi_landing_lag",
            getattr(attributes, "x6C", None),
        )
        enter = getattr(fighter, "enter_fall_special", None)
        if mobility is None or landing_lag is None or not callable(enter):
            return False
        enter(mobility=mobility, landing_lag=landing_lag)
        return True

    on_end = {
        ground_start: Transition(ground_start_1), ground_start_1: Transition(ground_move),
        ground_move: Transition(Action.WAIT), air_start: Transition(air_start_1),
        air_start_1: Transition(air_move),
    }
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_start_1: Transition(ground_start_1, preserve_state=True, keep_frame=True),
        air_move: Transition(ground_move, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_start_1: Transition(air_start_1, preserve_state=True, keep_frame=True),
        ground_move: Transition(air_move, preserve_state=True, keep_frame=True),
    }

    @on.animation_end(air_move)
    def aerial_move_end(self, fighter: Fighter, ctx: object) -> bool:
        """Match ``ftZd_SpecialAirHi_Anim``'s source FallSpecial handoff."""
        attributes = resource_attributes(ctx, self.resource)
        mobility = getattr(
            attributes,
            "specialhi_freefall_air_spd_mul",
            getattr(attributes, "x68", None),
        )
        landing_lag = getattr(
            attributes,
            "specialhi_landing_lag",
            getattr(attributes, "x6C", None),
        )
        if mobility is None or landing_lag is None:
            fighter.change_action(Action.FALL)
            return True
        enter = getattr(fighter, "enter_fall_special", None)
        if not callable(enter):
            fighter.change_action(Action.FALL)
            return True
        enter(mobility=mobility, landing_lag=landing_lag)
        return True


class TransformOutcome(str, Enum):
    """Result of the native Zelda-to-Sheik completion seam."""

    UNSUPPORTED = "unsupported"


class Transform(DownSpecial, DirectionalSpecial):
    """Transformation phases; character replacement is deliberately native."""

    ground = source_phase(355)
    ground_end = source_phase(356)
    air = source_phase(357)
    air_end = source_phase(358)
    _ACTIVE = (ground, ground_end, air, air_end)

    @on.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx: object) -> None:
        """Reset command 0 as ``ftZelda_SpecialLw_StartAction_Helper`` does."""
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and command:
            state.command = (0, *command[1:])

    @on.animation_end(ground_end, air_end)
    def native_completion(self, fighter: Fighter, ctx: object) -> TransformOutcome:
        """Report the replacement seam until the host owns identity swapping.

        The pinned callback invokes ``ftCommon_8007EFC8`` to replace Zelda with
        Sheik.  The Python host has no fighter identity/resource swap API, so
        silently ending the move as a completed transformation would be
        observably wrong.  A native runtime may consume this typed outcome and
        perform the replacement once that seam exists.
        """
        return TransformOutcome.UNSUPPORTED

    # The terminal source states intentionally have no fallback transition.
    # Until the native identity swap exists, animation end must preserve the
    # source action instead of pretending Zelda completed as Zelda.
    on_end = {ground: Transition(ground_end), air: Transition(air_end)}
    on_ground = {
        air: Transition(ground, preserve_state=True, keep_frame=True),
        air_end: Transition(ground_end, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground: Transition(air, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_end, preserve_state=True, keep_frame=True),
    }


class Zelda(Fighter):
    specials = Fighter.specials.replace(
        neutral=Nayru(), side=Din(), up=Farore(), down=Transform()
    )


__all__ = ["Zelda", "Nayru", "Din", "Farore", "Transform", "TransformOutcome"]
