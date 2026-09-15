"""Python objects backed by the scoped ``_skirmish_native`` ABI.

The module is deliberately ordinary Python: Pon supplies the private native
module, while these wrappers preserve opaque host identity and recursively
translate values at the Python boundary.
"""

from __future__ import annotations

from enum import Enum
from typing import Any

from fighter.api import MoveWait
from fighter.compat import ActionDescriptor, MotionBinding, f32, _F32, _normalized_action

try:
    from _skirmish_native import call as _call, call_named as _call_named
    from _skirmish_native import get as _get, set as _set
except ImportError:  # authoring tools can import this module without Pon
    _get = _set = _call = _call_named = None


def _require_native() -> tuple[Any, Any, Any, Any]:
    if _get is None:
        raise RuntimeError("_skirmish_native is only available inside the Pon host")
    return _get, _set, _call, _call_named


_METHODS = {
    "change_action", "set_action", "pass_as", "set_velocity", "apply_hitlag",
    "emit_projectile", "spawn_special_effect", "clear_special_effect", "enter_fall_special", "jump_input", "aerial_jump", "just_pressed", "resource",
    "action_finished", "set_motion_binding", "restore_pre_landing", "max_jumps",
    "frames", "array_length", "validate_attack", "apply", "wait",
}
_STATE_PROXIES: dict[tuple[str, str], type["NativeObject"]] = {}
_INTERNAL_STATE_FIELDS = {"_token", "_kind", "_path", "__dict__"}


def _read(token: int, path: str) -> Any:
    get, _, _, _ = _require_native()
    return wrap(get(token, path), token)


def _index_path(owner: Any, key: Any) -> str:
    if not isinstance(key, int):
        raise TypeError("native host indices must be integers")
    size = len(owner)
    index = key + size if key < 0 else key
    if index < 0 or index >= size:
        raise IndexError("native host index out of range")
    return f"[{index}]"


class _NativeMethod:
    __slots__ = ("_token", "_path")

    def __init__(self, token: int, path: str): self._token, self._path = token, path

    def __call__(self, *args: Any, **kwargs: Any) -> Any:
        _, _, call, call_named = _require_native()
        values = [unwrap(value) for value in args]
        if kwargs:
            return wrap(call_named(self._token, self._path, values, unwrap(kwargs)), self._token)
        return wrap(call(self._token, self._path, values), self._token)


class NativeObject:
    """An opaque host object identified by a scope token, kind, and path."""

    __slots__ = ("_token", "_kind", "_path")

    def __init__(self, token: int, kind: str, path: str):
        self._token, self._kind, self._path = token, kind, path

    @property
    def token(self) -> int: return self._token
    @property
    def kind(self) -> str: return self._kind
    @property
    def path(self) -> str: return self._path

    def __getattr__(self, name: str) -> "NativeMember":
        if name.startswith("_"): raise AttributeError(name)
        path = f"{self._path}.{name}"
        if name in _METHODS: return _NativeMethod(self._token, path)  # type: ignore[return-value]
        return _read(self._token, path)  # type: ignore[return-value]

    def __setattr__(self, name: str, value: Any) -> None:
        if name in NativeObject.__slots__:
            object.__setattr__(self, name, value)
            return
        if name.startswith("_"): raise AttributeError(name)
        _, setter, _, _ = _require_native()
        setter(self._token, f"{self._path}.{name}", unwrap(value))

    def __getitem__(self, key: Any) -> Any:
        return _read(self._token, f"{self._path}{_index_path(self, key)}")

    def __len__(self) -> int:
        value = _read(self._token, f"{self._path}.length")
        return int(value)

    def __iter__(self):
        for index in range(len(self)):
            yield self[index]

    def __repr__(self) -> str:
        return f"<{self._kind} host object at {self._path!r}>"

    # Pon dispatches property setters through its descriptor protocol, while
    # its Python-level __setattr__ hook is intentionally not consulted for
    # ordinary assignments. Keep the public field syntax and route declared
    # mutable host fields through the same scoped native setter used by
    # NativeMember.
    @property
    def damage(self) -> Any:
        return _read(self._token, f"{self._path}.damage")

    @damage.setter
    def damage(self, value: Any) -> None:
        _, setter, _, _ = _require_native()
        setter(self._token, f"{self._path}.damage", unwrap(value))

    @property
    def flags(self) -> Any:
        return _read(self._token, f"{self._path}.flags")

    @property
    def reflecting(self) -> Any:
        return _read(self._token, f"{self._path}.reflecting")

    @reflecting.setter
    def reflecting(self, value: Any) -> None:
        _, setter, _, _ = _require_native()
        setter(self._token, f"{self._path}.reflecting", unwrap(value))


class NativeMember:
    __slots__ = ("_owner", "_name")

    def __init__(self, owner: NativeObject | "NativeMember", name: str):
        self._owner, self._name = owner, name

    @property
    def _path(self) -> str:
        if self._name.startswith("["):
            return f"{self._owner._path}{self._name}"
        return f"{self._owner._path}.{self._name}"

    @property
    def _token(self) -> int: return self._owner._token

    def __getattr__(self, name: str) -> "NativeMember":
        path = f"{self._path}.{name}"
        if name in _METHODS: return _NativeMethod(self._token, path)  # type: ignore[return-value]
        return _read(self._token, path)  # type: ignore[return-value]

    def _value(self) -> Any:
        get, _, _, _ = _require_native()
        return wrap(get(self._token, self._path), self._token)

    def __call__(self, *args: Any, **kwargs: Any) -> Any:
        return _NativeMethod(self._token, self._path)(*args, **kwargs)

    def __setattr__(self, name: str, value: Any) -> None:
        if name.startswith("_"): object.__setattr__(self, name, value); return
        _, setter, _, _ = _require_native()
        setter(self._token, f"{self._path}.{name}", unwrap(value))

    def __eq__(self, other: Any) -> bool:
        value = self._value()
        # Action descriptors normalize both enum/string spellings. Apply the
        # same rule when the native member is the left operand so authors do
        # not need to swap comparison order around a host-backed action.
        if self._path.endswith(".action"):
            actual = _normalized_action(value)
            expected = _normalized_action(other)
            if actual is not None and expected is not None:
                return actual == expected
        return value == other

    def __ne__(self, other: Any) -> bool:
        return not self == other
    def __bool__(self) -> bool: return bool(self._value())
    def __int__(self) -> int: return int(self._value())
    def __float__(self) -> float: return float(self._value())
    def __getitem__(self, key: Any) -> Any:
        return _read(self._token, f"{self._path}{_index_path(self, key)}")

    def __len__(self) -> int: return int(_read(self._token, f"{self._path}.length"))

    def __iter__(self):
        for index in range(len(self)):
            yield self[index]
    def __repr__(self) -> str: return repr(self._value())


def wrap(value: Any, token: int | None = None) -> Any:
    """Recursively wrap tagged host records returned by the loader."""
    if value is None: return None
    if isinstance(value, dict):
        if value.get("__skirmish_native__") is True and {"token", "kind", "path"} <= value.keys():
            token = int(value["token"])
            kind = str(value["kind"])
            path = str(value["path"])
            proxy = _STATE_PROXIES.get((kind, path), NativeObject)
            return proxy(token, kind, path)
        return {key: wrap(item, token) for key, item in value.items()}
    if isinstance(value, list): return [wrap(item, token) for item in value]
    if isinstance(value, float): return f32(value)
    return value


def unwrap(value: Any) -> Any:
    if value is None: return None
    if isinstance(value, MoveWait): return value.as_dict()
    if isinstance(value, NativeObject):
        return {"__skirmish_native__": True, "token": value._token, "kind": value._kind, "path": value._path}
    if isinstance(value, NativeMember): return unwrap(value._value())
    if isinstance(value, ActionDescriptor): return unwrap(value.action)
    if isinstance(value, MotionBinding): return unwrap(value.as_dict())
    if isinstance(value, Enum): return unwrap(value.value)
    if isinstance(value, _F32): return float(value)
    if isinstance(value, dict): return {key: unwrap(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)): return [unwrap(item) for item in value]
    return value


_FIXED_FIELDS = (
    "action", "action_frame", "position", "velocity", "knockback", "depth",
    "ground_velocity", "ground_knockback", "ground_line", "facing", "percent", "hitlag",
    "hitstun", "grounded", "short_hop", "fast_fall", "floor_normal",
    "angle", "base_knockback", "knockback_growth", "hitbox_group", "projectile",
    "max_damage", "cancelled", "apply_damage", "apply_knockback", "apply_hitlag",
    "apply_hitstun", "reflect", "flags", "state", "action_state", "input",
    "previous_input", "locomotion", "shield", "aerial", "ecb", "parameters", "reflecting",
    "health", "mobility", "landing_lag", "jumps_used",
)


def _native_property(name: str) -> property:
    def read(owner: NativeObject) -> Any:
        return _read(owner._token, f"{owner._path}.{name}")

    def write(owner: NativeObject, value: Any) -> None:
        _, setter, _, _ = _require_native()
        setter(owner._token, f"{owner._path}.{name}", unwrap(value))

    return property(read, write)


def _native_method_property(name: str) -> property:
    """Expose host methods as stable descriptors for Pon attribute lookup."""
    def read(owner: NativeObject) -> _NativeMethod:
        return _NativeMethod(owner._token, f"{owner._path}.{name}")

    return property(read)


def _native_jump_input_property() -> property:
    def read(owner: NativeObject) -> Any:
        if owner._path == "context":
            return _NativeMethod(owner._token, "context.jump_input")
        return _read(owner._token, f"{owner._path}.jump_input")

    def write(owner: NativeObject, value: Any) -> None:
        _, setter, _, _ = _require_native()
        setter(owner._token, f"{owner._path}.jump_input", unwrap(value))

    return property(read, write)


def _native_next_action_property() -> property:
    def read(owner: NativeObject) -> Any:
        value = _read(owner._token, f"{owner._path}.next_action")
        return ActionDescriptor(value) if isinstance(value, str) else value

    return property(read)


def install_native_properties(
    state_fields: Any = (), action_state_fields: Any = ()
) -> None:
    """Install stable field descriptors used by native callback objects.

    Pon honors descriptor setters but does not invoke Python ``__setattr__``.
    Installation is idempotent and occurs at module load plus once for each
    exported state schema, so callback assignment remains ordinary Python.
    """
    for name in _FIXED_FIELDS:
        if not isinstance(name, str) or not name or name in _METHODS:
            continue
        if name not in NativeObject.__dict__:
            setattr(NativeObject, name, _native_property(name))
    for name in _METHODS:
        if name == "jump_input":
            continue
        if name not in NativeObject.__dict__:
            setattr(NativeObject, name, _native_method_property(name))
    if "jump_input" not in NativeObject.__dict__:
        setattr(NativeObject, "jump_input", _native_jump_input_property())
    if "next_action" not in NativeObject.__dict__:
        setattr(NativeObject, "next_action", _native_next_action_property())

    for path, fields in (
        ("fighter.state", state_fields),
        ("fighter.action_state", action_state_fields),
    ):
        names = tuple(fields)
        invalid = _INTERNAL_STATE_FIELDS.intersection(names)
        if invalid:
            raise ValueError(
                f"state field names are reserved by the native proxy: {sorted(invalid)!r}"
            )
        key = ("State", path)
        declared = {name for name in names if isinstance(name, str) and name}
        existing = _STATE_PROXIES.get(key)
        if existing is not None and set(existing.__native_fields__) == declared:
            continue
        attrs: dict[str, Any] = {"__slots__": (), "__native_fields__": tuple(sorted(declared))}
        for name in declared:
            attrs[name] = _native_property(name)
        _STATE_PROXIES[key] = type(
            f"NativeStateProxy_{path.rsplit('.', 1)[-1]}", (NativeObject,), attrs
        )


install_native_properties()


__all__ = ["NativeObject", "NativeMember", "wrap", "unwrap", "install_native_properties"]
