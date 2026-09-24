"""Small source-family behaviors shared by Captain Falcon and Ganondorf.

The two fighters use the same callbacks in the upstream source, but their
phase descriptors and ``Pl*.dat`` resources remain concrete declarations in
their fighter modules.  These bases therefore contain only callback policy;
``__init_subclass__`` derives action maps from each concrete move's phase
attributes so no Captain identity or action descriptor leaks across fighters.
"""

from __future__ import annotations

from dataclasses import replace
from typing import Any, ClassVar

from .actions import Action
from .api import MoveContext
from .compat import ActionState, Button, HitContext, motion, resource, source_action, validation
from .events import EventBinding, _action_name, on
from . import math
from .helpers import (
    directional_b_input,
    directional_b_reserved,
    resource_attributes,
    special_rules,
    start_fresh_open_special,
    start_open_special,
    start_action,
)
from .transitions import SpecialMove, Transition
from .standard import DownSpecial, NeutralSpecial, SideSpecial, UpSpecial


def _phase_events(
    events: tuple[EventBinding, ...], mapping: dict[str, tuple[str, ...]], cls: type[Any]
) -> tuple[EventBinding, ...]:
    """Replace actionless family callbacks with concrete phase references."""
    result = []
    for event in events:
        names = mapping.get(event.callback)
        if names is None:
            result.append(event)
            continue
        result.append(replace(
            event,
            actions=tuple(_action_name(getattr(cls, name)) for name in names),
        ))
    return tuple(result)


def _consume_command(fighter: Any, index: int) -> None:
    state = getattr(fighter, "action_state", None)
    command = getattr(state, "command", (0, 0, 0, 0))
    if len(command) == 4:
        values = list(command)
        values[index] = 0
        state.command = tuple(values)


class CaptainFamilyActionState(ActionState):
    """Transient state shared by Captain Falcon's and Ganondorf's specials."""

    command: tuple[int, int, int, int] = (0, 0, 0, 0)
    launch_armed: bool = False
    dive_released: bool = False


class _CaptainFamilySpecial(SpecialMove):
    _EVENT_PHASES: ClassVar[dict[str, tuple[str, ...]]] = {}

    def __init_subclass__(cls, **kwargs: Any) -> None:
        # Derive the transition tables before SpecialMove normalizes them.
        cls._derive_source_rules()
        super().__init_subclass__(**kwargs)

    @classmethod
    def events(cls) -> tuple[EventBinding, ...]:
        return _phase_events(super().events(), cls._EVENT_PHASES, cls)

    @classmethod
    def _derive_source_rules(cls) -> None:
        """Hook for each family to derive maps from concrete phase names."""


class CaptainNeutralSpecial(NeutralSpecial, _CaptainFamilySpecial):
    """Shared Falcon/Warlock Punch entry, IASA, and surface lifecycle."""

    air_motion = motion.profile(
        air=(motion.command_velocity_scale(
            index=1,
            value=1,
            multiplier=resource("neutral.attributes.specialn_vel_mul"),
        ),),
    )
    def __init_subclass__(cls, **kwargs: Any) -> None:
        if all(hasattr(cls, name) for name in ("ground", "air")):
            cls.on_end = {cls.ground: Transition(Action.WAIT), cls.air: Transition(Action.FALL)}
            cls.on_ground = {cls.air: Transition(cls.ground, preserve_state=True, keep_frame=True)}
            cls.on_air = {cls.ground: Transition(cls.air, preserve_state=True, keep_frame=True)}
            cls._EVENT_PHASES = {
                "enter": ("ground", "air"),
                "command_changed": ("air",),
                "command_velocity_scale": ("air",),
            }
        super().__init_subclass__(**kwargs)

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: MoveContext) -> bool:
        if directional_b_reserved(ctx):
            return False
        return start_fresh_open_special(
            fighter, ctx, self.resource, self.ground, self.air,
            active_actions=(self.ground, self.air),
        )

    @on.action_enter()
    def enter(self, fighter: Any, ctx: MoveContext) -> None:
        fighter.action_state.command = (0, 0, 0, 0)
        fighter.action_state.launch_armed = False

    @on.command_changed(0)
    def command_changed(self, fighter: Any, ctx: MoveContext) -> None:
        value = getattr(getattr(ctx, "event", None), "value", None)
        if fighter.action != self.air or not value:
            return
        _consume_command(fighter, 0)
        fighter.action_state.launch_armed = True
        attributes = resource_attributes(ctx, self.resource)
        fields = (
            "specialn_stick_range_y_neg", "specialn_stick_range_y_pos",
            "specialn_angle_diff", "specialn_vel_x",
        )
        if attributes is None or any(getattr(attributes, name, None) is None for name in fields):
            return
        minimum = attributes.specialn_stick_range_y_neg
        maximum = attributes.specialn_stick_range_y_pos
        if maximum <= minimum:
            return
        stick_y = min(ctx.input.stick[1], maximum)
        stick_y = max(stick_y - minimum, 0)
        if ctx.input.stick[1] < 0:
            stick_y = -stick_y
        angle = math.DEG_TO_RAD * stick_y * attributes.specialn_angle_diff / (maximum - minimum)
        fighter.velocity = math.velocity_from_angle(attributes.specialn_vel_x, angle, fighter.facing)

    @on.command_changed(1)
    def command_velocity_scale(self, fighter: Any, ctx: MoveContext) -> None:
        # ftCa_SpecialAirN_Phys branches on cmd_vars[1].  Command traces are
        # delivered to the authoring callback before the motion operation is
        # evaluated, so retain the native value in the serialized action
        # state rather than treating this callback as a no-op.  Without this,
        # the air punch always follows the ordinary-fall branch and the
        # ``motion.command_velocity_scale`` declaration can never match.
        value = getattr(getattr(ctx, "event", None), "value", None)
        if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 0xFFFFFFFF:
            return
        command = getattr(getattr(fighter, "action_state", None), "command", (0, 0, 0, 0))
        if len(command) != 4:
            return
        fighter.action_state.command = (command[0], value, command[2], command[3])

    @staticmethod
    def _valid_attributes(resource_value: Any) -> bool:
        attributes = getattr(resource_value, "attributes", None)
        names = (
            "specialn_stick_range_y_neg", "specialn_stick_range_y_pos",
            "specialn_angle_diff", "specialn_vel_x", "specialn_vel_mul",
        )
        return validation.fields(attributes, finite=names) and (
            attributes.specialn_stick_range_y_pos > attributes.specialn_stick_range_y_neg
        )

    @on.validate
    def validate(self, ctx: MoveContext) -> bool:
        resource_value = ctx.resource(self.resource)
        if resource_value is None:
            return True
        return (
            validation.command_trace(ctx, "neutral.script.ground", "neutral.ground")
            and validation.command_trace(ctx, "neutral.script.air", "neutral.air")
            and self._valid_attributes(resource_value)
        )


class CaptainSideSpecial(SideSpecial, _CaptainFamilySpecial):
    """Shared Raptor Boost/Gerudo Dragon start and follow-through policy."""

    air_motion = motion.profile(
        air=(motion.gravity(
            acceleration=resource("side.attributes.specials_grav"),
            terminal_velocity=resource("side.attributes.specials_terminal_vel"),
            delay=0,
        ),),
    )
    def __init_subclass__(cls, **kwargs: Any) -> None:
        if all(hasattr(cls, name) for name in ("ground_start", "ground", "air_start", "air")):
            cls._EVENT_PHASES = {
                "action_enter": ("ground_start", "air_start"),
                "before_hit": ("ground_start", "air_start"),
                "animation_end_ground": ("ground_start", "ground"),
                "animation_end_air": ("air_start", "air"),
                "landed": ("air_start", "air"),
            }
        super().__init_subclass__(**kwargs)

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: MoveContext) -> bool:
        if fighter.action in (self.ground_start, self.ground, self.air_start, self.air):
            return True
        if directional_b_input(ctx, self.resource, 0, "side_stick_threshold") is not True:
            return False
        if ctx.ground_open:
            start_action(fighter, self.ground_start)
            self._clear_ground_start_velocity(fighter)
        elif ctx.air_open:
            start_action(fighter, self.air_start)
            self._clear_air_start_velocity(fighter)
        else:
            return False
        return True

    @on.action_enter()
    def action_enter(self, fighter: Any, ctx: MoveContext) -> None:
        _consume_command(fighter, 0)
        _consume_command(fighter, 1)
        _consume_command(fighter, 2)
        if fighter.action == self.ground_start:
            self._clear_ground_start_velocity(fighter)
        elif fighter.action == self.air_start:
            self._clear_air_start_velocity(fighter)

    @on.before_hit()
    def before_hit(self, fighter: Any, hit: HitContext) -> None:
        if fighter.action == self.air_start:
            fighter.change_action(self.air)
            return
        if fighter.action != self.ground_start:
            return
        # The source reads Captain's special attributes from the fighter's
        # owned Pl*.dat resource.  A native HitContext is only the contact
        # event and does not expose a compatible resource lookup (and may
        # deliberately reject access), so keep this callback on the fighter
        # resource hot path.
        attributes = resource_attributes(fighter, self.resource)
        multiplier = getattr(attributes, "specials_gr_vel_x", None)
        if multiplier is None or not validation.finite(multiplier):
            return
        fighter.change_action(self.ground)
        velocity = fighter.velocity
        fighter.velocity = (velocity[0], 0.0)
        fighter.ground_velocity *= multiplier

    @on.animation_end()
    def animation_end_ground(self, fighter: Any, ctx: MoveContext) -> None:
        fighter.change_action(Action.WAIT)

    @on.animation_end()
    def animation_end_air(self, fighter: Any, ctx: MoveContext) -> None:
        lag = self._landing_lag(fighter, ctx)
        if lag is None:
            return
        if lag == 0:
            fighter.change_action(Action.FALL)
        else:
            fighter.enter_fall_special(mobility=1, landing_lag=lag)

    @on.landed()
    def landed(self, fighter: Any, ctx: MoveContext) -> bool:
        lag = self._landing_lag(fighter, ctx)
        if lag is None:
            return False
        fighter.enter_fall_special(mobility=1, landing_lag=lag)
        return True

    def _landing_lag(self, fighter: Any, ctx: MoveContext) -> Any:
        field = (
            "specials_miss_landing_lag"
            if fighter.action == self.air_start
            else "specials_hit_landing_lag"
        )
        attributes = resource_attributes(ctx, self.resource)
        lag = getattr(attributes, field, None)
        return lag if lag is not None and validation.number(lag) and lag >= 0 else None

    @staticmethod
    def _clear_ground_start_velocity(fighter: Any) -> None:
        fighter.velocity = (0.0, 0.0)
        fighter.ground_velocity = 0.0

    @staticmethod
    def _clear_air_start_velocity(fighter: Any) -> None:
        fighter.velocity = (0.0, 0.0)

    @on.validate
    def validate(self, ctx: MoveContext) -> bool:
        if ctx.resource(self.resource) is None:
            return True
        rules = special_rules(ctx)
        threshold = getattr(rules, "side_stick_threshold", None)
        if threshold is None or not validation.number(threshold) or not 0 < threshold <= 1:
            return False
        attributes = resource_attributes(ctx, self.resource)
        return validation.fields(
            attributes,
            nonnegative=("specials_miss_landing_lag", "specials_hit_landing_lag", "specials_grav", "specials_terminal_vel"),
        )


class CaptainUpSpecial(UpSpecial, _CaptainFamilySpecial):
    """Shared Falcon Dive/Dark Dive attacker-side lifecycle."""

    def __init_subclass__(cls, **kwargs: Any) -> None:
        if all(hasattr(cls, name) for name in ("ground", "air", "catch", "throw")):
            cls.on_air = {cls.ground: Transition(cls.air, preserve_state=True, keep_frame=True)}
            cls.on_end = {cls.catch: Transition(cls.throw), cls.throw: Transition(Action.FALL)}
            cls._EVENT_PHASES = {
                "enter": ("ground", "air"),
                "before_hit": ("ground", "air"),
                "animation_end": ("ground", "air"),
                "landed": ("air",),
                "command_changed": ("air",),
            }
        super().__init_subclass__(**kwargs)

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: MoveContext) -> bool:
        if fighter.action in (self.ground, self.air):
            return True
        if directional_b_input(ctx, self.resource, 1, "vertical_threshold", direction=1) is not True:
            return False
        return start_open_special(fighter, ctx, self.ground, self.air)

    @on.action_enter()
    def enter(self, fighter: Any, ctx: MoveContext) -> None:
        _consume_command(fighter, 0)
        _consume_command(fighter, 1)
        _consume_command(fighter, 2)
        fighter.action_state.dive_released = False

    @on.before_hit()
    def before_hit(self, fighter: Any, hit: HitContext) -> None:
        if fighter.action in (self.ground, self.air):
            fighter.change_action(self.catch)

    @on.command_changed(0)
    def command_changed(self, fighter: Any, ctx: MoveContext) -> None:
        value = getattr(getattr(ctx, "event", None), "value", None)
        if fighter.action != self.air or not value:
            return
        _consume_command(fighter, 0)
        fighter.action_state.dive_released = True
        attributes = resource_attributes(ctx, self.resource)
        threshold = getattr(attributes, "specialhi_input_var", None)
        stick_x = getattr(getattr(ctx, "input", None), "stick", (0.0, 0.0))[0]
        if threshold is not None and abs(stick_x) > threshold and stick_x:
            fighter.facing = 1.0 if stick_x > 0 else -1.0

    @on.animation_end()
    def animation_end(self, fighter: Any, ctx: MoveContext) -> None:
        if fighter.action == self.ground:
            self._enter_fall_special(fighter, ctx)
        elif fighter.action == self.air:
            self._enter_fall_special(fighter, ctx)

    @on.landed()
    def landed(self, fighter: Any, ctx: MoveContext) -> bool:
        if fighter.action != self.air:
            return False
        if not getattr(fighter.action_state, "dive_released", False):
            return False
        return self._enter_fall_special(fighter, ctx)

    def _enter_fall_special(self, fighter: Any, ctx: MoveContext) -> bool:
        attributes = resource_attributes(ctx, self.resource)
        mobility = getattr(attributes, "specialhi_freefall_air_spd_mul", None)
        landing_lag = getattr(attributes, "specialhi_landing_lag", None)
        if mobility is None or landing_lag is None or not validation.number(mobility):
            return False
        fighter.enter_fall_special(mobility=mobility, landing_lag=landing_lag)
        return True

    @on.validate
    def validate(self, ctx: MoveContext) -> bool:
        rules = special_rules(ctx)
        threshold = getattr(rules, "vertical_threshold", None)
        if threshold is None or not validation.number(threshold) or not 0 < threshold <= 1:
            return False
        attributes = resource_attributes(ctx, self.resource)
        if attributes is None:
            return True
        value = getattr(attributes, "specialhi_input_var", None)
        return value is not None and validation.number(value) and 0 < value <= 1


class CaptainDownSpecial(DownSpecial, _CaptainFamilySpecial):
    """Shared Falcon Kick/Wizard's Foot phase lifecycle."""

    # ftCa_SpecialLw_Coll enters the shared rebound motion (state 363) when
    # the kick's command cue meets a facing wall.
    # Keep this as a target reference only.  DarkDive owns the exported
    # descriptor for source state 363; declaring another phase here would
    # duplicate the action in Ganondorf's behavior table.
    throw_rebound = source_action(363)

    def __init_subclass__(cls, **kwargs: Any) -> None:
        if all(hasattr(cls, name) for name in ("ground", "ground_end", "air", "landing", "air_end", "ground_end_air")):
            cls.on_end = {
                cls.ground_end: Transition(Action.WAIT),
                cls.air: Transition(cls.air_end),
                cls.air_end: Transition(Action.FALL),
                cls.landing: Transition(Action.WAIT),
                cls.ground_end_air: Transition(Action.FALL),
            }
            cls._EVENT_PHASES = {
                "animation_end": ("ground",),
                "landed": ("air", "air_end"),
                "wall_rebound": ("ground", "air"),
            }
        super().__init_subclass__(**kwargs)

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: MoveContext) -> bool:
        if fighter.action in (self.ground, self.ground_end, self.air, self.air_end, self.landing, self.ground_end_air):
            return True
        if directional_b_input(ctx, self.resource, 1, "vertical_threshold", direction=-1) is not True:
            return False
        return start_open_special(fighter, ctx, self.ground, self.air)

    @on.action_enter()
    def action_enter(self, fighter: Any, ctx: MoveContext) -> None:
        _consume_command(fighter, 0)
        _consume_command(fighter, 1)
        _consume_command(fighter, 2)

    @on.animation_end()
    def animation_end(self, fighter: Any, ctx: MoveContext) -> None:
        if fighter.action == self.ground:
            fighter.change_action(self.ground_end if fighter.grounded else self.ground_end_air)

    @on.landed()
    def landed(self, fighter: Any, ctx: MoveContext) -> bool:
        if fighter.action not in (self.air, self.air_end):
            return False
        fighter.change_action(self.landing)
        return True

    @on.surface_contact()
    def wall_rebound(self, fighter: Any, ctx: MoveContext) -> bool:
        """Enter state 363 only for the source's facing-wall collision."""
        wall = getattr(ctx, "wall", None)
        if wall is None:
            wall = getattr(ctx, "wall_contact", False)
        if not wall or fighter.action not in (self.ground, self.air):
            return False
        rebound = getattr(self, "throw_rebound", None)
        if rebound is None:
            return False
        fighter.change_action(rebound)
        return True


__all__ = [
    "CaptainFamilyActionState",
    "CaptainNeutralSpecial",
    "CaptainSideSpecial",
    "CaptainUpSpecial",
    "CaptainDownSpecial",
]
