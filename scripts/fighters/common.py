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
    Button,
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


def stick_axis_reaches_threshold(value, threshold):
    """Return whether an axis reaches an inclusive directional threshold."""
    return abs(value) >= threshold


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


def fresh_special_input(ctx, resource):
    """Return whether a resource-backed B special has a fresh input."""
    return (ctx.resource(resource) is not None
            and ctx.input.just_pressed(Button.B))


def directional_b_input(ctx, resource, axis, threshold_attr, *, direction=None):
    """Return a directional B threshold result, or ``None`` if unavailable.

    ``direction`` may be supplied for a one-sided input (for example, ``1``
    for up); when omitted, the axis magnitude is tested.  Returning
    ``None`` keeps missing optional dispatch data distinct from an available
    input that is inside its threshold.
    """
    if not fresh_special_input(ctx, resource):
        return None
    rules = special_rules(ctx)
    threshold = getattr(rules, threshold_attr, None) if rules is not None else None
    if threshold is None:
        return None
    value = ctx.input.stick[axis]
    if direction is not None:
        value *= direction
    return stick_axis_reaches_threshold(value, threshold)


def start_open_special(fighter, ctx, ground_action, air_action):
    """Start a special on the currently open ground or aerial surface."""
    if not (ctx.ground_open or ctx.air_open):
        return False
    start_action(fighter, ground_action if ctx.ground_open else air_action)
    return True


__all__ = [
    "FighterBase",
    "directional_b_input",
    "fresh_special_input",
    "resource_attributes",
    "special_rules",
    "stick_axis_reaches_threshold",
    "start_action",
    "start_open_special",
]
