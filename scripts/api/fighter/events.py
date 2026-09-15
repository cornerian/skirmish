"""Finite native event decorators and their serializable metadata."""

from dataclasses import dataclass
from enum import Enum
from typing import Any, Callable


class Hook(str, Enum):
    INPUT_PRESSED = "input_pressed"
    INPUT_RELEASED = "input_released"
    STICK_CHANGED = "stick_changed"
    ACTION_AVAILABILITY_CHANGED = "action_availability_changed"
    ACTION_ENTERED = "action_entered"
    ACTION_EXITED = "action_exited"
    ANIMATION_ENDED = "animation_ended"
    SCHEDULED_DEADLINE = "scheduled_deadline"
    COMMAND_TRACE_CHANGED = "command_trace_changed"
    BEFORE_HIT = "before_hit"
    BEFORE_RECEIVE_HIT = "before_receive_hit"
    AFTER_HIT = "after_hit"
    AFTER_RECEIVE_HIT = "after_receive_hit"
    PROJECTILE_CONTACT = "projectile_contact"
    LANDED = "landed"
    SURFACE_CONTACT = "surface_contact"
    GROUND_AIR_CHANGED = "ground_air_changed"
    PLATFORM_DROP_DECISION = "platform_drop_decision"


@dataclass(frozen=True, slots=True)
class EventBinding:
    hook: Hook
    callback: str
    action: str | None = None
    actions: tuple[str, ...] = ()
    buttons: int | None = None
    marker: str | None = None
    track: str | None = None
    gate: str | None = None
    countdown: str | None = None
    countdown_phase: str = "physics"
    command_index: int | None = None
    deadline: int | None = None

    def as_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {"hook": self.hook.value, "callback": self.callback}
        for key in ("action", "marker", "track", "gate", "countdown", "command_index", "deadline"):
            value = getattr(self, key)
            if value is not None:
                result[key] = value
        if self.actions:
            result["actions"] = list(self.actions)
        if self.buttons is not None:
            result["buttons"] = self.buttons
        if self.countdown_phase != "physics":
            result["countdown_phase"] = self.countdown_phase
        return result


def _binding(hook: Hook, *, action: Any = None, actions: tuple[Any, ...] = (),
             buttons: tuple[Any, ...] = (), marker: str | None = None,
             track: str | None = None, gate: str | None = None,
             countdown: str | None = None,
             countdown_phase: str = "physics",
             deadline: int | None = None, command_index: int | None = None) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    def decorate(function: Callable[..., Any]) -> Callable[..., Any]:
        existing = list(getattr(function, "__fighter_events__", ()))
        names = tuple(_action_name(item) for item in actions)
        if action is not None:
            action_name = _action_name(action)
            names = names or (action_name,)
        if countdown_phase not in ("animation", "physics"):
            raise ValueError("countdown phase must be 'animation' or 'physics'")
        binding = EventBinding(hook, function.__name__, action=_action_name(action) if action is not None else None,
                               actions=names, buttons=_button_mask(buttons),
                               marker=marker, track=track, gate=gate,
                               countdown=countdown, countdown_phase=countdown_phase,
                               command_index=command_index, deadline=deadline)
        existing.append(binding)
        function.__fighter_events__ = tuple(existing)
        return function
    return decorate


def _action_name(value: Any) -> str:
    # Compatibility ActionDescriptor stores its canonical enum/string under
    # ``action``. Unwrap that before considering ordinary enum members; using
    # ``str(value)`` here would put a Python repr on the native wire.
    if isinstance(value, str):
        if not value:
            raise ValueError("action reference must be non-empty")
        return value
    action = getattr(value, "action", None)
    if action is not None and action is not value:
        return _action_name(action)
    if isinstance(value, Enum):
        return _action_name(value.value)
    name = getattr(value, "name", None)
    if isinstance(name, str) and name:
        return name
    raise TypeError(f"invalid action reference {value!r}")


def _button_name(value: Any) -> str:
    return getattr(value, "value", getattr(value, "name", str(value)))


_BUTTON_MASKS = {
    "A": 0x100, "B": 0x200, "Z": 0x10, "L": 0x40, "R": 0x20,
    "X": 0x400, "Y": 0x800, "DPAD_LEFT": 0x1, "DPAD_RIGHT": 0x2,
    "DPAD_DOWN": 0x4, "DPAD_UP": 0x8,
}


def _button_mask(buttons: tuple[Any, ...]) -> int | None:
    if not buttons:
        return None
    mask = 0
    for button in buttons:
        if isinstance(button, bool):
            raise ValueError("button must be a Button name or mask")
        if isinstance(button, int):
            mask |= button
            continue
        name = _button_name(button)
        if "." in name:
            namespace, name = name.split(".", 1)
            if namespace != "Button":
                raise ValueError(f"unknown button {button!r}")
        try:
            mask |= _BUTTON_MASKS[name]
        except KeyError as exc:
            raise ValueError(f"unknown button {button!r}") from exc
    if not 0 <= mask <= 0xFFFF:
        raise ValueError("button mask exceeds u16")
    return mask


class _On:
    def press(self, *buttons: Any, **kwargs: Any): return _binding(Hook.INPUT_PRESSED, buttons=buttons, actions=kwargs.get("actions", ()))
    def release(self, *buttons: Any, **kwargs: Any): return _binding(Hook.INPUT_RELEASED, buttons=buttons, actions=kwargs.get("actions", ()))
    def stick(self, function=None, **kwargs):
        decorator = _binding(Hook.STICK_CHANGED, actions=kwargs.get("actions", ()))
        return decorator(function) if callable(function) else decorator
    def availability(self, *actions: Any, **kwargs: Any):
        return _binding(Hook.ACTION_AVAILABILITY_CHANGED,
                        actions=kwargs.get("actions", actions), gate=kwargs.get("gate"))
    def enter(self, *actions: Any): return _binding(Hook.ACTION_ENTERED, actions=actions)
    def exit(self, *actions: Any): return _binding(Hook.ACTION_EXITED, actions=actions)
    def animation_end(self, *actions: Any): return _binding(Hook.ANIMATION_ENDED, actions=actions)
    def deadline(self, at: int, *actions: Any): return _binding(Hook.SCHEDULED_DEADLINE, deadline=at, actions=actions)
    def marker(self, name: str, *actions: Any, **kwargs: Any):
        return _binding(Hook.SCHEDULED_DEADLINE, marker=name,
                        track=kwargs.get("track"), actions=kwargs.get("actions", actions))
    def countdown(self, field: str, *actions: Any, **kwargs: Any):
        return _binding(Hook.SCHEDULED_DEADLINE, countdown=field,
                        actions=kwargs.get("actions", actions),
                        countdown_phase=kwargs.get("phase", "physics"))
    def command_changed(self, index: int, *actions: Any, **kwargs: Any): return _binding(Hook.COMMAND_TRACE_CHANGED, command_index=index, actions=kwargs.get("actions", actions))
    def _optional(self, hook, function=None, **kwargs):
        decorator = _binding(hook, **kwargs)
        return decorator(function) if callable(function) else decorator
    def before_hit(self, function=None, **kwargs): return self._optional(Hook.BEFORE_HIT, function, actions=kwargs.get("actions", ()))
    def before_receive_hit(self, function=None, **kwargs): return self._optional(Hook.BEFORE_RECEIVE_HIT, function, actions=kwargs.get("actions", ()))
    def after_hit(self, function=None, **kwargs): return self._optional(Hook.AFTER_HIT, function, actions=kwargs.get("actions", ()))
    def after_receive_hit(self, function=None, **kwargs): return self._optional(Hook.AFTER_RECEIVE_HIT, function, actions=kwargs.get("actions", ()))
    def projectile_contact(self, function=None): return self._optional(Hook.PROJECTILE_CONTACT, function)
    def landed(self, *actions, **kwargs):
        function = actions[0] if len(actions) == 1 and callable(actions[0]) else None
        values = () if function is not None else actions
        return self._optional(Hook.LANDED, function, actions=kwargs.get("actions", values))
    def surface_contact(self, *actions, **kwargs): return self._optional(Hook.SURFACE_CONTACT, None, actions=kwargs.get("actions", actions))
    def ground_air_changed(self, *actions, **kwargs): return self._optional(Hook.GROUND_AIR_CHANGED, None, actions=kwargs.get("actions", actions))
    def platform_drop(self, *actions, **kwargs): return self._optional(Hook.PLATFORM_DROP_DECISION, None, actions=kwargs.get("actions", actions))
    def validate(self, function):
        function.__fighter_validate__ = True
        return function
    action_enter = enter
    action_exit = exit
    action_availability_changed = availability
    stick_changed = stick
    animation_end = animation_end
    platform_drop_decision = platform_drop
    input_pressed = press
    input_released = release


on = _On()
