"""Donkey Kong's source-defined Spinning Kong phases.

Spinning Kong's source movement uses the typed Donkey Kong attribute layout;
the native animation resource still gates entry through its complete state.
"""

from fighter import DonkeyKongAttribute
from skirmish import (
    Action,
    Button,
    Fighter,
    MoveContext,
    UpSpecial,
    Transition,
    hook,
    motion,
    special_attribute,
    source_phase,
    start_complete_special,
)


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
            motion.gravity_multiplier(
                index=0,
                value=0,
                multiplier=special_attribute(DonkeyKongAttribute.SPECIAL_HI_AERIAL_GRAVITY),
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
    # ftDk_SpecialHi_Enter and both physics callbacks read the complete
    # native SpecialHi block.  Rejecting a partial table keeps a generic
    # resource pack from entering a state that would later dereference a
    # missing field.
    _ENTRY_ATTRIBUTES = tuple(DonkeyKongAttribute)

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        lookup = getattr(ctx, "resource", None)
        if lookup is not None and lookup(self.resource) is None:
            return False
        if any(fighter.special_attribute(attribute) is None for attribute in self._ENTRY_ATTRIBUTES):
            return False
        if fighter.action in self._ACTIVE:
            return True
        grounded = bool(ctx.ground_open)
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
    specials = Fighter.specials.replace(up=SpinningKong())


__all__ = ["DonkeyKong", "SpinningKong"]
