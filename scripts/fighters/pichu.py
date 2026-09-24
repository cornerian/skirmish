"""Pichu's source-defined motion declarations.

``ftPichu`` reuses Pikachu's special callbacks and motion table, but its
fighter initializer still has observable Pichu-specific data: it enables wall
jumps and loads the Pichu fighter/animation archives.  Keep that identity in
the script metadata so native hosts can consume it without duplicating the
source constants in their roster loader.
"""

from typing import ClassVar

from skirmish import Fighter, Parameters
from fighter.electric_family import (
    ELECTRIC_SPECIALS,
    ElectricAgility as Agility,
    ElectricQuickAttack as QuickAttack,
    ElectricThunder as Thunder,
    ElectricThunderJolt as ThunderJolt,
)


class PichuParameters(Parameters):
    """Initialization metadata from ``ftPc_Init``.

    The item kinds at ``ftPichuAttributes.x14/x18/xDC`` are archive-owned
    values and intentionally remain in the native resource package.  The
    wall-jump flag and archive names are stable fighter metadata and are safe
    to expose here.
    """

    can_walljump: bool = True
    data_file: str = "PlPc.dat"
    animation_data_file: str = "PlPcAJ.dat"
    # ftPk_SpecialHiStart1_Anim and its aerial twin suppress Pikachu's
    # electric burst for FTKIND_PICHU.  Keep this as typed host metadata until
    # the effect callback surface is available to fighter scripts.
    up_special_effects: bool = False
    # ftPk_SpecialN_Anim selects Pichu's distinct Thunder Jolt sound.
    thunder_jolt_sound: int = 230067


class Pichu(Fighter):
    # Keep the source initializer profile available to hosts that materialize
    # fighter parameters separately from the generic exported attributes.
    parameters = PichuParameters

    # These are source resource names, rather than synthesized gameplay
    # values.  They let tooling identify the same costume archives loaded by
    # ftPc_Init without making normal script export depend on a disc.
    costume_files: ClassVar[tuple[str, ...]] = (
        "PlPcNr.dat",
        "PlPcRe.dat",
        "PlPcBu.dat",
        "PlPcGr.dat",
    )
    specials = ELECTRIC_SPECIALS


__all__ = [
    "Pichu",
    "PichuParameters",
    "ThunderJolt",
    "QuickAttack",
    "Agility",
    "Thunder",
]
