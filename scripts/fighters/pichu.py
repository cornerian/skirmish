"""Pichu's source-defined motion declarations.

``ftPichu`` reuses Pikachu's special callbacks and motion table, but its
fighter initializer still has observable Pichu-specific data: it enables wall
jumps and loads the Pichu fighter/animation archives.  Keep that identity in
the script metadata so native hosts can consume it without duplicating the
source constants in their roster loader.
"""

from typing import ClassVar

from skirmish import ArticleId, Fighter, Parameters
from fighter.electric_family import (
    ELECTRIC_SPECIALS,
    ElectricAgility as Agility,
    ElectricQuickAttack as QuickAttack,
    ElectricThunder as Thunder,
    ElectricThunderJolt as ThunderJolt,
)


PICHU_THUNDER_COMMAND_ENDS = {
    360: 362,
    361: 362,
    364: 366,
    365: 366,
}


def _source_state(action):
    descriptor = getattr(action, "action", action)
    state = getattr(descriptor, "slippi_state", None)
    if state is not None:
        return state
    if isinstance(descriptor, str) and ":" in descriptor:
        try:
            return int(descriptor.rsplit(":", 1)[1])
        except ValueError:
            return None
    return None


def pichu_thunder_command_transition(action, command_value):
    """Return the source Thunder end phase when command variable 0 is set.

    ``ftPk_SpecialLwLoop{0,1}_Anim`` checks command variable 0 before its
    contact path and enters the matching end phase when it is nonzero.  This
    is separate from projectile contact: the source uses the same branch for
    grounded and aerial loop and hit phases.
    """
    if not command_value:
        return None
    destination = PICHU_THUNDER_COMMAND_ENDS.get(_source_state(action))
    if destination is None:
        return None
    # Keep the returned state in the same source identity space as the
    # caller.  The native host binds this state to Pichu's action descriptor.
    return destination


class PichuParameters(Parameters):
    """Initialization metadata from ``ftPc_Init``.

    The item kinds at ``ftPichuAttributes.x14/x18/xDC`` are archive-owned
    values and intentionally remain in the native resource package.  The
    wall-jump flag and archive names are stable fighter metadata and are safe
    to expose here.
    """

    can_walljump: bool = True
    data_file: str = "PlPc.dat"
    data_name: str = "ftDataPichu"
    animation_data_file: str = "PlPcAJ.dat"
    # ftPk_SpecialHiStart1_Anim and its aerial twin suppress Pikachu's
    # electric burst for FTKIND_PICHU.  Keep this as typed host metadata until
    # the effect callback surface is available to fighter scripts.
    up_special_effects: bool = False
    # ftPk_SpecialN_Anim selects Pichu's distinct Thunder Jolt sound.
    thunder_jolt_sound: int = 230067
    # ``ftPc_Init_OnLoad`` registers xDC, x14, and x18 as the Pichu Thunder,
    # ground Jolt, and air Jolt archives.  These identities are stable source
    # data even though article spawning remains a native host boundary.
    thunder_article_id: ArticleId = ArticleId.PICHU_THUNDER
    thunder_jolt_ground_article_id: ArticleId = ArticleId.PICHU_TJOLT_GROUND
    thunder_jolt_air_article_id: ArticleId = ArticleId.PICHU_TJOLT_AIR

    # The shared neutral callback consumes command variable 0 at the command
    # edge and uses command variable 1 as its one spawn latch.
    neutral_spawn_command: int = 0


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
    # ftPc_Init_CostumeStrings stores one joint and one material-animation
    # archive beside each costume model.  Keep the complete source tuple so
    # native loaders do not have to reconstruct names from the model path.
    costume_joint_files: ClassVar[tuple[str, ...]] = (
        "PlyPichu5K_Share_joint",
        "PlyPichu5KRe_Share_joint",
        "PlyPichu5KBu_Share_joint",
        "PlyPichu5KGr_Share_joint",
    )
    costume_material_animation_files: ClassVar[tuple[str, ...]] = (
        "PlyPichu5K_Share_matanim_joint",
        "PlyPichu5KRe_Share_matanim_joint",
        "PlyPichu5KBu_Share_matanim_joint",
        "PlyPichu5KGr_Share_matanim_joint",
    )
    demo_motion_files: ClassVar[tuple[str, ...]] = (
        "ftDemoResultMotionFilePichu",
        "ftDemoIntroMotionFilePichu",
        "ftDemoEndingMotionFilePichu",
        "ftDemoViWaitMotionFilePichu",
    )
    specials = ELECTRIC_SPECIALS


__all__ = [
    "Pichu",
    "PichuParameters",
    "ThunderJolt",
    "QuickAttack",
    "Agility",
    "Thunder",
    "PICHU_THUNDER_COMMAND_ENDS",
    "pichu_thunder_command_transition",
]
