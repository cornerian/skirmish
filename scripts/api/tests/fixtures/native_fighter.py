from dataclasses import dataclass

from skirmish import (
    AerialMoves,
    Attributes,
    DefenseMoves,
    Fighter,
    GetupMoves,
    GrabMoves,
    GroundedMoves,
    LedgeMoves,
    Move,
    MoveContext,
    MoveError,
    SmashMoves,
    SpecialMoves,
    TauntMoves,
    ThrowMoves,
    TiltMoves,
    export_definition,
    hook,
    register,
    validate_fighter,
)


@dataclass(frozen=True, slots=True)
class NativeAttributes(Attributes):
    projectile: str = "native_laser"
    weight: int = 100


class Special(Move):
    @hook.input_pressed("B")
    def run(self, value: int) -> int:
        return value + 1


class ExtraSpecial(Move):
    def run(self, value: int) -> int:
        return value + 2


class Ordinary(Move):
    def run(self, value: int) -> int:
        return value


@dataclass(frozen=True, slots=True)
class ExtendedSpecials(SpecialMoves):
    extra: Move


special = Special()
extra = ExtraSpecial()
other_extra = ExtraSpecial()
ordinary = Ordinary()


@register
class NativeFighter(Fighter):
    name = "native"
    attributes = NativeAttributes
    specials = ExtendedSpecials(special, ordinary, ordinary, other_extra, extra)
    aerials = AerialMoves(special, extra, ordinary, ordinary, ordinary)
    grounded = GroundedMoves(ordinary, ordinary, ordinary)
    tilts = TiltMoves(ordinary, ordinary, ordinary)
    smashes = SmashMoves(ordinary, ordinary, ordinary)
    grabs = GrabMoves(ordinary, ordinary, ordinary)
    throws = ThrowMoves(ordinary, ordinary, ordinary, ordinary)
    defense = DefenseMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    ledge = LedgeMoves(ordinary, ordinary, ordinary, ordinary, ordinary)
    getup = GetupMoves(ordinary, ordinary, ordinary, ordinary)
    taunt = TauntMoves(ordinary)


def exported_probe():
    exported = export_definition(NativeFighter).as_dict()
    extra_id = exported["movesets"]["specials"]["extra"]
    other_extra_id = exported["movesets"]["specials"]["down"]
    behavior_ids = [behavior["id"] for behavior in exported["behaviors"]]
    authored_slots = (
        len(exported["movesets"]["specials"])
        + len(exported["movesets"]["aerials"])
        + len(exported["movesets"]["grounded"])
        + len(exported["movesets"]["tilts"])
        + len(exported["movesets"]["smashes"])
        + len(exported["movesets"]["grabs"])
        + len(exported["movesets"]["throws"])
        + len(exported["movesets"]["defense"])
        + len(exported["movesets"]["ledge"])
        + len(exported["movesets"]["getup"])
        + len(exported["movesets"]["taunt"])
    )
    return [
        exported["name"],
        exported["parameters"]["projectile"],
        extra_id == exported["movesets"]["aerials"]["forward"],
        extra_id != other_extra_id,
        extra_id in behavior_ids,
        other_extra_id in behavior_ids,
        NativeFighter.__fighter_registered__,
        len(exported["behaviors"]),
        authored_slots,
    ]


bound_callback = NativeFighter.specials.neutral.run


def decorated_bound_callback(value: int) -> int:
    return bound_callback(value)


def incomplete_is_rejected() -> bool:
    try:
        class Incomplete(Fighter):
            name = "incomplete"
            attributes = NativeAttributes

        validate_fighter(Incomplete)
    except MoveError:
        return True
    return False
