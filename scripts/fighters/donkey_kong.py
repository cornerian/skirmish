"""Donkey Kong's source-defined Spinning Kong phases.

Spinning Kong's source movement uses the typed Donkey Kong attribute layout;
the native animation resource still gates entry through its complete state.
"""

from fighter import DonkeyKongAttribute
from skirmish import (
    Action,
    ActionState,
    Button,
    CommonParameter,
    Fighter,
    MoveContext,
    NeutralSpecial,
    UpSpecial,
    Transition,
    hook,
    motion,
    parameter,
    special_attribute,
    source_phase,
    start_complete_special,
)


class _HashableCases(dict):
    """Keep nested motion case maps usable as action descriptor keys."""

    def __hash__(self):
        return hash(tuple((key, repr(value)) for key, value in self.items()))


class DonkeyKongActionState(ActionState):
    """Portable mirror of the Giant Punch counters owned by ``ftDk``.

    The native fighter stores the charge count in ``u.dk.x222C`` and copies it
    to ``specialn.xC`` when B releases the loop.  Keeping those values in the
    action state lets a script host preserve the transition decision without
    making the move depend on a Python global.
    """

    command: tuple[int, int, int, int] = (0, 0, 0, 0)
    arm_swings: int = 0
    release_swings: int = 0
    cancel_pending: bool = False


def _dk_state(fighter: Fighter) -> DonkeyKongActionState:
    state = getattr(fighter, "action_state", None)
    if state is None:
        state = DonkeyKongActionState()
        fighter.action_state = state
    return state


def _max_arm_swings(fighter: Fighter, ctx: MoveContext) -> int | None:
    """Read the source SpecialN cap when the host exposes the typed value.

    Older standalone fixtures do not provide Donkey Kong's full attribute
    block; returning ``None`` keeps their existing start/loop behavior while a
    native resource host can expose the source field under either spelling.
    """

    candidates = [getattr(fighter, "special_n_max_arm_swings", None)]
    lookup = getattr(ctx, "resource", None)
    if callable(lookup):
        resource = lookup("specials.special_attributes")
        candidates.extend((
            getattr(resource, "max_arm_swings", None),
            getattr(resource, "special_n_max_arm_swings", None),
        ))
        if isinstance(resource, dict):
            candidates.extend((
                resource.get("max_arm_swings"),
                resource.get("special_n_max_arm_swings"),
            ))
    for value in candidates:
        if isinstance(value, int) and not isinstance(value, bool) and value > 0:
            return value
    return None


class GiantPunch(NeutralSpecial):
    """Donkey Kong's source charge, release, and cancel state graph.

    ``ftDk_SpecialNStart_Anim`` advances into the looping charge state.  The
    native IASA callback releases the punch on B and cancels on L/R; the
    charge counter, effects, hitboxes, and full charge decision stay owned by
    the native fighter callback.
    """

    ground_start = source_phase(369)
    ground_loop = source_phase(370, animation_loop=True)
    ground_cancel = source_phase(371)
    ground_punch = source_phase(372)
    ground_full = source_phase(373)
    air_start = source_phase(374)
    air_loop = source_phase(375, animation_loop=True)
    air_cancel = source_phase(376)
    air_punch = source_phase(377)
    air_full = source_phase(378)

    ground = ground_start
    air = air_start
    _ACTIVE = (
        ground_start, ground_loop, ground_cancel, ground_punch, ground_full,
        air_start, air_loop, air_cancel, air_punch, air_full,
    )
    _CHARGE = (ground_loop, air_loop)

    @staticmethod
    def _landing_lag(fighter: Fighter, ctx: MoveContext):
        """Read ftDonkeyAttributes::SpecialN.x38 when the host exposes it."""

        candidates = [
            getattr(fighter, "special_n_landing_lag", None),
            getattr(fighter, "specialn_landing_lag", None),
        ]
        lookup = getattr(ctx, "resource", None)
        if callable(lookup):
            resource = lookup("specials.special_attributes")
            candidates.extend((
                getattr(resource, "special_n_landing_lag", None),
                getattr(resource, "specialn_landing_lag", None),
            ))
            if isinstance(resource, dict):
                candidates.extend((
                    resource.get("special_n_landing_lag"),
                    resource.get("specialn_landing_lag"),
                ))
        for value in candidates:
            if isinstance(value, (int, float)) and not isinstance(value, bool) and value >= 0:
                return value
        return None

    @hook.action_enter(
        ground_start, ground_loop, ground_cancel, ground_punch, ground_full,
        air_start, air_loop, air_cancel, air_punch, air_full,
    )
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Reset command slots and preserve the source release count."""

        state = _dk_state(fighter)
        command = getattr(state, "command", None)
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, 0)
        state.cancel_pending = False
        if fighter.action in (self.ground_start, self.air_start):
            state.release_swings = 0

    def _entry(self, fighter: Fighter, ctx: MoveContext, grounded: bool):
        state = _dk_state(fighter)
        cap = _max_arm_swings(fighter, ctx)
        if cap is not None and state.arm_swings == cap:
            state.release_swings = state.arm_swings
            state.arm_swings = 0
            return (self.ground_full, 373) if grounded else (self.air_full, 378)
        return (self.ground_start, 369) if grounded else (self.air_start, 374)

    def _release(self, fighter: Fighter) -> None:
        state = _dk_state(fighter)
        state.release_swings = state.arm_swings
        state.arm_swings = 0
        fighter.change_action(
            self.ground_punch if fighter.action == self.ground_loop else self.air_punch
        )

    on_end = {
        ground_start: Transition(ground_loop),
        air_start: Transition(air_loop),
        ground_cancel: Transition(Action.WAIT),
        ground_punch: Transition(Action.WAIT),
        ground_full: Transition(Action.WAIT),
        air_cancel: Transition(Action.FALL),
        # Air release phases use the source callback below so their own
        # SpecialN landing lag reaches FallSpecial instead of being dropped.
    }
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_loop: Transition(ground_loop, preserve_state=True, keep_frame=True),
        air_cancel: Transition(ground_cancel, preserve_state=True, keep_frame=True),
        air_punch: Transition(ground_punch, preserve_state=True, keep_frame=True),
        air_full: Transition(ground_full, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_loop: Transition(air_loop, preserve_state=True, keep_frame=True),
        ground_cancel: Transition(air_cancel, preserve_state=True, keep_frame=True),
        ground_punch: Transition(air_punch, preserve_state=True, keep_frame=True),
        ground_full: Transition(air_full, preserve_state=True, keep_frame=True),
    }

    @hook.input_pressed(Button.B, Button.L, Button.R)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if fighter.action in self._CHARGE:
            input_state = getattr(ctx, "input", None)
            if input_state is not None:
                if input_state.just_pressed(Button.B):
                    self._release(fighter)
                    return True
                if input_state.just_pressed(Button.L) or input_state.just_pressed(Button.R):
                    # ftDk_Special*NLoop_IASA latches LR in x0 and waits for
                    # the loop animation's frame zero before entering the
                    # cancel state.  Cancelling immediately skips that
                    # boundary and changes the amount of charge the native
                    # animation can expose to the host.
                    state = _dk_state(fighter)
                    state.cancel_pending = True
                    if getattr(fighter, "action_frame", 1) == 0:
                        self.cancel_charge(fighter, ctx)
                    return True
            else:
                self._release(fighter)
                return True
            if _dk_state(fighter).cancel_pending and getattr(fighter, "action_frame", 1) == 0:
                self.cancel_charge(fighter, ctx)
                return True
            return True
        if fighter.action in self._ACTIVE:
            return True
        return start_complete_special(
            fighter,
            ctx,
            lambda grounded, _: self._entry(fighter, ctx, grounded),
            active_actions=self._ACTIVE,
        )

    @hook.animation_end(air_punch, air_full)
    def finish_air_release(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Match ftDk_SpecialAirN_Anim and _Full_Anim landing exits."""

        landing_lag = self._landing_lag(fighter, ctx)
        if landing_lag is not None and landing_lag != 0:
            enter = getattr(fighter, "enter_fall_special", None)
            if callable(enter):
                enter(mobility=1, landing_lag=landing_lag)
                return
        fighter.change_action(Action.FALL)

    def cancel_charge(self, fighter: Fighter, ctx: MoveContext) -> bool:
        fighter.change_action(
            self.ground_cancel if fighter.action == self.ground_loop else self.air_cancel
        )
        return True


_GROUND = source_phase(
    381,
    motion=motion.profile(
        ground=(motion.stick_steering(
            threshold=0.0,
            acceleration=special_attribute(DonkeyKongAttribute.SPECIAL_HI_GROUNDED_MOBILITY),
            target=special_attribute(DonkeyKongAttribute.SPECIAL_HI_GROUNDED_HORIZONTAL_VELOCITY),
        ),),
    ),
)
_AIR = source_phase(
    382,
    motion=motion.profile(
        air=(
            # ftDk_SpecialAirHi_Phys uses ordinary gravity after the aerial
            # collision callback raises cmd_vars[0].  The old declaration
            # always applied DK's reduced multiplier, so landing on a
            # platform and continuing in the aerial phase had the wrong
            # vertical acceleration.
            motion.command_branch(
                index=0,
                cases=_HashableCases({
                    0: (motion.gravity_multiplier(
                        index=0,
                        value=0,
                        multiplier=special_attribute(
                            DonkeyKongAttribute.SPECIAL_HI_AERIAL_GRAVITY
                        ),
                    ),),
                    1: (
                        motion.gravity(
                            acceleration=parameter(CommonParameter.GRAVITY),
                            terminal_velocity=parameter(CommonParameter.TERMINAL_VELOCITY),
                            delay=0,
                        ),
                    ),
                }),
            ),
            motion.stick_steering(
                threshold=0.0,
                acceleration=special_attribute(DonkeyKongAttribute.SPECIAL_HI_AERIAL_MOBILITY),
                target=special_attribute(DonkeyKongAttribute.SPECIAL_HI_AERIAL_HORIZONTAL_VELOCITY),
            ),
        ),
    ),
)


class SpinningKong(UpSpecial):
    """Source-faithful entry and surface transitions for Spinning Kong.

    The motion profiles cover source gravity selection and stick steering on
    both surfaces; native command zero selects DK's gravity multiplier and
    every other command falls back to ordinary fighter gravity.
    """

    # Generic packs without Donkey Kong's character-specific table leave this
    # behavior disabled at registration instead of partially linking it.
    resource = "specials.special_attributes"
    ground = _GROUND
    air = _AIR
    _ACTIVE = (ground, air)
    # The native grounded entry and physics callbacks only read x54/x5C.
    # Keep that path usable with a resource pack that has not populated the
    # aerial fields yet; the aerial path still fails closed because its
    # gravity, steering, and landing callbacks dereference the full block.
    _GROUND_ENTRY_ATTRIBUTES = (
        DonkeyKongAttribute.SPECIAL_HI_GROUNDED_HORIZONTAL_VELOCITY,
        DonkeyKongAttribute.SPECIAL_HI_GROUNDED_MOBILITY,
    )
    _AIR_ENTRY_ATTRIBUTES = tuple(DonkeyKongAttribute)

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        lookup = getattr(ctx, "resource", None)
        if lookup is not None and lookup(self.resource) is None:
            return False
        grounded = bool(ctx.ground_open)
        required = self._GROUND_ENTRY_ATTRIBUTES if grounded else self._AIR_ENTRY_ATTRIBUTES
        if any(fighter.special_attribute(attribute) is None for attribute in required):
            return False
        if fighter.action in self._ACTIVE:
            return True
        started = start_complete_special(
            fighter,
            ctx,
            lambda is_grounded, _: (self.ground, 381) if is_grounded else (self.air, 382),
            active_actions=self._ACTIVE,
            direction=1,
        )
        if started:
            self._reset_command_vars(fighter)
            self._set_entry_velocity(fighter, grounded)
        return started

    @staticmethod
    def _reset_command_vars(fighter: Fighter) -> None:
        """Match ftDk's entry reset of cmd_vars[0..3]."""
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", None)
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, 0)

    def _set_entry_velocity(self, fighter: Fighter, grounded: bool) -> None:
        """Apply the source entry velocity and clear vertical ground speed."""
        limit = fighter.special_attribute(
            DonkeyKongAttribute.SPECIAL_HI_GROUNDED_HORIZONTAL_VELOCITY
        )
        if limit is None:
            return
        if grounded:
            velocity = max(-limit, min(limit, fighter.ground_velocity))
            fighter.ground_velocity = velocity
            fighter.velocity = (velocity, 0.0)
        else:
            velocity = fighter.velocity
            aerial_velocity = fighter.special_attribute(
                DonkeyKongAttribute.SPECIAL_HI_AERIAL_VERTICAL_VELOCITY
            )
            fighter.set_velocity(
                max(-limit, min(limit, velocity[0])),
                velocity[1] if aerial_velocity is None else aerial_velocity,
            )
        fighter.max_jumps()

    def _clamp_surface_velocity(self, fighter: Fighter, grounded: bool) -> None:
        attribute = (
            DonkeyKongAttribute.SPECIAL_HI_GROUNDED_HORIZONTAL_VELOCITY
            if grounded
            else DonkeyKongAttribute.SPECIAL_HI_AERIAL_HORIZONTAL_VELOCITY
        )
        limit = fighter.special_attribute(attribute)
        if limit is None:
            return
        if grounded:
            fighter.ground_velocity = max(-limit, min(limit, fighter.ground_velocity))
        else:
            velocity = fighter.velocity
            fighter.set_velocity(max(-limit, min(limit, velocity[0])), velocity[1])

    @hook.ground_air_changed(ground, air)
    def surface_change(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Clamp horizontal velocity once at each source surface conversion."""
        if fighter.action == self.air and ctx.grounded:
            self._clamp_surface_velocity(fighter, grounded=True)
        elif fighter.action == self.ground and not ctx.grounded:
            self._clamp_surface_velocity(fighter, grounded=False)

    # ftDk_SpecialHi_Coll / ftDk_SpecialAirHi_Coll preserve the current
    # animation frame while switching between the two motion states.
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}
    on_end = {ground: Transition(Action.WAIT)}

    @hook.animation_end()
    def end_air(self, fighter: Fighter, ctx: MoveContext) -> None:
        # ftDk_SpecialAirHi_Anim selects Fall or FallSpecial from the typed
        # landing-lag field.  Input entry is gated on that field, so absence
        # here can only occur in a standalone callback host.
        if fighter.action != self.air:
            return
        # ftCommon_8007D60C restores the native jump budget and aerial
        # collision state before entering Fall or FallSpecial.
        fighter.max_jumps()
        landing_lag = fighter.special_attribute(
            DonkeyKongAttribute.SPECIAL_HI_LANDING_LAG
        )
        if landing_lag is None:
            return
        if landing_lag == 0:
            fighter.change_action(Action.FALL)
        else:
            fighter.enter_fall_special(mobility=1, landing_lag=landing_lag)


class DonkeyKong(Fighter):
    action_state = DonkeyKongActionState
    specials = Fighter.specials.replace(neutral=GiantPunch(), up=SpinningKong())


__all__ = ["DonkeyKong", "GiantPunch", "SpinningKong"]
