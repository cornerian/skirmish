"""Young Link's source-backed special motion declarations.

Young Link (``ftCLink`` in the decomp) shares Link's fighter callbacks, but
its source action namespace is bound to external id 21. The script exposes
the article-dependent branches that the current host API can represent:
boomerang empty phases, release forwarding, and held-bomb reuse.
"""

from __future__ import annotations

from typing import Any

from skirmish import Action, Fighter, Move, TauntMoves, action, source_action
from fighter.compat import Button
from fighter.events import on
from fighter.helpers import fresh_special_input, start_action
from fighter.link_family import (
    LinkDownSpecial as _FamilyDownSpecial,
    LinkNeutralSpecial as _FamilyNeutralSpecial,
    LinkSideSpecial as _FamilySideSpecial,
    LinkUpSpecial as _FamilyUpSpecial,
)
from fighter.transitions import Transition


def _phase(state: int, *, loop: bool = False, attack: str) -> Any:
    return action(
        source_action(state),
        slippi_state=state,
        attack=attack,
        animation_loop=loop,
    )


class YoungLinkNeutralSpecial(_FamilyNeutralSpecial):
    """Fire Arrow charge, hold, and release phases (344–349)."""

    ground_start = _phase(344, attack="neutral.ground_start")
    ground_loop = _phase(345, loop=True, attack="neutral.ground_loop")
    ground_end = _phase(346, attack="neutral.ground_end")
    air_start = _phase(347, attack="neutral.air_start")
    air_loop = _phase(348, loop=True, attack="neutral.air_loop")
    air_end = _phase(349, attack="neutral.air_end")
    _ENTRY_NAMES = ("ground_start", "air_start")
    _ACTIVE_NAMES = ("ground_start", "ground_loop", "air_start", "air_loop")
    on_end = {
        ground_start: Transition(ground_loop),
        air_start: Transition(air_loop),
        ground_end: Transition(Action.WAIT),
        air_end: Transition(Action.FALL),
    }

    @on.action_enter(ground_start, air_start)
    def reset_command_window(self, fighter: Any, ctx: Any) -> None:
        """Clear the four native command slots before a new arrow charge.

        ``ftLk_SpecialN_Enter`` and ``ftLk_SpecialAirN_Enter`` clear
        ``cmd_vars[0..3]`` before installing the shared arrow callbacks.  CLink
        uses those same Link callbacks, so stale command cues must not carry
        into a new charge motion.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", None)
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, 0)

    @on.release(Button.B)
    def release(self, fighter: Any, ctx: Any) -> bool:
        """Forward the source release edge before entering the End motion.

        ``ftLk_SpecialNStart_IASA`` and ``ftLk_SpecialNLoop_IASA`` both
        transition on the first frame with B released.  The article host owns
        copying charge and launch data into the CLink arrow, so preserve that
        optional callback at the same boundary before selecting the matching
        ground or aerial End state.
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


class YoungLinkSideSpecial(_FamilySideSpecial):
    """Boomerang throw/catch phases, including empty variants (350–355)."""

    ground_start = _phase(350, attack="side.ground_start")
    ground = _phase(351, attack="side.ground")
    ground_empty = _phase(352, attack="side.ground_empty")
    air_start = _phase(353, attack="side.air_start")
    air = _phase(354, attack="side.air")
    air_empty = _phase(355, attack="side.air_empty")
    _ENTRY_NAMES = ("ground_start", "air_start")
    _ACTIVE_NAMES = (
        "ground_start", "ground", "ground_empty",
        "air_start", "air", "air_empty",
    )
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

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
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

    @on.command_changed(0, actions=(ground, ground_empty, air, air_empty))
    def boomerang_release(self, fighter: Any, ctx: Any) -> None:
        """Forward native boomerang release to an article-aware host."""
        event = getattr(ctx, "event", None)
        if not getattr(event, "value", False):
            return
        # ``ftLk_SpecialS2_IASA`` only releases the article after the native
        # dash/smash command window accepts the directional input.  Keep the
        # host callback optional, but preserve those source gates when a host
        # provides the corresponding rules.
        specials = getattr(getattr(ctx, "rules", None), "specials", None)
        threshold = getattr(specials, "dash_smash_stick_threshold", None)
        window = getattr(specials, "dash_smash_window", None)
        early_frames = getattr(specials, "early_frames", None)
        # ``on21EC`` also adds common-data x44 to the dash-smash window.  The
        # portable host must provide both values; without them, preserve the
        # source gate by failing closed instead of guessing an effective end.
        if threshold is None or window is None or early_frames is None:
            return
        stick = getattr(getattr(ctx, "input", None), "stick", (0.0, 0.0))
        if abs(stick[0]) < threshold:
            return
        # Native on21EC accepts the dash-smash command only while x673 is
        # strictly below the configured window (plus the common offset).
        if getattr(fighter, "action_frame", 0) >= window + early_frames:
            return
        update = getattr(fighter, "update_boomerang_trajectory", None)
        if callable(update):
            update(ctx)


class YoungLinkUpSpecial(_FamilyUpSpecial):
    """Spin Attack ground and air phases (356–357)."""

    ground = _phase(356, attack="up.ground")
    air = _phase(357, attack="up.air")
    _ENTRY_NAMES = ("ground", "air")
    _ACTIVE_NAMES = ("ground", "air")
    # ``ftLk_SpecialAirHi_Anim`` enters FallSpecial with mobility enabled at
    # animation end.  The generic family transition to plain Fall loses that
    # aerial recovery state, so mirror Link's source callback here.
    on_end = {ground: Transition(Action.WAIT)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    @on.action_enter(air)
    def reset_aerial_jumps(self, fighter: Any, ctx: Any) -> None:
        """Apply ``ftLk_SpecialAirHi_Enter``'s jump-budget reset."""
        reset = getattr(fighter, "max_jumps", None)
        if callable(reset):
            reset()

    @on.animation_end(air)
    def enter_fall_special(self, fighter: Any, ctx: Any) -> bool:
        enter = getattr(fighter, "enter_fall_special", None)
        if callable(enter):
            enter(mobility=1)
            return True
        # Keep the source terminal transition safe for lightweight hosts that
        # have not exposed the native FallSpecial helper yet.
        change = getattr(fighter, "change_action", None)
        if not callable(change):
            return False
        change(Action.FALL)
        return True


class YoungLinkDownSpecial(_FamilyDownSpecial):
    """Bomb pull/throw ground and air phases (358–359)."""

    ground = _phase(358, attack="down.ground")
    air = _phase(359, attack="down.air")
    _ENTRY_NAMES = ("ground", "air")
    _ACTIVE_NAMES = ("ground", "air")
    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    @on.action_enter(ground, air)
    def reuse_held_bomb(self, fighter: Any, ctx: Any) -> None:
        """Preserve ftLk_SpecialLw_Enter's held-bomb branch when supported."""
        if not getattr(fighter, "link_bomb_held", False):
            return
        reuse = getattr(fighter, "reuse_held_bomb_special", None)
        if callable(reuse):
            reuse(airborne=fighter.action is self.air)


class YoungLinkAppeal(Move):
    """CLink's two Z-air appeal rows (342/343).

    ``ftCl_AppealS_Anim`` owns milk creation and cleanup in the native item
    layer. The authoring API can still retain both source motion identities so
    hosts can select the left/right row without collapsing them to Link's
    generic appeal action.
    """

    action = _phase(342, attack="taunt.right")
    left = _phase(343, attack="taunt.left")

    @on.animation_end(action, left)
    def animation_end(self, fighter: Any, ctx: Any) -> None:
        """Return from either CLink Z-air row through the common terminal path.

        ``ftCl_AppealS_Anim`` calls ``ft_8008A2BC`` after the appeal motion
        runs out.  That common helper selects grounded Wait or aerial Fall;
        leaving these two source motions without an explicit terminal rule
        would strand a portable host in the taunt action.
        """
        grounded = getattr(ctx, "grounded", getattr(fighter, "grounded", True))
        fighter.change_action(Action.WAIT if grounded else Action.FALL)

    @on.command_changed(1, actions=(action, left))
    def milk_command(self, fighter: Any, ctx: Any) -> None:
        """Forward CLink's command-triggered milk spawn/cleanup boundary.

        ``ftCl_AppealS_Anim`` handles ``cmd_vars[1] == 1`` by creating milk
        and ``== 2`` by running the native cleanup check.  Keep those values
        distinct and optional at the host boundary; unrelated command values
        must not create or destroy an appeal article.
        """
        event = getattr(ctx, "event", None)
        value = getattr(event, "value", None)
        if value == 1:
            active = getattr(fighter, "appeal_milk_active", None)
            if active is None:
                has_article = getattr(fighter, "has_active_article", None)
                if callable(has_article):
                    active = has_article("young_link_milk")
            # Native ftCl checks x18 before creating milk.  An absent host
            # fact cannot prove that x18 is NULL, so fail closed.
            if active is None or active:
                return
            spawn = getattr(fighter, "spawn_appeal_milk", None)
            if callable(spawn):
                spawn(ctx)
        elif value == 2:
            cleanup = getattr(fighter, "cleanup_appeal_milk", None)
            if callable(cleanup):
                cleanup(ctx)


class YoungLink(Fighter):
    """Source-state definition for the CLink fighter (external id 21)."""

    specials = Fighter.specials.replace(
        neutral=YoungLinkNeutralSpecial(),
        side=YoungLinkSideSpecial(),
        up=YoungLinkUpSpecial(),
        down=YoungLinkDownSpecial(),
    )
    taunt = TauntMoves(YoungLinkAppeal())


__all__ = [
    "YoungLink",
    "YoungLinkNeutralSpecial",
    "YoungLinkSideSpecial",
    "YoungLinkUpSpecial",
    "YoungLinkDownSpecial",
    "YoungLinkAppeal",
]
