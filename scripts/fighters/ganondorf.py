"""Ganondorf's source-backed special declarations.

``ftganon.c`` points its motion table at the Captain special callbacks, but
the phases and attributes are loaded from Ganondorf's own ``PlGn.dat``.  The
source-family bases supply only those callback lifecycles; every action and
resource path below remains Ganon-specific.
"""

from skirmish import Action, Fighter, on, source_phase
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

    @on.animation_end()
    def animation_end_rebound(self, fighter, ctx):
        """Finish the source wall-rebound motion into ordinary falling."""
        if fighter.action == self.throw_rebound:
            fighter.change_action(Action.FALL)


class WizardFoot(CaptainDownSpecial):
    ground = source_phase(357, animation=311, attack="down.ground")
    ground_end = source_phase(358, animation=312, attack="down.ground_end")
    air = source_phase(359, animation=313, attack="down.air")
    landing = source_phase(360, animation=314, attack="down.landing")
    air_end = source_phase(361, animation=316, attack="down.air_end")
    ground_end_air = source_phase(362, animation=315, attack="down.ground_end_air")

    def wall_rebound(self, fighter, ctx):
        """Require the source wall-collision command cue before state 363."""
        command = getattr(getattr(fighter, "action_state", None), "command", ())
        if not command or not command[0]:
            return False
        return super().wall_rebound(fighter, ctx)


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
