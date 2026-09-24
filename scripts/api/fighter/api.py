"""Immutable moveset descriptors and definition export."""

from dataclasses import dataclass, field, fields, is_dataclass, replace as dataclass_replace
import inspect
import sys
from pathlib import Path
from types import MappingProxyType
from typing import TYPE_CHECKING, Any, ClassVar
from enum import IntEnum

from .actions import Action
from .events import EventBinding, _action_name
from .roster import Roster, roster_for_module

if TYPE_CHECKING:
    from .compat import ActionDescriptor


class MoveError(ValueError):
    """Raised when a fighter authoring contract is incomplete or invalid."""


class FighterPart(IntEnum):
    """Native fighter-part ordinals used by pose-dependent authoring helpers.

    The numeric value is the source ``FtPart`` ordinal.  Unknown ordinals are
    intentionally accepted by the native host, so this stays a small useful
    vocabulary instead of becoming a mirror of every character skeleton.
    """

    L1ST_NB = 23


@dataclass(frozen=True, slots=True)
class MoveContext:
    """Per invocation context supplied by the host; moves do not own state."""
    fighter: Any
    action: Any

    def wait(self, frames: int) -> Any:
        """Return a finite native await request for the current action."""
        if isinstance(frames, bool) or not isinstance(frames, int) or frames < 0:
            raise ValueError("wait frames must be a non-negative integer")
        return MoveWait(frames)


@dataclass(frozen=True, slots=True)
class MoveWait:
    """Serializable declaration consumed by the native deadline scheduler."""
    frames: int

    def as_dict(self) -> dict[str, int | str]:
        return {"kind": "scheduled_deadline", "frames": self.frames}

    def __await__(self):
        # Pon treats the yielded declaration as the native await token. The
        # resumed value is deliberately ignored: Move.run continues with the
        # host-owned action/context rebound by the scoped bridge.
        yield self
        return None


class Move:
    """Reusable move code and immutable metadata."""
    __slots__ = ()
    resource: ClassVar[str | None] = None
    # Optional canonical engine action reference.  A move may represent more
    # than one action, in which case authors should expose those actions from
    # the fighter's explicit action table instead of relying on this field.
    action: ClassVar["str | ActionDescriptor | None"] = None

    async def run(self, action: MoveContext) -> Any:
        raise NotImplementedError(f"{type(self).__name__}.run must be implemented")

    @classmethod
    def action_name(cls) -> str:
        return cls.__name__

    @classmethod
    def events(cls) -> tuple[EventBinding, ...]:
        result: list[EventBinding] = []
        for name in dir(cls):
            value = getattr(cls, name)
            result.extend(getattr(value, "__fighter_events__", ()))
        return tuple(result)


@dataclass(frozen=True, slots=True, eq=False)
class ActionMove(Move):
    """A declarative move bound directly to one engine action.

    The action may be an :class:`ActionDescriptor` (preserving its metadata)
    or the canonical string form accepted by :attr:`Move.action`.  ActionMove
    deliberately has no behavior of its own; the host handles the action and
    the inherited ``Move.run`` remains unavailable for accidental execution.
    """

    # ``field()`` is intentional: ``Move.action`` is a ClassVar with a None
    # default, but each ActionMove must be constructed with an action.
    action: "str | Action | ActionDescriptor" = field()
    resource: str | None = None


@dataclass(frozen=True, slots=True)
class SpecialMoves:
    neutral: Move
    side: Move
    up: Move
    down: Move

    def replace(self, **changes: Move) -> "SpecialMoves":
        """Return this moveset with selected move slots replaced.

        The helper keeps fighter declarations focused on the one special they
        customize while retaining the immutable defaults supplied by
        :class:`Fighter`.  It deliberately accepts only dataclass fields and
        actual ``Move`` instances, so a typo cannot silently become a new
        declaration or a runtime failure.
        """
        names = {item.name for item in fields(type(self))}
        unknown = set(changes) - names
        if unknown:
            raise TypeError(
                f"unknown special move field(s): {', '.join(sorted(unknown))}"
            )
        for name, value in changes.items():
            if not isinstance(value, Move):
                raise TypeError(f"special move {name!r} must be a Move instance")
        return dataclass_replace(self, **changes)


@dataclass(frozen=True, slots=True)
class AerialMoves:
    neutral: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_AIR_N))
    forward: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_AIR_F))
    back: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_AIR_B))
    up: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_AIR_HI))
    down: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_AIR_LW))


@dataclass(frozen=True, slots=True)
class GroundedMoves:
    jab: Move = field(default_factory=lambda: ActionMove(Action.JAB))
    rapid_jab: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_100_LOOP))
    dash: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_DASH))

@dataclass(frozen=True, slots=True)
class TiltMoves:
    forward: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_S3_S))
    up: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_HI3))
    down: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_LW3))

@dataclass(frozen=True, slots=True)
class SmashMoves:
    forward: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_S4_S))
    up: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_HI4))
    down: Move = field(default_factory=lambda: ActionMove(Action.ATTACK_LW4))


@dataclass(frozen=True, slots=True)
class GrabMoves:
    standing: Move = field(default_factory=lambda: ActionMove(Action.CATCH))
    dash: Move = field(default_factory=lambda: ActionMove(Action.CATCH_DASH))
    pummel: Move = field(default_factory=lambda: ActionMove(Action.CATCH_ATTACK))


@dataclass(frozen=True, slots=True)
class ThrowMoves:
    forward: Move = field(default_factory=lambda: ActionMove(Action.THROW_F))
    back: Move = field(default_factory=lambda: ActionMove(Action.THROW_B))
    up: Move = field(default_factory=lambda: ActionMove(Action.THROW_HI))
    down: Move = field(default_factory=lambda: ActionMove(Action.THROW_LW))


@dataclass(frozen=True, slots=True)
class DefenseMoves:
    shield: Move = field(default_factory=lambda: ActionMove(Action.GUARD))
    spot_dodge: Move = field(default_factory=lambda: ActionMove(Action.ESCAPE_N))
    roll_forward: Move = field(default_factory=lambda: ActionMove(Action.ESCAPE_F))
    roll_back: Move = field(default_factory=lambda: ActionMove(Action.ESCAPE_B))
    air_dodge: Move = field(default_factory=lambda: ActionMove(Action.ESCAPE_AIR))


@dataclass(frozen=True, slots=True)
class LedgeMoves:
    wait: Move = field(default_factory=lambda: ActionMove(Action.CLIFF_WAIT))
    getup: Move = field(default_factory=lambda: ActionMove(Action.CLIFF_CLIMB))
    roll: Move = field(default_factory=lambda: ActionMove(Action.CLIFF_ESCAPE))
    attack: Move = field(default_factory=lambda: ActionMove(Action.CLIFF_ATTACK))
    jump: Move = field(default_factory=lambda: ActionMove(Action.CLIFF_JUMP))


@dataclass(frozen=True, slots=True)
class GetupMoves:
    neutral: Move = field(default_factory=lambda: ActionMove(Action.DOWN_STAND))
    roll_forward: Move = field(default_factory=lambda: ActionMove(Action.DOWN_FORWARD))
    roll_back: Move = field(default_factory=lambda: ActionMove(Action.DOWN_BACK))
    attack: Move = field(default_factory=lambda: ActionMove(Action.DOWN_ATTACK))


@dataclass(frozen=True, slots=True)
class TauntMoves:
    taunt: Move = field(default_factory=lambda: ActionMove(Action.APPEAL_SR))


@dataclass(frozen=True, slots=True)
class Attributes:
    """Explicit named fighter attributes; subclasses may add frozen fields."""
    @classmethod
    def as_dict(cls, value: Any) -> dict[str, Any]:
        if not is_dataclass(value):
            return dict(vars(value))
        return {field.name: getattr(value, field.name) for field in fields(value)}


@dataclass(frozen=True, slots=True)
class FighterDefinition:
    name: str
    attributes: MappingProxyType
    specials: MappingProxyType
    aerials: MappingProxyType
    grounded: MappingProxyType
    tilts: MappingProxyType
    smashes: MappingProxyType
    grabs: MappingProxyType
    throws: MappingProxyType
    defense: MappingProxyType
    ledge: MappingProxyType
    getup: MappingProxyType
    taunt: MappingProxyType
    moves: tuple[tuple[str, Move], ...]
    callbacks: tuple[EventBinding, ...]
    external_ids: tuple[int, ...] = ()
    state: Any = field(default_factory=dict)
    action_state: Any = field(default_factory=dict)
    behaviors: tuple[Any, ...] = ()
    _root_module: str | None = field(default=None, repr=False, compare=False)

    def as_dict(self) -> dict[str, Any]:
        identity: dict[int, str] = {}
        action_records: dict[str, dict[str, Any]] = {}
        behaviors: list[dict[str, Any]] = []
        for name, move in self.moves:
            key = identity.setdefault(id(move), f"move_{len(identity)}")
            if not any(item["id"] == key for item in behaviors):
                move_callbacks = [event.as_dict() for event in move.events()]
                move_actions: dict[str, Any] = {}
                for phase_name in dir(move):
                    phase = getattr(move, phase_name)
                    if hasattr(phase, "as_dict") and hasattr(phase, "action"):
                        move_actions[phase_name] = phase.as_dict()
                clock_records = []
                for attribute_name in dir(move):
                    clock = getattr(move, attribute_name)
                    if type(clock).__name__ == "ClockBinding" and hasattr(clock, "as_dict"):
                        record = clock.as_dict()
                        record["actions"] = [_action_name(action) for action in clock.actions]
                        clock_records.append(record)
                behaviors.append({"id": key, "resource": move.resource,
                                  "entry_action": _action_name(move.action) if move.action is not None else None,
                                  "run": (type(move).run.__qualname__
                                          if type(move).run is not Move.run
                                          and inspect.iscoroutinefunction(type(move).run)
                                          else None),
                                  "run_module": ("__root__"
                                                  if type(move).run is not Move.run
                                                  and inspect.iscoroutinefunction(type(move).run)
                                                  and type(move).run.__module__ == self._root_module
                                                  else type(move).run.__module__
                                                  if type(move).run is not Move.run
                                                  and inspect.iscoroutinefunction(type(move).run)
                                                  else None),
                                  "actions": move_actions,
                                  "callbacks": [{**event, "callback": f"{key}.{event['callback']}"} for event in move_callbacks],
                                  "clocks": clock_records,
                                  "validate": next((f"{key}.{name}" for name in dir(move)
                                                    if getattr(getattr(move, name), "__fighter_validate__", False)), None),
                                  })
            behavior = next(item for item in behaviors if item["id"] == key)
            for phase_name, phase in behavior["actions"].items():
                flattened = dict(phase)
                flattened["source_behavior"] = key
                action_records[f"{name}.{phase_name}"] = flattened
            if move.action is not None:
                if hasattr(move.action, "as_dict"):
                    record = dict(move.action.as_dict())
                else:
                    record = {"action": _action_name(move.action)}
                # Keep owner identity on the flattened action record so the
                # native linker can retain distinct resources for moves that
                # share one canonical engine action.
                record["source_behavior"] = key
                action_records[name] = record
        def declaration(value: Any) -> Any:
            if value is None:
                return {}
            if isinstance(value, type):
                annotations: dict[str, Any] = {}
                defaults: dict[str, Any] = {}
                for base in reversed(value.__mro__):
                    annotations.update(getattr(base, "__annotations__", {}))
                    defaults.update({key: item for key, item in base.__dict__.items() if key in annotations})
                return {key: defaults[key] for key in annotations if key in defaults}
            if hasattr(value, "as_dict"):
                return value.as_dict()
            return dict(value) if isinstance(value, MappingProxyType) else value
        exported = {
            "name": self.name,
            "parameters": dict(self.attributes),
            # These keys are consumed by the native FighterDefinition
            # decoder. Empty declarations are intentional and keep the
            # authoring wire format stable for fighters without persistent
            # script state or behavior groups.
            "external_ids": list(getattr(self, "external_ids", ())),
            "state": declaration(getattr(self, "state", None)),
            "action_state": declaration(getattr(self, "action_state", None)),
            "actions": action_records,
            "callbacks": [event.as_dict() for event in self.callbacks],
            "movesets": {"specials": dict(self.specials), "aerials": dict(self.aerials),
                         "grounded": dict(self.grounded), "tilts": dict(self.tilts),
                         "smashes": dict(self.smashes), "grabs": dict(self.grabs),
                         "throws": dict(self.throws), "defense": dict(self.defense),
                         "ledge": dict(self.ledge), "getup": dict(self.getup),
                         "taunt": dict(self.taunt)},
            "behaviors": behaviors,
        }
        from .compat import resolve_source_action
        from .compat import SourceAction

        def resolve(value: Any) -> Any:
            if isinstance(value, SourceAction):
                return resolve_source_action(value.reference, self.external_ids)
            if isinstance(value, str):
                if value.startswith("Action."):
                    prefix = "Action."
                    return prefix + resolve_source_action(value[len(prefix):], self.external_ids)
                return resolve_source_action(value, self.external_ids)
            if isinstance(value, tuple):
                return tuple(resolve(item) for item in value)
            if isinstance(value, list):
                return [resolve(item) for item in value]
            if isinstance(value, dict):
                return {key: resolve(item) for key, item in value.items()}
            if hasattr(value, "as_dict"):
                return resolve(value.as_dict())
            return value

        return resolve(exported)


class FighterMeta(type):
    def __new__(mcls, name: str, bases: tuple[type, ...], namespace: dict[str, Any]):
        cls = super().__new__(mcls, name, bases, namespace)
        if namespace.get("__abstract__", False) or not any(isinstance(base, FighterMeta) for base in bases):
            return cls
        required = ("attributes", "specials", "aerials", "grounded", "tilts", "smashes",
                    "grabs", "throws", "defense", "ledge", "getup", "taunt")
        # A concrete subclass may specialize its identity or parameters while
        # inheriting a complete declaration from another fighter.
        missing = [field for field in required if not hasattr(cls, field)]
        if missing:
            raise MoveError(f"concrete Fighter {name!r} must define: {', '.join(missing)}")
        # Keep discovery unambiguous: the abstract marker is intentionally
        # explicit on every validated concrete declaration.
        cls.__abstract__ = False
        return cls


class Fighter(metaclass=FighterMeta):
    """Authoring base with the standard instantiated move groups.

    A roster module only needs to declare ``class Mario(Fighter)`` and
    specialize the groups it implements.  The loader derives identity from
    that module's filename, so normal scripts contain no registration or
    duplicate roster metadata.
    """
    __slots__ = ()
    __abstract__ = True
    name: ClassVar[str]
    attributes: ClassVar[Any]
    specials: ClassVar[SpecialMoves]
    aerials: ClassVar[AerialMoves]
    grounded: ClassVar[GroundedMoves]
    tilts: ClassVar[TiltMoves]
    smashes: ClassVar[SmashMoves]
    grabs: ClassVar[GrabMoves]
    throws: ClassVar[ThrowMoves]
    defense: ClassVar[DefenseMoves]
    ledge: ClassVar[LedgeMoves]
    getup: ClassVar[GetupMoves]
    taunt: ClassVar[TauntMoves]


class FighterBase(metaclass=FighterMeta):
    """Strict declaration-only compatibility base.

    New fighter modules should inherit :class:`Fighter`.  This type remains
    available for external definitions that intentionally provide every move
    group themselves.
    """
    __abstract__ = True
    name: ClassVar[str]
    attributes: ClassVar[Any]
    specials: ClassVar[SpecialMoves]
    aerials: ClassVar[AerialMoves]
    grounded: ClassVar[GroundedMoves]
    tilts: ClassVar[TiltMoves]
    smashes: ClassVar[SmashMoves]
    grabs: ClassVar[GrabMoves]
    throws: ClassVar[ThrowMoves]
    defense: ClassVar[DefenseMoves]
    ledge: ClassVar[LedgeMoves]
    getup: ClassVar[GetupMoves]
    taunt: ClassVar[TauntMoves]



def _validate_group(group: Any, names: tuple[str, ...], label: str, expected: type[Any] | None = None) -> None:
    if expected is not None and not isinstance(group, expected):
        raise MoveError(f"{label} must be an explicit {label} value")
    for name in names:
        move = getattr(group, name, None)
        if not isinstance(move, Move):
            raise MoveError(f"{label}.{name} must be a Move instance")


def export_definition(fighter: type[Fighter]) -> FighterDefinition:
    from .registry import validate_fighter
    resolve_identity(fighter)
    validate_fighter(fighter)
    special_names = tuple(field.name for field in fields(fighter.specials))
    aerial_names = tuple(field.name for field in fields(fighter.aerials))
    group_specs = {
        group: tuple(field.name for field in fields(getattr(fighter, group)))
        for group in ("grounded", "tilts", "smashes", "grabs", "throws", "defense", "ledge", "getup", "taunt")
    }
    moves: list[tuple[str, Move]] = []
    for name in special_names:
        move = getattr(fighter.specials, name)
        moves.append((f"special.{name}", move))
    for name in aerial_names:
        move = getattr(fighter.aerials, name)
        moves.append((f"aerial.{name}", move))
    for group, names in group_specs.items():
        for name in names:
            move = getattr(getattr(fighter, group), name)
            moves.append((f"{group}.{name}", move))
    identity: dict[int, str] = {}
    for _, move in moves:
        identity.setdefault(id(move), f"move_{len(identity)}")
    specials = {name: identity[id(getattr(fighter.specials, name))] for name in special_names}
    aerials = {name: identity[id(getattr(fighter.aerials, name))] for name in aerial_names}
    group_exports = {
        group: {field.name: identity[id(getattr(getattr(fighter, group), field.name))]
                for field in fields(getattr(fighter, group))}
        for group in group_specs
    }
    seen: set[int] = set()
    callback_list: list[EventBinding] = []
    for _, move in moves:
        if id(move) in seen:
            continue
        seen.add(id(move))
        callback_list.extend(move.events())
    callbacks = tuple(callback_list)
    attrs = Attributes.as_dict(fighter.attributes() if isinstance(fighter.attributes, type) else fighter.attributes)
    fighter_callbacks: list[EventBinding] = []
    for member_name in dir(fighter):
        fighter_callbacks.extend(getattr(getattr(fighter, member_name), "__fighter_events__", ()))
    return FighterDefinition(
        name=fighter.name, attributes=MappingProxyType(attrs),
        specials=MappingProxyType(specials), aerials=MappingProxyType(aerials),
        **{group: MappingProxyType(group_exports[group]) for group in group_specs},
        moves=tuple(moves), callbacks=tuple(fighter_callbacks),
        external_ids=tuple(getattr(fighter, "external_ids", ())),
        state=getattr(fighter, "state", {}), action_state=getattr(fighter, "action_state", {}),
        behaviors=tuple(getattr(fighter, "behaviors", ())),
        _root_module=fighter.__module__,
    )


def resolve_identity(fighter: type[Fighter], module_name: str | None = None) -> None:
    """Fill script identity from trusted loader provenance or its module."""
    module_roster = roster_for_module(module_name) if module_name is not None else None
    if module_name is None and module_roster is None:
        module_roster = roster_for_module(getattr(fighter, "__module__", ""))
        module = sys.modules.get(getattr(fighter, "__module__", ""))
        source_file = getattr(module, "__file__", None)
        if source_file:
            module_roster = roster_for_module(Path(source_file).stem)
    declared_name = fighter.__dict__.get("name")
    declared_ids = fighter.__dict__.get("external_ids")
    if declared_ids is not None:
        if (isinstance(declared_ids, (str, bytes))
                or not isinstance(declared_ids, (tuple, list))
                or any(isinstance(identifier, bool) or not isinstance(identifier, int)
                       for identifier in declared_ids)
                or len(set(declared_ids)) != len(declared_ids)):
            raise MoveError("fighter external_ids must contain unique integers")
    if module_roster is None:
        if declared_name is None and declared_ids is None:
            raise MoveError(
                f"fighter {fighter.__name__!r} needs an explicit name or external_ids "
                "outside a canonical roster module"
            )
        if declared_ids is not None:
            _bind_source_actions(fighter, tuple(declared_ids))
        return
    expected_name = module_roster.slug
    expected_ids = (module_roster.external_id,)
    if declared_name is not None and declared_name != expected_name:
        raise MoveError(
            f"fighter {fighter.__name__!r} module conflicts with name {declared_name!r}"
        )
    if declared_ids is not None and tuple(declared_ids) != expected_ids:
        raise MoveError(
            f"fighter {fighter.__name__!r} module conflicts with external_ids {declared_ids!r}"
        )
    fighter.name = expected_name
    fighter.external_ids = expected_ids
    _bind_source_actions(fighter, expected_ids)


def _bind_source_actions(fighter: type[Fighter], external_ids: tuple[int, ...]) -> None:
    """Bind concrete move descriptors after roster identity is known.

    A move class is reusable authoring data.  Mutating its class attributes
    here would make the first fighter that resolves it permanently own its
    source-action identity (Falco and Fox intentionally share move objects,
    for example).  Instead, source-bearing move classes are specialized per
    fighter and the groups receive matching immutable instances.  The
    authored class and its instances remain untouched for the next consumer.
    """
    from .compat import ActionDescriptor, SourceAction, bind_source_action, resolve_source_action
    from .transitions import Transition, _action_key

    def bind_value(value: Any) -> Any:
        """Bind source actions through the small containers used by moves."""
        if isinstance(value, ActionDescriptor):
            action = bind_source_action(value.action, external_ids)
            metadata = tuple((key, bind_value(item)) for key, item in value.metadata)
            if action is value.action and all(
                bound is item for (_, bound), (_, item) in zip(metadata, value.metadata)
            ):
                return value
            return ActionDescriptor(action, metadata, value.animation_loop)
        if isinstance(value, SourceAction):
            return bind_source_action(value, external_ids)
        bound = bind_source_action(value, external_ids)
        if bound is not value:
            return bound
        if isinstance(value, tuple):
            items = tuple(bind_value(item) for item in value)
            return value if all(a is b for a, b in zip(items, value)) else items
        if isinstance(value, list):
            items = [bind_value(item) for item in value]
            return value if all(a is b for a, b in zip(items, value)) else items
        if isinstance(value, dict):
            items = {bind_value(key): bind_value(item) for key, item in value.items()}
            return value if all(
                key is old_key and item is old_item
                for (key, item), (old_key, old_item) in zip(items.items(), value.items())
            ) else items
        if isinstance(value, Transition):
            target = bind_value(value.target)
            return value if target is value.target else dataclass_replace(value, target=target)
        return value

    def bind_rules(rules: tuple[Any, ...]) -> tuple[Any, ...]:
        bound_rules = []
        for rule in rules:
            source_name = resolve_source_action(rule.source_name, external_ids)
            target = bind_value(rule.transition.target)
            transition = Transition(
                target,
                preserve_state=rule.transition.preserve_state,
                keep_frame=rule.transition.keep_frame,
            )
            bound_rules.append(dataclass_replace(
                rule,
                source=_action_key(source_name),
                source_name=source_name,
                transition=transition,
            ))
        return tuple(bound_rules)

    def bound_type(cls: type[Any]) -> type[Any]:
        """Return an identity-local subclass when ``cls`` owns source data."""
        if getattr(cls, "__source_bound_ids__", None) == external_ids:
            return cls
        template = getattr(cls, "__source_template__", cls)
        original_rules = getattr(template, "__transition_rules__", ())
        rules = bind_rules(original_rules) if original_rules else ()
        namespace: dict[str, Any] = {}
        for base in reversed(template.__mro__):
            for name, value in base.__dict__.items():
                if name in {
                    "__dict__", "__weakref__", "__transition_rules__",
                    # SpecialMove derives its normalized rules from these
                    # maps during class creation.  Replaying a bound map
                    # would expose string Source.* keys to that declaration
                    # hook before the identity-local rules are installed.
                    "on_end", "on_ground", "on_air",
                }:
                    continue
                bound = bind_value(value)
                if bound is not value:
                    namespace[name] = bound
        if not namespace and rules == original_rules:
            return cls
        namespace["__module__"] = template.__module__
        namespace["__qualname__"] = f"{template.__qualname__}__bound_{external_ids[0]}"
        namespace["__source_template__"] = template
        namespace["__source_bound_ids__"] = external_ids
        # A few move bases validate required declaration fields from the
        # subclass namespace in ``__init_subclass__`` (for example article
        # moves require ``article_id`` and paired ground/air phases).  Those
        # fields must be present while the identity-local subclass is built;
        # installing them after ``type(...)`` is too late for validation.
        for required_name in ("article_id", "ground", "air"):
            if required_name in template.__dict__:
                namespace[required_name] = bind_value(template.__dict__[required_name])
        specialized = type(namespace["__qualname__"], (template,), namespace)
        for map_name in ("on_end", "on_ground", "on_air"):
            declaration = getattr(template, map_name, None)
            if isinstance(declaration, dict):
                bound_map = {}
                for source, transition in declaration.items():
                    bound_source = bind_source_action(source, external_ids)
                    if isinstance(bound_source, str) and bound_source.startswith("Source."):
                        bound_source = ActionDescriptor(bound_source)
                    target = bind_value(transition.target)
                    bound_map[bound_source] = dataclass_replace(transition, target=target)
                setattr(specialized, map_name, bound_map)
        if original_rules:
            specialized.__transition_rules__ = rules
        return specialized

    def bound_instance(move: Move) -> Move:
        cls = bound_type(type(move))
        if cls is type(move):
            return move
        specialized = object.__new__(cls)
        if hasattr(move, "__dict__"):
            specialized.__dict__.update(move.__dict__)
        for base in type(move).__mro__:
            slots = base.__dict__.get("__slots__", ())
            if isinstance(slots, str):
                slots = (slots,)
            for slot in slots:
                if slot in {"__dict__", "__weakref__"} or not hasattr(move, slot):
                    continue
                object.__setattr__(specialized, slot, getattr(move, slot))
        return specialized

    bound_moves: dict[int, Move] = {}

    for group_name in (
        "specials", "aerials", "grounded", "tilts", "smashes", "grabs",
        "throws", "defense", "ledge", "getup", "taunt",
    ):
        group = getattr(fighter, group_name, None)
        if group is None:
            continue
        replacements: dict[str, Move] = {}
        for move_field in fields(type(group)):
            move = getattr(group, move_field.name)
            move_key = id(move)
            if move_key not in bound_moves:
                bound_moves[move_key] = bound_instance(move)
            bound = bound_moves[move_key]
            if bound is not move:
                replacements[move_field.name] = bound
        if replacements:
            setattr(fighter, group_name, dataclass_replace(group, **replacements))


# Construct the shared defaults after the classes exist, so ``standard`` can
# share the Move/SpecialMove definitions without an import cycle.  Fighter is
# the ordinary public base; FighterBase deliberately remains declaration-only.
from .standard import OpenSpecial, SpecialRoot

Fighter.specials = SpecialMoves(
    OpenSpecial(SpecialRoot.NEUTRAL),
    OpenSpecial(SpecialRoot.SIDE),
    OpenSpecial(SpecialRoot.UP),
    OpenSpecial(SpecialRoot.DOWN),
)
Fighter.attributes = Attributes()
Fighter.aerials = AerialMoves()
Fighter.grounded = GroundedMoves()
Fighter.tilts = TiltMoves()
Fighter.smashes = SmashMoves()
Fighter.grabs = GrabMoves()
Fighter.throws = ThrowMoves()
Fighter.defense = DefenseMoves()
Fighter.ledge = LedgeMoves()
Fighter.getup = GetupMoves()
Fighter.taunt = TauntMoves()
