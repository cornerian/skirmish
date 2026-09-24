"""Mewtwo's source motion states and resource-gated special entry.

The fighter callbacks in ``ftMewtwo`` create Shadow Ball and Disable articles,
and Confusion creates a native reflect/grab object. Those article/effect
resources are not part of the authoring package, so this module declares the
native motion graph and its safe lifecycle only.
"""

from __future__ import annotations

from typing import Any, ClassVar

from skirmish import (
    Action,
    ActionState,
    Button,
    DownSpecial,
    Fighter,
    NeutralSpecial,
    SideSpecial,
    Transition,
    UpSpecial,
    directional_match,
    fresh_special_input,
    hook,
    source_phase,
    start_action,
)


class MewtwoActionState(ActionState):
    """Small native state mirror for the flags used by ftMewtwo callbacks.

    Articles and collision objects stay owned by the native resource bridge;
    these values are the fighter-local decisions that remain observable when
    those resources are unavailable (and are therefore useful to replay and
    headless hosts too).
    """

    command: tuple[int, int, int, int] = (0, 0, 0, 0)
    shadow_ball_held: bool = False
    shadow_ball_released: bool = False
    shadow_ball_full: bool = False
    shadow_ball_charge: int = 0
    disable_fired: bool = False
    confusion_reflecting: bool = False
    confusion_grabbed: bool = False
    confusion_air_boosted: bool = False
    teleport_active: bool = False


def _fighter_state(fighter: Any) -> MewtwoActionState:
    state = getattr(fighter, "action_state", None)
    if state is None:
        state = MewtwoActionState()
        fighter.action_state = state
    return state


def _event_value(ctx: Any) -> int:
    value = getattr(getattr(ctx, "event", None), "value", 0)
    return int(value or 0)


def _set_reflecting(fighter: Any, value: bool) -> None:
    """Mirror ftMt_SpecialS_ReflectThink through the portable fighter flags."""
    flags = getattr(fighter, "flags", None)
    if flags is not None and hasattr(flags, "reflecting"):
        flags.reflecting = value
    state = _fighter_state(fighter)
    state.confusion_reflecting = value


def _state(phase: Any) -> int:
    return dict(phase.metadata)["slippi_state"]


class _MewtwoSpecial:
    """Shared B dispatch and native ground/air lifecycle for Mewtwo moves."""

    _ENTRY: ClassVar[tuple[Any, Any]]
    _ACTIVE: ClassVar[tuple[Any, ...]]

    @hook.input_pressed(Button.B, Button.L, Button.R)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        pressed = getattr(getattr(ctx, "input", None), "just_pressed", None)
        if fighter.action in getattr(self, "_CANCELS", ()) and pressed is not None:
            if pressed(Button.L) or pressed(Button.R):
                cancel = getattr(self, "_cancel_input", None)
                if cancel is not None:
                    return cancel(fighter, ctx)
        if fighter.action in self._ACTIVE:
            return True
        if not fresh_special_input(ctx, self.resource):
            return False
        if not directional_match(ctx, self.root):
            return False
        if not (ctx.ground_open or ctx.air_open):
            return False
        phase = self._ENTRY[0 if ctx.ground_open else 1]
        if not fighter.has_complete_animation(_state(phase)):
            return False
        start_action(fighter, phase)
        return True


class ShadowBall(NeutralSpecial, _MewtwoSpecial):
    """Source states 341--350; article creation remains resource-owned."""

    ground_start = source_phase(341)
    ground_loop = source_phase(342, animation_loop=True)
    ground_full = source_phase(343, animation_loop=True)
    ground_cancel = source_phase(344)
    ground_end = source_phase(345)
    air_start = source_phase(346)
    air_loop = source_phase(347, animation_loop=True)
    air_full = source_phase(348, animation_loop=True)
    air_cancel = source_phase(349)
    air_end = source_phase(350)
    _ACTIVE = (
        ground_start, ground_loop, ground_full, ground_cancel, ground_end,
        air_start, air_loop, air_full, air_cancel, air_end,
    )
    _ENTRY = (ground_start, air_start)
    _STARTS = (ground_start, air_start)
    _LOOPS = (ground_loop, air_loop, ground_full, air_full)
    _RELEASES = (ground_loop, air_loop, ground_full, air_full)
    _CANCELS = (ground_loop, air_loop, ground_full, air_full)

    @hook.action_enter(*_STARTS)
    def enter(self, fighter: Any, ctx: Any) -> None:
        state = _fighter_state(fighter)
        state.command = (0, 0, 0, 0)
        state.shadow_ball_held = False
        state.shadow_ball_released = False
        state.shadow_ball_full = False
        state.shadow_ball_charge = 0
        # ftMewtwo_SpecialN_ChangeAction clears vertical movement and starts
        # grounded charge from rest.  Hosts that expose velocity use the same
        # inexpensive observable reset; minimal descriptor hosts can omit it.
        if ctx is not None and getattr(ctx, "grounded", getattr(fighter, "grounded", False)):
            if hasattr(fighter, "set_velocity"):
                fighter.set_velocity(0.0, 0.0)
            elif hasattr(fighter, "ground_velocity"):
                fighter.ground_velocity = 0.0

    @hook.command_changed(3, actions=_STARTS)
    def create_held_shadow(self, fighter: Any, ctx: Any) -> None:
        if _event_value(ctx):
            _fighter_state(fighter).shadow_ball_held = True

    @hook.command_changed(1, actions=_RELEASES)
    def release_shadow(self, fighter: Any, ctx: Any) -> None:
        if _event_value(ctx) and _fighter_state(fighter).shadow_ball_held:
            _fighter_state(fighter).shadow_ball_released = True

    @hook.before_receive_hit(actions=_ACTIVE)
    def on_damage(self, fighter: Any, ctx: Any) -> None:
        """ftMt_SpecialN_OnTakeDamage removes an unfinished held ball."""
        state = _fighter_state(fighter)
        state.shadow_ball_held = False
        state.shadow_ball_released = False
        state.shadow_ball_charge = 0

    @hook.input_released(Button.B, actions=_RELEASES)
    def release_input(self, fighter: Any, ctx: Any) -> bool:
        """ftMt_SpecialNLoop_IASA enters End when B is released."""
        if fighter.action not in self._RELEASES:
            return False
        fighter.change_action(
            self.ground_end if fighter.action in (self.ground_loop, self.ground_full)
            else self.air_end
        )
        _fighter_state(fighter).shadow_ball_released = True
        return True

    def _cancel_input(self, fighter: Any, ctx: Any) -> bool:
        """LR cancels the held article and enters the source cancel state."""
        if fighter.action not in self._CANCELS:
            return False
        fighter.change_action(
            self.ground_cancel if fighter.action in (self.ground_loop, self.ground_full)
            else self.air_cancel
        )
        state = _fighter_state(fighter)
        state.shadow_ball_held = False
        state.shadow_ball_released = False
        return True

    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_loop: Transition(ground_loop, preserve_state=True, keep_frame=True),
        air_full: Transition(ground_full, preserve_state=True, keep_frame=True),
        air_cancel: Transition(ground_cancel, preserve_state=True, keep_frame=True),
        air_end: Transition(ground_end, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_loop: Transition(air_loop, preserve_state=True, keep_frame=True),
        ground_full: Transition(air_full, preserve_state=True, keep_frame=True),
        ground_cancel: Transition(air_cancel, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_end, preserve_state=True, keep_frame=True),
    }
    on_end = {
        ground_start: Transition(ground_loop),
        air_start: Transition(air_loop),
        ground_cancel: Transition(Action.WAIT),
        air_cancel: Transition(Action.FALL),
        ground_end: Transition(Action.WAIT),
        air_end: Transition(Action.FALL),
    }


class Confusion(SideSpecial, _MewtwoSpecial):
    """Source states 351--352; reflect/grab behavior needs native resources."""

    ground = source_phase(351)
    air = source_phase(352)
    _ACTIVE = (ground, air)
    _ENTRY = (ground, air)

    @hook.action_enter(ground, air)
    def enter(self, fighter: Any, ctx: Any) -> None:
        state = _fighter_state(fighter)
        # The native boost is one-shot across a ground/air phase transition;
        # a fresh grounded entry starts a new Confusion and clears the latch.
        if fighter.action == self.ground:
            state.confusion_air_boosted = False
            state.confusion_grabbed = False
            _set_reflecting(fighter, False)
        # ftMt_SpecialAirS_Enter applies the one-time air boost.  Resource
        # authors may expose it as ``side.attributes.air_boost``; absent data
        # deliberately leaves the host's current velocity untouched.
        if fighter.action == self.air and not getattr(state, "confusion_air_boosted", False) and hasattr(fighter, "set_velocity"):
            lookup = getattr(ctx, "resource", None)
            attrs = lookup("side.attributes") if lookup is not None else None
            boost = getattr(attrs, "air_boost", None)
            if boost is not None:
                velocity = getattr(fighter, "velocity", (0.0, 0.0))
                fighter.set_velocity(velocity[0], boost)
                state.confusion_air_boosted = True

    @hook.command_changed(0, actions=(ground, air))
    def grab_command(self, fighter: Any, ctx: Any) -> None:
        """Mirror ``ftMewtwo_SetGrabVictim`` after the grab command fires.

        The native callback consumes command variable 0 only when a victim
        exists.  A host can expose that object on either the event context or
        fighter; keeping the check here prevents a stray animation marker
        from claiming a grab.
        """
        if not _event_value(ctx):
            return
        victim = getattr(ctx, "victim", None)
        if victim is None:
            victim = getattr(fighter, "victim_gobj", None)
        if victim is not None:
            _fighter_state(fighter).confusion_grabbed = True

    @hook.command_changed(1, actions=(ground, air))
    def reflect_command(self, fighter: Any, ctx: Any) -> None:
        value = _event_value(ctx)
        if value == 1:
            _set_reflecting(fighter, True)
        elif value == 2:
            _set_reflecting(fighter, False)

    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}
    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}


class Teleport(UpSpecial, _MewtwoSpecial):
    """Source states 353--358; travel direction is selected by native attrs."""

    ground_start = source_phase(353)
    ground_travel = source_phase(354, animation_loop=True)
    ground_end = source_phase(355)
    air_start = source_phase(356)
    air_travel = source_phase(357, animation_loop=True)
    air_end = source_phase(358)
    _ACTIVE = (
        ground_start, ground_travel, ground_end,
        air_start, air_travel, air_end,
    )
    _ENTRY = (ground_start, air_start)

    @hook.action_enter(ground_start, air_start)
    def enter(self, fighter: Any, ctx: Any) -> None:
        # Both source start handlers clear ground movement.  Aerial Teleport
        # preserves momentum but halves it before choosing its launch vector.
        if fighter.action == self.ground_start:
            if hasattr(fighter, "ground_velocity"):
                fighter.ground_velocity = 0.0
            if hasattr(fighter, "set_velocity"):
                fighter.set_velocity(0.0, 0.0)
        elif hasattr(fighter, "velocity") and hasattr(fighter, "set_velocity"):
            velocity = fighter.velocity
            fighter.set_velocity(velocity[0] * 0.5, velocity[1] * 0.5)
        _fighter_state(fighter).teleport_active = False

    @hook.action_enter(ground_travel, air_travel)
    def begin_travel(self, fighter: Any, ctx: Any) -> None:
        _fighter_state(fighter).teleport_active = True

    @hook.landed(actions=(air_travel,))
    def travel_landed(self, fighter: Any, ctx: Any) -> None:
        """Land only after the native Teleport travel timer permits it.

        ``ftMt_SpecialAirHiLost_Coll`` checks the travel timer before taking
        the air-to-ground transition.  Hosts that expose that result provide
        ``teleport_timer_ready`` on the contact context; older hosts retain
        the historical immediate landing behavior when the field is absent.
        """
        if ctx is not None and hasattr(ctx, "teleport_timer_ready"):
            if not bool(ctx.teleport_timer_ready):
                return
        fighter.change_action(
            self.ground_travel, preserve_state=True, keep_frame=True
        )

    @hook.landed(actions=(air_end,))
    def land_end(self, fighter: Any, ctx: Any) -> None:
        """The aerial end state enters SpecialHi landing lag on contact."""
        fighter.change_action(Action.SPECIAL_HI_LANDING)
        _fighter_state(fighter).teleport_active = False

    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_travel: Transition(air_travel, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_end, preserve_state=True, keep_frame=True),
    }
    on_end = {
        ground_start: Transition(ground_travel),
        air_start: Transition(air_travel),
        ground_end: Transition(Action.WAIT),
        air_end: Transition(Action.FALL),
    }


class Disable(DownSpecial, _MewtwoSpecial):
    """Source states 359--360; the Disable article is intentionally omitted."""

    ground = source_phase(359)
    air = source_phase(360)
    _ACTIVE = (ground, air)
    _ENTRY = (ground, air)

    @hook.action_enter(ground, air)
    def enter(self, fighter: Any, ctx: Any) -> None:
        state = _fighter_state(fighter)
        state.disable_fired = False
        if fighter.action == self.air and hasattr(fighter, "set_velocity"):
            velocity = getattr(fighter, "velocity", (0.0, 0.0))
            fighter.set_velocity(velocity[0], 0.0)

    @hook.command_changed(0, actions=(ground, air))
    def create_disable(self, fighter: Any, ctx: Any) -> None:
        # The command is the exact source spawn point.  Native item creation
        # remains resource owned; retaining this bit lets either host consume
        # it without coupling the authoring script to item internals.
        if _event_value(ctx):
            _fighter_state(fighter).disable_fired = True

    @hook.before_receive_hit(actions=_ACTIVE)
    def on_damage(self, fighter: Any, ctx: Any) -> None:
        """ftMt_SpecialLw_SetCall destroys Disable when the owner is hit."""
        _fighter_state(fighter).disable_fired = False

    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}
    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}


class Mewtwo(Fighter):
    action_state = MewtwoActionState
    specials = Fighter.specials.replace(
        neutral=ShadowBall(),
        side=Confusion(),
        up=Teleport(),
        down=Disable(),
    )


__all__ = [
    "Mewtwo",
    "MewtwoActionState",
    "ShadowBall",
    "Confusion",
    "Teleport",
    "Disable",
]
