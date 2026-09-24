"""Typed host boundary for Pikachu's source Thunder chain.

``ftPk_SpecialLw_SpawnEffect`` delegates the actual article allocation to
``it_802B1DF8``.  The portable fighter API does not own that item graph yet,
so this module keeps the source inputs explicit and gives native hosts one
allocation-free plan to consume.  It deliberately does not approximate
article motion or collision in Python.
"""

from __future__ import annotations

from dataclasses import dataclass
from math import pi
from typing import Any, Callable, Sequence

from .articles import ArticleId


@dataclass(frozen=True, slots=True)
class ThunderAttributes:
    """The six ``ftPikachuAttributes`` fields consumed by the spawn callback."""

    velocity_y: float  # xC0
    effect_offset_y: float  # xCC
    spawn_offset_y: float  # xD0
    segment_count: int  # xD4
    segment_delay: int  # xD8
    article_kind: ArticleId = ArticleId.PIKACHU_THUNDER  # xDC

    def __post_init__(self) -> None:
        if self.segment_count < 0:
            raise ValueError("Thunder segment_count must be non-negative")
        if self.segment_delay < 0:
            raise ValueError("Thunder segment_delay must be non-negative")
        if self.article_kind not in (
            ArticleId.PIKACHU_THUNDER,
            ArticleId.PICHU_THUNDER,
        ):
            raise ValueError("Thunder must use a Pikachu-family article kind")


@dataclass(frozen=True, slots=True)
class ThunderSegment:
    """One source Thunder segment before the native item host allocates it."""

    index: int
    delay: int
    velocity: tuple[float, float, float]
    article_id: ArticleId = ArticleId.PIKACHU_THUNDER


@dataclass(frozen=True, slots=True)
class ThunderChainPlan:
    """Immutable source spawn request passed across the article boundary."""

    position: tuple[float, float, float]
    effect_position: tuple[float, float, float]
    segments: tuple[ThunderSegment, ...]
    article_id: ArticleId


def build_thunder_chain(
    fighter_position: Sequence[float], attrs: ThunderAttributes
) -> ThunderChainPlan:
    """Mirror ``it_802B1DF8``'s deterministic chain initialization.

    The decomp sets every segment's initial position to the fighter callback's
    position, stores one shared upward velocity, and increments the delay only
    after each allocation.  The item host remains responsible for ownership,
    state tables, collision, and link propagation.
    """
    if len(fighter_position) != 3:
        raise ValueError("fighter_position must contain x, y, and z")
    x, y, z = (float(value) for value in fighter_position)
    position = (x, y + attrs.spawn_offset_y, z)
    effect_position = (x, y + attrs.spawn_offset_y + attrs.effect_offset_y, z)
    velocity = (0.0, attrs.velocity_y, 0.0)
    segments = tuple(
        ThunderSegment(
            index=i,
            delay=i * attrs.segment_delay,
            velocity=velocity,
            article_id=attrs.article_kind,
        )
        for i in range(attrs.segment_count)
    )
    return ThunderChainPlan(position, effect_position, segments, attrs.article_kind)


def thunder_jolt_ground_angle(facing: float, attribute_angle: float) -> float:
    """Mirror ``it_802B3554``'s initial ground travel angle.

    The native callback uses the attribute angle for right-facing Jolts and
    ``pi + abs(angle)`` for every other facing value.  Keeping this helper
    separate from article allocation lets the native item host apply the
    source transform once its production resource is available.
    """
    angle = float(attribute_angle)
    return angle if float(facing) == 1.0 else pi + abs(angle)


def spawn_thunder_chain(
    fighter: Any,
    plan: ThunderChainPlan,
    *,
    resource_available: bool = False,
    sink_name: str = "spawn_pikachu_thunder_chain",
) -> bool:
    """Submit one plan to the native article host, returning whether it ran.

    Requiring a named chain sink prevents silently degrading a multi-segment
    article into repeated generic projectile spawns.  The callback accepts the
    owner and immutable plan so a native host can preserve all links and
    per-segment delays without per-frame Python work.
    """
    if not resource_available or not plan.segments:
        return False
    sink = getattr(fighter, sink_name, None)
    if not callable(sink):
        return False
    sink(plan)
    return True


__all__ = [
    "ThunderAttributes",
    "ThunderSegment",
    "ThunderChainPlan",
    "build_thunder_chain",
    "thunder_jolt_ground_angle",
    "spawn_thunder_chain",
]
