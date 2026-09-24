"""Small, reusable helpers for fighter callback authoring.

These functions deliberately operate on the host callback/context protocol
instead of importing a fighter implementation.  They keep optional native
resources and dispatch rules explicit while centralizing the repetitive parts
of special-input handling.
"""

from __future__ import annotations

from typing import Any, Callable, Iterable

from .compat import Button


def _stick_axis(stick: Any, axis: int) -> Any:
    """Read an input axis without making author callbacks shape fragile.

    Native contexts always expose a two element stick, but standalone
    authoring and older hosts can omit an axis while constructing a callback
    context.  The native neutral value is the least surprising fallback and
    keeps the hot path to one indexed lookup in the normal case.
    """
    try:
        return stick[axis]
    except (AttributeError, IndexError, KeyError, TypeError):
        return 0.0


def _input_stick(ctx: Any) -> Any:
    """Return the callback stick or an all neutral stand-in."""
    return getattr(getattr(ctx, "input", None), "stick", ())


def special_rules(ctx: Any) -> Any:
    """Return the host's shared special-dispatch rules, when available."""
    rules = getattr(ctx, "rules", None)
    return getattr(rules, "specials", None)


def directional_match(ctx: Any, root: Any) -> bool:
    """Match a B input to a standard neutral/side/up/down special root.

    Keeping this small predicate in the helper module lets family scripts use
    the same dispatch policy without importing the standard move classes.
    ``root`` is compared by its enum value so this remains usable by typed
    and lightweight standalone contexts alike.
    """
    rules = special_rules(ctx)
    if rules is None:
        return getattr(root, "value", root) == "neutral"

    stick = _input_stick(ctx)
    x = _stick_axis(stick, 0)
    y = _stick_axis(stick, 1)
    side_threshold = getattr(rules, "side_stick_threshold", None)
    vertical_threshold = getattr(rules, "vertical_threshold", None)
    root_value = getattr(root, "value", root)

    if root_value == "side":
        return side_threshold is not None and abs(x) >= side_threshold
    if root_value == "up":
        return vertical_threshold is not None and y >= vertical_threshold
    if root_value == "down":
        return vertical_threshold is not None and y <= -vertical_threshold

    side_reserved = side_threshold is not None and abs(x) >= side_threshold
    vertical_reserved = vertical_threshold is not None and (
        y >= vertical_threshold or y <= -vertical_threshold
    )
    return not (side_reserved or vertical_reserved)


def stick_axis_reaches_threshold(value: Any, threshold: Any) -> bool:
    """Return whether an axis reaches an inclusive directional threshold."""
    return abs(value) >= threshold


def any_stick_axis_reaches_thresholds(
    stick: Any, axis_thresholds: Iterable[tuple[int, Any]]
) -> bool:
    """Return whether any configured stick axis reaches its threshold."""
    return any(
        stick_axis_reaches_threshold(_stick_axis(stick, axis), threshold)
        for axis, threshold in axis_thresholds
        if threshold is not None
    )


def resource_attributes(ctx: Any, resource: Any = None) -> Any:
    """Resolve a resource's attributes from a callback context.

    ``resource`` may be a resource object, a resource path, or omitted to use
    the callback's owned resource.  Paths ending in ``.attributes`` are
    already attribute values in the native resource lookup protocol.
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


def start_action(fighter: Any, action: Any) -> None:
    """Enter a fresh action at its first animation frame."""
    fighter.change_action(action)
    fighter.action_frame = 1


def fresh_special_input(ctx: Any, resource: Any) -> bool:
    """Return whether a resource-backed B special has a fresh input."""
    lookup = getattr(ctx, "resource", None)
    input_state = getattr(ctx, "input", None)
    if lookup is None or input_state is None:
        return False
    # The common native predicate tests the edge first.  Besides matching the
    # source's short circuit, this avoids a resource proxy lookup on nearly
    # every frame where B was not pressed.
    return fresh_b_input(ctx) and lookup(resource) is not None


def fresh_b_input(ctx: Any) -> bool:
    """Return whether B is freshly pressed, without consulting resources."""
    input_state = getattr(ctx, "input", None)
    if input_state is None:
        return False
    return bool(input_state.just_pressed(Button.B))


def directional_fresh_b(
    ctx: Any,
    direction: int,
) -> bool:
    """Return whether a fresh B points through a configured threshold.

    This is the resource-independent counterpart to :func:`directional_b_input`.
    Missing dispatch rules remain permissive for standalone authoring contexts;
    configured rules apply the requested one-sided threshold.
    """
    if not fresh_b_input(ctx):
        return False
    rules = special_rules(ctx)
    threshold = getattr(rules, "vertical_threshold", None) if rules is not None else None
    if threshold is None:
        return True
    value = _stick_axis(_input_stick(ctx), 1)
    if direction > 0:
        return value >= threshold
    if direction < 0:
        return value <= -threshold
    return stick_axis_reaches_threshold(value, threshold)


def directional_b_input(
    ctx: Any,
    resource: Any,
    axis: int,
    threshold_attr: str,
    *,
    direction: int | None = None,
) -> bool | None:
    """Return a directional B threshold result, or ``None`` if unavailable.

    ``direction`` may be supplied for a one-sided input (for example, ``1``
    for up); when omitted, the axis magnitude is tested.  ``None`` keeps
    missing optional dispatch data distinct from an available input inside its
    threshold.
    """
    if not fresh_special_input(ctx, resource):
        return None
    rules = special_rules(ctx)
    threshold = getattr(rules, threshold_attr, None) if rules is not None else None
    if threshold is None:
        return None
    value = _stick_axis(_input_stick(ctx), axis)
    if direction is None:
        return stick_axis_reaches_threshold(value, threshold)
    if direction > 0:
        return value >= threshold
    if direction < 0:
        return value <= -threshold
    return False


def directional_b_reserved(
    ctx: Any,
    *,
    vertical: str = "vertical_threshold",
    horizontal: str = "horizontal_threshold",
) -> bool:
    """Return whether a fresh B input belongs to a directional special.

    Missing rules remain permissive for standalone authoring contexts, while
    configured rules use the same inclusive threshold as
    :func:`directional_b_input`.
    """
    rules = special_rules(ctx)
    if rules is None:
        return False
    stick = _input_stick(ctx)
    threshold = getattr(rules, vertical, None)
    if threshold is not None and stick_axis_reaches_threshold(_stick_axis(stick, 1), threshold):
        return True
    # New dispatch rules name the horizontal threshold semantically.  Keep
    # the historical ``horizontal_threshold`` lookup as a compatibility
    # fallback for old Captain/Fox contexts and fixtures.
    threshold = getattr(rules, "side_stick_threshold", None)
    if threshold is None:
        threshold = getattr(rules, horizontal, None)
    return threshold is not None and stick_axis_reaches_threshold(_stick_axis(stick, 0), threshold)


def start_fresh_open_special(
    fighter: Any,
    ctx: Any,
    resource: Any,
    ground_action: Any,
    air_action: Any,
    *,
    active_actions: Iterable[Any] = (),
) -> bool:
    """Accept fresh B and enter or continue a ground/aerial special."""
    if not fresh_special_input(ctx, resource):
        return False
    if fighter.action in active_actions:
        return True
    return start_open_special(fighter, ctx, ground_action, air_action)


def start_open_special(
    fighter: Any, ctx: Any, ground_action: Any, air_action: Any
) -> bool:
    """Start a special on the currently open ground or aerial surface."""
    if not (getattr(ctx, "ground_open", False) or getattr(ctx, "air_open", False)):
        return False
    start_action(
        fighter,
        ground_action if getattr(ctx, "ground_open", False) else air_action,
    )
    return True


def select_facing_phase(
    grounded: bool,
    facing: float,
    ground_left: Any,
    ground_right: Any,
    air_left: Any,
    air_right: Any,
    ground_left_state: int,
    ground_right_state: int,
    air_left_state: int,
    air_right_state: int,
) -> tuple[Any, int]:
    """Select a source phase and state from surface and facing."""
    if grounded:
        return (ground_left, ground_left_state) if facing < 0 else (ground_right, ground_right_state)
    return (air_left, air_left_state) if facing < 0 else (air_right, air_right_state)


def start_complete_special(
    fighter: Any,
    ctx: Any,
    select_phase: Callable[[bool, float], tuple[Any, int]],
    *,
    active_actions: Iterable[Any] = (),
    direction: int | None = None,
) -> bool:
    """Enter a selected phase only when its native animation is complete."""
    if fighter.action in active_actions:
        return True
    if not (getattr(ctx, "ground_open", False) or getattr(ctx, "air_open", False)):
        return False
    if direction is not None and not directional_fresh_b(ctx, direction):
        return False
    if direction is None and not fresh_b_input(ctx):
        return False
    phase, state = select_phase(bool(getattr(ctx, "ground_open", False)), fighter.facing)
    if not fighter.has_complete_animation(state):
        return False
    start_action(fighter, phase)
    return True


def frame_preserving_surface_pairs(
    ground_left: Any, ground_right: Any, air_left: Any, air_right: Any
) -> tuple[dict[Any, Any], dict[Any, Any]]:
    """Build matching ground/air transitions that retain state and frame."""
    from .transitions import Transition

    return (
        {
            air_left: Transition(ground_left, preserve_state=True, keep_frame=True),
            air_right: Transition(ground_right, preserve_state=True, keep_frame=True),
        },
        {
            ground_left: Transition(air_left, preserve_state=True, keep_frame=True),
            ground_right: Transition(air_right, preserve_state=True, keep_frame=True),
        },
    )


__all__ = [
    "any_stick_axis_reaches_thresholds",
    "directional_match",
    "directional_b_input",
    "directional_b_reserved",
    "fresh_b_input",
    "directional_fresh_b",
    "fresh_special_input",
    "resource_attributes",
    "special_rules",
    "start_action",
    "start_fresh_open_special",
    "start_open_special",
    "select_facing_phase",
    "start_complete_special",
    "frame_preserving_surface_pairs",
    "stick_axis_reaches_threshold",
]
