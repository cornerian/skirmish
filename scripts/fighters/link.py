"""Link's source-backed special motion declarations."""

from __future__ import annotations

from typing import Any

from skirmish import Action, Fighter, action, source_action
from fighter.link_family import (
    LinkDownSpecial as _FamilyDownSpecial,
    LinkNeutralSpecial as _FamilyNeutralSpecial,
    LinkSideSpecial as _FamilySideSpecial,
    LinkUpSpecial as _FamilyUpSpecial,
)
from fighter.compat import Button
from fighter.events import on
from fighter.helpers import fresh_special_input, start_action
from fighter.transitions import Transition


def _phase(state: int, *, loop: bool = False, attack: str) -> Any:
    return action(source_action(state), slippi_state=state, attack=attack,
                  animation_loop=loop)


class LinkNeutralSpecial(_FamilyNeutralSpecial):
    ground_start = _phase(344, attack="neutral.ground_start")
    ground_loop = _phase(345, loop=True, attack="neutral.ground_loop")
    ground_end = _phase(346, attack="neutral.ground_end")
    air_start = _phase(347, attack="neutral.air_start")
    air_loop = _phase(348, loop=True, attack="neutral.air_loop")
    air_end = _phase(349, attack="neutral.air_end")
    _ENTRY_NAMES = ("ground_start", "air_start")
    _ACTIVE_NAMES = ("ground_start", "ground_loop", "air_start", "air_loop")
    on_end = {
        ground_start: Transition(ground_loop), air_start: Transition(air_loop),
        ground_end: Transition(Action.WAIT), air_end: Transition(Action.FALL),
    }

    @on.action_enter(ground_start, air_start)
    def reset_command_window(self, fighter: Any, ctx: Any) -> None:
        """Clear Link's four command slots when a fresh arrow starts.

        ``ftLk_SpecialN_Enter`` and ``ftLk_SpecialAirN_Enter`` clear
        ``cmd_vars[0..3]`` before installing the arrow callback.  Keeping
        that reset at the script boundary prevents a command cue left by a
        previous special from being consumed by the new charge motion.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", None)
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, 0)

    @on.release(Button.B)
    def release(self, fighter: Any, ctx: Any) -> bool:
        """Release the drawn arrow when the source charge loop sees B up.

        ``ftLk_SpecialNStart_IASA`` and ``ftLk_SpecialNLoop_IASA`` both
        enter the matching ``End`` motion on the first frame without B.  The
        item callback that copies charge and launch angle into the arrow is
        owned by the article host, so forward the edge through the optional
        ``release_arrow`` method after selecting the native end phase.
        """
        destination = {
            self.ground_start: self.ground_end,
            self.ground_loop: self.ground_end,
            self.air_start: self.air_end,
            self.air_loop: self.air_end,
        }.get(fighter.action)
        if destination is None:
            return False
        release = getattr(fighter, "release_arrow", None)
        if callable(release):
            release(ctx)
        fighter.change_action(destination)
        return True
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


class LinkSideSpecial(_FamilySideSpecial):
    ground_start = _phase(350, attack="side.ground_start")
    ground = _phase(351, attack="side.ground")
    ground_empty = _phase(352, attack="side.ground_empty")
    air_start = _phase(353, attack="side.air_start")
    air = _phase(354, attack="side.air")
    air_empty = _phase(355, attack="side.air_empty")
    _ENTRY_NAMES = ("ground_start", "air_start")
    _ACTIVE_NAMES = ("ground_start", "ground", "ground_empty", "air_start", "air", "air_empty")
    on_end = {
        ground_start: Transition(Action.WAIT), ground: Transition(Action.WAIT),
        ground_empty: Transition(Action.WAIT), air_start: Transition(Action.FALL),
        air: Transition(Action.FALL), air_empty: Transition(Action.FALL),
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

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        if fighter.action in self._active_actions():
            return True
        if not fresh_special_input(ctx, self.resource) or not self._direction_matches(ctx):
            return False
        if not (ctx.ground_open or ctx.air_open):
            return False
        held = bool(getattr(fighter, "used_boomerang", False) or
                    getattr(fighter, "boomerang_active", False))
        phase = ("ground_empty" if held else "ground_start") if ctx.ground_open else (
            "air_empty" if held else "air_start"
        )
        start_action(fighter, getattr(self, phase))
        return True

    @on.command_changed(0, actions=(ground, ground_empty, air, air_empty))
    def boomerang_release(self, fighter: Any, ctx: Any) -> None:
        """Forward the source command that releases a held boomerang.

        ``ftLk_SpecialS`` computes the return angle and distance in the native
        article callback.  The script boundary can still preserve the event
        and hand it to hosts that own article simulation; hosts without that
        optional method retain the native article path unchanged.
        """
        event = getattr(ctx, "event", None)
        if not getattr(event, "value", False):
            return
        specials = getattr(getattr(ctx, "rules", None), "specials", None)
        threshold = getattr(specials, "dash_smash_stick_threshold", None)
        if threshold is not None:
            stick = getattr(getattr(ctx, "input", None), "stick", (0.0, 0.0))
            if abs(stick[0]) < threshold:
                return
        window = getattr(specials, "dash_smash_window", None)
        if window is not None and getattr(fighter, "action_frame", 0) > window:
            return
        update = getattr(fighter, "update_boomerang_trajectory", None)
        if callable(update):
            update(ctx)


class LinkUpSpecial(_FamilyUpSpecial):
    ground = _phase(356, attack="up.ground")
    air = _phase(357, attack="up.air")
    _ENTRY_NAMES = ("ground", "air")
    _ACTIVE_NAMES = ("ground", "air")
    # ftLk_SpecialAirHi_Anim calls ftCo_80096900 when its animation ends;
    # that enters FallSpecial and restores aerial mobility.  The ordinary
    # family terminal transition would incorrectly enter plain Fall.
    on_end = {ground: Transition(Action.WAIT)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    @on.action_enter(air)
    def enter_air_spin(self, fighter: Any, ctx: Any) -> None:
        """Apply the source aerial-entry jump reset.

        ``ftLk_SpecialAirHi_Enter`` resets ``x1968_jumpsUsed`` to the
        fighter's maximum before the spin starts.  The launch velocity is
        owned by the native attribute host, but the jump reset is a stable
        fighter-side invariant and is exposed by the common authoring API.
        """
        reset = getattr(fighter, "max_jumps", None)
        if callable(reset):
            reset()

    @on.animation_end(air)
    def enter_fall_special(self, fighter: Any, ctx: Any) -> bool:
        enter = getattr(fighter, "enter_fall_special", None)
        if not callable(enter):
            return False
        enter(mobility=1)
        return True


class LinkDownSpecial(_FamilyDownSpecial):
    ground = _phase(358, attack="down.ground")
    air = _phase(359, attack="down.air")
    _ENTRY_NAMES = ("ground", "air")
    _ACTIVE_NAMES = ("ground", "air")
    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    @on.action_enter(ground, air)
    def reuse_held_bomb(self, fighter: Any, ctx: Any) -> None:
        """Preserve ``ftLk_SpecialLw_Enter``'s held-bomb branch.

        Native Link checks for an existing Link bomb before starting the
        normal pull animation and routes that item through the common throw
        state. Article ownership stays in the host; a host that exposes the
        branch can consume it through this optional callback.
        """
        if not getattr(fighter, "link_bomb_held", False):
            return
        reuse = getattr(fighter, "reuse_held_bomb_special", None)
        if callable(reuse):
            reuse(airborne=fighter.action is self.air)


class Link(Fighter):
    specials = Fighter.specials.replace(
        neutral=LinkNeutralSpecial(), side=LinkSideSpecial(),
        up=LinkUpSpecial(), down=LinkDownSpecial(),
    )


__all__ = ["Link", "LinkNeutralSpecial", "LinkSideSpecial", "LinkUpSpecial", "LinkDownSpecial"]
