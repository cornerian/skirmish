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


def special_rules(ctx):
    """Return the host's shared special-dispatch rules, when available."""
    rules = getattr(ctx, "rules", None)
    return getattr(rules, "specials", None)


def resource_attributes(ctx, resource=None):
    """Resolve a resource's attributes from a callback context.

    ``resource`` may be a resource object, a resource path, or omitted to use
    the callback's owned resource.  Accepting paths keeps resource lookup and
    attribute access in one small, reusable authoring primitive while
    preserving the host's optional-resource behavior.
    """
    if isinstance(resource, str):
        lookup = getattr(ctx, "resource", None)
        if lookup is None:
            return None
        path = resource
        resource = lookup(path)
        if path.endswith(".attributes"):
            return resource
    elif resource is None:
        lookup = getattr(ctx, "resource", None)
        if lookup is None:
            return None
        resource = lookup()
    return getattr(resource, "attributes", None)


def start_action(fighter, action):
    """Enter a fresh action at its first animation frame."""
    fighter.change_action(action)
    fighter.action_frame = 1


__all__ = ["FighterBase", "resource_attributes", "special_rules", "start_action"]
