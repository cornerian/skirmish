"""Fox fighter definition and special move policies for the class based native authoring API."""

from skirmish import (
    Action,
    ActionState,
    Button,
    Fighter,
    HitContext,
    MotionBinding,
    MoveContext,
    Parameters,
    SpecialMove,
    Transition,
    SpecialMoves,
    action,
    clock,
    f32,
    hook,
    math,
    motion,
    parameter,
    register as fighter,
    resource as bind_resource,
    validation,
)
from shared.common import (
    FighterBase,
    fresh_special_input,
    resource_attributes,
    special_rules,
    start_action,
    start_open_special,
)


class FoxActionState(ActionState):
    command: tuple[int, int, int, int] = (0, 0, 0, 0)
    repeat_armed: bool = False
    fire_pending: bool = False
    gravity_delay: f32 = 0.0
    release_lag: f32 = 0.0
    is_release: bool = False
    turn_frames: f32 = 0.0
    turned: bool = False
    looping: bool = False
    travel_remaining: f32 = 0.0
    ground_travel_frames: f32 = 0.0
    travel_angle: f32 = 0.0


class FoxParameters(Parameters):
    projectile_kind: str = "fox_laser"


class Blaster(SpecialMove):
    resource = "neutral"

    ground_start = action(
        Action.SPECIAL_N_START,
        slippi_state=341,
        animation=295,
        attack="neutral.start.ground",
        command_trace="neutral.script.start.ground",
    )
    ground_loop = action(
        Action.SPECIAL_N_LOOP,
        slippi_state=342,
        animation=296,
        attack="neutral.loop_phase.ground",
        command_trace="neutral.script.loop_phase.ground",
    )
    ground_end = action(
        Action.SPECIAL_N_END,
        slippi_state=343,
        animation=297,
        attack="neutral.end.ground",
        command_trace="neutral.script.end.ground",
    )
    air_start = action(
        Action.SPECIAL_AIR_N_START,
        slippi_state=344,
        animation=298,
        attack="neutral.start.air",
        command_trace="neutral.script.start.air",
    )
    air_loop = action(
        Action.SPECIAL_AIR_N_LOOP,
        slippi_state=345,
        animation=299,
        attack="neutral.loop_phase.air",
        command_trace="neutral.script.loop_phase.air",
    )
    air_end = action(
        Action.SPECIAL_AIR_N_END,
        slippi_state=346,
        animation=300,
        attack="neutral.end.air",
        command_trace="neutral.script.end.air",
    )

    @hook.action_enter(ground_start, air_start)
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        fighter.action_state.command = (0, 0, 0, 0)
        fighter.action_state.repeat_armed = False
        fighter.action_state.fire_pending = False

    @hook.input_pressed(Button.B)
    def press(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if not fresh_special_input(ctx, self.resource):
            return False
        resource = ctx.resource(self.resource)

        active_actions = (
            self.ground_start,
            self.air_start,
            self.ground_loop,
            self.air_loop,
        )
        if fighter.action in active_actions:
            if fighter.action_state.command[0] != 0:
                fighter.action_state.repeat_armed = True
            return True

        if fighter.action in (self.ground_end, self.air_end):
            return True

        thresholds = resource.neutral_thresholds
        stick_x = ctx.input.stick[0]
        stick_y = ctx.input.stick[1]
        if stick_x >= thresholds[0] or stick_x <= -thresholds[0]:
            return False
        if stick_y >= thresholds[1] or stick_y <= -thresholds[1]:
            return False

        if not start_open_special(fighter, ctx, self.ground_start, self.air_start):
            return False

        if ctx.ground_open:
            fighter.ground_velocity = 0
            fighter.velocity[0] = 0
            fighter.velocity[1] = 0
        return True

    @hook.animation_end(
        ground_start,
        air_start,
        ground_loop,
        air_loop,
        ground_end,
        air_end,
    )
    def animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        resource = ctx.resource("neutral")
        if resource is None:
            return

        if fighter.action == self.ground_start:
            fighter.change_action(self.ground_loop, preserve_state=True)
        elif fighter.action == self.air_start:
            fighter.change_action(self.air_loop, preserve_state=True)
        elif fighter.action == self.ground_loop:
            if fighter.action_state.repeat_armed:
                fighter.change_action(self.ground_loop, preserve_state=True)
                fighter.action_state.repeat_armed = False
            else:
                fighter.change_action(self.ground_end)
        elif fighter.action == self.air_loop:
            if fighter.action_state.repeat_armed:
                fighter.change_action(self.air_loop, preserve_state=True)
                fighter.action_state.repeat_armed = False
            else:
                fighter.change_action(self.air_end)
        elif fighter.action == self.ground_end:
            fighter.change_action(Action.WAIT)
        elif fighter.action == self.air_end:
            attributes = resource_attributes(ctx, resource)
            landing_lag = attributes.landing_lag
            if landing_lag == 0:
                fighter.change_action(Action.FALL)
            else:
                fighter.enter_fall_special(mobility=1, landing_lag=landing_lag)

    @hook.command_changed(2, actions=(ground_loop, air_loop))
    def command_changed(self, fighter: Fighter, ctx: MoveContext) -> None:
        value = ctx.event.value
        if value is None or value == 0:
            return

        resource = ctx.resource("neutral")
        if resource is None:
            return

        attributes = resource_attributes(ctx, resource)
        laser = resource.laser
        ecb = fighter.ecb.current
        ecb_midpoint = (ecb.top[1] + ecb.bottom[1]) * 0.5
        angle = attributes.angle
        if fighter.facing != 1:
            angle = (-angle) + math.pi

        fighter.emit_projectile(
            kind=ctx.parameters.projectile_kind,
            position=(
                fighter.position[0],
                fighter.position[1] + ecb_midpoint,
                fighter.depth,
            ),
            angle=angle,
            speed=attributes.speed,
            lifetime=laser.lifetime,
            hitboxes=laser.hitboxes,
            move_id=laser.move_id,
        )
        command = fighter.action_state.command
        fighter.action_state.command = (command[0], command[1], 0, command[3])

    @hook.validate
    def validate(self, ctx: MoveContext) -> bool:
        resource = ctx.resource("neutral")
        if resource is None:
            return True

        thresholds = resource.neutral_thresholds
        if thresholds is None or len(thresholds) != 2:
            return False
        if not validation.number(thresholds[0], True):
            return False
        if not validation.number(thresholds[1], True):
            return False
        if thresholds[0] <= 0 or thresholds[1] <= 0:
            return False
        if thresholds[0] > 1 or thresholds[1] > 1:
            return False

        attributes = resource_attributes(ctx, resource)
        laser = resource.laser
        if attributes is None or laser is None:
            return False
        if not validation.fields(
            attributes,
            finite=("angle",),
            nonnegative=("speed", "landing_lag"),
        ):
            return False
        if not validation.fields(laser, positive=("lifetime",)):
            return False
        if not validation.hitboxes(laser.hitboxes):
            return False
        if not validation.number(laser.move_id, True):
            return False
        if ctx.resource("neutral.script") is None:
            return False

        return True



class Illusion(SpecialMove):
    """Fox's six-phase Illusion policy, shared by Falco's resource variant."""

    resource = "side"

    start_ground_motion = motion.profile(
        ground=(
            motion.ground_friction(
                amount=parameter("fighter.ground_friction"),
            ),
        ),
        air=(
            motion.gravity(
                acceleration=bind_resource("side.attributes.start_fall_accel"),
                terminal_velocity=parameter("movement.terminal_velocity"),
                delay=bind_resource("side.attributes.gravity_delay"),
            ),
            motion.air_friction(
                amount=bind_resource("side.attributes.start_air_friction"),
            ),
        ),
    )
    start_air_motion = start_ground_motion

    dash_ground_motion = motion.profile(
        ground=(
            motion.target_track(
                path=bind_resource("side.dash.ground_trans_n"),
                multiply_by_facing=True,
                end="no_op",
            ),
        ),
        air=(
            motion.velocity_track(
                path=bind_resource("side.dash.air_trans_n"),
                multiply_x_by_facing=True,
                end="no_op",
            ),
        ),
    )
    dash_air_motion = dash_ground_motion

    end_ground_motion = motion.profile(
        ground=(
            motion.ground_friction(
                amount=bind_resource("side.attributes.end_ground_friction"),
            ),
        ),
        air=(
            motion.gravity(
                acceleration=bind_resource("side.attributes.end_fall_accel"),
                terminal_velocity=parameter("movement.terminal_velocity"),
                delay=bind_resource("side.attributes.end_gravity_delay"),
            ),
            motion.air_friction(
                amount=bind_resource("side.attributes.end_air_friction"),
            ),
        ),
    )
    end_air_motion = end_ground_motion

    ground_start = action(
        Action.SPECIAL_S_START,
        slippi_state=347,
        animation=301,
        attack="side.start.ground",
        motion=start_ground_motion,
        profile_delay_field="gravity_delay",
    )

    ground_dash = action(
        Action.SPECIAL_S,
        slippi_state=348,
        animation=302,
        attack="side.dash.ground",
        motion=dash_ground_motion,
        profile_delay_field="gravity_delay",
    )
    ground_end = action(
        Action.SPECIAL_S_END,
        slippi_state=349,
        animation=303,
        collision_mode="clamp",
        attack="side.end.ground",
        motion=end_ground_motion,
        profile_delay_field="gravity_delay",
    )
    air_start = action(
        Action.SPECIAL_AIR_S_START,
        slippi_state=350,
        animation=304,
        ledge_catchable=True,
        attack="side.start.air",
        motion=start_air_motion,
        profile_delay_field="gravity_delay",
    )
    air_dash = action(
        Action.SPECIAL_AIR_S,
        slippi_state=351,
        animation=305,
        ledge_catchable=True,
        attack="side.dash.air",
        motion=dash_air_motion,
        profile_delay_field="gravity_delay",
    )
    air_end = action(
        Action.SPECIAL_AIR_S_END,
        slippi_state=352,
        animation=306,
        ledge_catchable=True,
        attack="side.end.air",
        motion=end_air_motion,
        profile_delay_field="gravity_delay",
    )

    on_end = {
        ground_start: Transition(ground_dash, preserve_state=True),
        air_start: Transition(air_dash, preserve_state=True),
        ground_end: Transition(Action.WAIT),
    }
    # The source side is deliberately opposite the map name: a ground
    # contact converts an airborne phase into its grounded counterpart.
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_dash: Transition(ground_dash, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_dash: Transition(air_dash, preserve_state=True, keep_frame=True),
    }

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        resource = ctx.resource("side")
        rules = special_rules(ctx)
        if resource is None or rules is None or not ctx.input.just_pressed(Button.B):
            return False

        if fighter.action in (self.ground_dash, self.air_dash):
            self._enter_end(fighter, resource)
            return True
        if fighter.action in (
            self.ground_start,
            self.ground_end,
            self.air_start,
            self.air_end,
        ):
            return True
        if not (ctx.ground_open or ctx.air_open):
            return False

        horizontal = ctx.input.stick[0]
        if abs(horizontal) < rules.side_stick_threshold:
            return False
        if ctx.air_open and abs(ctx.input.stick[1]) >= rules.vertical_threshold:
            return False
        if ctx.ground_open and fighter.locomotion.side_special_b_age != 0:
            return False

        if horizontal * fighter.facing < -rules.turn_threshold:
            fighter.facing = -fighter.facing
        if ctx.ground_open:
            start_action(fighter, self.ground_start)
        else:
            start_action(fighter, self.air_start)
        return True

    @hook.action_enter(
        ground_start,
        air_start,
        ground_dash,
        air_dash,
        ground_end,
        air_end,
    )
    def action_enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        resource = ctx.resource("side")
        if resource is None:
            return
        attributes = resource_attributes(ctx, resource)
        if fighter.action == self.ground_start:
            retention_loss = (-resource.ground_speed_retention) + 1.0
            retained = fighter.ground_velocity - fighter.ground_velocity * retention_loss
            fighter.ground_velocity = retained / attributes.entry_speed_div
            fighter.action_state.gravity_delay = attributes.gravity_delay
        elif fighter.action == self.air_start:
            fighter.velocity = (
                fighter.velocity[0] / attributes.entry_speed_div,
                0.0,
            )
            fighter.max_jumps()
            fighter.action_state.gravity_delay = attributes.gravity_delay
        elif fighter.action == self.ground_end:
            fighter.ground_velocity = attributes.ground_end_speed * fighter.facing
            fighter.action_state.gravity_delay = attributes.end_gravity_delay
        elif fighter.action == self.air_end:
            fighter.velocity = (attributes.air_end_speed * fighter.facing, 0.0)
            fighter.action_state.gravity_delay = attributes.end_gravity_delay

    @hook.animation_end(
        ground_dash,
        air_dash,
        air_end,
    )
    def animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        resource = ctx.resource("side")
        if resource is None:
            return

        if fighter.action == self.ground_dash:
            self._enter_end(fighter, resource)
        elif fighter.action == self.air_dash:
            self._enter_end(fighter, resource)
        elif fighter.action == self.air_end:
            attributes = resource_attributes(ctx, resource)
            fighter.enter_fall_special(
                mobility=attributes.freefall_mobility,
                landing_lag=attributes.landing_lag,
            )

    @hook.landed(air_end)
    def landed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        resource = ctx.resource("side")
        escape_air = ctx.resource("escape_air")
        if resource is None or escape_air is None:
            return False
        attributes = resource_attributes(ctx, resource)
        fighter.enter_landing_special(
            escape_air.landing_animation_end,
            attributes.landing_lag,
        )
        return True

    def _enter_end(self, fighter: Fighter, resource) -> None:
        attributes = resource_attributes(None, resource)
        if fighter.grounded:
            fighter.ground_velocity = attributes.ground_end_speed * fighter.facing
            fighter.change_action(self.ground_end)
        else:
            fighter.velocity[0] = attributes.air_end_speed * fighter.facing
            fighter.velocity[1] = 0.0
            fighter.change_action(self.air_end)
        fighter.action_state.gravity_delay = attributes.end_gravity_delay

    @hook.validate
    def validate(self, ctx: MoveContext) -> bool:
        resource = ctx.resource("side")
        if resource is None:
            return True
        rules = special_rules(ctx)
        if rules is None:
            return False

        if not validation.fields(
            rules,
            finite=(
                "side_stick_threshold",
                "turn_threshold",
                "vertical_threshold",
                "air_drift_recovery_step",
            ),
        ):
            return False
        if not validation.number(resource.ground_speed_retention, True):
            return False
        if not 0.0 <= resource.ground_speed_retention <= 1.0:
            return False

        attributes = resource_attributes(ctx, resource)
        if not validation.fields(
            attributes,
            nonnegative=(
                "gravity_delay",
                "start_air_friction",
                "end_ground_friction",
                "end_air_friction",
                "end_gravity_delay",
                "freefall_mobility",
            ),
            finite=(
                "entry_speed_div",
                "start_fall_accel",
                "ground_end_speed",
                "air_end_speed",
                "end_fall_accel",
                "landing_lag",
            ),
        ):
            return False
        if attributes.entry_speed_div == 0.0 or attributes.landing_lag <= 0.0:
            return False

        if ctx.array_length("side.dash.ground_trans_n") != ctx.frames(
            "side.dash.ground"
        ):
            return False
        if ctx.array_length("side.dash.air_trans_n") != ctx.frames(
            "side.dash.air"
        ):
            return False
        for value in resource.dash.ground_trans_n:
            if value is not None and not validation.finite(value):
                return False
        for value in resource.dash.air_trans_n:
            if (
                value is None
                or len(value) != 2
                or not validation.finite(value[0])
                or not validation.finite(value[1])
            ):
                return False

        # The ghost trace was added after the original fixture.  Validate it
        # when supplied, without making old hitbox-free packs invent a trace.
        if ctx.resource("side.script") is not None:
            if not validation.command_trace(
                ctx, "side.script.dash.ground", "side.dash.ground"
            ):
                return False
            if not validation.command_trace(
                ctx, "side.script.dash.air", "side.dash.air"
            ):
                return False
        return True



class FireFox(SpecialMove):
    resource = "up"

    hold_air_motion = motion.profile(
        air=(
            motion.gravity(
                acceleration=bind_resource("up.attributes.hold_fall_accel"),
                terminal_velocity=parameter("movement.terminal_velocity"),
                delay=bind_resource("up.attributes.gravity_delay"),
            ),
            motion.air_friction(
                amount=bind_resource("up.attributes.hold_air_friction"),
            ),
        ),
    )
    travel_ground_motion = motion.profile(
        ground=(
            motion.ground_friction_after(
                starts_at=bind_resource("up.attributes.duration_end"),
                before=0.0,
                after=bind_resource("up.attributes.reverse_accel"),
            ),
        ),
    )
    travel_air_motion = motion.profile(
        air=(
            motion.directional_acceleration(
                starts_at=bind_resource("up.attributes.duration_end"),
                magnitude=bind_resource("up.attributes.reverse_accel"),
            ),
        ),
    )
    landing_motion = motion.profile(
        ground=(
            motion.ground_friction(
                amount=bind_resource("up.attributes.landing_friction"),
            ),
        ),
    )
    bound_motion = motion.profile(
        air=(
            motion.velocity_track(
                path=bind_resource("up.bound.transn_y"),
                component="y",
                end="no_op",
            ),
            motion.drift_clamp(
                maximum=parameter("movement.air_drift_max"),
                acceleration=bind_resource("up.attributes.air_drift_clamp_accel"),
            ),
        ),
    )
    hold_ground = action(
        Action.SPECIAL_HI_HOLD,
        slippi_state=353,
        animation=307,
        attack="up.hold.ground",
    )
    hold_air = action(
        Action.SPECIAL_HI_HOLD_AIR,
        slippi_state=354,
        animation=308,
        ledge_catchable=True,
        attack="up.hold.air",
        motion=hold_air_motion,
        profile_delay_field="gravity_delay",
    )
    travel_ground = action(
        Action.SPECIAL_HI,
        slippi_state=355,
        animation=309,
        animation_loop=True,
        attack="up.travel.ground",
        motion=travel_ground_motion,
    )
    travel_air = action(
        Action.SPECIAL_AIR_HI,
        slippi_state=356,
        animation=310,
        ledge_catchable=True,
        animation_loop=True,
        wants_redirect=True,
        attack="up.travel.air",
        motion=travel_air_motion,
    )
    landing = action(
        Action.SPECIAL_HI_LANDING,
        slippi_state=357,
        animation=311,
        attack="up.landing",
        motion=landing_motion,
    )
    fall = action(
        Action.SPECIAL_HI_FALL,
        slippi_state=358,
        animation=312,
        ledge_catchable=True,
        attack="up.fall",
    )
    bound = action(
        Action.SPECIAL_HI_BOUND,
        slippi_state=359,
        animation=313,
        collision_mode="clamp",
        ledge_catchable=True,
        attack="up.bound.pose",
        motion=bound_motion,
    )
    ground_time = clock(field="ground_travel_frames", actions=(travel_ground,))

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        resource = ctx.resource("up")
        rules = special_rules(ctx)
        if resource is None or rules is None or not ctx.input.just_pressed(Button.B):
            return False

        attributes = resource_attributes(ctx, resource)
        if fighter.action in (
            self.hold_ground,
            self.hold_air,
            self.travel_ground,
            self.travel_air,
            self.landing,
            self.fall,
            self.bound,
        ):
            return True

        if ctx.ground_open and fighter.grounded:
            if fighter.locomotion.up_special_b_age != 0:
                return False
            ground_velocity = fighter.ground_velocity / attributes.entry_speed_div
            start_action(fighter, self.hold_ground)
            fighter.ground_velocity = ground_velocity
            fighter.action_state.gravity_delay = attributes.gravity_delay
            return True

        if not (ctx.air_open and not fighter.grounded):
            return False
        if not (ctx.input.stick[1] >= rules.vertical_threshold):
            return False

        velocity_x = fighter.velocity[0] / attributes.entry_speed_div
        start_action(fighter, self.hold_air)
        fighter.set_velocity(velocity_x, 0.0)
        fighter.action_state.gravity_delay = attributes.gravity_delay
        return True

    @hook.action_enter(
        hold_ground,
        hold_air,
        travel_ground,
        travel_air,
        landing,
        fall,
        bound,
    )
    def action_entered(self, fighter: Fighter, ctx: MoveContext) -> None:
        resource = ctx.resource("up")
        if resource is None:
            return
        attributes = resource_attributes(ctx, resource)

        if fighter.action == self.travel_air:
            fighter.set_motion_binding(
                MotionBinding(
                    facing=fighter.facing,
                    cosine=math.cos(fighter.action_state.travel_angle),
                    sine=math.sin(fighter.action_state.travel_angle),
                    ground_scale=1.0,
                )
            )
        elif fighter.action == self.bound:
            fighter.velocity[0] = fighter.velocity[0] * attributes.bound_speed_mul
        elif fighter.action == self.fall:
            fighter.clear_special_effect()

    @hook.animation_end(hold_ground, hold_air)
    def hold_animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        attributes = ctx.resource("up.attributes")
        if attributes is None:
            return

        if fighter.action == self.hold_ground:
            stick = ctx.input.stick
            magnitude = abs(stick[0]) + abs(stick[1])
            angle = math.angle_xy(fighter.floor_normal, stick)
            along_floor = (
                not (magnitude < attributes.direction_stick_min)
                and not (angle < math.HALF_PI)
                and not ctx.on_platform
            )
            if along_floor:
                fighter.facing = math.facing(stick[0])
                fighter.action_state.ground_travel_frames = 0.0
                fighter.change_action(self.travel_ground)
                fighter.action_state.travel_remaining = attributes.duration
                fighter.ground_velocity = attributes.speed * fighter.facing
                normal_x = -fighter.floor_normal[0] * fighter.facing
                fighter.action_state.travel_angle = math.atan2(
                    normal_x,
                    fighter.floor_normal[1],
                )
                fighter.set_motion_binding(
                    MotionBinding(
                        facing=fighter.facing,
                        cosine=math.cos(fighter.action_state.travel_angle),
                        sine=math.sin(fighter.action_state.travel_angle),
                        ground_scale=1.0,
                    )
                )
                return

            fighter.grounded = False
            fighter.ground_line = None
            fighter.ground_velocity = 0.0
            self._launch_air(fighter, ctx)
            return

        self._launch_air(fighter, ctx)

    @hook.countdown("travel_remaining", travel_ground, travel_air, phase="animation")
    def travel_deadline(self, fighter: Fighter, ctx: MoveContext) -> None:
        if fighter.action == self.travel_ground:
            fighter.change_action(self.landing)
        elif fighter.action == self.travel_air:
            fighter.change_action(self.fall)

    @hook.animation_end(landing, fall, bound)
    def terminal_animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        attributes = ctx.resource("up.attributes")
        if attributes is None:
            return

        if fighter.action == self.landing:
            fighter.change_action(Action.WAIT)
        elif fighter.action == self.fall:
            fighter.enter_fall_special(
                mobility=attributes.freefall_mobility,
                landing_lag=attributes.landing_lag,
            )
        elif fighter.action == self.bound:
            if fighter.grounded:
                fighter.change_action(Action.WAIT)
            else:
                fighter.enter_fall_special(
                    mobility=attributes.freefall_mobility,
                    landing_lag=attributes.landing_lag,
                )

    @hook.marker("bound_exit", track="up.bound.exit_flags", actions=(bound,))
    def bound_exit_marker(self, fighter: Fighter, ctx: MoveContext) -> None:
        if fighter.grounded:
            return
        attributes = ctx.resource("up.attributes")
        if attributes is None:
            return
        fighter.enter_fall_special(
            mobility=attributes.freefall_mobility,
            landing_lag=attributes.landing_lag,
        )

    @hook.landed(travel_air, fall, bound)
    def landed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if fighter.action == self.bound:
            return True
        if fighter.action == self.fall:
            fighter.change_action(self.landing)
            # The native transition contributes the generic frame advance;
            # the source Fall collision path's extra entry advance leaves the
            # landing animation observed at frame 15 on this same step.
            fighter.action_frame = 14
            return True
        if fighter.action != self.travel_air:
            return False

        attributes = ctx.resource("up.attributes")
        if attributes is None:
            return False
        if (
            fighter.action_state.ground_travel_frames < attributes.bounce_frames
            and ctx.on_platform
        ):
            fighter.change_action(self.travel_ground, preserve_state=True)
            return True

        gate = math.DEG_TO_RAD * (90.0 + attributes.bound_angle_degrees)
        if not (math.angle_xy(fighter.floor_normal, ctx.pre_landing.velocity) < gate):
            start_action(fighter, self.bound)
            fighter.velocity[0] = fighter.velocity[0] * attributes.bound_speed_mul
            return True

        fighter.restore_pre_landing()
        fighter.facing = math.facing(fighter.velocity[0])
        facing_velocity = fighter.velocity[0] * fighter.facing
        fighter.action_state.travel_angle = math.atan2(
            fighter.velocity[1],
            facing_velocity,
        )
        fighter.set_motion_binding(
            MotionBinding(
                facing=fighter.facing,
                cosine=math.cos(fighter.action_state.travel_angle),
                sine=math.sin(fighter.action_state.travel_angle),
                ground_scale=1.0,
            )
        )
        return True

    @hook.ground_air_changed(hold_ground, hold_air, travel_ground)
    def ground_air_changed(self, fighter: Fighter, ctx: MoveContext) -> None:
        destination = None
        if fighter.action == self.hold_ground and not ctx.grounded:
            destination = self.hold_air
        elif fighter.action == self.hold_air and ctx.grounded:
            destination = self.hold_ground
        elif fighter.action == self.travel_ground and not ctx.grounded:
            destination = self.travel_air
        if destination is None:
            return None
        fighter.change_action(destination, preserve_state=True, keep_frame=True)
        return None

    @hook.surface_contact(travel_ground, travel_air)
    def surface_contact(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if fighter.action == self.travel_ground:
            normal = fighter.floor_normal
            fighter.action_state.travel_angle = math.atan2(
                -normal[0] * fighter.facing,
                normal[1],
            )
            fighter.set_motion_binding(
                MotionBinding(
                    facing=fighter.facing,
                    cosine=math.cos(fighter.action_state.travel_angle),
                    sine=math.sin(fighter.action_state.travel_angle),
                    ground_scale=1.0,
                )
            )
            return True

        surface = ctx.ceiling if ctx.ceiling is not None else ctx.wall
        if surface is None:
            return False
        attributes = ctx.resource("up.attributes")
        if attributes is None:
            return False

        gate = math.DEG_TO_RAD * (90.0 + attributes.bound_angle_degrees)
        if not (math.angle_xy(surface.normal, fighter.velocity) < gate):
            return False
        fighter.facing = math.facing(fighter.velocity[0])
        facing_velocity = fighter.velocity[0] * fighter.facing
        fighter.action_state.travel_angle = math.atan2(
            fighter.velocity[1],
            facing_velocity,
        )
        fighter.set_motion_binding(
            MotionBinding(
                facing=fighter.facing,
                cosine=math.cos(fighter.action_state.travel_angle),
                sine=math.sin(fighter.action_state.travel_angle),
                ground_scale=1.0,
            )
        )
        return True

    @hook.validate
    def validate(self, ctx: MoveContext) -> bool:
        resource = ctx.resource()
        attributes = resource_attributes(ctx, resource)
        required = (
            "gravity_delay",
            "entry_speed_div",
            "hold_air_friction",
            "hold_fall_accel",
            "direction_stick_min",
            "duration",
            "duration_end",
            "speed",
            "reverse_accel",
            "landing_friction",
            "bound_speed_mul",
            "facing_stick_min",
            "freefall_mobility",
            "landing_lag",
            "bound_angle_degrees",
            "air_drift_clamp_accel",
            "bounce_frames",
        )
        if not validation.fields(
            attributes,
            finite=required,
            nonnegative=(
                "gravity_delay",
                "hold_air_friction",
                "direction_stick_min",
                "duration_end",
                "reverse_accel",
                "landing_friction",
                "facing_stick_min",
                "freefall_mobility",
                "air_drift_clamp_accel",
            ),
            positive=("duration", "landing_lag"),
        ):
            return False
        if attributes.entry_speed_div == 0:
            return False

        bound = ctx.resource("up.bound")
        poses = ctx.frames("up.bound.pose")
        if bound is None or bound.transn_y is None or bound.exit_flags is None:
            return False
        if len(bound.transn_y) != poses or len(bound.exit_flags) != poses:
            return False
        for value in bound.transn_y:
            if not validation.finite(value):
                return False
        for value in bound.exit_flags:
            if value != True and value != False:
                return False
        return True

    def _launch_air(self, fighter: Fighter, ctx: MoveContext) -> None:
        attributes = ctx.resource().attributes
        stick = ctx.input.stick
        minimum = attributes.direction_stick_min
        magnitude = abs(stick[0]) + abs(stick[1])
        angle = math.HALF_PI
        if magnitude >= minimum:
            if abs(stick[0]) > attributes.facing_stick_min:
                fighter.facing = math.facing(stick[0])
            angle = math.atan2(stick[1], stick[0] * fighter.facing)

        fighter.change_action(self.travel_air)
        fighter.action_state.travel_angle = angle
        fighter.set_motion_binding(
            MotionBinding(
                facing=fighter.facing,
                cosine=math.cos(angle),
                sine=math.sin(angle),
                ground_scale=1.0,
            )
        )
        fighter.action_state.travel_remaining = attributes.duration
        fighter.action_state.ground_travel_frames = 0.0
        # Keep ``facing * (speed * cos(angle))`` grouped like the source.
        fighter.set_velocity(
            fighter.facing * (attributes.speed * math.cos(angle)),
            attributes.speed * math.sin(angle),
        )
        fighter.max_jumps()



class Shine(SpecialMove):
    """Fox/Falco's five-phase down special (Reflector)."""

    resource = "down"

    air_motion = motion.profile(
        air=(
            motion.gravity(
                acceleration=bind_resource("down.attributes.fall_accel"),
                terminal_velocity=parameter("movement.terminal_velocity"),
                delay=bind_resource("down.attributes.gravity_delay"),
            ),
            motion.drift_or_friction(
                recovery_step=parameter("rules.specials.air_drift_recovery_step"),
            ),
        ),
    )

    ground_start = action(
        Action.SPECIAL_LW_START,
        slippi_state=360,
        animation=314,
        attack="down.start.ground",
    )
    ground_loop = action(
        Action.SPECIAL_LW,
        slippi_state=361,
        animation=315,
        # The reflector's loop animation cycles while the action frame keeps
        # advancing for release-lag, turn, and jump decisions.
        animation_loop=True,
        attack="down.loop_phase.ground",
    )
    ground_hit = action(
        Action.SPECIAL_LW_HIT,
        slippi_state=362,
        animation=316,
        attack="down.hit.ground",
    )
    ground_end = action(
        Action.SPECIAL_LW_END,
        slippi_state=363,
        animation=317,
        attack="down.end.ground",
    )
    ground_turn = action(
        Action.SPECIAL_LW_TURN,
        slippi_state=364,
        animation=315,
        attack="down.turn.ground",
    )

    air_start = action(
        Action.SPECIAL_AIR_LW_START,
        slippi_state=365,
        animation=319,
        attack="down.start.air",
        motion=air_motion,
    )
    air_loop = action(
        Action.SPECIAL_AIR_LW,
        slippi_state=366,
        animation=320,
        animation_loop=True,
        attack="down.loop_phase.air",
        motion=air_motion,
    )
    air_hit = action(
        Action.SPECIAL_AIR_LW_HIT,
        slippi_state=367,
        animation=321,
        attack="down.hit.air",
        motion=air_motion,
    )
    air_end = action(
        Action.SPECIAL_AIR_LW_END,
        slippi_state=368,
        animation=322,
        attack="down.end.air",
        motion=air_motion,
    )
    air_turn = action(
        Action.SPECIAL_AIR_LW_TURN,
        slippi_state=369,
        animation=320,
        attack="down.turn.air",
        motion=air_motion,
    )

    _GROUND_PHASES = (
        ground_start,
        ground_loop,
        ground_hit,
        ground_end,
        ground_turn,
    )
    _AIR_PHASES = (air_start, air_loop, air_hit, air_end, air_turn)
    _PHASES = (
        ground_start,
        ground_loop,
        ground_hit,
        ground_end,
        ground_turn,
        air_start,
        air_loop,
        air_hit,
        air_end,
        air_turn,
    )
    _START_PHASES = (ground_start, air_start)
    _LOOP_PHASES = (ground_loop, air_loop)
    _TURN_PHASES = (ground_turn, air_turn)
    _HIT_PHASES = (ground_hit, air_hit)
    _END_PHASES = (ground_end, air_end)
    _ACTIVE_PHASES = (
        ground_loop,
        air_loop,
        ground_turn,
        air_turn,
        ground_hit,
        air_hit,
    )

    @hook.action_enter(*_START_PHASES)
    def enter_start(self, fighter: Fighter, ctx: MoveContext) -> None:
        # Fresh input owns resource initialization; transfer preserves fields.
        fighter.action_state.looping = False
        # A platform drop enters the air start through a preserving pass
        # transition and must retain the reflector for that frame. Grounded
        # starts are fresh input and clear any stale reflector state.
        if fighter.action != self.air_start:
            fighter.flags.reflecting = False

    @hook.action_enter(*_END_PHASES)
    def enter_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        fighter.flags.reflecting = False

    @hook.action_exit(*_PHASES)
    def exit_phase(self, fighter: Fighter, ctx: MoveContext) -> None:
        preserving_platform_drop = (
            fighter.action == self.air_start
        )
        phase_action = (
            fighter.action == self.ground_start
            or fighter.action == self.ground_loop
            or fighter.action == self.ground_turn
            or fighter.action == self.ground_hit
            or fighter.action == self.ground_end
            or fighter.action == self.air_start
            or fighter.action == self.air_loop
            or fighter.action == self.air_turn
            or fighter.action == self.air_hit
            or fighter.action == self.air_end
        )
        if not phase_action and not preserving_platform_drop:
            fighter.flags.reflecting = False

    @hook.input_pressed(Button.B)
    def press_b(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if fighter.action in self._PHASES:
            return True
        rules = special_rules(ctx)
        resource = ctx.resource()
        if rules is None or not ctx.input.just_pressed(Button.B):
            return False

        threshold = rules.vertical_threshold
        grounded_start = (
            fighter.grounded
            and ctx.ground_open
            and ctx.input.stick[1] < -threshold
        )
        aerial_start = (
            not fighter.grounded
            and ctx.air_open
            and ctx.input.stick[1] <= -threshold
        )
        if not (grounded_start or aerial_start):
            return False

        attributes = ctx.resource("down.attributes")
        start_action(
            fighter,
            self.ground_start if grounded_start else self.air_start,
        )
        fighter.action_state.release_lag = attributes.release_lag
        fighter.action_state.gravity_delay = attributes.gravity_delay
        fighter.action_state.is_release = False
        fighter.action_state.looping = False
        if aerial_start:
            fighter.velocity[1] = 0
            fighter.velocity[0] = fighter.velocity[0] / attributes.air_momentum_div
        return True

    @hook.input_released(
        Button.B,
        actions=(
            ground_loop,
            air_loop,
            ground_turn,
            air_turn,
            ground_hit,
            air_hit,
            ground_start,
            air_start,
        ),
    )
    def release_b(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if fighter.action not in self._PHASES:
            return False
        fighter.action_state.is_release = True
        # A delivered deadline cannot be revisited, so late release exits here.
        if (
            fighter.action in self._LOOP_PHASES
            and fighter.action_state.release_lag <= 0
        ):
            self._enter_end(fighter)
        return True

    @hook.stick_changed(actions=_LOOP_PHASES)
    def stick_changed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if fighter.action not in self._LOOP_PHASES:
            return False
        if self._turn_requested(fighter, ctx):
            self._enter_turn(fighter, ctx)
            return True
        return self._try_jump(fighter, ctx)

    @hook.input_pressed(actions=_LOOP_PHASES)
    def press_jump(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if fighter.action not in self._LOOP_PHASES:
            return False
        if self._turn_requested(fighter, ctx):
            self._enter_turn(fighter, ctx)
            return True
        return self._try_jump(fighter, ctx)

    @hook.action_availability_changed(gate="jump_available", actions=_LOOP_PHASES)
    def jump_availability(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if not ctx.available or fighter.action not in self._LOOP_PHASES:
            return False
        if self._turn_requested(fighter, ctx):
            self._enter_turn(fighter, ctx)
            return True
        return self._try_jump(fighter, ctx)

    @hook.animation_end(*_START_PHASES, *_END_PHASES, *_HIT_PHASES)
    def animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        if fighter.action in self._START_PHASES:
            self._enter_loop(fighter)
        elif fighter.action in self._END_PHASES:
            fighter.change_action(Action.WAIT if fighter.grounded else Action.FALL)
        elif fighter.action in self._HIT_PHASES:
            self._complete_hit_or_loop(fighter)

    @hook.animation_end(*_LOOP_PHASES)
    def loop_last_phase(self, fighter: Fighter, ctx: MoveContext) -> None:
        # Kept for non-looping resource definitions.  The production loop
        # descriptors use ``animation_loop`` so the native sampler cycles
        # poses without resetting the action frame or dispatching this hook.
        fighter.action_state.looping = True

    @hook.countdown("release_lag", *_ACTIVE_PHASES)
    def release_lag_elapsed(self, fighter: Fighter, ctx: MoveContext) -> None:
        # Loop's source IASA exits immediately when both predicates hold.
        # Turn and Hit defer the same decision to their own finite boundary.
        if fighter.action in self._LOOP_PHASES and fighter.action_state.is_release:
            self._enter_end(fighter)

    @hook.countdown("turn_frames", *_TURN_PHASES)
    def turn_frames_elapsed(self, fighter: Fighter, ctx: MoveContext) -> None:
        self._complete_hit_or_loop(fighter)

    @hook.projectile_contact
    def projectile_contact(self, fighter: Fighter, hit: HitContext) -> None:
        if fighter.flags.reflecting and hit.projectile and hit.damage <= hit.max_damage:
            hit.reflect = True

    @hook.landed(*_AIR_PHASES)
    def landed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if fighter.action not in self._AIR_PHASES or not ctx.grounded:
            return False
        return self._transfer(fighter, True)

    @hook.ground_air_changed(*_PHASES)
    def ground_air_changed(self, fighter: Fighter, ctx: MoveContext) -> None:
        self._transfer(fighter, ctx.grounded)

    @hook.platform_drop_decision(ground_start, ground_loop)
    def platform_drop(self, fighter: Fighter, ctx: MoveContext) -> bool:
        locomotion = ctx.resource("locomotion")
        eligible = (
            ctx.on_platform
            and locomotion is not None
            and ctx.input.stick[1] <= -locomotion.pass_stick_threshold
            and fighter.locomotion.tilt_y_age < locomotion.pass_window
        )
        if not eligible:
            return False

        destination = self.air_start if fighter.action == self.ground_start else self.air_loop
        # pass_as resets action variables and preserves the current frame.
        if not ctx.pass_as(destination, locomotion.pass_velocity, keep_frame=True):
            return False
        fighter.flags.reflecting = True
        return True

    def _turn_requested(self, fighter: Fighter, ctx: MoveContext) -> bool:
        locomotion = ctx.resource("locomotion")
        if locomotion is None:
            return False
        return ctx.input.stick[0] * fighter.facing <= locomotion.turn_threshold

    def _try_jump(self, fighter: Fighter, ctx: MoveContext) -> bool:
        jump_source = ctx.jump_input()
        if fighter.grounded:
            if jump_source is None:
                return False
            fighter.short_hop = False
            fighter.locomotion.jump_input = jump_source
            fighter.change_action(Action.JUMP_SQUAT, preserve_state=True)
            fighter.action_frame = 0
            return True
        return ctx.aerial_jump() if jump_source is not None else False

    def _enter_loop(self, fighter: Fighter) -> None:
        destination = self.ground_loop if fighter.grounded else self.air_loop
        self._move_transition(fighter, destination)
        fighter.flags.reflecting = True

    def _enter_turn(self, fighter: Fighter, ctx: MoveContext) -> None:
        attributes = ctx.resource("down.attributes")
        destination = self.ground_turn if fighter.grounded else self.air_turn
        self._move_transition(fighter, destination)
        fighter.action_state.turn_frames = attributes.turn_frames - 1
        command = fighter.action_state.command
        fighter.action_state.command = (1, command[1], command[2], command[3])
        fighter.action_state.turned = True
        fighter.facing = -fighter.facing
        fighter.flags.reflecting = True

    def _enter_end(self, fighter: Fighter) -> None:
        destination = self.ground_end if fighter.grounded else self.air_end
        self._move_transition(fighter, destination)

    def _transfer(self, fighter: Fighter, grounded: bool) -> bool:
        destination = self._surface_destination(fighter.action, grounded)
        if destination is None:
            return False
        destination_grounded = destination in self._GROUND_PHASES
        if destination_grounded != grounded:
            return False
        fighter.change_action(destination, preserve_state=True, keep_frame=True)
        if destination in self._ACTIVE_PHASES:
            fighter.flags.reflecting = True
        return True

    def _complete_hit_or_loop(self, fighter: Fighter) -> None:
        released = (
            fighter.action_state.release_lag <= 0
            and fighter.action_state.is_release
        )
        if released:
            self._enter_end(fighter)
        else:
            self._enter_loop(fighter)

    def _move_transition(self, fighter: Fighter, destination) -> None:
        fighter.change_action(
            destination,
            preserve_fields=("release_lag", "is_release", "gravity_delay"),
        )

    def _surface_destination(self, current, grounded):
        if current == self.ground_start:
            destination = self.air_start
        elif current == self.air_start:
            destination = self.ground_start
        elif current == self.ground_loop:
            destination = self.air_loop
        elif current == self.air_loop:
            destination = self.ground_loop
        elif current == self.ground_turn:
            destination = self.air_turn
        elif current == self.air_turn:
            destination = self.ground_turn
        elif current == self.ground_hit:
            destination = self.air_hit
        elif current == self.air_hit:
            destination = self.ground_hit
        elif current == self.ground_end:
            destination = self.air_end
        elif current == self.air_end:
            destination = self.ground_end
        else:
            return None
        destination_is_grounded = destination in self._GROUND_PHASES
        return destination if destination_is_grounded == grounded else None

    @hook.validate
    def validate(self, ctx: MoveContext) -> bool:
        resource = ctx.resource()
        rules = special_rules(ctx)
        locomotion = ctx.resource("locomotion")
        if rules is None or locomotion is None:
            return False

        attributes = resource_attributes(ctx, resource)
        if not validation.fields(
            attributes,
            finite=(
                "release_lag",
                "turn_frames",
                "gravity_delay",
                "air_momentum_div",
                "fall_accel",
            ),
        ):
            return False
        if attributes.release_lag < 0 or attributes.turn_frames <= 0:
            return False
        if attributes.gravity_delay < 0 or attributes.air_momentum_div == 0:
            return False
        if attributes.fall_accel < 0:
            return False

        reflect = ctx.resource("down.reflect")
        if reflect is None:
            return False
        if not validation.number(reflect.bone, True):
            return False
        if not validation.number(reflect.max_damage, True):
            return False
        if reflect.offset is None or len(reflect.offset) != 3:
            return False
        if not validation.number(reflect.size, True):
            return False
        if not validation.finite(reflect.damage_mul):
            return False
        if not validation.finite(reflect.speed_mul):
            return False
        if not validation.number(reflect.behavior, True):
            return False
        for value in reflect.offset:
            if not validation.finite(value):
                return False

        return True




@fighter
class Fox(FighterBase):
    """Fox's resource-backed native fighter definition."""

    name = "fox"
    external_ids = (2,)
    parameters = FoxParameters
    attributes = FoxParameters
    action_state = FoxActionState

    specials = SpecialMoves(Blaster(), Illusion(), FireFox(), Shine())


__all__ = ["Fox", "Blaster", "Illusion", "FireFox", "Shine", "FoxActionState", "FoxParameters"]
