"""Peach's source-backed special-motion declarations.

The native Peach callbacks use four finite motion families (states 352--368).
The item archives behind Toad, the parasol, turnips, and the Peach Bomber
explosion are not part of the authoring package, so this module declares the
fighter-side phases and their safe motion/surface exits without pretending to
spawn or simulate those articles.
"""

from typing import Any, ClassVar

from skirmish import (
    Action,
    ActionState,
    Button,
    DownSpecial,
    Fighter,
    NeutralSpecial,
    SideSpecial,
    Transition,
    UpSpecial,
    directional_match,
    fresh_special_input,
    source_phase,
    start_action,
)
from fighter.transitions import SpecialMove
from fighter.events import on


class PeachActionState(ActionState):
    """Small host-owned latch for the side-special wall/ceiling branch."""

    side_blocked: bool = False


class _PeachSpecial(SpecialMove):
    """Shared B gate for Peach's explicit native phase families."""

    _ACTIVE: ClassVar[tuple[Any, ...]] = ()

    @staticmethod
    def _reset_command_slots(fighter: Any, count: int = 4) -> None:
        """Clear the native command window when a special motion starts.

        The Peach entry callbacks explicitly clear ``cmd_vars`` before
        installing their accessory callbacks.  Keeping that reset at the
        script boundary prevents a cue left by the previous motion from
        selecting a hit, wall, or jump branch on the new motion.  Hosts that
        do not expose a command tuple simply keep their existing behavior.
        """
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", None)
        if not isinstance(command, (tuple, list)) or len(command) < count:
            return
        state.command = (0,) * count + tuple(command[count:])

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        if fighter.action in self._ACTIVE:
            return True
        if not fresh_special_input(ctx, self.resource):
            return False
        if not directional_match(ctx, self.root):
            return False
        if not (ctx.ground_open or ctx.air_open):
            return False
        start_action(fighter, self.ground if ctx.ground_open else self.air)
        return True


class PeachNeutralSpecial(NeutralSpecial, _PeachSpecial):
    ground = source_phase(365)
    ground_hit = source_phase(366)
    air = source_phase(367)
    air_hit = source_phase(368)
    _ACTIVE = (ground, ground_hit, air, air_hit)

    on_end = {
        ground: Transition(Action.WAIT),
        ground_hit: Transition(Action.WAIT),
        air: Transition(Action.FALL),
        air_hit: Transition(Action.FALL),
    }
    on_ground = {
        air: Transition(ground, preserve_state=True, keep_frame=True),
        air_hit: Transition(ground_hit, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground: Transition(air, preserve_state=True, keep_frame=True),
        ground_hit: Transition(air_hit, preserve_state=True, keep_frame=True),
    }

    @on.action_enter(ground, air)
    def reset_command_window(self, fighter: Any, ctx: Any) -> None:
        """Match ``ftPe_SpecialN_Enter``/``reset``'s four-slot clear."""
        self._reset_command_slots(fighter)

    @on.command_changed(1, actions=(ground, air))
    def accessory_hit(self, fighter: Any, ctx: Any) -> None:
        """Enter the source hit phases when the Toad command arms them.

        ``ftPe_SpecialN`` uses command variable 1 to switch from the normal
        animation into the hit animation.  The article and collision work
        remains native, but this phase change is observable at the script
        boundary and must not be left to a generic animation end.
        """
        value = getattr(getattr(ctx, "event", None), "value", 0)
        if value != 1:
            return
        target = self.ground_hit if fighter.action is self.ground else self.air_hit
        fighter.change_action(target)

    @on.before_hit(actions=(ground, air))
    def hit_contact(self, fighter: Any, hit: Any) -> None:
        """Enter the source hit motion on the Toad shield contact edge.

        ``ftPe_SpecialN`` installs ``onUnkHit`` as the shield collision
        callback.  The callback changes to the ground or air hit motion and
        restarts it at frame nine; a command trace only arms the shield.  The
        command callback above remains useful to authoring harnesses that
        model the legacy trace directly, while native contact dispatch uses
        this explicit collision edge.
        """
        if fighter.action is self.ground:
            fighter.change_action(self.ground_hit)
        elif fighter.action is self.air:
            fighter.change_action(self.air_hit)


class PeachSideSpecial(SideSpecial, _PeachSpecial):
    ground_start = source_phase(354)
    ground_end = source_phase(355)
    # Kept because it is present in ftPeach's native motion table.  Its
    # callback row is empty and the current source path never enters it.
    ground_jump = source_phase(356)
    air_start = source_phase(357)
    air_end0 = source_phase(358)
    air_end1 = source_phase(359)
    air_jump = source_phase(360)
    ground, air = ground_start, air_start
    _ACTIVE = (
        ground_start,
        ground_end,
        ground_jump,
        air_start,
        air_end0,
        air_end1,
        air_jump,
    )

    # The source enters the airborne jump after either start animation.  A
    # terminal jump/end phase then returns through the native surface state.
    on_end = {
        air_jump: Transition(air_end0),
        ground_end: Transition(Action.WAIT),
        air_end0: Transition(Action.FALL),
        air_end1: Transition(Action.FALL),
    }
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_jump: Transition(ground_end, preserve_state=True, keep_frame=True),
        air_end0: Transition(ground_end, preserve_state=True, keep_frame=True),
        air_end1: Transition(ground_end, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_end0, preserve_state=True, keep_frame=True),
    }

    @on.action_enter(ground_start, air_start)
    def reset_command_window(self, fighter: Any, ctx: Any) -> None:
        """Match ``ftPe_SpecialS_Enter``'s four command-variable reset."""
        self._reset_command_slots(fighter)

    @on.action_enter(ground_start, air_start)
    def enter_start(self, fighter: Any, ctx: Any) -> None:
        fighter.action_state.side_blocked = False

    @on.command_changed(0, actions=(ground_start, air_start))
    def blocked_start(self, fighter: Any, ctx: Any) -> None:
        value = getattr(getattr(ctx, "event", None), "value", 0)
        if value:
            fighter.action_state.side_blocked = True

    @on.animation_end(ground_start, air_start)
    def finish_start(self, fighter: Any, ctx: Any) -> None:
        """Match ``SpecialS(Start)_Anim``'s cmd_var[0] branch."""
        if getattr(fighter.action_state, "side_blocked", False):
            target = self.ground_end if fighter.action is self.ground_start else self.air_end1
        else:
            target = self.air_jump
        fighter.change_action(target)

    @on.command_changed(2, actions=(air_jump,))
    def wall_end(self, fighter: Any, ctx: Any) -> None:
        """Select the source wall-hit end phase during the parasol kick."""
        value = getattr(getattr(ctx, "event", None), "value", 0)
        if value:
            fighter.change_action(self.air_end1)

    @on.command_changed(3, actions=(air_jump,))
    def jump_end(self, fighter: Any, ctx: Any) -> None:
        """End the jump when the native command-3 cue is reached.

        ``ftPe_SpecialAirSJump_Anim`` checks ``cmd_vars[3]`` every frame and
        enters ``SpecialAirSEnd``; command variable 2 has already selected
        the wall-hit variant when that branch applies.  Bomber article
        creation remains a native item callback after this transition.
        """
        value = getattr(getattr(ctx, "event", None), "value", 0)
        if value:
            fighter.change_action(self.air_end0)


class PeachUpSpecial(UpSpecial, _PeachSpecial):
    ground = source_phase(361)
    ground_end = source_phase(362)
    air = source_phase(363)
    air_end = source_phase(364)
    _ACTIVE = (ground, ground_end, air, air_end)

    # Parasol/fall-special article handling is native-only.  The start phases
    # therefore terminate into the generic fall state instead of inventing a
    # parasol article phase; the later source end rows retain their native
    # finite ground/air exits when a host supplies them.
    on_end = {
        ground: Transition(Action.FALL),
        ground_end: Transition(Action.WAIT),
        air: Transition(Action.FALL),
        air_end: Transition(Action.FALL),
    }
    on_ground = {
        air: Transition(ground, preserve_state=True, keep_frame=True),
        air_end: Transition(ground_end, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground: Transition(air, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_end, preserve_state=True, keep_frame=True),
    }

    @on.action_enter(ground, air)
    def reset_command_window(self, fighter: Any, ctx: Any) -> None:
        """Match ``ftPe_SpecialHi``'s command 0/1/2 reset."""
        self._reset_command_slots(fighter, count=3)


class PeachDownSpecial(DownSpecial, _PeachSpecial):
    ground = source_phase(352)
    air = source_phase(353)
    _ACTIVE = (ground, air)

    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    @on.action_enter(ground, air)
    def throw_held_turnip(self, fighter: Any, ctx: Any) -> None:
        """Forward the source held-turnip branch to an article-aware host.

        ``ftPe_SpecialLw_Enter`` calls the common light-throw entry when a
        Peach turnip is already held, instead of pulling a new vegetable.
        The optional host callback owns that item handoff and common throw
        action; this declaration preserves the fighter-side branch without
        manufacturing article state.
        """
        if not getattr(fighter, "peach_turnip_held", False):
            return
        throw = getattr(fighter, "throw_held_turnip_special", None)
        if callable(throw):
            throw(airborne=fighter.action is self.air)


class Peach(Fighter):
    action_state = PeachActionState
    specials = Fighter.specials.replace(
        neutral=PeachNeutralSpecial(),
        side=PeachSideSpecial(),
        up=PeachUpSpecial(),
        down=PeachDownSpecial(),
    )


__all__ = [
    "Peach",
    "PeachNeutralSpecial",
    "PeachSideSpecial",
    "PeachUpSpecial",
    "PeachDownSpecial",
    "PeachActionState",
]
