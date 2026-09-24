"""Ganondorf's source-backed special declarations.

``ftganon.c`` points its motion table at the Captain special callbacks, but
the phases and attributes are loaded from Ganondorf's own ``PlGn.dat``.  The
source-family bases supply only those callback lifecycles; every action and
resource path below remains Ganon-specific.
"""

from skirmish import (
    Action,
    CommonParameter,
    Fighter,
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


class WarlockPunch(CaptainNeutralSpecial):
    ground = source_phase(
        347,
        animation=301,
        attack="neutral.ground",
        command_trace="neutral.script.ground",
    )
    air = source_phase(
        348,
        animation=302,
        attack="neutral.air",
        command_trace="neutral.script.air",
        motion=CaptainNeutralSpecial.air_motion,
    )

    @on.action_enter()
    def enter(self, fighter, ctx):
        """Reset the source throw latch before either punch motion.

        ftCa_SpecialN_Enter and ftCa_SpecialAirN_Enter clear ``throw_flags``
        before changing motion.  The family state reset handles the command
        traces; this fighter-side field is the remaining reset representable
        by the authoring API.
        """
        super().enter(fighter, ctx)
        if hasattr(fighter, "throw_flags"):
            fighter.throw_flags = 0


class GerudoDragon(CaptainSideSpecial):
    ground_start = source_phase(349, animation=303, attack="side.ground_start")
    ground = source_phase(350, animation=304, attack="side.ground")
    air_start = source_phase(351, animation=305, attack="side.air_start")
    air = source_phase(
        352,
        animation=306,
        attack="side.air",
        motion=CaptainSideSpecial.air_motion,
    )


class DarkDive(CaptainUpSpecial):
    ground = source_phase(353, animation=307, attack="up.ground")
    air = source_phase(354, animation=308, attack="up.air")
    catch = source_phase(355, animation=309)
    throw = source_phase(356, animation=310)
    # ftCa_MS_SpecialHiThrow1 is part of ftGanon's motion table as well.  The
    # native collision callback can enter it for the wall rebound branch.
    throw_rebound = source_phase(363, animation=317)

    @on.animation_end(throw_rebound)
    def animation_end_rebound(self, fighter, ctx):
        """Finish the source wall-rebound motion into ordinary falling."""
        fighter.change_action(Action.FALL)


class WizardFoot(CaptainDownSpecial):
    ground = source_phase(357, animation=311, attack="down.ground")
    ground_end = source_phase(358, animation=312, attack="down.ground_end")
    air = source_phase(359, animation=313, attack="down.air")
    landing = source_phase(360, animation=314, attack="down.landing")
    # ftCa_MS_SpecialAirLwEndAir (state 361) uses
    # ftCa_SpecialAirLwEndAir_Phys: ordinary air physics plus aerial
    # friction.  ftganon.c points this row at the same callback as Falcon's
    # table, so retain that source motion profile on Ganon's concrete phase.
    air_end = source_phase(
        361,
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
    ground_end_air = source_phase(362, animation=315, attack="down.ground_end_air")

    @on.action_enter()
    def action_enter(self, fighter, ctx):
        """Reset the shared throw latch on each Wizard's Foot entry.

        ``ftCa_SpecialLw_Enter`` and ``ftCa_SpecialAirLw_Enter`` clear
        ``throw_flags`` together with the three command variables.  The
        family callback already consumes those command variables, so keep the
        remaining fighter side reset local to Ganondorf's concrete move.
        """
        super().action_enter(fighter, ctx)
        if hasattr(fighter, "throw_flags"):
            fighter.throw_flags = 0

    def wall_rebound(self, fighter, ctx):
        """Mirror the grounded Wizard's Foot source wall rebound branch.

        ``ftCa_SpecialLw_Coll`` clears the three command variables and
        ``throw_flags`` before changing motion to state 363.  The separate
        ``ftCa_SpecialAirLw_Coll`` callback only enters aerial end state 361.
        """
        if fighter.action != self.ground:
            return False
        command = getattr(getattr(fighter, "action_state", None), "command", ())
        if not command or not command[0]:
            return False
        # ftCa_SpecialLw_Coll only takes the rebound branch for the wall on
        # the side opposite the fighter's facing direction.  The shared
        # family callback intentionally accepts legacy boolean wall events;
        # when a host provides a normal, preserve the source-facing test here
        # without changing the common Captain-family implementation.
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
        state = getattr(fighter, "action_state", None)
        if state is not None and len(command) == 4:
            state.command = (0, 0, 0, command[3])
        if hasattr(fighter, "throw_flags"):
            fighter.throw_flags = 0
        if not super().wall_rebound(fighter, ctx):
            return False
        return True


class Ganondorf(Fighter):
    action_state = CaptainFamilyActionState
    specials = Fighter.specials.replace(
        neutral=WarlockPunch(),
        side=GerudoDragon(),
        up=DarkDive(),
        down=WizardFoot(),
    )


__all__ = [
    "Ganondorf",
    "CaptainFamilyActionState",
    "WarlockPunch",
    "GerudoDragon",
    "DarkDive",
    "WizardFoot",
]
