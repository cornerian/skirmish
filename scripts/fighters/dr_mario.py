"""Dr. Mario's source-backed special motion states.

The native Dr. Mario table reuses Mario's special callbacks for side, up, and
down specials.  Their article and effect ownership stays in the host; this
module describes the fighter-side source phases and surface lifecycle.
"""

import math

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


def _set_entry_velocity(fighter: Fighter, ctx, action, ground: bool) -> None:
    """Apply the source cape entry velocity adjustment when exposed."""
    velocity = getattr(fighter, "velocity", None)
    setter = getattr(fighter, "set_velocity", None)
    if not isinstance(velocity, (tuple, list)) or len(velocity) < 2:
        return
    try:
        horizontal, vertical = float(velocity[0]), float(velocity[1])
    except (TypeError, ValueError):
        return
    if ground:
        vertical = 0.0
    else:
        attributes = resource_attributes(ctx, action.resource)
        divisor = getattr(attributes, "vel_x_decay", None)
        if isinstance(divisor, bool) or not isinstance(divisor, (int, float)):
            return
        if divisor <= 0 or not math.isfinite(float(divisor)):
            return
        horizontal /= float(divisor)
    if callable(setter):
        setter(horizontal, vertical)
    else:
        fighter.velocity = (horizontal, vertical)


class Megavitamin(B0ArticleSpecial):
    article_id = ArticleId.DR_MARIO_VITAMIN
    ground, air = b0_source_phases(343, 344)

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx) -> None:
        """Reset the shared Mario throw latch when the pill motion starts.

        ``ftMr_SpecialN_Enter`` and ``ftMr_SpecialAirN_Enter`` clear the
        command zero cue and the fighter's throw flags before selecting the
        motion.  ``B0ArticleSpecial.enter`` already owns the command reset;
        keep the Dr. Mario specific latch reset beside it so stale input from
        an interrupted special cannot create a later pill.
        """
        super().enter(fighter, ctx)
        if hasattr(fighter, "throw_flags"):
            fighter.throw_flags = 0


class _SourcePair(DirectionalSpecial):
    """Shared ground/air lifecycle for Mario-family source specials."""

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


class SuperSheet(SideSpecial, _SourcePair):
    ground = source_phase(345)
    air = source_phase(346)

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx) -> None:
        """Reset the cape window and apply Mario's source entry velocity."""
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, command[3])
        # The source changeAction path clears the native reflection latch;
        # animation commands enable it again during the active cape window.
        flags = getattr(fighter, "flags", None)
        if flags is not None and hasattr(flags, "reflecting"):
            flags.reflecting = False
        _set_entry_velocity(fighter, ctx, self, fighter.action is self.ground)

    @hook.command_changed(1, actions=(ground, air))
    def reflect_command(self, fighter: Fighter, ctx) -> None:
        """Mirror the source cape reflection command window."""
        flags = getattr(fighter, "flags", None)
        if flags is None or not hasattr(flags, "reflecting"):
            return
        event = getattr(ctx, "event", None)
        value = getattr(event, "value", 0)
        if value == 1:
            flags.reflecting = True
        elif value == 0:
            flags.reflecting = False
        # The source has no branch for other command values.  In particular,
        # cmd_vars[1] == 2 leaves both native reflection latches unchanged.

    @hook.action_exit(ground, air)
    def exit(self, fighter: Fighter, ctx) -> None:
        """Clear reflection when the side-special phase actually ends.

        Ground/air surface transitions preserve the source reflection latch;
        leaving both side-special phases resets it with the cape lifecycle.
        """
        if fighter.action in (self.ground, self.air):
            return
        flags = getattr(fighter, "flags", None)
        if flags is not None and hasattr(flags, "reflecting"):
            flags.reflecting = False

    @hook.projectile_contact
    def projectile_contact(self, fighter: Fighter, hit: HitContext) -> None:
        """Mirror ftMr_SpecialS's active-cape reflection gate."""
        if fighter.flags.reflecting and hit.projectile and hit.damage <= hit.max_damage:
            hit.reflect = True


class SuperJumpPunch(UpSpecial, _SourcePair):
    ground = source_phase(347)
    air = source_phase(348)

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx) -> None:
        """Clear ``cmd_vars[0]`` before the uppercut motion starts.

        ``ftMr_SpecialHi_Enter`` and its aerial sibling clear the command
        latch and throw flags before selecting the motion.  The host owns the
        throw state, so expose the command reset at the script boundary and
        leave the other command slots intact.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, command[1], command[2], command[3])
        if hasattr(fighter, "throw_flags"):
            fighter.throw_flags = 0
        if fighter.action is self.air:
            velocity = getattr(fighter, "velocity", None)
            if not isinstance(velocity, (tuple, list)) or len(velocity) < 2:
                return
            attributes = resource_attributes(ctx, self.resource)
            multiplier = getattr(attributes, "specialhi_vel_x", None)
            if (
                isinstance(multiplier, bool)
                or not isinstance(multiplier, (int, float))
                or not math.isfinite(float(multiplier))
            ):
                return
            try:
                horizontal = float(velocity[0]) * float(multiplier)
            except (TypeError, ValueError):
                return
            if callable(getattr(fighter, "set_velocity", None)):
                fighter.set_velocity(horizontal, 0.0)
            else:
                fighter.velocity = (horizontal, 0.0)

    @hook.animation_end(ground, air)
    def enter_fall_special(self, fighter: Fighter, ctx) -> bool:
        """Match ``ftMr_SpecialHi_Anim``'s shared FallSpecial exit.

        Dr. Mario's motion table points at Mario's up-special animation
        callbacks, so both ground and aerial entries call the common
        ``ftCo_80096900`` fall-special transition when the animation ends.
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


class DrTornado(DownSpecial, _SourcePair):
    ground = source_phase(349)
    air = source_phase(350)

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx) -> None:
        """Reset the tornado command cues initialized by ``doStartMotion``."""
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, command[3])

    def _consume_air_tap(self, fighter: Fighter) -> bool:
        """Apply the source aerial tap latch and consume its command cue."""
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if not isinstance(command, (tuple, list)) or len(command) < 4:
            return False
        if not command[1]:
            return False
        state.command = (command[0], 0, command[2], command[3])
        # ftMr_SpecialAirLw_Anim stores x2234_tornadoCharge here.  Keep the
        # latch on action state so the next aerial entry/physics boundary can
        # observe it after the command slot has been consumed.
        state.tornado_charge = True
        return True

    @hook.command_changed(1, actions=(air,))
    def tap_command(self, fighter: Fighter, ctx) -> None:
        """Consume the aerial tap cue used by ``ftMr_SpecialAirLw_Anim``."""
        self._consume_air_tap(fighter)

    @hook.animation_end(air)
    def finish_air(self, fighter: Fighter, ctx) -> bool:
        """Process the final tap cue and source FallSpecial handoff.

        Dr. Mario uses Mario's ``ftMr_SpecialAirLw_Anim`` callback.  It
        consumes command 1 before checking the animation end, then enters
        ``ftCo_80096900`` with mobility 1 when ``speciallw.landing_lag`` is
        nonzero; zero uses ordinary Fall.  Consuming a tap alone never
        suppresses that terminal transition.
        """
        self._consume_air_tap(fighter)
        attributes = resource_attributes(ctx, self.resource)
        landing_lag = getattr(
            attributes,
            "speciallw_landing_lag",
            getattr(attributes, "landing_lag", None),
        )
        enter = getattr(fighter, "enter_fall_special", None)
        if (
            isinstance(landing_lag, (int, float))
            and not isinstance(landing_lag, bool)
            and landing_lag != 0
            and callable(enter)
        ):
            enter(mobility=1, landing_lag=landing_lag)
            return True
        return False


class DrMario(Fighter):
    specials = Fighter.specials.replace(
        neutral=Megavitamin(),
        side=SuperSheet(),
        up=SuperJumpPunch(),
        down=DrTornado(),
    )


__all__ = [
    "DrMario",
    "Megavitamin",
    "SuperSheet",
    "SuperJumpPunch",
    "DrTornado",
]
