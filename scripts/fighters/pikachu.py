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


# Native ``ftPk_SpecialSStart_Anim`` enters the hold pose when the start
# animation ends.  The hold callback advances to dash after its attribute
# controlled counter expires, while the airborne travel pose falls through
# to its end pose in ``ftPk_SpecialAirS1_Anim``.  Keep these source branches
# local to Pikachu's authoring module until the shared family can expose the
# corresponding animation/countdown callbacks without changing its ABI.
QUICK_ATTACK_ANIMATION_ENDS = {
    QuickAttack.ground_start: QuickAttack.ground_hold,
    QuickAttack.air_start: QuickAttack.air_hold,
    QuickAttack.air_travel: QuickAttack.air_end,
}


def advance_quick_attack_animation(fighter) -> bool:
    """Apply one source animation-end transition, returning whether it fired."""
    destination = QUICK_ATTACK_ANIMATION_ENDS.get(fighter.action)
    if destination is None:
        return False
    fighter.change_action(destination)
    return True


def advance_quick_attack_hold(fighter, frames_held: int, hold_limit: int) -> bool:
    """Apply ``ftPk_SpecialSHold_Anim`` once its native counter expires."""
    if frames_held <= hold_limit:
        return False
    destination = {
        QuickAttack.ground_hold: QuickAttack.ground_dash,
        QuickAttack.air_hold: QuickAttack.air_dash,
    }.get(fighter.action)
    if destination is None:
        return False
    fighter.change_action(destination)
    return True


SKULL_BASH_COMMAND_ENDS = {
    Thunder.ground_loop: Thunder.ground_end,
    Thunder.ground_hit: Thunder.ground_end,
    Thunder.air_loop: Thunder.air_end,
    Thunder.air_hit: Thunder.air_end,
}


def skull_bash_command_transition(action, command_value):
    """Return Skull Bash's terminal phase when native command 0 is nonzero."""
    if not command_value:
        return None
    return SKULL_BASH_COMMAND_ENDS.get(action)


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
    "QUICK_ATTACK_ANIMATION_ENDS",
    "advance_quick_attack_animation",
    "advance_quick_attack_hold",
    "SKULL_BASH_COMMAND_ENDS",
    "skull_bash_command_transition",
    "Agility",
    "Thunder",
    "SOURCE_MOTION_STATES",
]
