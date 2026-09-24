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
    SideSpecial,
    Transition,
    UpSpecial,
    b0_source_phases,
    source_phase,
)


class Megavitamin(B0ArticleSpecial):
    article_id = ArticleId.DR_MARIO_VITAMIN
    ground, air = b0_source_phases(343, 344)


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
