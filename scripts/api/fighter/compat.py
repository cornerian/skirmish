"""Typed declarations shared by the Python authoring surface.

These objects describe data consumed by the native registration bridge. They
do not execute motion or validation and contain no rollback state.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
import math
import struct
from typing import Any, Mapping

from .actions import Action


class Button(str, Enum):
    A = "A"; B = "B"; X = "X"; Y = "Y"; Z = "Z"; L = "L"; R = "R"


def _initialize_typed_defaults(instance: Any, cls: type[Any], values: dict[str, Any]) -> None:
    annotations: dict[str, Any] = {}
    defaults: dict[str, Any] = {}
    for base in reversed(cls.__mro__):
        annotations.update(getattr(base, "__annotations__", {}))
        for name in annotations:
            if name in base.__dict__:
                defaults[name] = base.__dict__[name]
    for name in annotations:
        if name in values:
            setattr(instance, name, values.pop(name))
        elif name in defaults:
            setattr(instance, name, defaults[name])
    for name, value in values.items():
        setattr(instance, name, value)


class ActionState:
    def __init__(self, **values: Any): _initialize_typed_defaults(self, type(self), values)


class Parameters:
    """Materialized parameter record, including annotated class defaults."""
    def __init__(self, **values: Any): _initialize_typed_defaults(self, type(self), values)


class HitContext:
    def __init__(self, **values: Any): _initialize_typed_defaults(self, type(self), values)


class _F32:
    """Builtin-valued wrapper whose basic arithmetic rounds to binary32."""

    __slots__ = ("value",)

    def __init__(self, value: Any):
        self.value = _round_binary(float(value))

    def __float__(self) -> float:
        return self.value

    def __bool__(self) -> bool:
        return self.value != 0.0

    def _compare(self, other: Any, operation: Any) -> bool:
        return operation(self.value, _numeric_value(other))

    def _binary(self, other: Any, operation: Any) -> "_F32":
        return _F32(operation(self.value, _numeric_value(other)))

    __add__ = lambda self, other: self._binary(other, lambda a, b: a + b)
    __radd__ = lambda self, other: self._binary(other, lambda a, b: b + a)
    __sub__ = lambda self, other: self._binary(other, lambda a, b: a - b)
    __rsub__ = lambda self, other: self._binary(other, lambda a, b: b - a)
    __mul__ = lambda self, other: self._binary(other, lambda a, b: a * b)
    __rmul__ = lambda self, other: self._binary(other, lambda a, b: b * a)
    __truediv__ = lambda self, other: self._binary(other, _ieee_div)
    __rtruediv__ = lambda self, other: self._binary(other, lambda a, b: _ieee_div(b, a))
    __neg__ = lambda self: _F32(-self.value)
    __pos__ = lambda self: _F32(self.value)
    def __abs__(self) -> "_F32":
        return _F32(abs(self.value))
    __eq__ = lambda self, other: self._compare(other, lambda a, b: a == b) if _is_numeric(other) else False
    __ne__ = lambda self, other: self._compare(other, lambda a, b: a != b) if _is_numeric(other) else True
    __lt__ = lambda self, other: self._compare(other, lambda a, b: a < b) if _is_numeric(other) else False
    __le__ = lambda self, other: self._compare(other, lambda a, b: a <= b) if _is_numeric(other) else False
    __gt__ = lambda self, other: self._compare(other, lambda a, b: a > b) if _is_numeric(other) else False
    __ge__ = lambda self, other: self._compare(other, lambda a, b: a >= b) if _is_numeric(other) else False


def _numeric_value(value: Any) -> float:
    return value.value if isinstance(value, _F32) else float(value)


def _is_numeric(value: Any) -> bool:
    return isinstance(value, (int, float, _F32)) and not isinstance(value, bool)


def _ieee_div(a: float, b: float) -> float:
    if b != 0.0:
        return a / b
    if a == 0.0:
        return math.nan
    return math.copysign(math.inf, math.copysign(1.0, a) * math.copysign(1.0, b))


def _round_binary(value: float) -> float:
    try:
        return struct.unpack("<f", struct.pack("<f", value))[0]
    except OverflowError:
        return math.copysign(math.inf, value)


def f32(value: Any) -> float:
    """Convert to binary32 and retain binary32 rounding for basic arithmetic."""
    if isinstance(value, _F32):
        return _F32(value.value)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError("f32 expects a number")
    try:
        rounded = struct.unpack("<f", struct.pack("<f", _numeric_value(value)))[0]
    except OverflowError:
        rounded = math.copysign(math.inf, float(value))
    return _F32(rounded)


from .transitions import SpecialMove


@dataclass(frozen=True, slots=True)
class MotionBinding:
    """Per-fighter numeric binding; immutable profile data stays native."""
    facing: float = 1.0
    cosine: float = 1.0
    sine: float = 0.0
    ground_scale: float = 1.0

    def __post_init__(self) -> None:
        for name in ("facing", "cosine", "sine", "ground_scale"):
            object.__setattr__(self, name, f32(getattr(self, name)))

    def as_dict(self) -> dict[str, float]:
        return {name: float(getattr(self, name)) for name in ("facing", "cosine", "sine", "ground_scale")}


@dataclass(frozen=True, slots=True)
class ActionDescriptor:
    action: Any
    metadata: tuple[tuple[str, Any], ...] = ()
    animation_loop: bool = False

    def __hash__(self) -> int: return hash((self.action, self.metadata, self.animation_loop))

    def __eq__(self, other: Any) -> bool:
        if isinstance(other, ActionDescriptor):
            return (self.action == other.action and self.metadata == other.metadata
                    and self.animation_loop == other.animation_loop)
        return _normalized_action(self.action) == _normalized_action(other)

    def as_dict(self) -> dict[str, Any]:
        value = self.action.name if isinstance(self.action, Action) else self.action
        return {"action": f"Action.{value}", "animation_loop": self.animation_loop,
                **{key: _encode(item) for key, item in self.metadata}}


def action(value: Any, *, animation_loop: bool = False, **metadata: Any) -> ActionDescriptor:
    return ActionDescriptor(value, tuple(metadata.items()), animation_loop)


def _normalized_action(value: Any) -> str | None:
    """Return the ABI's spelling-independent action key for comparisons."""
    # NativeMember deliberately exposes only `_value()` for unwrapping. Do
    # not probe a dynamic `.action` attribute here: doing so turns a native
    # action member into `fighter.action.action` and can trigger an unrelated
    # host lookup while merely comparing values.
    if type(value).__name__ == "NativeMember":
        return _normalized_action(value._value())
    if isinstance(value, ActionDescriptor):
        value = value.action
    if isinstance(value, Action):
        value = value.value
    if isinstance(value, str):
        return value.removeprefix("Action.").replace("_", "").lower()
    nested = getattr(value, "action", None)
    if nested is not None and nested is not value:
        return _normalized_action(nested)
    return None


@dataclass(frozen=True, slots=True)
class _Reference:
    callee: str
    path: str

    def __call__(self, path: str) -> "_Reference": return type(self)(self.callee, path)
    def as_dict(self) -> dict[str, Any]: return {"callee": self.callee, "args": [self.path], "kwargs": {}}
    def __str__(self) -> str: return self.path


resource = _Reference("resource", "")
parameter = _Reference("parameter", "")


@dataclass(frozen=True, slots=True)
class ClockBinding:
    field: str
    actions: tuple[Any, ...] = ()

    def as_dict(self) -> dict[str, Any]:
        return {"field": self.field, "actions": [_action_reference(value) for value in self.actions]}


def clock(*args: Any, **metadata: Any) -> ClockBinding:
    if len(args) > 1: raise TypeError("clock accepts at most one positional field")
    if args and "field" in metadata: raise TypeError("clock field supplied twice")
    if args: metadata["field"] = args[0]
    field = metadata.pop("field", None)
    if not isinstance(field, str) or not field: raise TypeError("clock requires a non-empty field")
    actions = metadata.pop("actions", ())
    if metadata: raise TypeError(f"clock got unexpected keyword argument {next(iter(metadata))!r}")
    return ClockBinding(field, tuple(actions))


@dataclass(frozen=True, slots=True)
class MotionConstructor:
    name: str
    args: tuple[Any, ...] = ()
    kwargs: tuple[tuple[str, Any], ...] = ()

    def as_dict(self) -> dict[str, Any]:
        return {"callee": f"motion.{self.name}", "args": [_encode(v) for v in self.args],
                "kwargs": {k: _encode(v) for k, v in self.kwargs}}

    def __hash__(self) -> int: return hash((self.name, self.args, self.kwargs))


def _encode(value: Any) -> Any:
    if hasattr(value, "as_dict"): return value.as_dict()
    if isinstance(value, Enum): return value.value
    if isinstance(value, (tuple, list)): return [_encode(item) for item in value]
    if isinstance(value, Mapping): return {key: _encode(item) for key, item in value.items()}
    return value


def _action_reference(value: Any) -> Any:
    if isinstance(value, ActionDescriptor):
        action_value = value.action.name if isinstance(value.action, Action) else value.action
        return f"Action.{action_value}"
    if isinstance(value, Action): return f"Action.{value.name}"
    return _encode(value)


class _Motion:
    def __getattr__(self, name: str):
        if name.startswith("_"): raise AttributeError(name)
        def constructor(*args: Any, **kwargs: Any) -> MotionConstructor:
            return MotionConstructor(name, tuple(args), tuple(kwargs.items()))
        return constructor


motion = _Motion()


class _Validation:
    @staticmethod
    def finite(value: Any) -> bool:
        value = _unwrap_native_scalar(value)
        return not isinstance(value, bool) and (isinstance(value, (int, float, _F32))) and math.isfinite(_numeric_value(value))

    @classmethod
    def number(cls, value: Any, integer: bool = False) -> bool:
        # Kept as ``integer`` for source compatibility; native ABI calls this
        # flag ``nonnegative`` and does not require an integral value.
        value = _unwrap_native_scalar(value)
        numeric = _numeric_value(value) if isinstance(value, _F32) else value
        return cls.finite(value) and abs(float(numeric)) <= 1_000_000.0 and (not integer or numeric >= 0)

    @classmethod
    def fields(cls, value: Any, **categories: Any) -> bool:
        if value is None or any(name not in {"finite", "nonnegative", "positive", "nonzero"} for name in categories): return False
        for mode, names in categories.items():
            if isinstance(names, str): names = (names,)
            try:
                for name in names:
                    field = value.get(name) if isinstance(value, Mapping) else getattr(value, name)
                    field = _unwrap_native_scalar(field)
                    if not cls.number(field): return False
                    if mode == "nonnegative" and field < 0: return False
                    if mode == "positive" and field <= 0: return False
                    if mode == "nonzero" and field == 0: return False
            except (AttributeError, KeyError, TypeError): return False
        return True

    @classmethod
    def hitboxes(cls, value: Any, **_: Any) -> bool:
        if isinstance(value, (str, bytes)) or value is None:
            return False
        try:
            if not 1 <= len(value) <= 4:
                return False
            for hitbox in value:
                get = hitbox.get if isinstance(hitbox, Mapping) else lambda key: getattr(hitbox, key)
                radius = get("radius")
                angle = get("angle") if (isinstance(hitbox, Mapping) and "angle" in hitbox) or hasattr(hitbox, "angle") else get("angle_degrees")
                center = get("center")
                radius = _unwrap_native_scalar(radius)
                angle = _unwrap_native_scalar(angle)
                if not cls.number(radius) or radius < 0 or not cls.finite(angle):
                    return False
                if len(center) != 3 or any(not cls.finite(coordinate) for coordinate in center):
                    return False
            return True
        except (AttributeError, KeyError, TypeError):
            return False

    @staticmethod
    def command_trace(context: Any, command_path: Any = None, pose_path: Any = None, **_: Any) -> bool:
        """Validate a nullable command trace against its native pose count.

        Pon resource arrays are opaque host members, so shape and scalar
        checks must go through the context's native length/path operations.
        This keeps malformed traces from being accepted merely because their
        paths are non-empty.
        """
        if context is None or not all(isinstance(path, str) and bool(path)
                                      for path in (command_path, pose_path)):
            return False
        try:
            resource = context.resource(command_path)
            if resource is None:
                return False
            command_rows = resource.cmd_vars
            interrupt_rows = resource.allow_interrupt
            frame_count = context.frames(pose_path)
            if not isinstance(frame_count, int) or not 1 <= frame_count <= 4096:
                return False
            if (context.array_length(f"{command_path}.cmd_vars") != frame_count
                    or context.array_length(f"{command_path}.allow_interrupt") != frame_count):
                return False
            for index in range(frame_count):
                if context.array_length(f"{command_path}.cmd_vars", index) != 4:
                    return False
                row = command_rows[index]
                for column in range(4):
                    value = row[column]
                    if type(value).__name__ == "NativeMember":
                        value = value._value()
                    if type(value).__name__ == "_F32":
                        value = _numeric_value(value)
                    if value is not None and not _valid_command_value(value):
                        return False
                interrupt = interrupt_rows[index]
                if type(interrupt).__name__ == "NativeMember":
                    interrupt = interrupt._value()
                if type(interrupt) is not bool:
                    return False
            return True
        except (AttributeError, KeyError, IndexError, TypeError, ValueError):
            return False


def _valid_command_value(value: Any) -> bool:
    if type(value) is int:
        return 0 <= value <= 0xFFFFFFFF
    if type(value) is not float or not math.isfinite(value) or value != int(value) or value < 0:
        return False
    integer = int(value)
    return 0 <= integer <= 0xFFFFFFFF and float(integer) == value


validation = _Validation()


def _unwrap_native_scalar(value: Any) -> Any:
    """Read a scalar returned by a lazy native resource proxy."""
    if type(value).__name__ == "NativeMember":
        return value._value()
    return value
