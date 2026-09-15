"""Small declarative action-transition support for finite move phases."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, ClassVar, Mapping

from .actions import Action
from .api import Move
from .events import EventBinding, Hook


def _action_key(value: Any) -> str:
    """Normalize an authoring action without touching native proxy members."""
    if type(value).__name__ == "NativeMember":
        from .compat import _normalized_action
        normalized = _normalized_action(value)
        if normalized is None:
            raise TypeError(f"invalid action reference {value!r}")
        return normalized.replace("_", "").lower()
    if hasattr(value, "action"):
        nested = value.action
        if nested is not value:
            return _action_key(nested)
    if isinstance(value, Action):
        return value.value.replace("_", "").lower()
    if isinstance(value, str):
        return value.removeprefix("Action.").replace("_", "").lower()
    name = getattr(value, "name", None)
    if isinstance(name, str) and name:
        return name.replace("_", "").lower()
    raise TypeError(f"invalid action reference {value!r}")


def _validate_action(value: Any) -> None:
    if isinstance(value, Action):
        return
    # Import lazily to avoid compat -> transitions import cycles.
    from .compat import ActionDescriptor
    if isinstance(value, ActionDescriptor) and isinstance(value.action, (Action, str)):
        return
    raise TypeError("transition action references must be Action or ActionDescriptor")


def _wire_action_name(value: Any) -> str:
    if isinstance(value, Action):
        return value.value
    if isinstance(value, str):
        return value.removeprefix("Action.")
    target = getattr(value, "action", None)
    if isinstance(target, Action):
        return target.value
    if isinstance(target, str):
        return target.removeprefix("Action.")
    raise TypeError(f"invalid action reference {value!r}")


@dataclass(frozen=True, slots=True)
class Transition:
    target: Any
    preserve_state: bool = False
    keep_frame: bool = False

    def __post_init__(self) -> None:
        _validate_action(self.target)
        if not isinstance(self.preserve_state, bool) or not isinstance(self.keep_frame, bool):
            raise TypeError("transition flags must be bool")


@dataclass(frozen=True, slots=True)
class _Rule:
    event: str
    source: str
    source_name: str
    grounded: bool | None
    transition: Transition


class SpecialMove(Move):
    """Move base with finite, class-defined action transition rules.

    ``on_end`` handles animation completion. ``on_ground`` maps an airborne
    source into a grounded target, while ``on_air`` maps a grounded source
    into an airborne target. Rules are normalized once when the subclass is
    created and never rebuilt during callback execution.
    """

    on_end: ClassVar[Mapping[Any, Transition]] = {}
    on_ground: ClassVar[Mapping[Any, Transition]] = {}
    on_air: ClassVar[Mapping[Any, Transition]] = {}
    __transition_rules__: ClassVar[tuple[_Rule, ...]] = ()

    def __init_subclass__(cls, **kwargs: Any) -> None:
        super().__init_subclass__(**kwargs)
        merged: dict[tuple[str, str], _Rule] = {}
        for base in cls.__mro__[1:]:
            for rule in getattr(base, "__transition_rules__", ()):
                merged.setdefault((rule.event, rule.source), rule)
        for name, grounded in (("on_end", None), ("on_ground", True), ("on_air", False)):
            values = cls.__dict__.get(name, {})
            # Pon's restricted stdlib may provide a dict-like mapping whose
            # concrete type does not register with ``typing.Mapping``.
            if not callable(getattr(values, "items", None)):
                raise TypeError(f"{name} must be a mapping of action to Transition")
            for source, transition in values.items():
                _validate_action(source)
                if not isinstance(transition, Transition):
                    raise TypeError(f"{name} values must be Transition instances")
                rule = _Rule(name, _action_key(source), _wire_action_name(source), grounded, transition)
                merged[(name, rule.source)] = rule
        cls.__transition_rules__ = tuple(merged.values())

    @classmethod
    def events(cls) -> tuple[EventBinding, ...]:
        result = list(super().events())
        rules = cls.__transition_rules__
        end = tuple(rule.source_name for rule in rules if rule.event == "on_end")
        ground = tuple(rule.source_name for rule in rules if rule.event in ("on_ground", "on_air"))
        if end:
            result.append(EventBinding(Hook.ANIMATION_ENDED, "_transition_animation_end", actions=end))
        if ground:
            actions = tuple(dict.fromkeys(ground))
            result.append(EventBinding(Hook.GROUND_AIR_CHANGED, "_transition_ground_air", actions=actions))
            landed = tuple(rule.source_name for rule in rules if rule.event == "on_ground")
            if landed:
                result.append(EventBinding(Hook.LANDED, "_transition_ground_air", actions=landed))
        return tuple(result)

    def _transition_animation_end(self, fighter: Any, ctx: Any) -> None:
        self._apply_transition("on_end", fighter, ctx)

    def _transition_ground_air(self, fighter: Any, ctx: Any) -> None:
        self._apply_transition("on_ground" if bool(ctx.grounded) else "on_air", fighter, ctx)

    def _apply_transition(self, event: str, fighter: Any, ctx: Any) -> bool:
        current = _action_key(fighter.action)
        # The host reports the destination contact state in the event context;
        # the fighter proxy can still expose the pre-transition state here.
        grounded = bool(ctx.grounded) if ctx is not None and hasattr(ctx, "grounded") else bool(fighter.grounded)
        for rule in self.__transition_rules__:
            if rule.event != event or rule.source != current:
                continue
            if rule.grounded is not None and rule.grounded != grounded:
                continue
            target = rule.transition.target
            target_value = target.action if hasattr(target, "action") else target
            fighter.change_action(
                target_value,
                preserve_state=rule.transition.preserve_state,
                keep_frame=rule.transition.keep_frame,
            )
            return True
        return False


__all__ = ["SpecialMove", "Transition"]
