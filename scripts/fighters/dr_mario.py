"""Dr. Mario's source-backed special motion states.

The native Dr. Mario table reuses Mario's special callbacks for side, up, and
down specials.  Their article and effect ownership stays in the host; this
module describes the fighter-side source phases and surface lifecycle.
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
        """Reset Mario's cape command window on every Dr. entry."""
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, command[3])
        # The source changeAction path clears the native reflection latch;
        # animation commands enable it again during the active cape window.
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


class DrTornado(DownSpecial, _SourcePair):
    ground = source_phase(349)
    air = source_phase(350)


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
