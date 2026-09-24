"""Sheik's source-defined special phases.

Needle and chain articles remain native-owned; this module declares their
source phases and resource gates without inventing article behavior.
"""

from __future__ import annotations

from typing import Any

from skirmish import (
    Action, ActionState, Button, DirectionalSpecial, DownSpecial, Fighter,
    NeutralSpecial, SideSpecial, Transition, UpSpecial, on, source_phase,
)
from fighter.helpers import resource_attributes


def _button_held(ctx: Any, button: Button) -> bool:
    """Read the host held-button API, preserving legacy event contexts."""
    input_state = getattr(ctx, "input", None)
    held_method = getattr(input_state, "held", None)
    if callable(held_method):
        return bool(held_method(button))
    held = getattr(input_state, "held_buttons", None)
    if held is None:
        return True
    if isinstance(held, (tuple, list, set, frozenset)):
        return button in held
    return bool(held)


class SheikActionState(ActionState):
    """Small script-visible latch mirrored by the native chain IASA.

    ``ftSk_SpecialS_IASA`` stores the B-release request and lets the chain
    animation callback consume it after ``ftSeakAttributes.x14``.  Keeping
    the edge in action state makes the request replay-safe without guessing
    the packed fighter attribute (no typed Sheik attribute resource exists
    yet).
    """

    chain_release_latched: bool = False


class Needles(NeutralSpecial, DirectionalSpecial):
    """Needle charge/release states from ``ftseakspecialn.c``.

    ``ftSk_SpecialNLoop_IASA`` selects cancel versus end while B is held or
    released; article spawning and the six-charge bookkeeping stay native.
    """

    ground_start = source_phase(341)
    ground_loop = source_phase(342, animation_loop=True)
    ground_cancel = source_phase(343)
    ground_end = source_phase(344)
    air_start = source_phase(345)
    air_loop = source_phase(346, animation_loop=True)
    air_cancel = source_phase(347)
    air_end = source_phase(348)
    ground = ground_start
    air = air_start
    _ACTIVE = (ground_start, ground_loop, air_start, air_loop)

    @on.input_pressed(Button.B, Button.L, Button.R)
    def input_pressed(self, fighter: Fighter, ctx: Any) -> bool:
        """Select B entry or the native loop-only shoulder cancel branch."""
        input_state = getattr(ctx, "input", None)
        just_pressed = getattr(input_state, "just_pressed", None)
        if callable(just_pressed) and (
            just_pressed(Button.L) or just_pressed(Button.R)
        ):
            destination = {
                self.ground_loop: self.ground_cancel,
                self.air_loop: self.air_cancel,
            }.get(fighter.action)
            if destination is not None:
                # ftSk_SpecialN{,AirN}Loop_IASA checks !held B first.  A
                # frame containing both B release and LR press therefore
                # shoots the charged needles instead of cancelling.
                if not _button_held(ctx, Button.B):
                    end = {
                        self.ground_loop: self.ground_end,
                        self.air_loop: self.air_end,
                    }[fighter.action]
                    fighter.change_action(end)
                    return True
                fighter.change_action(destination)
                return True
        if callable(just_pressed) and not just_pressed(Button.B):
            return False
        return DirectionalSpecial.input_pressed(self, fighter, ctx)

    @on.release(Button.B, actions=(ground_loop, air_loop))
    def release(self, fighter: Fighter, ctx: Any) -> bool:
        """Enter the throw phase only after the charge loop has started.

        ``ftSk_SpecialNStart_IASA`` and its aerial counterpart are empty in
        the source.  The input dispatcher therefore must not interpret an
        early B release as an end transition while the held needle article is
        still being created by the start animation.
        """
        destination = {
            self.ground_loop: self.ground_end,
            self.air_loop: self.air_end,
        }.get(fighter.action)
        if destination is None:
            return False
        fighter.change_action(destination)
        return True

    on_end = {
        ground_start: Transition(ground_loop), air_start: Transition(air_loop),
        ground_cancel: Transition(Action.WAIT), ground_end: Transition(Action.WAIT),
        air_cancel: Transition(Action.FALL), air_end: Transition(Action.FALL),
    }
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_loop: Transition(ground_loop, preserve_state=True, keep_frame=True),
        air_cancel: Transition(ground_cancel, preserve_state=True, keep_frame=True),
        air_end: Transition(ground_end, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_loop: Transition(air_loop, preserve_state=True, keep_frame=True),
        ground_cancel: Transition(air_cancel, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_end, preserve_state=True, keep_frame=True),
    }


class Chain(SideSpecial, DirectionalSpecial):
    """Chain extension/retraction states from ``ftseakspecials.c``.

    The loop phase exits through the native chain reach/release condition,
    so only its start and terminal animation completions are declarative.
    """

    ground_start = source_phase(349)
    ground_loop = source_phase(350, animation_loop=True)
    ground_end = source_phase(351)
    air_start = source_phase(352)
    air_loop = source_phase(353, animation_loop=True)
    air_end = source_phase(354)
    ground = ground_start
    air = air_start
    _ACTIVE = (ground_start, ground_loop, ground_end, air_start, air_loop, air_end)

    @on.release(Button.B)
    def release(self, fighter: Fighter, ctx: Any) -> bool:
        """Latch B release for the native, attribute-gated chain callback.

        ``ftSk_SpecialS_IASA`` (``ftseakspecials.c:738-758``) sets the
        release latch, while ``ftSk_SpecialS_Anim``
        (``ftseakspecials.c:677-735``) waits until its packed
        ``ftSeakAttributes.x14`` minimum frame before selecting 351/354.
        The script deliberately leaves the action unchanged: transitioning
        here would bypass that source-controlled delay.
        """
        destination = {
            self.ground_loop: self.ground_end,
            self.air_loop: self.air_end,
        }.get(fighter.action)
        if destination is None:
            return False
        state = getattr(fighter, "action_state", None)
        if state is not None:
            state.chain_release_latched = True
        # Keep native IASA/callback processing alive.  The destination map is
        # retained above as an explicit source audit of the only legal exits.
        _ = destination
        return False

    @on.action_enter(ground_loop, air_loop)
    def enter_loop(self, fighter: Fighter, ctx: Any) -> None:
        state = getattr(fighter, "action_state", None)
        if state is not None:
            state.chain_release_latched = False

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


class Vanish(UpSpecial, DirectionalSpecial):
    """Vanish's two start phases and terminal travel phase.

    Direction selection, velocity, collision teleporting, invisibility, and
    the native article/effect callbacks remain owned by ``ftseakspecialhi.c``.
    """

    ground_start = source_phase(355)
    ground_start_1 = source_phase(356)
    ground_move = source_phase(357)
    air_start = source_phase(358)
    air_start_1 = source_phase(359)
    air_move = source_phase(360)
    ground = ground_start
    air = air_start
    _ACTIVE = (ground_start, ground_start_1, ground_move, air_start, air_start_1, air_move)

    @on.animation_end(air_move)
    def enter_fall_special(self, fighter: Fighter, ctx: Any) -> bool:
        """Match ``ftSk_SpecialAirHi_Anim``'s FallSpecial handoff.

        The source passes ``ftSeakAttributes.x58`` and ``x5C`` to
        ``ftCo_80096900`` when state 360 ends.  Named resource fields are
        accepted when available, with the source offsets retained as the
        compatibility fallback until Sheik's attributes are typed.
        """
        attributes = resource_attributes(ctx, self.resource)
        mobility = getattr(
            attributes,
            "specialhi_freefall_air_spd_mul",
            getattr(attributes, "specialhi_freefall_mobility", getattr(attributes, "x58", None)),
        )
        landing_lag = getattr(
            attributes,
            "specialhi_landing_lag",
            getattr(attributes, "x5C", getattr(attributes, "x5c", None)),
        )
        enter = getattr(fighter, "enter_fall_special", None)
        if mobility is None or landing_lag is None or not callable(enter):
            return False
        enter(mobility=mobility, landing_lag=landing_lag)
        return True

    on_end = {
        ground_start: Transition(ground_start_1),
        ground_move: Transition(Action.WAIT),
        air_start: Transition(air_start_1),
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
    """Transformation phases from ``ftseakspeciallw.c``.

    The character swap and transform effects are native callbacks; these
    phases preserve their grounded/aerial terminal exits for the host.
    """

    ground = source_phase(361)
    ground_end = source_phase(362)
    air = source_phase(363)
    air_end = source_phase(364)
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


class Sheik(Fighter):
    action_state = SheikActionState
    specials = Fighter.specials.replace(
        neutral=Needles(), side=Chain(), up=Vanish(), down=Transform()
    )


__all__ = ["Sheik", "Needles", "Chain", "Vanish", "Transform", "SheikActionState"]
