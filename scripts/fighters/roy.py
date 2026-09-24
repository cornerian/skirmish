"""Roy's source-state special declarations."""

from skirmish import Fighter, source_phase
from fighter.emblem_family import (
    EmblemDownSpecial,
    EmblemNeutralSpecial,
    EmblemSideSpecial,
    EmblemUpSpecial,
)


class FlareBlade(EmblemNeutralSpecial):
    """Roy's Flare Blade phases from ``ftMars_Init_MotionStateTable``."""

    ground_start = source_phase(341)
    ground_loop = source_phase(342, animation_loop=True)
    ground_end0 = source_phase(343)
    ground_end1 = source_phase(344)
    air_start = source_phase(345)
    air_loop = source_phase(346, animation_loop=True)
    air_end0 = source_phase(347)
    air_end1 = source_phase(348)



class DoubleEdgeDance(EmblemSideSpecial):
    """Roy's four stage Dancing Blade phase tree (ground and air)."""

    ground_start = source_phase(349)
    ground_2_up = source_phase(350)
    ground_2_down = source_phase(351)
    ground_3_up = source_phase(352)
    ground_3_neutral = source_phase(353)
    ground_3_down = source_phase(354)
    ground_4_up = source_phase(355)
    ground_4_neutral = source_phase(356)
    ground_4_down = source_phase(357)
    air_start = source_phase(358)
    air_2_up = source_phase(359)
    air_2_down = source_phase(360)
    air_3_up = source_phase(361)
    air_3_neutral = source_phase(362)
    air_3_down = source_phase(363)
    air_4_up = source_phase(364)
    air_4_neutral = source_phase(365)
    air_4_down = source_phase(366)


class Blazer(EmblemUpSpecial):
    """Roy's ground and aerial Blazer states."""

    ground = source_phase(367)
    air = source_phase(368)


class Counter(EmblemDownSpecial):
    """Roy's Counter start and hit states."""

    ground = source_phase(369)
    ground_hit = source_phase(370)
    air = source_phase(371)
    air_hit = source_phase(372)


class Roy(Fighter):
    specials = Fighter.specials.replace(
        neutral=FlareBlade(),
        side=DoubleEdgeDance(),
        up=Blazer(),
        down=Counter(),
    )


__all__ = ["Roy", "FlareBlade", "DoubleEdgeDance", "Blazer", "Counter"]
