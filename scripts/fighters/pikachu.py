"""Pikachu's source-defined special motion declarations.

The native ``ftPikachu`` table reserves states 341 through 366 for the four
special families.  The executable policies live in ``electric_family`` so
Pichu can use the same source behavior without duplicating transition code;
this module keeps Pikachu's source index explicit for tooling and review.
Article creation and collision driven velocity updates still require host
surfaces that are not part of the portable fighter API.
"""

from skirmish import Fighter
from fighter.electric_family import (
    ELECTRIC_SPECIALS,
    ElectricAgility as Agility,
    ElectricQuickAttack as QuickAttack,
    ElectricThunder as Thunder,
    ElectricThunderJolt as ThunderJolt,
)


class Pikachu(Fighter):
    specials = ELECTRIC_SPECIALS


# Keep the names aligned with the symbols in ``ftpikachu.c``.  A tuple rather
# than a derived range makes omissions visible during code review and gives
# native exporters a stable, allocation-free source index.
SOURCE_MOTION_STATES = (
    ("neutral.ground", 341),
    ("neutral.air", 342),
    ("side.ground_start", 343),
    ("side.ground_hold", 344),
    ("side.ground_travel", 345),
    ("side.ground_end", 346),
    ("side.ground_dash", 347),
    ("side.air_start", 348),
    ("side.air_hold", 349),
    ("side.air_travel", 350),
    ("side.air_end", 351),
    ("side.air_dash", 352),
    ("up.ground_start", 353),
    ("up.ground_move", 354),
    ("up.ground_end", 355),
    ("up.air_start", 356),
    ("up.air_move", 357),
    ("up.air_end", 358),
    ("down.ground_start", 359),
    ("down.ground_loop", 360),
    ("down.ground_hit", 361),
    ("down.ground_end", 362),
    ("down.air_start", 363),
    ("down.air_loop", 364),
    ("down.air_hit", 365),
    ("down.air_end", 366),
)


__all__ = [
    "Pikachu",
    "ThunderJolt",
    "QuickAttack",
    "Agility",
    "Thunder",
    "SOURCE_MOTION_STATES",
]
