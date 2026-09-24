"""Samus's source-defined Screw Attack movement."""

from fighter import SamusAttribute
from skirmish import (
    Action,
    Button,
    CommonParameter,
    DirectionalSpecial,
    DownSpecial,
    Fighter,
    MoveContext,
    NeutralSpecial,
    SideSpecial,
    UpSpecial,
    Transition,
    hook,
    motion,
    parameter,
    special_attribute,
    source_phase,
    start_complete_special,
)


class ChargeShot(NeutralSpecial, DirectionalSpecial):
    """Samus's source charge shot state graph.

    Article creation and charge bookkeeping remain native owned.  These
    declarations expose the exact motion states and safe animation exits.
    """

    ground_start = source_phase(343)
    ground_hold = source_phase(344, animation_loop=True)
    ground_cancel = source_phase(345)
    ground_fire = source_phase(346)
    air_start = source_phase(347)
    air_fire = source_phase(348)
    ground = ground_start
    air = air_start
    _ACTIVE = (ground_start, ground_hold, ground_cancel, ground_fire, air_start, air_fire)
    on_end = {
        ground_start: Transition(ground_hold),
        ground_cancel: Transition(Action.WAIT),
        ground_fire: Transition(Action.WAIT),
        air_start: Transition(air_fire),
    }
    on_ground = {air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
                 air_fire: Transition(ground_fire, preserve_state=True, keep_frame=True)}
    on_air = {ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
              ground_hold: Transition(air_fire, preserve_state=True, keep_frame=True),
              ground_cancel: Transition(air_fire, preserve_state=True, keep_frame=True),
              ground_fire: Transition(air_fire, preserve_state=True, keep_frame=True)}

    @hook.action_enter(ground_start, air_start)
    def enter_start(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Clear the source command latches when Charge Shot starts.

        ``ftSs_SpecialN_{,Air}N_Enter`` clears all four command variables
        before the animation callbacks begin.  Leaving a stale command 0/1
        or article event latched can otherwise make a newly started shot
        skip its charge entry or re-run a previous callback.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", None)
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, 0)

    @hook.input_pressed(Button.B, Button.L, Button.R)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        """Match the source B release and LR cancel paths while charging."""
        if fighter.action == self.ground_hold:
            input_state = getattr(ctx, "input", None)
            if input_state is not None and not input_state.just_pressed(Button.B):
                if input_state.just_pressed(Button.L) or input_state.just_pressed(Button.R):
                    fighter.change_action(self.ground_cancel)
                    return True
            fighter.change_action(self.ground_fire)
            return True
        return DirectionalSpecial.input_pressed(self, fighter, ctx)

    @hook.animation_end(air_fire)
    def finish_air_fire(self, fighter: Fighter, ctx: MoveContext) -> bool:
        """Match ``ftSs_SpecialAirN_Anim``'s landing-lag exit."""
        lag = fighter.special_attribute(SamusAttribute.SPECIAL_N_AERIAL_LANDING_LAG)
        if lag is None:
            return False
        if lag == 0:
            fighter.change_action(Action.FALL)
            return True
        enter = getattr(fighter, "enter_fall_special", None)
        if not callable(enter):
            return False
        enter(mobility=1, landing_lag=lag)
        return True


class Missile(SideSpecial, DirectionalSpecial):
    """Samus's missile states; projectile spawning is native article logic."""

    ground = source_phase(349)
    ground_smash = source_phase(350)
    air = source_phase(351)
    air_smash = source_phase(352)
    _ACTIVE = (ground, ground_smash, air, air_smash)
    on_end = {
        ground: Transition(Action.WAIT), ground_smash: Transition(Action.WAIT),
        air: Transition(Action.FALL), air_smash: Transition(Action.FALL),
    }
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True),
                 air_smash: Transition(ground_smash, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True),
              ground_smash: Transition(air_smash, preserve_state=True, keep_frame=True)}


class Bomb(DownSpecial, DirectionalSpecial):
    """Samus's bomb states; bomb article ownership stays in the native host."""

    ground = source_phase(341)
    air = source_phase(342)
    ground_bomb = source_phase(355)
    air_bomb = source_phase(356)
    _ACTIVE = (ground, air, ground_bomb, air_bomb)
    on_end = {
        ground: Transition(Action.WAIT), ground_bomb: Transition(Action.WAIT),
        air: Transition(Action.FALL), air_bomb: Transition(Action.FALL),
    }
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True),
                 air_bomb: Transition(ground_bomb, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True),
              ground_bomb: Transition(air_bomb, preserve_state=True, keep_frame=True)}


_GROUND = source_phase(
    353,
    motion=motion.profile(
        ground=(motion.ground_friction_above_walk(),),
    ),
)
_AIR = source_phase(
    354,
    motion=motion.profile(
        air=(
            motion.gravity(
                acceleration=parameter(CommonParameter.GRAVITY),
                terminal_velocity=parameter(CommonParameter.TERMINAL_VELOCITY),
                delay=0,
            ),
            motion.stick_steering(
                threshold=0.0,
                acceleration=special_attribute(SamusAttribute.SCREW_ATTACK_STEERING_ACCELERATION),
                target=special_attribute(SamusAttribute.SCREW_ATTACK_HORIZONTAL_CLAMP),
            ),
        ),
    ),
)


class ScrewAttack(UpSpecial):
    """Ground and aerial Screw Attack states, gated by native resources."""

    resource = "specials.special_attributes"
    ground = _GROUND
    air = _AIR
    _ACTIVE = (ground, air)
    _ATTRIBUTES = tuple(SamusAttribute)

    @hook.action_enter(ground, air)
    def action_enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Reset source command state and spend the aerial jump on entry.

        ``ftSs_SpecialHi_Enter`` clears all four command variables for both
        variants.  The native entry also calls the common jump reset helper;
        keep the optional call so standalone authoring fixtures remain small.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", None)
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, 0)
        # ``ftSs_SpecialAirHi_Enter`` exhausts the aerial jump budget via
        # ``ftCommon_8007D60C``.  The grounded entry instead uses
        # ``ftCommon_8007D7FC`` and must leave the budget untouched.
        max_jumps = getattr(fighter, "max_jumps", None)
        if fighter.action == self.air and callable(max_jumps):
            max_jumps()

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if fighter.action in self._ACTIVE:
            return True
        lookup = getattr(ctx, "resource", None)
        if lookup is not None and lookup(self.resource) is None:
            return False
        if any(fighter.special_attribute(attribute) is None for attribute in self._ATTRIBUTES):
            return False
        started = start_complete_special(
            fighter,
            ctx,
            lambda grounded, _: (self.ground, 353) if grounded else (self.air, 354),
            active_actions=self._ACTIVE,
            direction=1,
        )
        if started:
            self.action_enter(fighter, ctx)
            self._enter(fighter, bool(ctx.ground_open))
        return started

    @staticmethod
    def _enter(fighter: Fighter, grounded: bool) -> None:
        if grounded:
            limit = fighter.special_attribute(SamusAttribute.SCREW_ATTACK_HORIZONTAL_CLAMP)
            velocity = fighter.ground_velocity
            if limit is not None:
                velocity = max(-limit, min(limit, velocity))
            fighter.ground_velocity = velocity
            fighter.velocity = (velocity, 0.0)
            return
        velocity = fighter.velocity
        launch = fighter.special_attribute(SamusAttribute.SCREW_ATTACK_AERIAL_LAUNCH_VELOCITY)
        limit = fighter.special_attribute(SamusAttribute.SCREW_ATTACK_HORIZONTAL_CLAMP)
        if launch is not None and limit is not None:
            fighter.set_velocity(max(-limit, min(limit, velocity[0])), launch)

    @hook.stick(actions=_ACTIVE)
    def turn(self, fighter: Fighter, ctx: MoveContext) -> None:
        if fighter.action not in self._ACTIVE:
            return
        command = fighter.action_state.command
        if command[1] or command[0]:
            return
        threshold = fighter.special_attribute(SamusAttribute.SCREW_ATTACK_TURNAROUND_STICK_THRESHOLD)
        if threshold is None or abs(ctx.input.stick[0]) <= threshold:
            return
        if ctx.input.stick[0] * fighter.facing < 0:
            fighter.facing = -fighter.facing
            fighter.action_state.command = (command[0], 1, command[2], command[3])

    @hook.command_changed(0, actions=_ACTIVE)
    def launch(self, fighter: Fighter, ctx: MoveContext) -> None:
        if not getattr(getattr(ctx, "event", None), "value", 0):
            return
        # ``ftSs_SpecialHi_Phys`` calls ``ftCommon_8007D60C`` when command 0
        # launches the Screw Attack, for both grounded and aerial variants.
        # The entry callback already performs the same source operation for
        # the aerial variant; retain both calls at their source boundaries.
        max_jumps = getattr(fighter, "max_jumps", None)
        if callable(max_jumps):
            max_jumps()
        speed = fighter.special_attribute(SamusAttribute.SCREW_ATTACK_LAUNCH_HORIZONTAL_VELOCITY)
        if speed is None:
            return
        if fighter.action == self.ground:
            frame = fighter.action_frame
            fighter.change_action(self.air)
            fighter.action_frame = frame
        velocity = fighter.velocity
        fighter.set_velocity(speed * fighter.facing, velocity[1])
        command = fighter.action_state.command
        fighter.action_state.command = (0, command[1], command[2], command[3])

    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    @hook.animation_end(ground, air)
    def animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        lag = fighter.special_attribute(SamusAttribute.SCREW_ATTACK_LANDING_LAG)
        if lag is None:
            return
        if lag == 0:
            fighter.change_action(Action.FALL)
            return
        multiplier = fighter.special_attribute(
            SamusAttribute.SCREW_ATTACK_LANDING_TRANSITION_MULTIPLIER
        )
        if multiplier is not None:
            fighter.enter_fall_special(mobility=multiplier, landing_lag=lag)


class Samus(Fighter):
    specials = Fighter.specials.replace(
        neutral=ChargeShot(), side=Missile(), up=ScrewAttack(), down=Bomb()
    )


__all__ = ["Samus", "ChargeShot", "Missile", "ScrewAttack", "Bomb"]
