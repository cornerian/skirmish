"""Marth's source-state special declarations."""

from skirmish import Fighter, source_phase
from fighter.emblem_family import (
    EmblemDownSpecial,
    EmblemNeutralSpecial,
    EmblemSideSpecial,
    EmblemUpSpecial,
)


class ShieldBreaker(EmblemNeutralSpecial):
    ground_start = source_phase(341)
    ground_loop = source_phase(342, animation_loop=True)
    ground_end0 = source_phase(343)
    ground_end1 = source_phase(344)
    air_start = source_phase(345)
    air_loop = source_phase(346, animation_loop=True)
    air_end0 = source_phase(347)
    air_end1 = source_phase(348)


class DancingBlade(EmblemSideSpecial):
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


class DolphinSlash(EmblemUpSpecial):
    ground = source_phase(367)
    air = source_phase(368)


class Counter(EmblemDownSpecial):
    ground = source_phase(369)
    ground_hit = source_phase(370)
    air = source_phase(371)
    air_hit = source_phase(372)



class Marth(Fighter):
    specials = Fighter.specials.replace(
        neutral=ShieldBreaker(),
        side=DancingBlade(),
        up=DolphinSlash(),
        down=Counter(),
    )


__all__ = ["Marth", "ShieldBreaker", "DancingBlade", "DolphinSlash", "Counter"]
