"""Mario's source-backed special motion states.

Mario's side, up, and down callbacks are fighter-local motion handlers in the
pinned ``ftMario`` source.  Their cape and effect/article ownership remains a
host boundary; this module describes the native motion phases and surface
lifecycle exposed by the authoring API.
"""

from skirmish import (
    Action,
    ArticleId,
    B0ArticleSpecial,
    DirectionalSpecial,
    DownSpecial,
    Fighter,
    HitContext,
    SideSpecial,
    Transition,
    UpSpecial,
    b0_source_phases,
    hook,
    source_phase,
)
from fighter.helpers import resource_attributes


class Fireball(B0ArticleSpecial):
    article_id = ArticleId.MARIO_FIRE
    ground, air = b0_source_phases(343, 344)

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx) -> None:
        """Reset the source throw latch when the fireball motion starts.

        ``ftMr_SpecialN_Enter`` clears ``cmd_vars[0]`` and ``throw_flags``
        before selecting either the ground or aerial motion.  The shared B0
        entry already clears the command slot; mirror the fighter-level reset
        here so a stale throw cue cannot leak into a new fireball.
        """
        super().enter(fighter, ctx)
        if hasattr(fighter, "throw_flags"):
            fighter.throw_flags = 0


class _SourcePair(DirectionalSpecial):
    """Shared ground/air lifecycle for Mario's source special pairs."""

    on_end = {"ground": Transition(Action.WAIT), "air": Transition(Action.FALL)}

    def __init_subclass__(cls, **kwargs):
        if "ground" in cls.__dict__ and "air" in cls.__dict__:
            cls._ACTIVE = (cls.ground, cls.air)
            cls.on_end = {
                cls.ground: Transition(Action.WAIT),
                cls.air: Transition(Action.FALL),
            }
            cls.on_ground = {
                cls.air: Transition(cls.ground, preserve_state=True, keep_frame=True)
            }
            cls.on_air = {
                cls.ground: Transition(cls.air, preserve_state=True, keep_frame=True)
            }
        super().__init_subclass__(**kwargs)


class Cape(SideSpecial, _SourcePair):
    ground = source_phase(345)
    air = source_phase(346)

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx) -> None:
        """Reset the native cape command window on every entry.

        ``changeAction`` in ``ftmariospecials.c`` clears command variables 0,
        1, and 2 before the accessory callback creates the cape. The fourth
        slot is owned by the common command stream and is preserved here.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, command[3])
        # ``changeAction`` also clears the fighter-level reflect latch.  Keep
        # this defensive so small authoring fixtures without ``flags`` remain
        # valid while native hosts get the same entry invariant.
        flags = getattr(fighter, "flags", None)
        if flags is not None and hasattr(flags, "reflecting"):
            flags.reflecting = False

    @hook.command_changed(1, actions=(ground, air))
    def reflect_command(self, fighter: Fighter, ctx) -> None:
        """Expose the source animation's command-1 reflect window.

        ``ftMr_SpecialS_Phys`` raises its internal reflect state when command
        variable 1 becomes one and clears it when the command returns to zero.
        The host's projectile callback consumes the fighter-level latch, so
        mirror that transition at the same native command boundary.
        """
        flags = getattr(fighter, "flags", None)
        if flags is None or not hasattr(flags, "reflecting"):
            return
        event = getattr(ctx, "event", None)
        flags.reflecting = bool(getattr(event, "value", 0))

    @hook.projectile_contact
    def projectile_contact(self, fighter: Fighter, hit: HitContext) -> None:
        """Mirror the native cape reflect callback's eligibility gate.

        The host owns the animation command that enables ``reflecting`` and
        the projectile handoff itself.  Mario's fighter callback only accepts
        projectile contacts while that flag is active and while the incoming
        damage is within the cape reflection descriptor's maximum.
        """
        if fighter.flags.reflecting and hit.projectile and hit.damage <= hit.max_damage:
            hit.reflect = True


class SuperJumpPunch(UpSpecial, _SourcePair):
    ground = source_phase(347)
    air = source_phase(348)

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx) -> None:
        """Clear the up-special command latch on every source entry.

        ``ftMr_SpecialHi_Enter`` and ``ftMr_SpecialAirHi_Enter`` both clear
        ``cmd_vars[0]`` before starting the motion.  The remaining command
        slots belong to the common animation stream, so preserve them while
        reproducing that fighter-local reset.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, command[1], command[2], command[3])
        if hasattr(fighter, "throw_flags"):
            fighter.throw_flags = 0

    @hook.animation_end(ground, air)
    def enter_fall_special(self, fighter: Fighter, ctx) -> bool:
        """Match ``ftMr_SpecialHi_Anim``'s shared FallSpecial exit.

        Both source callbacks call ``ftCo_80096900`` with Mario's freefall
        mobility and special landing lag.  The native fighter host owns the
        actual fall-special action and may expose it through
        ``enter_fall_special``; keep the callback optional for lightweight
        authoring contexts.
        """
        attributes = resource_attributes(ctx, self.resource)
        mobility = getattr(
            attributes,
            "specialhi_freefall_air_spd_mul",
            getattr(attributes, "specialhi_freefall_mobility", None),
        )
        landing_lag = getattr(attributes, "specialhi_landing_lag", None)
        enter = getattr(fighter, "enter_fall_special", None)
        if mobility is None or landing_lag is None or not callable(enter):
            return False
        enter(mobility=mobility, landing_lag=landing_lag)
        return True


class MarioTornado(DownSpecial, _SourcePair):
    ground = source_phase(349)
    air = source_phase(350)

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx) -> None:
        """Reset the command cues initialized by ``doStartMotion``."""
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, command[3])

    @hook.command_changed(1, actions=(air,))
    def tap_command(self, fighter: Fighter, ctx) -> None:
        """Consume the aerial tap cue used by ``ftMr_SpecialAirLw_Anim``.

        The native animation callback turns command variable 1 into the
        persistent tornado-charge flag and clears the command slot. The host
        owns that persistent physics flag; consuming the cue here preserves
        the observable one-shot command behavior.
        """
        event = getattr(ctx, "event", None)
        if not getattr(event, "value", 0):
            return
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (command[0], 0, command[2], command[3])


class Mario(Fighter):
    specials = Fighter.specials.replace(
        neutral=Fireball(),
        side=Cape(),
        up=SuperJumpPunch(),
        down=MarioTornado(),
    )


__all__ = ["Mario", "Fireball", "Cape", "SuperJumpPunch", "MarioTornado"]
