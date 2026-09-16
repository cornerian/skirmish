"""Shared fighter authoring defaults.

Character scripts inherit :class:`FighterBase` and only declare the resources
that differ from the engine's standard move groups.
"""

from skirmish import (
    AerialMoves,
    DefenseMoves,
    Fighter,
    GetupMoves,
    GrabMoves,
    GroundedMoves,
    LedgeMoves,
    SmashMoves,
    TauntMoves,
    ThrowMoves,
    TiltMoves,
)


class FighterBase(Fighter):
    """Common default move groups shared by native fighter definitions."""

    __abstract__ = True

    aerials = AerialMoves()
    grounded = GroundedMoves()
    tilts = TiltMoves()
    smashes = SmashMoves()
    grabs = GrabMoves()
    throws = ThrowMoves()
    defense = DefenseMoves()
    ledge = LedgeMoves()
    getup = GetupMoves()
    taunt = TauntMoves()


__all__ = ["FighterBase"]
