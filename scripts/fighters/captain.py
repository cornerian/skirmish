"""Captain Falcon's source-backed special declarations.

The callback policy shared with Ganondorf lives in ``fighter.captain_family``;
this module owns Falcon's action descriptors, animation identities, and
resource attack paths.
"""

from skirmish import (
    Action,
    CommonParameter,
    Fighter,
    action,
    motion,
    on,
    parameter,
    source_phase,
)
from fighter.captain_family import (
    CaptainFamilyActionState,
    CaptainDownSpecial,
    CaptainNeutralSpecial,
    CaptainSideSpecial,
    CaptainUpSpecial,
)


class FalconPunch(CaptainNeutralSpecial):
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
        motion=CaptainNeutralSpecial.air_motion,
    )


class FalconDive(CaptainUpSpecial):
    ground = action(Action.SPECIAL_HI, slippi_state=353, animation=307, attack="up.ground")
    air = action(Action.SPECIAL_AIR_HI, slippi_state=354, animation=308, attack="up.air")
    catch = action(Action.SPECIAL_HI_CATCH, slippi_state=355, animation=309)
    throw = action(Action.SPECIAL_HI_THROW, slippi_state=356, animation=310)
    # ftCa_MS_SpecialHiThrow1 (state 363) is the wall rebound throw motion.
    # The shared callback currently exposes the normal throw hand-off only;
    # retaining this source phase keeps the native motion identity available
    # for the collision callback when that host event is surfaced.
    throw_rebound = source_phase(363, animation=317)

    @on.animation_end(throw_rebound)
    def throw_rebound_animation_end(self, fighter, ctx):
        """Leave the wall-rebound continuation through ordinary fall.

        ``ftCa_SpecialHiThrow1_Anim`` has no fighter-specific branch: once
        state 363's animation is exhausted it enters common fall.  The
        collision and throw side effects remain native host responsibilities,
        but this terminal transition is deterministic and can be represented
        in the authoring layer.
        """
        fighter.change_action(Action.FALL)


class RaptorBoost(CaptainSideSpecial):
    ground_start = action(Action.SPECIAL_S_START, slippi_state=349, animation=303, attack="side.ground_start")
    ground = action(Action.SPECIAL_S, slippi_state=350, animation=304, attack="side.ground")
    air_start = action(Action.SPECIAL_AIR_S_START, slippi_state=351, animation=305, attack="side.air_start")
    air = action(
        Action.SPECIAL_AIR_S,
        slippi_state=352,
        animation=306,
        attack="side.air",
        motion=CaptainSideSpecial.air_motion,
    )

    @on.action_enter()
    def action_enter(self, fighter, ctx):
        """Reset all four command vars at the Raptor Boost entry point.

        ``ftCa_SpecialS_Enter`` and ``setupAirStart`` call
        ``resetCmdVarsGround`` and explicitly clear cmd_vars[0..3].  The
        shared family callback consumes only the three slots used by its
        authoring hooks, so Falcon's concrete entry must also clear slot 3.
        """
        super().action_enter(fighter, ctx)
        command = getattr(getattr(fighter, "action_state", None), "command", ())
        if len(command) == 4:
            fighter.action_state.command = (command[0], command[1], command[2], 0)

    @on.before_hit()
    def before_hit(self, fighter, hit):
        """Require the native start animation's command cue before detection.

        ``ftCa_SpecialS_OnDetect`` gates both fighter and item detection on
        ``cmd_vars[0]``.  A contact during the startup animation therefore
        leaves Falcon in the startup state until the resource command arrives;
        the shared family callback must not turn every contact into the
        follow-through state.
        """
        command = getattr(getattr(fighter, "action_state", None), "command", ())
        if not command or not command[0]:
            return
        super().before_hit(fighter, hit)


class FalconKick(CaptainDownSpecial):
    ground = action(Action.SPECIAL_LW, slippi_state=357, animation=311, attack="down.ground")
    ground_end = action(Action.SPECIAL_LW_GROUND_END, slippi_state=358, animation=312, attack="down.ground_end")
    air = action(Action.SPECIAL_AIR_LW, slippi_state=359, animation=313, attack="down.air")
    landing = action(Action.SPECIAL_AIR_LW_LANDING_END, slippi_state=360, animation=314, attack="down.landing")
    air_end = action(
        Action.SPECIAL_AIR_LW_END_AIR,
        slippi_state=361,
        animation=316,
        attack="down.air_end",
        motion=motion.profile(
            air=(
                motion.gravity(
                    acceleration=parameter(CommonParameter.GRAVITY),
                    terminal_velocity=parameter(CommonParameter.TERMINAL_VELOCITY),
                    delay=0,
                ),
                motion.air_friction(amount=parameter(CommonParameter.AERIAL_FRICTION)),
            ),
        ),
    )
    ground_end_air = action(
        Action.SPECIAL_LW_END_AIR,
        slippi_state=362,
        animation=315,
        attack="down.ground_end_air",
    )

    @on.animation_end()
    def animation_end(self, fighter, ctx):
        if fighter.action is self.ground:
            self._clear_kick_commands(fighter)
        super().animation_end(fighter, ctx)

    def _transition_animation_end(self, fighter, ctx):
        """Clear kick command cues before the source recovery transition.

        ``ftCa_SpecialLw_Anim`` and ``ftCa_SpecialAirLw_Anim`` clear
        ``cmd_vars[0..2]`` before entering their ground or aerial end state.
        Keep slot 3 intact because the native helper does not touch it.
        """
        if fighter.action is self.air:
            self._clear_kick_commands(fighter)
        super()._transition_animation_end(fighter, ctx)

    @staticmethod
    def _clear_kick_commands(fighter):
        command = getattr(getattr(fighter, "action_state", None), "command", ())
        if len(command) == 4:
            fighter.action_state.command = (0, 0, 0, command[3])

    def wall_rebound(self, fighter, ctx):
        """Mirror ``ftCa_SpecialLw_Coll``'s command-gated wall handoff.

        The native callback only enters ``ftCa_MS_SpecialHiThrow1`` after its
        command variable is set.  Surface contact alone must not create the
        rebound state.
        """
        command = getattr(getattr(fighter, "action_state", None), "command", ())
        # ftCa_SpecialLw_Coll owns the rebound branch; the aerial collision
        # callbacks (ftCa_SpecialAirLw_Coll and its end states) do not.
        if fighter.action is not self.ground or not command or not command[0]:
            return False
        # ``ftCa_SpecialLw_Coll`` only rebounds from the wall in the fighter's
        # facing direction: Captain facing right uses the left-wall flag and
        # facing left uses the right-wall flag. Native surface contexts expose
        # the outward wall normal; retain the legacy boolean form for hosts
        # that have not surfaced that geometry yet.
        wall = getattr(ctx, "wall", None)
        if wall is None:
            wall = getattr(ctx, "wall_contact", None)
        normal = getattr(wall, "normal", None)
        facing = getattr(fighter, "facing", None)
        if normal is not None and facing is not None:
            try:
                if len(normal) < 1 or normal[0] * facing >= 0:
                    return False
            except (TypeError, IndexError):
                return False
        # ftCa_Special_Inline_SetFlags clears cmd_vars[0..2] before
        # Fighter_ChangeMotionState enters ftCa_MS_SpecialHiThrow1 (state
        # 363).  Reset first so state-entry hooks observe the native values.
        state = getattr(fighter, "action_state", None)
        if len(command) == 4:
            state.command = (0, 0, 0, command[3])
        return super().wall_rebound(fighter, ctx)


class CaptainFalcon(Fighter):
    action_state = CaptainFamilyActionState
    specials = Fighter.specials.replace(
        neutral=FalconPunch(),
        side=RaptorBoost(),
        up=FalconDive(),
        down=FalconKick(),
    )


__all__ = [
    "CaptainFalcon",
    "CaptainFamilyActionState",
    "FalconPunch",
    "RaptorBoost",
    "FalconDive",
    "FalconKick",
]
