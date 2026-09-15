"""Fighter class validation and registration."""

from typing import TypeVar

from .api import (AerialMoves, DefenseMoves, Fighter, GetupMoves, GrabMoves,
                  GroundedMoves, LedgeMoves, MoveError, SmashMoves, SpecialMoves, TauntMoves, TiltMoves,
                  ThrowMoves, _validate_group)

T = TypeVar("T", bound=type[Fighter])


def validate_fighter(fighter: type[Fighter]) -> None:
    if not isinstance(fighter, type) or not issubclass(fighter, Fighter):
        raise MoveError("registered value must be a Fighter subclass")
    name = getattr(fighter, "name", None)
    if not isinstance(name, str) or not name:
        raise MoveError("fighter name must be a non-empty string")
    if not hasattr(fighter, "attributes"):
        raise MoveError("fighter must define attributes")
    groups = (
        ("specials", SpecialMoves, ("neutral", "side", "up", "down")),
        ("aerials", AerialMoves, ("neutral", "forward", "back", "up", "down")),
        ("grounded", GroundedMoves, ("jab", "rapid_jab", "dash")),
        ("tilts", TiltMoves, ("forward", "up", "down")),
        ("smashes", SmashMoves, ("forward", "up", "down")),
        ("grabs", GrabMoves, ("standing", "dash", "pummel")),
        ("throws", ThrowMoves, ("forward", "back", "up", "down")),
        ("defense", DefenseMoves, ("shield", "spot_dodge", "roll_forward", "roll_back", "air_dodge")),
        ("ledge", LedgeMoves, ("wait", "getup", "roll", "attack", "jump")),
        ("getup", GetupMoves, ("neutral", "roll_forward", "roll_back", "attack")),
        ("taunt", TauntMoves, ("taunt",)),
    )
    for label, expected, names in groups:
        if not hasattr(fighter, label):
            raise MoveError(f"fighter must define {label}")
        _validate_group(getattr(fighter, label), names, label, expected)


def register(fighter: T) -> T:
    validate_fighter(fighter)
    # Registration is a declaration marker only. Module/source loading owns
    # discovery and the native registry owns cross-definition name conflicts.
    setattr(fighter, "__fighter_registered__", True)
    return fighter


def registered_fighters() -> tuple[type[Fighter], ...]:
    """Return no process-global state; loaders discover classes per module."""
    return ()
