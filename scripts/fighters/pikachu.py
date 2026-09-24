"""Pikachu's source-defined special motion declarations.

The native ``ftPikachu`` table reserves states 341 through 366 for the four
special families.  The executable policies live in ``electric_family`` so
Pichu can use the same source behavior without duplicating transition code;
this module keeps Pikachu's source index explicit for tooling and review.
Article creation and collision driven velocity updates still require host
surfaces that are not part of the portable fighter API.
"""

from typing import ClassVar, Sequence

from skirmish import ArticleId, Fighter, Parameters
from fighter.electric_family import (
    ELECTRIC_SPECIALS,
    ElectricAgility as Agility,
    ElectricQuickAttack as QuickAttack,
    ElectricThunder as Thunder,
    ElectricThunderJolt as ThunderJolt,
)


class PikachuParameters(Parameters):
    """Stable identity metadata from ``ftPk_Init``.

    The source initializer names the fighter and animation archives and
    exposes four costume data files.  Keeping those values on the fighter
    definition lets resource loaders select the native assets without making
    the special authoring code depend on archive discovery.
    """

    data_file: str = "PlPk.dat"
    data_name: str = "ftDataPikachu"
    animation_data_file: str = "PlPkAJ.dat"
    # ``ftPk_Init_OnLoad`` registers xDC, specialn_itkind, and
    # specialairn_itkind as the Thunder, ground Jolt, and aerial Jolt
    # article archives.  Keep these identities available at the same host
    # boundary as Pichu's corresponding initializer metadata.
    thunder_article_id: ArticleId = ArticleId.PIKACHU_THUNDER
    thunder_jolt_ground_article_id: ArticleId = ArticleId.PIKACHU_TJOLT_GROUND
    thunder_jolt_air_article_id: ArticleId = ArticleId.PIKACHU_TJOLT_AIR
    # The shared Pikachu callbacks emit the up-special effect for Pikachu;
    # only the Pichu kind is explicitly suppressed in ``ftPk_SpecialHi``.
    up_special_effects: bool = True
    thunder_jolt_sound: int = 240076
    # ``ftPk_SpecialN_Anim`` consumes command variable 0 as its spawn edge.
    neutral_spawn_command: int = 0


class Pikachu(Fighter):
    parameters = PikachuParameters
    costume_files: ClassVar[tuple[str, ...]] = (
        "PlPkNr.dat",
        "PlPkRe.dat",
        "PlPkBu.dat",
        "PlPkGr.dat",
    )
    costume_joint_files: ClassVar[tuple[str, ...]] = (
        "PlyPikachu5K_Share_joint",
        "PlyPikachu5KRe_Share_joint",
        "PlyPikachu5KBu_Share_joint",
        "PlyPikachu5KGr_Share_joint",
    )
    costume_material_animation_files: ClassVar[tuple[str, ...]] = (
        "PlyPikachu5K_Share_matanim_joint",
        "PlyPikachu5KRe_Share_matanim_joint",
        "PlyPikachu5KBu_Share_matanim_joint",
        "PlyPikachu5KGr_Share_matanim_joint",
    )
    demo_motion_files: ClassVar[tuple[str, ...]] = (
        "ftDemoResultMotionFilePikachu",
        "ftDemoIntroMotionFilePikachu",
        "ftDemoEndingMotionFilePikachu",
        "ftDemoViWaitMotionFilePikachu",
    )
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


def quick_attack_effect_offset(
    random_x: float = 0.0,
    random_y: float = 0.0,
    *,
    aerial: bool = False,
    terminal: bool = False,
    pichu: bool = False,
) -> tuple[float, float] | None:
    """Return the source ``efSync_Spawn(1012)`` offset for one callback.

    ``ftPk_SpecialHiStart1_Anim`` decrements its segment counter first.  A
    terminal callback emits at XRotN without jitter; intermediate ground and
    aerial callbacks use independent ``HSD_Randf`` samples with widths 6 and
    10.  Both callbacks suppress the effect for Pichu.  The caller supplies
    the already sampled values so this helper stays deterministic and does
    not replace the host RNG.
    """
    if pichu:
        return None
    if terminal:
        return (0.0, 0.0)
    width = 10.0 if aerial else 6.0
    return (width * float(random_x) - width / 2.0,
            width * float(random_y) - width / 2.0)


def thunder_jolt_spawn_position(
    position: Sequence[float], offset: Sequence[float], facing: float, scale: float
) -> tuple[float, float, float]:
    """Mirror the position arithmetic in ``ftPk_SpecialN_Anim``.

    Ground and aerial callbacks use their respective offset attributes but
    both pass the fighter's configured ``specialn_itkind`` to the article
    spawn routine.  Article ownership and allocation remain host-owned.
    """
    if len(position) != 3 or len(offset) != 2:
        raise ValueError("position must have 3 values and offset must have 2")
    x, y, _ = (float(value) for value in position)
    dx, dy = (float(value) for value in offset)
    return (x + float(scale) * dx * float(facing), y + float(scale) * dy, 0.0)


SKULL_BASH_COMMAND_ENDS = {
    Thunder.ground_loop: Thunder.ground_end,
    Thunder.ground_hit: Thunder.ground_end,
    Thunder.air_loop: Thunder.air_end,
    Thunder.air_hit: Thunder.air_end,
}


def _source_state(action) -> int | None:
    """Read a source state from an authored or roster-bound action.

    Authoring callbacks receive ``SourceAction``/``ActionDescriptor`` values
    before roster binding, while native hosts may call the same callback with
    a qualified wire value such as ``Source.13:360``.  The source callback is
    keyed by the numeric motion state, so accepting both forms keeps the
    transition stable at that ABI boundary.
    """
    state = getattr(action, "slippi_state", None)
    if isinstance(state, int) and not isinstance(state, bool):
        return state
    descriptor = getattr(action, "action", action)
    state = getattr(descriptor, "slippi_state", None)
    if isinstance(state, int) and not isinstance(state, bool):
        return state
    if isinstance(descriptor, int) and not isinstance(descriptor, bool):
        return descriptor
    if isinstance(descriptor, str):
        value = descriptor.rsplit(":", 1)[-1]
        if value.isdecimal():
            return int(value)
    return None


def skull_bash_command_transition(action, command_value):
    """Return Skull Bash's terminal phase when native command 0 is nonzero.

    ``ftPk_SpecialLwLoop{0,1}_Anim`` checks command variable 0 before the
    Thunder contact path and enters the matching ground or aerial end phase.
    """
    if not command_value:
        return None
    destination = SKULL_BASH_COMMAND_ENDS.get(action)
    if destination is not None:
        return destination
    state = _source_state(action)
    destination_state = {
        360: 362,
        361: 362,
        364: 366,
        365: 366,
    }.get(state)
    if destination_state is None:
        return None
    if isinstance(action, str):
        prefix = action.rsplit(":", 1)[0] if ":" in action else "Source"
        return f"{prefix}:{destination_state}"
    return destination_state


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
    "PikachuParameters",
    "ThunderJolt",
    "QuickAttack",
    "QUICK_ATTACK_ANIMATION_ENDS",
    "advance_quick_attack_animation",
    "advance_quick_attack_hold",
    "quick_attack_effect_offset",
    "thunder_jolt_spawn_position",
    "SKULL_BASH_COMMAND_ENDS",
    "skull_bash_command_transition",
    "Agility",
    "Thunder",
    "SOURCE_MOTION_STATES",
]
