"""Luigi's source-backed special motion states.

The native Luigi implementation owns the fireball article and the cyclone,
misfire, and launch effects.  This module declares their source motion
phases, input routing, and surface/animation lifecycle so those native
effects can remain attached at the runtime boundary.
"""

from skirmish import (
    Action,
    ArticleId,
    B0ArticleSpecial,
    Button,
    DirectionalSpecial,
    DownSpecial,
    Fighter,
    SideSpecial,
    Transition,
    UpSpecial,
    hook,
    source_phase,
    b0_source_phases,
)
from fighter.helpers import resource_attributes


class Fireball(B0ArticleSpecial):
    article_id = ArticleId.LUIGI_FIRE
    ground, air = b0_source_phases(341, 342)

    @hook.action_enter(ground, air)
    def enter(self, fighter, ctx) -> None:
        """Reset Luigi's native fireball command and throw latches on entry."""
        super().enter(fighter, ctx)
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, command[1], command[2], command[3])
        if hasattr(fighter, "throw_flags"):
            fighter.throw_flags = 0

class GreenMissile(SideSpecial, DirectionalSpecial):
    """Green Missile's source phases (343 through 354).

    The charge and misfire decision is made by native character attributes.
    Both launch variants therefore share the declared phase lifecycle while
    retaining every source motion state in the exported definition.
    """

    ground_start = source_phase(343)
    ground_hold = source_phase(344)
    ground_s2 = source_phase(345)
    ground_end = source_phase(346)
    ground = source_phase(347)
    ground_misfire = source_phase(348)
    air_start = source_phase(349)
    air_hold = source_phase(350)
    air_s2 = source_phase(351)
    air_end = source_phase(352)
    air = source_phase(353)
    air_misfire = source_phase(354)
    _ACTIVE = (
        ground_start, ground_hold, ground_s2, ground_end, ground,
        ground_misfire, air_start, air_hold, air_s2, air_end, air,
        air_misfire,
    )
    _LAUNCH = (ground, ground_misfire, air, air_misfire)

    on_end = {
        ground_start: Transition(ground_hold),
        ground_hold: Transition(ground),
        # ftLg_SpecialSFly_Enter always enters the aerial S2 state, including
        # when launch began from the ground.  Ground S2 remains declared for
        # native table completeness but is not selected by this callback.
        ground: Transition(air_s2),
        ground_misfire: Transition(air_s2),
        ground_end: Transition(Action.WAIT),
        air_start: Transition(air_hold),
        air_hold: Transition(air),
        air: Transition(air_s2),
        air_misfire: Transition(air_s2),
        air_s2: Transition(air_end),
        air_end: Transition(Action.FALL),
    }
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_hold: Transition(ground_hold, preserve_state=True, keep_frame=True),
        air: Transition(ground, preserve_state=True, keep_frame=True),
        air_misfire: Transition(ground_misfire, preserve_state=True, keep_frame=True),
        # ftLg_SpecialAirS2_Coll and ftLg_SpecialAirSEnd_Coll enter the
        # grounded end state afresh after landing/wall contact.
        air_s2: Transition(ground_end),
        air_end: Transition(ground_end),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_hold: Transition(air_hold, preserve_state=True, keep_frame=True),
        ground: Transition(air, preserve_state=True, keep_frame=True),
        ground_misfire: Transition(air_misfire, preserve_state=True, keep_frame=True),
        ground_end: Transition(Action.FALL),
    }

    def _command_ready(self, fighter, ctx) -> bool:
        """Read the source launch cue from the event or command trace."""
        event = getattr(ctx, "event", None)
        if event is not None and hasattr(event, "value"):
            return bool(event.value)
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        return bool(command and command[0])

    def _transition_animation_end(self, fighter, ctx) -> None:
        """Keep launch and unused ground-flight phases source-gated.

        ``ftLg_SpecialS_Anim`` and its misfire sibling enter flight only when
        command slot 0 is raised by the launch animation.  The ground S2 row
        has an empty source animation callback; only the aerial S2 callback
        enters the end row.
        """
        if fighter.action in self._LAUNCH and not self._command_ready(fighter, ctx):
            return
        if fighter.action == self.ground_s2:
            return
        super()._transition_animation_end(fighter, ctx)

    @hook.command_changed(0, actions=_LAUNCH)
    def launch_command(self, fighter, ctx) -> None:
        """Enter flight at the native command cue, before animation end."""
        if self._command_ready(fighter, ctx):
            self._transition_animation_end(fighter, ctx)

    @hook.action_enter(ground_start, air_start)
    def enter_start(self, fighter, ctx) -> None:
        """Reset native command variable 0 before charge begins."""
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, command[1], command[2], command[3])

    @hook.input_released(Button.B, actions=(ground_hold, air_hold))
    def release_charge(self, fighter, ctx) -> bool:
        """Release B from charge and enter the native launch phase.

        Native IASA launches immediately on release; animation completion is
        only the fallback for a held charge that reaches its cap.
        """
        destination = {
            self.ground_hold: self.ground,
            self.air_hold: self.air,
        }.get(fighter.action)
        if destination is None:
            return False
        fighter.change_action(destination)
        return True


class SuperJumpPunch(UpSpecial, DirectionalSpecial):
    """Luigi's two source Super Jump Punch states."""

    ground = source_phase(355)
    air = source_phase(356)
    _ACTIVE = (ground, air)
    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    @hook.action_enter(ground, air)
    def enter(self, fighter, ctx) -> None:
        """Reset the command and throw latches used by native entry."""
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, command[1], command[2], command[3])
        if hasattr(fighter, "throw_flags"):
            fighter.throw_flags = 0

    @hook.animation_end(ground, air)
    def enter_fall_special(self, fighter, ctx) -> bool:
        """Match both native Super Jump Punch callbacks' FallSpecial exit."""
        attributes = resource_attributes(ctx, self.resource)
        mobility = getattr(
            attributes,
            "specialhi_freefall_air_spd_mul",
            getattr(attributes, "specialhi_freefall_mobility", None),
        )
        landing_lag = getattr(attributes, "specialhi_landing_lag", None)
        enter = getattr(fighter, "enter_fall_special", None)
        if mobility is None or landing_lag is None or not callable(enter):
            return False
        enter(mobility=mobility, landing_lag=landing_lag)
        return True


class Cyclone(DownSpecial, DirectionalSpecial):
    """Luigi Cyclone's grounded and aerial source states."""

    ground = source_phase(357)
    air = source_phase(358)
    _ACTIVE = (ground, air)
    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    @hook.action_enter(ground, air)
    def enter(self, fighter, ctx) -> None:
        """Reset native command variables used by Cyclone's tap physics."""
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if isinstance(command, (tuple, list)) and len(command) >= 4:
            state.command = (0, 0, 0, command[3])

    def _consume_air_charge(self, fighter) -> None:
        """Consume command variable 1 as soon as the native cue arrives.

        ``ftLg_SpecialAirLw_Anim`` checks this slot on every animation tick,
        rather than only at the terminal frame.  Routing the command event
        directly keeps the persistent charge flag available to the next
        aerial Cyclone entry without introducing a per-frame Python callback.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", ())
        if not isinstance(command, (tuple, list)) or len(command) < 4 or not command[1]:
            return
        state.command = (command[0], 0, command[2], command[3])
        state.cyclone_charge = True

    @hook.command_changed(1, actions=(air,))
    def charge_command(self, fighter, ctx) -> None:
        self._consume_air_charge(fighter)

    @hook.animation_end(air)
    def finish_air(self, fighter, ctx) -> None:
        """Consume a late command cue before the native move ends."""
        self._consume_air_charge(fighter)

    def _transition_animation_end(self, fighter, ctx) -> None:
        if fighter.action == self.air:
            attributes = resource_attributes(ctx, self.resource)
            landing_lag = getattr(
                attributes,
                "cyclone_landing_lag",
                getattr(attributes, "speciallw_landing_lag", None),
            )
            if landing_lag is not None:
                if landing_lag == 0:
                    fighter.change_action(Action.FALL)
                else:
                    enter = getattr(fighter, "enter_fall_special", None)
                    if callable(enter):
                        enter(mobility=1, landing_lag=landing_lag)
                        return
        super()._transition_animation_end(fighter, ctx)

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter, ctx) -> bool:
        """Apply the source aerial tap when the command window is armed.

        The native physics callback checks command variable 2 while B is
        held.  The callback host exposes fresh B presses, so this preserves
        the same observable transition at the input boundary.
        """
        command = getattr(getattr(fighter, "action_state", None), "command", ())
        if len(command) < 3 or not command[2] or fighter.action != self.ground:
            return super().input_pressed(fighter, ctx)
        fighter.change_action(self.air)
        return True


class Luigi(Fighter):
    specials = Fighter.specials.replace(
        neutral=Fireball(),
        side=GreenMissile(),
        up=SuperJumpPunch(),
        down=Cyclone(),
    )


__all__ = ["Luigi", "Fireball", "GreenMissile", "SuperJumpPunch", "Cyclone"]
