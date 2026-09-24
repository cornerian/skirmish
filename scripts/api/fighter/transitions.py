"""Small declarative action-transition support for finite move phases."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, ClassVar, Mapping

from .actions import Action
from .api import Move
from .events import EventBinding, Hook


def _source_state_key(reference: str) -> str:
    """Keep the fighter identity when normalizing a source action."""
    if reference.startswith("Source."):
        parts = reference.removeprefix("Source.").split(":")
        if len(parts) in (1, 2) and all(part.isdigit() for part in parts):
            return f"Source.{':'.join(parts)}"
    return reference


def _canonical_source_key(reference: str) -> str:
    """Normalize native case-folded source identities to the wire spelling."""
    if reference.lower().startswith("source."):
        return f"Source.{reference.split('.', 1)[1]}"
    return reference


def _action_key(value: Any) -> str:
    """Normalize an authoring action without touching native proxy members."""
    if type(value).__name__ == "NativeMember":
        from .compat import _normalized_action
        normalized = _normalized_action(value)
        if normalized is None:
            raise TypeError(f"invalid action reference {value!r}")
        return _canonical_source_key(normalized)
    from .compat import CustomAction, SourceAction
    if isinstance(value, (CustomAction, SourceAction)):
        return _source_state_key(value.reference)
    if hasattr(value, "action"):
        nested = value.action
        if nested is not value:
            return _action_key(nested)
    if isinstance(value, Action):
        return value.value.replace("_", "").lower()
    if isinstance(value, str):
        # ``Action.Source...`` is an export spelling, not a native runtime
        # action.  Keeping it distinct prevents an alternate wire form from
        # bypassing the concrete source identity bound to this move.
        if value.startswith("Action.Source."):
            return value
        value = value.removeprefix("Action.")
        value = _source_state_key(value)
        return value if value.startswith(("Custom.", "Source.")) else value.replace("_", "").lower()
    name = getattr(value, "name", None)
    if isinstance(name, str) and name:
        return name.replace("_", "").lower()
    raise TypeError(f"invalid action reference {value!r}")


def _validate_action(value: Any) -> None:
    if isinstance(value, Action):
        return
    # Import lazily to avoid compat -> transitions import cycles.
    from .compat import ActionDescriptor, CustomAction, SourceAction
    if isinstance(value, (CustomAction, SourceAction)):
        return
    if isinstance(value, ActionDescriptor) and isinstance(value.action, (Action, str, CustomAction, SourceAction)):
        return
    raise TypeError("transition action references must be Action or ActionDescriptor")


def _wire_action_name(value: Any) -> str:
    if isinstance(value, Action):
        return value.value
    from .compat import CustomAction, SourceAction
    if isinstance(value, (CustomAction, SourceAction)):
        return value.reference
    if isinstance(value, str):
        return value.removeprefix("Action.")
    target = getattr(value, "action", None)
    if isinstance(target, Action):
        return target.value
    if isinstance(target, str):
        return target.removeprefix("Action.")
    if isinstance(target, (CustomAction, SourceAction)):
        return target.reference
    raise TypeError(f"invalid action reference {value!r}")


def _bind_source_target(value: Any, current: Any) -> Any:
    """Qualify a source target from the current runtime source identity."""
    from .compat import ActionDescriptor, SourceAction, resolve_source_action

    target = value.action if isinstance(value, ActionDescriptor) else value
    if not isinstance(target, SourceAction):
        return value
    current = current._value() if type(current).__name__ == "NativeMember" else current
    if isinstance(current, ActionDescriptor):
        current = current.action
    reference = getattr(current, "reference", current)
    if not isinstance(reference, str) or not reference.startswith("Source.") or ":" not in reference:
        return value
    external = reference.removeprefix("Source.").split(":", 1)[0]
    qualified = resolve_source_action(target.reference, (int(external),))
    if isinstance(value, ActionDescriptor):
        return ActionDescriptor(qualified, value.metadata, value.animation_loop)
    return qualified


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
    # The host can deliver contact events every frame.  Keep the normalized
    # rules in an index so dispatch is constant time rather than walking all
    # phases on every callback.  The first element is the rule tuple used to
    # build the index; identity-bound subclasses replace that tuple after
    # class creation and are refreshed lazily by _apply_transition.
    __transition_index__: ClassVar[
        tuple[tuple[_Rule, ...], dict[tuple[str, str, bool | None], _Rule]]
    ] = ((), {})

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
        cls.__transition_index__ = (cls.__transition_rules__, {
            (rule.event, rule.source, rule.grounded): rule
            for rule in cls.__transition_rules__
        })

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
        # Some native lifecycle paths provide no payload because the fighter
        # proxy already contains the authoritative contact state.  Keep the
        # dispatch entry point consistent with _apply_transition's fallback.
        grounded = (
            bool(ctx.grounded)
            if ctx is not None and hasattr(ctx, "grounded")
            else bool(fighter.grounded)
        )
        self._apply_transition("on_ground" if grounded else "on_air", fighter, ctx)

    def _apply_transition(self, event: str, fighter: Any, ctx: Any) -> bool:
        current = _action_key(fighter.action)
        # The host reports the destination contact state in the event context;
        # the fighter proxy can still expose the pre-transition state here.
        grounded = bool(ctx.grounded) if ctx is not None and hasattr(ctx, "grounded") else bool(fighter.grounded)
        rules = self.__transition_rules__
        indexed_rules, index = type(self).__transition_index__
        if indexed_rules is not rules:
            # _bind_source_actions specializes shared move classes per roster
            # identity and installs a fresh rule tuple after __init_subclass__.
            # Rebuild once for that specialized class instead of retaining
            # stale unqualified source keys.
            index = {
                (rule.event, rule.source, rule.grounded): rule
                for rule in rules
            }
            type(self).__transition_index__ = (rules, index)
        rule = index.get((event, current, grounded))
        if rule is None:
            # Animation completion is independent of contact state.
            rule = index.get((event, current, None))
        if rule is None:
            return False
        target = rule.transition.target
        target_value = _bind_source_target(target, fighter.action)
        target_value = target_value.action if hasattr(target_value, "action") else target_value
        fighter.change_action(
            target_value,
            preserve_state=rule.transition.preserve_state,
            keep_frame=rule.transition.keep_frame,
        )
        return True


__all__ = ["SpecialMove", "Transition"]
