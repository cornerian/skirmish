"""Canonical public CSS roster identities.

The values here are the Slippi/CSS external character identities.  Keeping
the slug and numeric identity together prevents authoring declarations from
drifting apart when a fighter is renamed or registered.
"""

from enum import Enum


class Roster(Enum):
    """The 26 playable Melee CSS identities exposed by the authoring API."""

    CAPTAIN_FALCON = ("captain-falcon", 0)
    DONKEY_KONG = ("donkey-kong", 1)
    FOX = ("fox", 2)
    GAME_AND_WATCH = ("game-and-watch", 3)
    KIRBY = ("kirby", 4)
    BOWSER = ("bowser", 5)
    LINK = ("link", 6)
    LUIGI = ("luigi", 7)
    MARIO = ("mario", 8)
    MARTH = ("marth", 9)
    MEWTWO = ("mewtwo", 10)
    NESS = ("ness", 11)
    PEACH = ("peach", 12)
    PIKACHU = ("pikachu", 13)
    ICE_CLIMBERS = ("ice-climbers", 14)
    JIGGLYPUFF = ("jigglypuff", 15)
    SAMUS = ("samus", 16)
    YOSHI = ("yoshi", 17)
    ZELDA = ("zelda", 18)
    SHEIK = ("sheik", 19)
    FALCO = ("falco", 20)
    YOUNG_LINK = ("young-link", 21)
    DR_MARIO = ("dr-mario", 22)
    ROY = ("roy", 23)
    PICHU = ("pichu", 24)
    GANONDORF = ("ganondorf", 25)

    def __init__(self, slug: str, external_id: int) -> None:
        self.slug = slug
        self.external_id = external_id


def roster_for_module(module: str) -> Roster | None:
    """Resolve a source module's stem to its canonical roster entry."""
    stem = module.rsplit(".", 1)[-1].replace("-", "_")
    if stem == "captain":
        return Roster.CAPTAIN_FALCON
    return next((entry for entry in Roster if entry.slug.replace("-", "_") == stem), None)


__all__ = ["Roster", "roster_for_module"]
