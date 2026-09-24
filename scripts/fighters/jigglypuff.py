"""Jigglypuff's source-defined special motion families."""

from fighter import JigglypuffPoundAttribute, JigglypuffRolloutAttribute

from skirmish import (
    Action,
    Button,
    CommonParameter,
    Fighter,
    SideSpecial,
    UpSpecial,
    DownSpecial,
    NeutralSpecial,
    Transition,
    frame_preserving_surface_pairs,
    f32,
    fresh_b_input,
    hook,
    math,
    motion,
    parameter,
    select_facing_phase,
    source_phase,
    special_attribute,
    special_rules,
    start_complete_special,
    start_action,
    directional_match,
    validation,
    velocity_from_angle,
)


def _phase_action(state: int):
    return source_phase(state)


def _clear_command_zero(fighter: Fighter) -> None:
    """Clear the first animation command slot on native special entry.

    ``ftPurin_SpecialHi_SetVars`` and ``ftPr_SpecialLw_Enter`` both clear
    ``cmd_vars[0]`` before their animation command stream runs.  The script
    host represents that slot as the first value of ``action_state.command``;
    keep authoring and the native entry callback aligned when that state is
    exposed by a host.
    """
    action_state = getattr(fighter, "action_state", None)
    command = getattr(action_state, "command", None)
    if isinstance(command, (tuple, list)) and command:
        action_state.command = (0, *command[1:])


def _clear_command_slots(fighter: Fighter) -> None:
    """Clear all native command variables on Rollout/Pound entry."""
    action_state = getattr(fighter, "action_state", None)
    command = getattr(action_state, "command", None)
    if isinstance(command, (tuple, list)) and len(command) == 4:
        action_state.command = (0, 0, 0, 0)


class Roll(NeutralSpecial):
    """Rollout's native charge, release, turn, and terminal phases.

    The native callbacks own scale, hit capsule, and wall interaction.  The
    script still exposes the complete motion graph so those callbacks can be
    supplied by a host while the ordinary runtime gets the same input and
    surface lifecycle.
    """

    ground_start_left = _phase_action(346)
    ground_start_right = _phase_action(347)
    ground_loop = source_phase(348, animation_loop=True)
    ground_full = source_phase(349, animation_loop=True)
    ground_release = _phase_action(350)
    ground_turn = _phase_action(351)
    ground_end_left = _phase_action(352)
    ground_end_right = _phase_action(353)
    air_start_left = _phase_action(354)
    air_start_right = _phase_action(355)
    air_loop = source_phase(356, animation_loop=True)
    air_full = source_phase(357, animation_loop=True)
    air_release = _phase_action(358)
    air_turn = _phase_action(359)
    air_end_left = _phase_action(360)
    air_end_right = _phase_action(361)
    hit = _phase_action(362)

    _START = (ground_start_left, ground_start_right, air_start_left, air_start_right)
    _CHARGE = (ground_loop, ground_full, air_loop, air_full)
    _ACTIVE = _START + _CHARGE + (
        ground_release, ground_turn, ground_end_left, ground_end_right,
        air_release, air_turn, air_end_left, air_end_right, hit,
    )

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx) -> bool:
        if fighter.action in self._ACTIVE:
            return True
        if not fresh_b_input(ctx) or not directional_match(ctx, self.root):
            return False
        if not (ctx.ground_open or ctx.air_open):
            return False
        if ctx.ground_open:
            phase, state = (
                (self.ground_start_left, 346)
                if fighter.facing < 0 else (self.ground_start_right, 347)
            )
        else:
            phase, state = (
                (self.air_start_left, 354)
                if fighter.facing < 0 else (self.air_start_right, 355)
            )
        if not fighter.has_complete_animation(state):
            return False
        # ftPr_SpecialN_Enter clears cmd_vars[0..3] before the start motion.
        _clear_command_slots(fighter)
        start_action(fighter, phase)
        return True

    @hook.input_released(Button.B)
    def release(self, fighter: Fighter, ctx) -> None:
        if fighter.action in (self.ground_loop, self.ground_full):
            fighter.change_action(self.ground_release, preserve_state=True, keep_frame=True)
        elif fighter.action in (self.air_loop, self.air_full):
            fighter.change_action(self.air_release, preserve_state=True, keep_frame=True)

    @staticmethod
    def _attribute(fighter, attribute, default=None):
        getter = getattr(fighter, "special_attribute", None)
        if getter is None:
            return default
        value = getter(attribute)
        return default if value is None or not validation.finite(value) else value

    @hook.stick_changed(actions=(ground_release,))
    def reverse(self, fighter: Fighter, ctx) -> bool:
        """Enter Rollout's ground turn phase for opposite-facing input."""
        if fighter.action != self.ground_release:
            return False
        stick = getattr(getattr(ctx, "input", None), "stick", (0.0, 0.0))
        rules = special_rules(ctx)
        threshold = getattr(rules, "side_stick_threshold", None) if rules else None
        if threshold is None:
            return False
        if stick[0] * fighter.facing > -threshold:
            return False
        fighter.change_action(self.ground_turn, preserve_state=True, keep_frame=True)
        fighter.facing = -fighter.facing
        return True

    @hook.animation_end(*_START)
    def start_end(self, fighter: Fighter, ctx) -> None:
        fighter.change_action(
            self.ground_loop if fighter.action in (self.ground_start_left, self.ground_start_right)
            else self.air_loop,
            preserve_state=True,
        )

    @hook.animation_end(ground_release, ground_turn, air_release, air_turn)
    def rolling_end(self, fighter: Fighter, ctx) -> None:
        grounded = fighter.action in (self.ground_release, self.ground_turn)
        left = fighter.facing < 0
        if grounded:
            fighter.change_action(self.ground_end_left if left else self.ground_end_right)
        else:
            fighter.change_action(self.air_end_left if left else self.air_end_right)

    @hook.animation_end(ground_end_left, ground_end_right, air_end_left, air_end_right)
    def terminal_end(self, fighter: Fighter, ctx) -> None:
        fighter.change_action(
            Action.WAIT if fighter.action in (self.ground_end_left, self.ground_end_right)
            else Action.FALL
        )

    @hook.after_hit(actions=(ground_release, ground_turn, air_release, air_turn))
    def after_roll_hit(self, fighter: Fighter, ctx) -> None:
        # ftPr_SpecialS_8013D764 enters state 362 after a successful roll hit.
        fighter.change_action(self.hit, preserve_state=True, keep_frame=True)

    @hook.surface_contact(ground_release, air_release)
    def wall_bounce(self, fighter: Fighter, ctx) -> bool:
        """Reverse Rollout's native release travel on a facing wall."""
        wall = getattr(ctx, "wall", None)
        if wall is None:
            wall = getattr(ctx, "wall_contact", False)
        if not wall or not hasattr(fighter, "velocity"):
            return False
        velocity = fighter.velocity
        if not isinstance(velocity, (tuple, list)) or len(velocity) < 2:
            return False
        scale = self._attribute(
            fighter, JigglypuffRolloutAttribute.WALL_SPEED_SCALE, 1.0
        )
        if hasattr(fighter, "set_velocity"):
            fighter.set_velocity(-velocity[0] * scale, velocity[1])
        fighter.facing = -fighter.facing
        if hasattr(fighter, "ground_velocity"):
            fighter.ground_velocity = -fighter.ground_velocity * scale
        state = getattr(fighter, "action_state", None)
        charge = getattr(state, "charge", None)
        if charge is not None:
            state.charge = charge * scale
        return True

    @hook.animation_end(hit)
    def hit_end(self, fighter: Fighter, ctx) -> None:
        fighter.change_action(Action.WAIT if fighter.grounded else Action.FALL)

    @hook.landed(hit)
    def hit_landed(self, fighter: Fighter, ctx) -> bool:
        """Match SpecialNHit_Coll's immediate floor-contact exit."""
        if fighter.action != self.hit:
            return False
        # ftPr_SpecialNHit_Coll calls the grounded exit when the hit capsule
        # meets the floor.  The native callback enters WAIT for the grounded
        # case and only uses FALL for an aerial hit; keeping this split also
        # prevents a grounded Rollout hit from remaining in an aerial action.
        fighter.change_action(Action.WAIT if fighter.grounded else Action.FALL)
        return True

    on_ground = {
        air_start_left: Transition(ground_start_left, preserve_state=True, keep_frame=True),
        air_start_right: Transition(ground_start_right, preserve_state=True, keep_frame=True),
        air_loop: Transition(ground_loop, preserve_state=True, keep_frame=True),
        air_full: Transition(ground_full, preserve_state=True, keep_frame=True),
        air_release: Transition(ground_release, preserve_state=True, keep_frame=True),
        air_turn: Transition(ground_turn, preserve_state=True, keep_frame=True),
        air_end_left: Transition(ground_end_left, preserve_state=True, keep_frame=True),
        air_end_right: Transition(ground_end_right, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start_left: Transition(air_start_left, preserve_state=True, keep_frame=True),
        ground_start_right: Transition(air_start_right, preserve_state=True, keep_frame=True),
        ground_loop: Transition(air_loop, preserve_state=True, keep_frame=True),
        ground_full: Transition(air_full, preserve_state=True, keep_frame=True),
        ground_release: Transition(air_release, preserve_state=True, keep_frame=True),
        ground_turn: Transition(air_turn, preserve_state=True, keep_frame=True),
        ground_end_left: Transition(air_end_left, preserve_state=True, keep_frame=True),
        ground_end_right: Transition(air_end_right, preserve_state=True, keep_frame=True),
    }


class Rest(DownSpecial):
    """Rest phases backed by the native complete-animation resources."""

    ground_left = _phase_action(369)
    air_left = _phase_action(370)
    ground_right = _phase_action(371)
    air_right = _phase_action(372)
    _ACTIVE = (ground_left, air_left, ground_right, air_right)

    @staticmethod
    def _phase(grounded: bool, facing: float):
        return select_facing_phase(
            grounded, facing,
            Rest.ground_left, Rest.ground_right, Rest.air_left, Rest.air_right,
            369, 371, 370, 372,
        )

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx) -> bool:
        started = start_complete_special(
            fighter, ctx, self._phase, active_actions=self._ACTIVE, direction=-1
        )
        if started:
            _clear_command_zero(fighter)
        return started

    # Surface changes keep the matching source motion state and native frame.
    on_ground, on_air = frame_preserving_surface_pairs(
        ground_left, ground_right, air_left, air_right
    )
    on_end = {
        ground_left: Transition(Action.WAIT),
        ground_right: Transition(Action.WAIT),
        air_left: Transition(Action.FALL),
        air_right: Transition(Action.FALL),
    }


class Sing(UpSpecial):
    """Sing's source phases, including its facing-specific animations."""

    ground_left = _phase_action(365)
    air_left = _phase_action(366)
    ground_right = _phase_action(367)
    air_right = _phase_action(368)
    _ACTIVE = (ground_left, air_left, ground_right, air_right)

    @staticmethod
    def _phase(grounded: bool, facing: float):
        return select_facing_phase(
            grounded, facing,
            Sing.ground_left, Sing.ground_right, Sing.air_left, Sing.air_right,
            365, 367, 366, 368,
        )

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx) -> bool:
        started = start_complete_special(
            fighter, ctx, self._phase, active_actions=self._ACTIVE, direction=1
        )
        if started:
            _clear_command_zero(fighter)
        return started

    on_ground, on_air = frame_preserving_surface_pairs(
        ground_left, ground_right, air_left, air_right
    )
    on_end = {
        ground_left: Transition(Action.WAIT),
        ground_right: Transition(Action.WAIT),
        air_left: Transition(Action.FALL),
        air_right: Transition(Action.FALL),
    }


_POUND_GROUND_MOTION = motion.profile(
    ground=(motion.ground_friction_above_walk(),),
)
_POUND_AIR_MOTION = motion.profile(
    air=(motion.command_branch(
        index=1,
        cases={
            0: (
                motion.gravity(
                    acceleration=parameter(CommonParameter.GRAVITY),
                    terminal_velocity=parameter(CommonParameter.TERMINAL_VELOCITY),
                    delay=0,
                ),
                motion.air_friction(amount=parameter(CommonParameter.AERIAL_FRICTION)),
            ),
            1: (
                motion.command_velocity_scale(
                    index=1,
                    value=1,
                    multiplier=special_attribute(
                        JigglypuffPoundAttribute.VELOCITY_MULTIPLIER
                    ),
                ),
            ),
            2: (
                motion.gravity(
                    acceleration=parameter(CommonParameter.GRAVITY),
                    terminal_velocity=parameter(CommonParameter.TERMINAL_VELOCITY),
                    delay=0,
                ),
                motion.drift_or_friction(
                    recovery_step=parameter(CommonParameter.AIR_DRIFT_RECOVERY_STEP)
                ),
            ),
        },
    ),),
)


class Pound(SideSpecial):
    """Pound's source command trace, launch, and recovery policy."""

    ground = source_phase(
        363,
        command_trace="side.script.ground",
        motion=_POUND_GROUND_MOTION,
    )
    air = source_phase(
        364,
        command_trace="side.script.air",
        motion=_POUND_AIR_MOTION,
    )
    _ACTIVE = (ground, air)
    _TRACE_PATHS = ("side.script.ground", "side.script.air")

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx) -> bool:
        if fighter.action in self._ACTIVE:
            return True
        if not fresh_b_input(ctx) or not (ctx.ground_open or ctx.air_open):
            return False
        rules = special_rules(ctx)
        threshold = getattr(rules, "side_stick_threshold", None) if rules else None
        if threshold is not None and abs(ctx.input.stick[0]) < threshold:
            return False
        grounded = bool(ctx.ground_open)
        phase, state = (self.ground, 363) if grounded else (self.air, 364)
        if not fighter.has_complete_animation(state):
            return False
        # ftPr_SpecialS_Enter clears cmd_vars[0..3] before the start motion.
        _clear_command_slots(fighter)
        start_action(fighter, phase)
        return True

    @hook.command_changed(0, actions=(air,))
    def launch(self, fighter: Fighter, ctx) -> None:
        if not getattr(getattr(ctx, "event", None), "value", 0):
            return
        attributes = tuple(
            fighter.special_attribute(attribute)
            for attribute in (
                JigglypuffPoundAttribute.STICK_ANGLE_MIN,
                JigglypuffPoundAttribute.STICK_ANGLE_MAX,
                JigglypuffPoundAttribute.MAX_LAUNCH_ANGLE,
                JigglypuffPoundAttribute.LAUNCH_SPEED,
            )
        )
        if any(value is None or not validation.finite(value) for value in attributes):
            return
        minimum, maximum, degrees, speed = attributes
        if maximum <= minimum:
            return
        stick_y = f32(ctx.input.stick[1])
        magnitude = min(abs(stick_y), maximum) - minimum
        magnitude = max(magnitude, 0)
        if stick_y < 0:
            magnitude = -magnitude
        angle = math.DEG_TO_RAD * (magnitude * degrees / (maximum - minimum))
        fighter.set_velocity(*velocity_from_angle(speed, angle, fighter.facing))

        command = getattr(getattr(fighter, "action_state", None), "command", None)
        if isinstance(command, (tuple, list)) and len(command) == 4:
            fighter.action_state.command = (0, command[1], command[2], command[3])

    @hook.validate
    def validate(self, ctx) -> bool:
        lookup = getattr(ctx, "resource", None)
        return lookup is not None and all(lookup(path) is not None for path in self._TRACE_PATHS)

    on_ground = {air.action: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground.action: Transition(air, preserve_state=True, keep_frame=True)}
    on_end = {
        ground.action: Transition(Action.WAIT),
        air.action: Transition(Action.FALL),
    }


class Jigglypuff(Fighter):
    specials = Fighter.specials.replace(
        neutral=Roll(),
        side=Pound(),
        up=Sing(),
        down=Rest(),
    )


__all__ = ["Jigglypuff", "Roll", "Pound", "Rest", "Sing"]
