"""Small ordinary Python bootstrap used by the native Pon loader.

The loader deliberately uses the authoring API's validated exporter instead of
parsing source text. It returns only plain dictionaries for Rust metadata and
retains bound methods in a separate callback table owned by the Pon module.
"""

from typing import Any
import inspect

from fighter.api import Fighter, Move, MoveContext, export_definition
from fighter.registry import validate_fighter
from ._native import install_native_properties, unwrap, wrap


def discover(namespace: dict[str, Any]) -> type[Fighter]:
    """Find the one registered Fighter class in an executed module."""
    module_name = namespace.get("__name__")
    candidates = [
        value
        for value in namespace.values()
        if isinstance(value, type) and issubclass(value, Fighter)
        and value is not Fighter and not value.__dict__.get("__abstract__", False)
        and getattr(value, "__module__", None) == module_name
    ]
    if len(candidates) == 1:
        validate_fighter(candidates[0])
        return candidates[0]
    raise ValueError("module must export exactly one Fighter subclass")


def export(namespace: dict[str, Any]) -> dict[str, Any]:
    """Export validated metadata and stable callback names from a module."""
    fighter = discover(namespace)
    definition = export_definition(fighter)
    plain = definition.as_dict()
    state_fields = tuple(plain.get("state", {}).keys())
    action_state_fields = tuple(plain.get("action_state", {}).keys())
    install_native_properties(state_fields, action_state_fields)
    callbacks: dict[str, Any] = {}
    moves: list[Move] = []
    identity: dict[int, str] = {}
    for _, move in definition.moves:
        key = identity.setdefault(id(move), f"move_{len(identity)}")
        if key == f"move_{len(moves)}":
            moves.append(move)
        run = getattr(move, "run", None)
        if (callable(run) and type(move).run is not Move.run
                and inspect.iscoroutinefunction(type(move).run)):
            callbacks.setdefault(f"{key}.run", run)
        for event in move.events():
            callback = getattr(move, event.callback, None)
            if callable(callback):
                callbacks.setdefault(f"{key}.{event.callback}", callback)
        # Validators are declarations in the exported behavior table rather
        # than event bindings, so retain them explicitly in the same stable
        # callback table. Rust resolves the exported name during registration
        # and can therefore fail closed if a callback is ever lost.
        for name in dir(move):
            callback = getattr(move, name, None)
            if callable(callback) and getattr(callback, "__fighter_validate__", False):
                callbacks.setdefault(f"{key}.{name}", callback)
    fighter_instance = fighter()
    for event in definition.callbacks:
        callback = getattr(fighter_instance, event.callback, None)
        if callable(callback):
            callbacks.setdefault(event.callback, callback)
    for behavior in plain.get("behaviors", ()):  # verify metadata/table parity at load
        name = behavior.get("validate")
        if name is not None and name not in callbacks:
            raise ValueError(f"validator {name!r} is not exported")
    # Keep the callable objects in registration order. The native loader uses
    # this immutable order to bind callback slots once per prepared thread;
    # dispatch therefore never has to perform a gameplay name lookup.
    return {
        "definition": _plain(plain),
        "callback_names": tuple(callbacks.keys()),
        "callbacks": tuple(callbacks.values()),
        "moves": tuple(moves),
    }


def dispatch(bundle: dict[str, Any], index: int, args: tuple[Any, ...]) -> Any:
    """Invoke one retained bound callback by its prepared slot."""
    callbacks = bundle["callbacks"]
    if not isinstance(index, int) or index < 0 or index >= len(callbacks):
        raise ValueError(f"callback slot {index!r} is not exported")
    callback = callbacks[index]
    wrapped = [wrap(value) for value in args]
    return unwrap(callback(*wrapped))


def move_args(
    bundle: dict[str, Any],
    behavior_index: int,
    fighter_descriptor: Any,
    action_descriptor: Any,
) -> tuple[Any, MoveContext]:
    """Build a retained move instance and fresh host context for one step."""
    moves = bundle["moves"]
    if not isinstance(behavior_index, int) or behavior_index < 0 or behavior_index >= len(moves):
        raise ValueError(f"move index {behavior_index!r} is not exported")
    return moves[behavior_index], MoveContext(
        fighter=wrap(fighter_descriptor), action=wrap(action_descriptor)
    )


def _plain(value: Any) -> Any:
    """Remove authoring scalar wrappers from the metadata wire format."""
    value = unwrap(value)
    if isinstance(value, dict):
        return {key: _plain(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_plain(item) for item in value]
    if isinstance(value, float):
        return float(value)
    return value
