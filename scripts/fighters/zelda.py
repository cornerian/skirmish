"""Zelda's source-defined special phases.

Din Fire is article-backed.  Its Python declaration intentionally stops at
the native phase graph and resource gate; article ownership stays in the host.
Transformation likewise exposes no cross-character replacement policy.
"""

from __future__ import annotations

from skirmish import (
    Action, ArticleId, Button, DirectionalSpecial, DownSpecial, Fighter,
    NeutralSpecial, SideSpecial, Transition, UpSpecial, on, source_phase,
)


class Nayru(NeutralSpecial, DirectionalSpecial):
    """Nayru's Love ground and aerial phases."""

    ground = source_phase(341)
    air = source_phase(342)
    _ACTIVE = (ground, air)

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
        article once per cue.  The native article host may provide an exact
        joint position through ``din_fire_spawn_position``; the position
        fallback keeps this callback useful for lightweight hosts that expose
        only the fighter root.  Article motion, ownership, and contact
        callbacks remain native responsibilities.
        """
        event = getattr(ctx, "event", None)
        if not getattr(event, "value", False):
            return

        spawn = getattr(fighter, "spawn_article", None)
        if not callable(spawn):
            return
        position_provider = getattr(fighter, "din_fire_spawn_position", None)
        position = position_provider() if callable(position_provider) else None
        if position is None:
            position = getattr(fighter, "position", None)
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

    on_end = {
        ground_start: Transition(ground_start_1), ground_start_1: Transition(ground_move),
        ground_move: Transition(Action.WAIT), air_start: Transition(air_start_1),
        air_start_1: Transition(air_move), air_move: Transition(Action.FALL),
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


class Transform(DownSpecial, DirectionalSpecial):
    """Transformation phases; character replacement is deliberately native."""

    ground = source_phase(355)
    ground_end = source_phase(356)
    air = source_phase(357)
    air_end = source_phase(358)
    _ACTIVE = (ground, ground_end, air, air_end)

    on_end = {
        ground: Transition(ground_end), ground_end: Transition(Action.WAIT),
        air: Transition(air_end), air_end: Transition(Action.FALL),
    }
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


__all__ = ["Zelda", "Nayru", "Din", "Farore", "Transform"]
