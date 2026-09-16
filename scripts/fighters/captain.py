"""Captain Falcon's declarative fighter definition.

This is intentionally a partial authoring slice.  Falcon Punch (ground and
air entry, command-variable launch cue, ground/air conversion, and terminal
recovery) and the representable Falcon Dive entry/lifecycle are wired to
resource data; side and down specials are retained as canonical engine actions
with no translated Python behavior yet.  Hitbox, animation, and parameter
values belong to the validated native resource pack and are not duplicated
here.
"""

import math

from skirmish import (
    Action,
    ActionState,
    Button,
    Fighter,
    MoveContext,
    Parameters,
    SpecialMove,
    SpecialMoves,
    Transition,
    action,
    hook,
    register as fighter,
    validation,
    ActionMove,
)
from shared.common import FighterBase


# The compact authoring enum does not yet expose the engine's ordinary
# landing action; its canonical wire spelling is still representable as a
# string descriptor.
_LANDING_ACTION = "landing"


def _special_rules(ctx: MoveContext):
    rules = getattr(ctx, "rules", None)
    return getattr(rules, "specials", None)


def _directional_b(ctx: MoveContext) -> bool:
    """Use host-provided dispatch thresholds when the host exposes them."""
    rules = _special_rules(ctx)
    if rules is None:
        return False
    stick = getattr(ctx.input, "stick", (0.0, 0.0))
    vertical = getattr(rules, "vertical_threshold", None)
    horizontal = getattr(rules, "horizontal_threshold", None)
    return ((vertical is not None and abs(stick[1]) >= vertical)
            or (horizontal is not None and abs(stick[0]) >= horizontal))


class CaptainFalconActionState(ActionState):
    """Only transient flags owned by the Falcon Punch policy."""

    launch_armed: bool = False
    dive_released: bool = False
    kick_active: bool = False


class CaptainFalconParameters(Parameters):
    """Reserved for resource-backed Captain Falcon parameters."""


class FalconPunch(SpecialMove):
    """Ground/air Falcon Punch policy; numeric data remains resource-owned."""

    resource = "neutral"

    ground = action(
        Action.SPECIAL_N_START,
        slippi_state=347,
        attack="neutral.ground",
        command_trace="neutral.script.ground",
    )
    air = action(
        Action.SPECIAL_AIR_N_START,
        slippi_state=348,
        attack="neutral.air",
        command_trace="neutral.script.air",
    )

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        fighter.action_state.launch_armed = False

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if ctx.resource(self.resource) is None or not ctx.input.just_pressed(Button.B):
            return False
        # Generic dispatch reserves directional B inputs for the directional
        # specials.  If no dispatch rules are supplied, preserve the API's
        # permissive authoring behavior for standalone move tests/hosts.
        if _directional_b(ctx):
            return False
        if fighter.action in (self.ground, self.air):
            return True
        if not (ctx.ground_open or ctx.air_open):
            return False
        fighter.change_action(self.ground if ctx.ground_open else self.air)
        fighter.action_frame = 1
        return True

    @hook.command_changed(0, actions=(air,))
    def command_changed(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Expose the airborne IASA command cue without inventing physics.

        The upstream ground IASA callback is empty.  Air IASA consumes a
        non-zero ``cmd_vars[0]`` and computes velocity from resource-owned
        parameters; the command callback performs that same calculation when
        those attributes are available, while retaining the cue flag for
        hosts that schedule IASA separately.
        """
        value = getattr(getattr(ctx, "event", None), "value", None)
        if fighter.action == self.air and value:
            fighter.action_state.launch_armed = True
            resource = ctx.resource(self.resource)
            attributes = getattr(resource, "attributes", None)
            fields = (
                "specialn_stick_range_y_neg",
                "specialn_stick_range_y_pos",
                "specialn_angle_diff",
                "specialn_vel_x",
            )
            if attributes is None or any(getattr(attributes, name, None) is None for name in fields):
                return
            minimum = attributes.specialn_stick_range_y_neg
            maximum = attributes.specialn_stick_range_y_pos
            if maximum <= minimum:
                return
            stick_y = ctx.input.stick[1]
            stick_y = min(stick_y, maximum)
            stick_y -= minimum
            stick_y = max(stick_y, 0)
            if ctx.input.stick[1] < 0:
                stick_y = -stick_y
            angle = 3.141592653589793 / 180 * (
                stick_y * attributes.specialn_angle_diff / (maximum - minimum)
            )
            fighter.velocity[1] = attributes.specialn_vel_x * math.sin(angle)
            fighter.velocity[0] = attributes.specialn_vel_x * fighter.facing * math.cos(angle)

    @hook.animation_end(ground, air)
    def animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        if fighter.action == self.ground:
            fighter.change_action(Action.WAIT)
        else:
            fighter.change_action(Action.FALL)

    # The source callbacks convert these same two motion states in either
    # direction while preserving animation/state; the host applies the
    # transition at collision time.
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    @hook.validate
    def validate(self, ctx: MoveContext) -> bool:
        resource = ctx.resource(self.resource)
        if resource is None:
            return True
        # Resource presence is meaningful only when both action traces exist;
        # validation.command_trace checks row width, frame count, and values.
        return validation.command_trace(
            ctx, "neutral.script.ground", "neutral.ground"
        ) and validation.command_trace(ctx, "neutral.script.air", "neutral.air") and self._valid_attributes(resource)

    @staticmethod
    def _valid_attributes(resource) -> bool:
        attributes = getattr(resource, "attributes", None)
        names = (
            "specialn_stick_range_y_neg", "specialn_stick_range_y_pos",
            "specialn_angle_diff", "specialn_vel_x", "specialn_vel_mul",
        )
        if attributes is None or any(not hasattr(attributes, name) for name in names):
            return False
        values = [getattr(attributes, name) for name in names]
        if not all(validation.finite(value) for value in values):
            return False
        return values[1] > values[0]


class FalconDive(SpecialMove):
    """Representable Falcon Dive entry and recovery semantics.

    The upstream catch/throw target interaction, velocity profiles, ledge
    checks, and animation markers remain native/resource-owned.  This class
    deliberately models only state selection, command cue state, landing,
    and the source-confirmed terminal fall-special call when its attributes
    are available from the host.
    """

    resource = "up"

    ground = action(Action.SPECIAL_HI, slippi_state=353)
    air = action(Action.SPECIAL_AIR_HI, slippi_state=354)

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        fighter.action_state.dive_released = False

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if (ctx.resource(self.resource) is None
                or not ctx.input.just_pressed(Button.B)):
            return False
        rules = _special_rules(ctx)
        if rules is None:
            return False
        threshold = getattr(rules, "vertical_threshold", None)
        if threshold is None or ctx.input.stick[1] < threshold:
            return False
        if fighter.action in (self.ground, self.air):
            return True
        if ctx.ground_open:
            fighter.change_action(self.ground)
        elif ctx.air_open:
            fighter.change_action(self.air)
        else:
            return False
        fighter.action_frame = 1
        return True

    @hook.command_changed(0, actions=(air,))
    def command_changed(self, fighter: Fighter, ctx: MoveContext) -> None:
        # Source IASA consumes a non-zero cmd_vars[0] and marks the dive as
        # launched.  Direction turning and movement remain native gaps.
        value = getattr(getattr(ctx, "event", None), "value", None)
        if fighter.action == self.air and value:
            fighter.action_state.dive_released = True

    @hook.animation_end(ground, air)
    def animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        attributes = ctx.resource("up.attributes")
        if attributes is None:
            return
        mobility = getattr(attributes, "specialhi_freefall_air_spd_mul", None)
        landing_lag = getattr(attributes, "specialhi_landing_lag", None)
        if mobility is None or landing_lag is None:
            return
        fighter.enter_fall_special(mobility=mobility, landing_lag=landing_lag)

    @hook.landed(air)
    def landed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if fighter.action != self.air:
            return False
        if not fighter.action_state.dive_released:
            fighter.change_action(_LANDING_ACTION)
            return True
        attributes = ctx.resource("up.attributes")
        if attributes is None:
            return False
        mobility = getattr(attributes, "specialhi_freefall_air_spd_mul", None)
        landing_lag = getattr(attributes, "specialhi_landing_lag", None)
        if mobility is None or landing_lag is None:
            return False
        fighter.enter_fall_special(mobility=mobility, landing_lag=landing_lag)
        return True

    @hook.validate
    def validate(self, ctx: MoveContext) -> bool:
        rules = _special_rules(ctx)
        if rules is None:
            return True
        threshold = getattr(rules, "vertical_threshold", None)
        return (threshold is not None and validation.number(threshold)
                and 0 < threshold <= 1)


class FalconKick(SpecialMove):
    """Observed aerial Falcon Kick entry and finite recovery phase only.

    Ground Kick, rebound, wall interaction, hit effects, and command traces
    remain deliberately unimplemented because they are not represented by
    the captured slice or this API contract.
    """

    resource = "down"

    air = action(Action.SPECIAL_AIR_LW, slippi_state=359)
    air_end = action(Action.SPECIAL_AIR_LW_END, slippi_state=361)

    @hook.action_enter(air, air_end)
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        # Source entry clears command variables; no numeric physics is
        # duplicated here.
        fighter.action_state.kick_active = fighter.action == self.air

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if (ctx.resource(self.resource) is None
                or not ctx.input.just_pressed(Button.B)):
            return False
        if fighter.action in (self.air, self.air_end):
            return True
        rules = _special_rules(ctx)
        threshold = getattr(rules, "vertical_threshold", None) if rules is not None else None
        if threshold is None or ctx.input.stick[1] > -threshold or not ctx.air_open:
            return False
        fighter.change_action(self.air)
        fighter.action_frame = 1
        return True

    @hook.animation_end(air, air_end)
    def animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        if fighter.action == self.air:
            fighter.change_action(self.air_end)
        elif fighter.action == self.air_end:
            fighter.change_action(Action.FALL)

    @hook.validate
    def validate(self, ctx: MoveContext) -> bool:
        rules = _special_rules(ctx)
        if rules is None:
            return True
        threshold = getattr(rules, "vertical_threshold", None)
        return (threshold is not None and validation.number(threshold)
                and 0 < threshold <= 1)


@fighter
class CaptainFalcon(FighterBase):
    name = "captain-falcon"
    external_ids = (0,)
    parameters = CaptainFalconParameters
    attributes = CaptainFalconParameters
    action_state = CaptainFalconActionState
    specials = SpecialMoves(
        FalconPunch(),
        ActionMove(Action.SPECIAL_S),
        FalconDive(),
        FalconKick(),
    )


__all__ = [
    "CaptainFalcon",
    "CaptainFalconActionState",
    "CaptainFalconParameters",
    "FalconPunch",
    "FalconDive",
    "FalconKick",
]
