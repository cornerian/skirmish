"""Deterministic scalar and vector math exposed by the native fighter host.

The functions in this module are deliberately thin ABI calls.  Their
implementations live in Rust and use the game's compatibility math, including
the binary32 rounding and source-specific trigonometry.  Keeping the calls
stateless means a callback does not need to borrow or publish a host token.
"""

from __future__ import annotations

from typing import Any

from .compat import f32

try:
    from _skirmish_math import (
        angle_xy as _angle_xy,
        atan2 as _atan2,
        cos as _cos,
        facing as _facing,
        sin as _sin,
    )
except ImportError:  # Authoring tools can import fighter without Pon.
    _angle_xy = _atan2 = _cos = _facing = _sin = None


# These are the exact binary32 constants used by the Starlark source and the
# native math implementation.  Constructing them through f32 also keeps the
# authoring representation consistent with every other numeric binding.
PI = f32(3.1415927410125732)
HALF_PI = f32(1.5707963705062866)
DEG_TO_RAD = f32(0.01745329238474369)

# Lower-case aliases match Python's conventional spelling and the existing
# Fox source. Upper-case aliases are retained for compatibility with older
# resource naming.
pi = PI
half_pi = HALF_PI
deg_to_rad = DEG_TO_RAD


def _require_native(name: str, function: Any) -> Any:
    if function is None:
        raise RuntimeError(f"fighter.math.{name} is only available inside the Pon host")
    return function


def sin(value: Any) -> Any:
    """Return the game's binary32 sine for ``value`` (radians)."""
    return f32(_require_native("sin", _sin)(float(value)))


def cos(value: Any) -> Any:
    """Return the game's binary32 cosine for ``value`` (radians)."""
    return f32(_require_native("cos", _cos)(float(value)))


def atan2(y: Any, x: Any) -> Any:
    """Return the game's binary32 ``atan2f(y, x)`` (radians)."""
    return f32(_require_native("atan2", _atan2)(float(y), float(x)))


def angle_xy(first: Any, second: Any) -> Any:
    """Return the source-compatible angle between two XY vectors."""
    return f32(_require_native("angle_xy", _angle_xy)(
        [float(value) for value in first], [float(value) for value in second]
    ))


def facing(value: Any) -> Any:
    """Return ``1`` for nonnegative values and ``-1`` otherwise."""
    return f32(_require_native("facing", _facing)(float(value)))


__all__ = [
    "PI", "HALF_PI", "DEG_TO_RAD", "pi", "half_pi", "deg_to_rad",
    "sin", "cos", "atan2", "angle_xy", "facing",
]
