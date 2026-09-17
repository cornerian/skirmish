"""Captain Falcon's declarative fighter definition.

This is intentionally a partial authoring slice.  Falcon Punch (ground and
air entry, command-variable launch cue, ground/air conversion, and terminal
recovery), Raptor Boost's contact-independent entry and recovery phases,
Falcon Dive, and the observed aerial Falcon Kick landing phase are wired to
resource data.  Hit detection, physics, effects, and other native-only
callbacks remain outside this declarative layer.  Hitbox, animation, and
parameter values belong to the validated native resource pack and are not
duplicated here.
"""

import math

from skirmish import (
    Action,
    ActionState,
    Button,
    Fighter,
    HitContext,
    MoveContext,
    Parameters,
    SpecialMove,
    SpecialMoves,
    Transition,
    action,
    hook,
    motion,
    parameter,
    resource as bind_resource,
    register as fighter,
    validation,
)
from shared.common import (
    directional_b_input,
    FighterBase,
    fresh_special_input,
    any_stick_axis_reaches_thresholds,
    resource_attributes,
    special_rules,
    start_action,
    start_open_special,
    stick_axis_reaches_threshold,
)


def _directional_b(ctx: MoveContext) -> bool:
    """Use host-provided dispatch thresholds when the host exposes them."""
    rules = special_rules(ctx)
    if rules is None:
        return False
    stick = getattr(ctx.input, "stick", (0.0, 0.0))
    vertical = getattr(rules, "vertical_threshold", None)
    horizontal = getattr(rules, "horizontal_threshold", None)
    return any_stick_axis_reaches_thresholds(
        stick,
        ((1, vertical), (0, horizontal)),
    )


class CaptainFalconActionState(ActionState):
    """Transient flags and source command variables owned by Falcon Punch."""

    command: tuple[int, int, int, int] = (0, 0, 0, 0)
    launch_armed: bool = False
    dive_released: bool = False


class CaptainFalconParameters(Parameters):
    """Reserved for resource-backed Captain Falcon parameters."""


class FalconPunch(SpecialMove):
    """Ground/air Falcon Punch policy; numeric data remains resource-owned."""

    resource = "neutral"

    # ftCa_SpecialAirN_Phys scales both self-velocity components only for the
    # source cmd_vars[1] == 1 branch.  The command tuple is supplied by the
    # current action state when the native profile runs, so unmatched values
    # leave the profile unapplied and preserve native fallback physics.
    air_motion = motion.profile(
        air=(motion.command_velocity_scale(
            index=1,
            value=1,
            multiplier=bind_resource("neutral.attributes.specialn_vel_mul"),
        ),),
    )

    ground = action(
        Action.SPECIAL_N_START,
        slippi_state=347,
        animation=301,
        attack="neutral.ground",
        command_trace="neutral.script.ground",
    )
    air = action(
        Action.SPECIAL_AIR_N_START,
        slippi_state=348,
        animation=302,
        attack="neutral.air",
        command_trace="neutral.script.air",
        motion=air_motion,
    )

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        fighter.action_state.command = (0, 0, 0, 0)
        fighter.action_state.launch_armed = False

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if not fresh_special_input(ctx, self.resource):
            return False
        # Generic dispatch reserves directional B inputs for the directional
        # specials.  If no dispatch rules are supplied, preserve the API's
        # permissive authoring behavior for standalone move tests/hosts.
        if _directional_b(ctx):
            return False
        if fighter.action in (self.ground, self.air):
            return True
        if not start_open_special(fighter, ctx, self.ground, self.air):
            return False
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
            attributes = resource_attributes(ctx, self.resource)
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
            velocity_x = attributes.specialn_vel_x * fighter.facing * math.cos(angle)
            velocity_y = attributes.specialn_vel_x * math.sin(angle)
            # Assign the vector atomically; indexed writes target a transient
            # native member and do not update the host-owned velocity field.
            fighter.velocity = (velocity_x, velocity_y)

    @hook.command_changed(1, actions=(air,))
    def command_velocity_scale(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Retain the current source command for the native air profile.

        The host writes ``fighter.action_state.command`` before dispatching
        this notification.  The callback is intentionally empty: command 1
        is consumed by the declarative physics operation, while command 0's
        launch behavior remains in ``command_changed`` above.
        """
        return None

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

    The upstream victim attachment and throw-hit physics remain native/
    resource-owned.  The attacker-side catch and throw states are still
    represented because they are observable in Slippi and their source
    animation lifecycle is deterministic.
    """

    resource = "up"

    ground = action(
        Action.SPECIAL_HI,
        slippi_state=353,
        animation=307,
        attack="up.ground",
    )
    air = action(
        Action.SPECIAL_AIR_HI,
        slippi_state=354,
        animation=308,
        attack="up.air",
    )
    catch = action(Action.SPECIAL_HI_CATCH, slippi_state=355, animation=309)
    throw = action(Action.SPECIAL_HI_THROW, slippi_state=356, animation=310)

    # ftCa_SpecialHi_Coll converts grounded SpecialHi to its airborne phase
    # when the move leaves the ground, retaining the native state and frame.
    # Landing remains owned by the dedicated landed hook below: the source
    # does not unconditionally convert aerial Dive back to grounded SpecialHi.
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        fighter.action_state.dive_released = False

    @hook.before_hit(actions=(ground, air))
    def before_hit(self, fighter: Fighter, hit: HitContext) -> None:
        """Enter Falcon Dive's attacker-side catch state after contact.

        ``ftCa_SpecialLw_800E5128`` is installed by both Dive entry paths and
        changes the attacker to motion state 355.  The native callback also
        captures and aligns the victim; the host invokes this hook only for a
        supported hit, so this callback intentionally changes no victim state.
        """
        if fighter.action in (self.ground, self.air):
            fighter.change_action(self.catch)

    @hook.animation_end(catch)
    def catch_animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Match ``doCatchAnim``'s state-355 to state-356 transition."""
        fighter.change_action(self.throw)

    @hook.animation_end(throw)
    def throw_animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        """The native throw animation ends with ordinary aerial Fall."""
        fighter.change_action(Action.FALL)

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if not fresh_special_input(ctx, self.resource):
            return False
        if directional_b_input(
            ctx, self.resource, 1, "vertical_threshold", direction=1
        ) is not True:
            return False
        if fighter.action in (self.ground, self.air):
            return True
        if ctx.ground_open:
            start_action(fighter, self.ground)
        elif ctx.air_open:
            start_action(fighter, self.air)
        else:
            return False
        return True

    @hook.command_changed(0, actions=(air,))
    def command_changed(self, fighter: Fighter, ctx: MoveContext) -> None:
        # Source IASA consumes a non-zero cmd_vars[0], marks the dive as
        # launched, and updates facing when the horizontal stick magnitude
        # exceeds PlCa's resource-owned input threshold.  The accompanying
        # model rotation and catch physics remain native gaps.
        value = getattr(getattr(ctx, "event", None), "value", None)
        if fighter.action == self.air and value:
            fighter.action_state.dive_released = True
            attributes = resource_attributes(ctx, "up.attributes")
            threshold = (getattr(attributes, "specialhi_input_var", None)
                         if attributes is not None else None)
            input_state = getattr(ctx, "input", None)
            if input_state is None:
                return
            stick_x = input_state.stick[0]
            if (threshold is not None and abs(stick_x) > threshold
                    and stick_x != 0):
                fighter.facing = 1.0 if stick_x > 0 else -1.0

    @hook.animation_end(ground, air)
    def animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        attributes = resource_attributes(ctx, "up.attributes")
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
            # Let collision::land enter ordinary LANDING so it can apply the
            # native landing interrupt/post-enter setup.
            return False
        attributes = resource_attributes(ctx, "up.attributes")
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
        rules = special_rules(ctx)
        if rules is not None:
            threshold = getattr(rules, "vertical_threshold", None)
            if (threshold is None or not validation.number(threshold)
                    or not 0 < threshold <= 1):
                return False
        attributes = resource_attributes(ctx, "up.attributes")
        if attributes is None:
            return True
        input_var = getattr(attributes, "specialhi_input_var", None)
        return (input_var is not None and validation.number(input_var)
                and 0 < input_var <= 1)


class RaptorBoost(SpecialMove):
    """Captain Falcon's contact-independent Raptor Boost phases.

    The native start callbacks clear source-owned velocity and the native
    collision callbacks decide whether a start becomes the follow-through.
    Contact is represented by the fighter's before-hit hook, while item
    contacts, per-frame physics, effects, and wall behavior remain native.
    """

    resource = "side"

    # ftCa_SpecialAirS_Phys applies the character gravity continuously during
    # the follow-through (state 352).  The start state has a source-side
    # cmd_vars[1] gate, so it intentionally does not share this profile.
    air_motion = motion.profile(
        air=(
            motion.gravity(
                acceleration=bind_resource("side.attributes.specials_grav"),
                terminal_velocity=bind_resource("side.attributes.specials_terminal_vel"),
                delay=0,
            ),
        ),
    )

    ground_start = action(
        Action.SPECIAL_S_START,
        slippi_state=349,
        animation=303,
        attack="side.ground_start",
    )
    ground = action(
        Action.SPECIAL_S,
        slippi_state=350,
        animation=304,
        attack="side.ground",
    )
    air_start = action(
        Action.SPECIAL_AIR_S_START,
        slippi_state=351,
        animation=305,
        attack="side.air_start",
    )
    air = action(
        Action.SPECIAL_AIR_S,
        slippi_state=352,
        animation=306,
        attack="side.air",
        motion=air_motion,
    )

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if directional_b_input(
            ctx, self.resource, 0, "side_stick_threshold"
        ) is not True:
            return False

        if fighter.action in (self.ground_start, self.ground, self.air_start, self.air):
            return True
        if ctx.ground_open:
            start_action(fighter, self.ground_start)
            self._clear_ground_start_velocity(fighter)
        elif ctx.air_open:
            start_action(fighter, self.air_start)
            self._clear_air_start_velocity(fighter)
        else:
            return False
        return True

    @hook.action_enter(ground_start, air_start)
    def action_enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Mirror the source entry velocity reset for direct action entry."""
        if fighter.action == self.ground_start:
            self._clear_ground_start_velocity(fighter)
        elif fighter.action == self.air_start:
            self._clear_air_start_velocity(fighter)

    @hook.before_hit(actions=(ground_start, air_start))
    def before_hit(self, fighter: Fighter, hit: HitContext) -> None:
        """Enter Raptor Boost's follow-through on a fighter hurtbox hit.

        ``ftCa_SpecialS_OnDetect`` clears grounded vertical velocity and
        scales ground traction by the captured ``specials_gr_vel_x`` resource
        attribute.  The aerial path only clears z velocity, which has no
        planar equivalent in the host.  Grounded contact with missing or
        non-finite resource data leaves the action untouched.
        """
        if fighter.action not in (self.ground_start, self.air_start):
            return
        if fighter.action == self.air_start:
            fighter.change_action(self.air)
            return

        resource = fighter.resource(self.resource)
        attributes = getattr(resource, "attributes", None)
        multiplier = getattr(attributes, "specials_gr_vel_x", None)
        if multiplier is None or not validation.finite(multiplier):
            return
        fighter.change_action(self.ground)
        velocity = fighter.velocity
        fighter.velocity = (velocity[0], 0.0)
        fighter.ground_velocity *= multiplier

    @hook.animation_end(ground_start, ground)
    def animation_end_ground(self, fighter: Fighter, ctx: MoveContext) -> None:
        fighter.change_action(Action.WAIT)

    @hook.animation_end(air_start, air)
    def animation_end_air(self, fighter: Fighter, ctx: MoveContext) -> None:
        lag_name = (
            "specials_miss_landing_lag"
            if fighter.action == self.air_start
            else "specials_hit_landing_lag"
        )
        lag = self._landing_lag(ctx, lag_name)
        if lag is None:
            return
        if lag == 0:
            fighter.change_action(Action.FALL)
        else:
            fighter.enter_fall_special(mobility=1, landing_lag=lag)

    @hook.landed(air_start, air)
    def landed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        lag_name = (
            "specials_miss_landing_lag"
            if fighter.action == self.air_start
            else "specials_hit_landing_lag"
        )
        lag = self._landing_lag(ctx, lag_name)
        if lag is None:
            return False
        # ftCo_LandingFallSpecial_Enter is represented by the host's
        # fall-special entry; unlike animation completion, native landing
        # consumes this callback even when the resource lag is zero.
        fighter.enter_fall_special(mobility=1, landing_lag=lag)
        return True

    @classmethod
    def _horizontal_threshold(cls, ctx: MoveContext):
        rules = special_rules(ctx)
        threshold = getattr(rules, "side_stick_threshold", None)
        if (threshold is None or not validation.number(threshold)
                or not 0 < threshold <= 1):
            return None
        return threshold

    def _landing_lag(self, ctx: MoveContext, field: str):
        resource = ctx.resource(self.resource)
        attributes = resource_attributes(ctx, resource)
        lag = getattr(attributes, field, None) if attributes is not None else None
        if lag is None or not validation.number(lag) or lag < 0:
            return None
        return lag

    @staticmethod
    def _clear_ground_start_velocity(fighter: Fighter) -> None:
        # The tuple assignment is the native ABI's atomic velocity setter;
        # indexed writes can address a transient member instead of host state.
        fighter.velocity = (0.0, 0.0)
        fighter.ground_velocity = 0.0

    @staticmethod
    def _clear_air_start_velocity(fighter: Fighter) -> None:
        fighter.velocity = (0.0, 0.0)

    @hook.validate
    def validate(self, ctx: MoveContext) -> bool:
        resource = ctx.resource(self.resource)
        if resource is None:
            return True
        if self._horizontal_threshold(ctx) is None:
            return False
        attributes = resource_attributes(ctx, resource)
        return validation.fields(
            attributes,
            nonnegative=(
                "specials_miss_landing_lag",
                "specials_hit_landing_lag",
                "specials_grav",
                "specials_terminal_vel",
            ),
        )


class FalconKick(SpecialMove):
    """Captain Falcon Kick's source-distinct motion lifecycle.

    The entry and terminal motion identities are resource-backed.  Rebound,
    wall interaction, hit effects, and per-frame physics remain native-only.
    """

    resource = "down"

    ground = action(
        Action.SPECIAL_LW,
        slippi_state=357,
        animation=311,
        attack="down.ground",
    )
    ground_end = action(
        Action.SPECIAL_LW_GROUND_END,
        slippi_state=358,
        animation=312,
        attack="down.ground_end",
    )
    air = action(
        Action.SPECIAL_AIR_LW,
        slippi_state=359,
        animation=313,
        attack="down.air",
    )
    landing = action(
        Action.SPECIAL_AIR_LW_LANDING_END,
        slippi_state=360,
        animation=314,
        attack="down.landing",
    )
    air_end = action(
        Action.SPECIAL_AIR_LW_END_AIR,
        slippi_state=361,
        animation=316,
        attack="down.air_end",
        motion=motion.profile(
            air=(
                motion.gravity(
                    acceleration=parameter("movement.gravity"),
                    terminal_velocity=parameter("movement.terminal_velocity"),
                    delay=0,
                ),
                motion.air_friction(
                    amount=parameter("movement.aerial_friction"),
                ),
            ),
        ),
    )
    ground_end_air = action(
        Action.SPECIAL_LW_END_AIR,
        slippi_state=362,
        animation=315,
        attack="down.ground_end_air",
    )

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if not fresh_special_input(ctx, self.resource):
            return False
        if fighter.action in (
            self.ground,
            self.ground_end,
            self.air,
            self.air_end,
            self.landing,
        ):
            return True
        rules = special_rules(ctx)
        threshold = getattr(rules, "vertical_threshold", None) if rules is not None else None
        if (threshold is None or ctx.input.stick[1] > -threshold
                or not (ctx.ground_open or ctx.air_open)):
            return False
        # Generic dispatch exposes the two legal surfaces independently.  A
        # simultaneous opening is grounded by precedence, matching the
        # native grounded branch; otherwise the aerial motion is selected.
        start_open_special(fighter, ctx, self.ground, self.air)
        return True

    @hook.animation_end(ground, ground_end, air, air_end, landing, ground_end_air)
    def animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        if fighter.action == self.ground:
            # The native grounded Kick callback checks GA state at animation
            # completion: a ground move that has left the stage uses its
            # distinct airborne end motion (state 362 / motion 315).
            fighter.change_action(self.ground_end if fighter.grounded else self.ground_end_air)
        elif fighter.action == self.ground_end:
            fighter.change_action(Action.WAIT)
        elif fighter.action == self.air:
            fighter.change_action(self.air_end)
        elif fighter.action == self.air_end:
            fighter.change_action(Action.FALL)
        elif fighter.action == self.ground_end_air:
            fighter.change_action(Action.FALL)
        elif fighter.action == self.landing:
            fighter.change_action(Action.WAIT)

    @hook.landed(air, air_end)
    def landed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        """Consume collision landing into Captain's source landing motion."""
        if fighter.action not in (self.air, self.air_end):
            return False
        fighter.change_action(self.landing)
        return True

    @hook.validate
    def validate(self, ctx: MoveContext) -> bool:
        rules = special_rules(ctx)
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
        RaptorBoost(),
        FalconDive(),
        FalconKick(),
    )


__all__ = [
    "CaptainFalcon",
    "CaptainFalconActionState",
    "CaptainFalconParameters",
    "FalconPunch",
    "RaptorBoost",
    "FalconDive",
    "FalconKick",
]
