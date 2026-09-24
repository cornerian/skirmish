"""Small shared motion declarations for Pikachu and Pichu.

The two fighters use the same special-motion tables in ``ftPikachu``.  This
module captures only the common input and surface/animation transitions.  The
neutral special intentionally has no article callback: the Thunder Jolt and
Thunder item archives are not part of the authoring data set yet.
"""

from __future__ import annotations

from typing import Any, ClassVar

from .actions import Action
from .api import MoveContext, SpecialMoves
from .compat import Button, source_phase
from .events import on
from .helpers import (
    directional_b_reserved,
    fresh_b_input,
    special_rules,
    start_action,
)
from .standard import DownSpecial, NeutralSpecial, SideSpecial, UpSpecial
from .transitions import SpecialMove, Transition


def _threshold(ctx: MoveContext, axis: int, value: float, direction: int = 0) -> bool:
    """Match one directional B region, leaving missing rules permissive."""
    if not fresh_b_input(ctx):
        return False
    rules = special_rules(ctx)
    limit = (
        getattr(rules, "side_stick_threshold", None)
        if axis == 0
        else getattr(rules, "vertical_threshold", None)
    ) if rules is not None else None
    if limit is None:
        return True
    if direction > 0:
        return value >= limit
    if direction < 0:
        return value <= -limit
    return abs(value) >= limit


class ElectricNeutralSpecial(NeutralSpecial):
    """Native neutral phases without inventing a projectile/article API."""

    ground: ClassVar[Any]
    air: ClassVar[Any]
    ground_state: ClassVar[int] = 341
    air_state: ClassVar[int] = 342
    _ACTIVE: ClassVar[tuple[Any, Any]]

    def __init_subclass__(cls, **kwargs: Any) -> None:
        if hasattr(cls, "ground") and hasattr(cls, "air"):
            cls._ACTIVE = (cls.ground, cls.air)
            cls.on_ground = {cls.air: Transition(cls.ground, preserve_state=True, keep_frame=True)}
            cls.on_air = {cls.ground: Transition(cls.air, preserve_state=True, keep_frame=True)}
            cls.on_end = {cls.ground: Transition(Action.WAIT), cls.air: Transition(Action.FALL)}
        super().__init_subclass__(**kwargs)

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: MoveContext) -> bool:
        if fighter.action in self._ACTIVE:
            return True
        lookup = getattr(ctx, "resource", None)
        if lookup is None or lookup(self.resource) is None:
            return False
        if not fresh_b_input(ctx) or directional_b_reserved(ctx):
            return False
        if not (ctx.ground_open or ctx.air_open):
            return False
        grounded = bool(ctx.ground_open)
        phase, state = (self.ground, self.ground_state) if grounded else (self.air, self.air_state)
        if not fighter.has_complete_animation(state):
            return False
        start_action(fighter, phase)
        return True


class _ElectricEntry(SpecialMove):
    """Shared complete-animation entry and ground/air phase pairing."""

    ground: ClassVar[Any]
    air: ClassVar[Any]
    ground_state: ClassVar[int]
    air_state: ClassVar[int]
    _ACTIVE: ClassVar[tuple[Any, Any]]
    _direction: ClassVar[int]
    _axis: ClassVar[int]

    def __init_subclass__(cls, **kwargs: Any) -> None:
        if hasattr(cls, "ground") and hasattr(cls, "air"):
            cls._ACTIVE = (cls.ground, cls.air)
            cls.on_ground = {cls.air: Transition(cls.ground, preserve_state=True, keep_frame=True)}
            cls.on_air = {cls.ground: Transition(cls.air, preserve_state=True, keep_frame=True)}
        super().__init_subclass__(**kwargs)

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: MoveContext) -> bool:
        if fighter.action in self._ACTIVE:
            return True
        if not (ctx.ground_open or ctx.air_open):
            return False
        lookup = getattr(ctx, "resource", None)
        if lookup is None or lookup(self.resource) is None:
            return False
        stick = getattr(ctx.input, "stick", (0.0, 0.0))
        if not _threshold(ctx, self._axis, stick[self._axis], self._direction):
            return False
        grounded = bool(ctx.ground_open)
        phase, state = (self.ground, self.ground_state) if grounded else (self.air, self.air_state)
        if not fighter.has_complete_animation(state):
            return False
        start_action(fighter, phase)
        return True


class ElectricSideSpecial(SideSpecial, _ElectricEntry):
    """Quick Attack's shared directional entry and phase pairing."""

    _direction = 0
    _axis = 0
    ground_start: ClassVar[Any]
    ground_hold: ClassVar[Any]
    ground_dash: ClassVar[Any]
    ground_travel: ClassVar[Any]
    ground_end: ClassVar[Any]
    air_start: ClassVar[Any]
    air_hold: ClassVar[Any]
    air_dash: ClassVar[Any]
    air_travel: ClassVar[Any]
    air_end: ClassVar[Any]

    def __init_subclass__(cls, **kwargs: Any) -> None:
        names = ("ground_start", "ground_hold", "ground_dash", "ground_travel", "ground_end",
                 "air_start", "air_hold", "air_dash", "air_travel", "air_end")
        if all(hasattr(cls, name) for name in names):
            cls.ground, cls.air = cls.ground_start, cls.air_start
            cls.ground_state, cls.air_state = 343, 348
            cls._ACTIVE = (cls.ground_start, cls.ground_hold, cls.ground_dash,
                           cls.ground_travel, cls.ground_end, cls.air_start,
                           cls.air_hold, cls.air_dash, cls.air_travel, cls.air_end)
            cls.on_ground = {
                cls.air_start: Transition(cls.ground_start, preserve_state=True, keep_frame=True),
                cls.air_hold: Transition(cls.ground_hold, preserve_state=True, keep_frame=True),
                cls.air_dash: Transition(cls.ground_dash, preserve_state=True, keep_frame=True),
                cls.air_travel: Transition(cls.ground_travel, preserve_state=True, keep_frame=True),
                cls.air_end: Transition(cls.ground_end, preserve_state=True, keep_frame=True),
            }
            cls.on_air = {
                cls.ground_start: Transition(cls.air_start, preserve_state=True, keep_frame=True),
                cls.ground_hold: Transition(cls.air_hold, preserve_state=True, keep_frame=True),
                cls.ground_dash: Transition(cls.air_dash, preserve_state=True, keep_frame=True),
                cls.ground_travel: Transition(cls.air_travel, preserve_state=True, keep_frame=True),
                cls.ground_end: Transition(cls.air_end, preserve_state=True, keep_frame=True),
            }
            cls.on_end = {
                cls.ground_start: Transition(cls.ground_hold),
                cls.air_start: Transition(cls.air_hold),
                cls.ground_end: Transition(Action.WAIT),
                cls.air_end: Transition(Action.FALL),
            }
        super().__init_subclass__(**kwargs)

    @on.input_released(Button.B)
    def release(self, fighter: Any, ctx: MoveContext) -> bool:
        if fighter.action == self.ground_hold:
            fighter.change_action(self.ground_dash)
            return True
        if fighter.action == self.air_hold:
            fighter.change_action(self.air_dash)
            return True
        return False


class ElectricUpSpecial(UpSpecial, _ElectricEntry):
    """Quick Attack's source up-start phases and terminal states."""

    _direction = 1
    _axis = 1
    ground_start: ClassVar[Any]
    ground_move: ClassVar[Any]
    ground_end: ClassVar[Any]
    air_start: ClassVar[Any]
    air_move: ClassVar[Any]
    air_end: ClassVar[Any]

    def __init_subclass__(cls, **kwargs: Any) -> None:
        if all(hasattr(cls, name) for name in (
            "ground_start", "ground_move", "ground_end", "air_start", "air_move", "air_end"
        )):
            cls.ground, cls.air = cls.ground_start, cls.air_start
            cls.ground_state, cls.air_state = 353, 356
            cls._ACTIVE = (cls.ground_start, cls.ground_move, cls.ground_end,
                           cls.air_start, cls.air_move, cls.air_end)
            cls.on_ground = {
                cls.air_start: Transition(cls.ground_start, preserve_state=True, keep_frame=True),
                cls.air_move: Transition(cls.ground_move, preserve_state=True, keep_frame=True),
                cls.air_end: Transition(cls.ground_end, preserve_state=True, keep_frame=True),
            }
            cls.on_air = {
                cls.ground_start: Transition(cls.air_start, preserve_state=True, keep_frame=True),
                cls.ground_move: Transition(cls.air_move, preserve_state=True, keep_frame=True),
                cls.ground_end: Transition(cls.air_end, preserve_state=True, keep_frame=True),
            }
            cls.on_end = {
                cls.ground_start: Transition(cls.ground_move),
                cls.air_start: Transition(cls.air_move),
                cls.ground_end: Transition(Action.WAIT),
                cls.air_end: Transition(Action.FALL),
            }
        super().__init_subclass__(**kwargs)


class ElectricDownSpecial(DownSpecial, _ElectricEntry):
    """Skull Bash's source phases and Thunder-contact transition.

    ``ftPk_SpecialLwLoop{0,1}_Anim`` enters the matching hit phase when the
    Thunder article reaches the fighter.  The contact hook is deliberately
    limited to those loop phases; contact during start, hit, or end is not a
    native transition.
    """

    _direction = -1
    _axis = 1
    ground_start: ClassVar[Any]
    ground_loop: ClassVar[Any]
    ground_hit: ClassVar[Any]
    ground_end: ClassVar[Any]
    air_start: ClassVar[Any]
    air_loop: ClassVar[Any]
    air_hit: ClassVar[Any]
    air_end: ClassVar[Any]

    @on.projectile_contact
    def projectile_contact(self, fighter: Any, ctx: MoveContext) -> bool:
        destination = {
            self.ground_loop: self.ground_hit,
            self.air_loop: self.air_hit,
        }.get(fighter.action)
        if destination is None:
            return False
        fighter.change_action(destination)
        return True

    @on.command_changed(0)
    def command_changed(self, fighter: Any, ctx: MoveContext) -> bool:
        """Exit Thunder loop or hit phases when native command 0 is set."""
        event = getattr(ctx, "event", None)
        if not getattr(event, "value", 0):
            return False
        destination = {
            self.ground_loop: self.ground_end,
            self.ground_hit: self.ground_end,
            self.air_loop: self.air_end,
            self.air_hit: self.air_end,
        }.get(fighter.action)
        if destination is None:
            # Roster binding qualifies shared source actions per fighter.
            action = getattr(fighter.action, "action", fighter.action)
            state = getattr(action, "slippi_state", None)
            if state is None and isinstance(action, str) and ":" in action:
                try:
                    state = int(action.rsplit(":", 1)[1])
                except ValueError:
                    state = None
            destination = {
                360: self.ground_end,
                361: self.ground_end,
                364: self.air_end,
                365: self.air_end,
            }.get(state)
        if destination is None:
            return False
        fighter.change_action(destination)
        return True

    def __init_subclass__(cls, **kwargs: Any) -> None:
        names = ("ground_start", "ground_loop", "ground_hit", "ground_end",
                 "air_start", "air_loop", "air_hit", "air_end")
        if all(hasattr(cls, name) for name in names):
            cls.ground, cls.air = cls.ground_start, cls.air_start
            cls.ground_state, cls.air_state = 359, 363
            cls._ACTIVE = (cls.ground_start, cls.ground_loop, cls.ground_hit,
                           cls.ground_end, cls.air_start, cls.air_loop,
                           cls.air_hit, cls.air_end)
            cls.on_ground = {
                cls.air_start: Transition(cls.ground_start, preserve_state=True, keep_frame=True),
                cls.air_loop: Transition(cls.ground_loop, preserve_state=True, keep_frame=True),
                cls.air_hit: Transition(cls.ground_hit, preserve_state=True, keep_frame=True),
                cls.air_end: Transition(cls.ground_end, preserve_state=True, keep_frame=True),
            }
            cls.on_air = {
                cls.ground_start: Transition(cls.air_start, preserve_state=True, keep_frame=True),
                cls.ground_loop: Transition(cls.air_loop, preserve_state=True, keep_frame=True),
                cls.ground_hit: Transition(cls.air_hit, preserve_state=True, keep_frame=True),
                cls.ground_end: Transition(cls.air_end, preserve_state=True, keep_frame=True),
            }
            cls.on_end = {
                cls.ground_start: Transition(cls.ground_loop),
                cls.air_start: Transition(cls.air_loop),
                cls.ground_end: Transition(Action.WAIT),
                cls.air_end: Transition(Action.FALL),
            }
        super().__init_subclass__(**kwargs)


# Pikachu and Pichu share one native special table.  Keep the source phases
# here so the fighter modules only identify the concrete class and select the
# ready moveset; identity binding still happens per fighter during export.
class ElectricThunderJolt(ElectricNeutralSpecial):
    ground = source_phase(341)
    air = source_phase(342)


class ElectricQuickAttack(ElectricSideSpecial):
    ground_start = source_phase(343)
    ground_hold = source_phase(344)
    ground_travel = source_phase(345)
    ground_end = source_phase(346)
    ground_dash = source_phase(347)
    air_start = source_phase(348)
    air_hold = source_phase(349)
    air_travel = source_phase(350)
    air_end = source_phase(351)
    air_dash = source_phase(352)


class ElectricAgility(ElectricUpSpecial):
    ground_start = source_phase(353)
    ground_move = source_phase(354)
    ground_end = source_phase(355)
    air_start = source_phase(356)
    air_move = source_phase(357)
    air_end = source_phase(358)


class ElectricThunder(ElectricDownSpecial):
    ground_start = source_phase(359)
    ground_loop = source_phase(360)
    ground_hit = source_phase(361)
    ground_end = source_phase(362)
    air_start = source_phase(363)
    air_loop = source_phase(364)
    air_hit = source_phase(365)
    air_end = source_phase(366)


ELECTRIC_SPECIALS = SpecialMoves(
    neutral=ElectricThunderJolt(),
    side=ElectricQuickAttack(),
    up=ElectricAgility(),
    down=ElectricThunder(),
)


__all__ = [
    "ElectricNeutralSpecial", "ElectricSideSpecial", "ElectricUpSpecial", "ElectricDownSpecial",
    "ElectricThunderJolt", "ElectricQuickAttack", "ElectricAgility", "ElectricThunder",
    "ELECTRIC_SPECIALS",
]
