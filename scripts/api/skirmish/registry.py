"""Scoped class based fighter registration and lookup."""

from __future__ import annotations

from typing import TypeVar

from fighter.api import Fighter, MoveError
from fighter.registry import register, validate_fighter

T = TypeVar("T", bound=type[Fighter])


class FighterRegistry:
    """Identity index owned by one source bundle or loader invocation."""

    __slots__ = ("_by_name", "_by_external_id", "_ordered")

    def __init__(self) -> None:
        self._by_name: dict[str, type[Fighter]] = {}
        self._by_external_id: dict[int, type[Fighter]] = {}
        self._ordered: list[type[Fighter]] = []

    def register(self, fighter: T) -> T:
        """Validate and add ``fighter``; repeated registration is idempotent."""
        validate_fighter(fighter)
        name = fighter.__dict__.get("name")
        if not isinstance(name, str) or not name:
            raise MoveError("fighter name must be a non-empty string")
        # Identity is sometimes filled by ``resolve_identity`` after source
        # discovery, so name-only declarations remain registerable.
        external_ids = tuple(getattr(fighter, "external_ids", ()))
        previous_name = self._by_name.get(name)
        if previous_name is not None and previous_name is not fighter:
            raise MoveError(f"fighter name {name!r} is already registered")
        for external_id in external_ids:
            previous_id = self._by_external_id.get(external_id)
            if previous_id is not None and previous_id is not fighter:
                raise MoveError(f"fighter external id {external_id!r} is already registered")
        if previous_name is None:
            self._ordered.append(fighter)
        self._by_name[name] = fighter
        for external_id in external_ids:
            self._by_external_id[external_id] = fighter
        return fighter

    def lookup(self, key: str | int) -> type[Fighter] | None:
        """Return a fighter by canonical name or external id."""
        if isinstance(key, str):
            return self._by_name.get(key)
        if isinstance(key, int) and not isinstance(key, bool):
            return self._by_external_id.get(key)
        raise TypeError("fighter lookup key must be a name or integer external id")

    def fighters(self) -> tuple[type[Fighter], ...]:
        """Return fighters in registration order."""
        return tuple(self._ordered)


__all__ = ["FighterRegistry", "register", "validate_fighter"]
