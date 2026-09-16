"""Captain Falcon's declarative fighter definition.

This is intentionally a partial authoring slice.  Falcon Punch (ground and
air entry, command-variable launch cue, ground/air conversion, and terminal
recovery) is wired to resource data; side, up, and down specials are retained
as canonical engine actions with no translated Python behavior yet.  Hitbox,
animation, and parameter values belong to the validated native resource pack
and are not duplicated here.
"""

from skirmish import (
    Action,
    ActionState,
    Button,
    Fighter,
    MoveContext,
    Parameters,
    SpecialMove,
    SpecialMoves,
    Transition,
    action,
    hook,
    register as fighter,
    validation,
    ActionMove,
)
from shared.common import FighterBase


class CaptainFalconActionState(ActionState):
    """Only transient flags owned by the Falcon Punch policy."""

    launch_armed: bool = False


class CaptainFalconParameters(Parameters):
    """Reserved for resource-backed Captain Falcon parameters."""


class FalconPunch(SpecialMove):
    """Ground/air Falcon Punch policy; numeric data remains resource-owned."""

    resource = "neutral"

    ground = action(
        Action.SPECIAL_N_START,
        slippi_state=347,
        attack="neutral.ground",
        command_trace="neutral.script.ground",
    )
    air = action(
        Action.SPECIAL_AIR_N_START,
        slippi_state=348,
        attack="neutral.air",
        command_trace="neutral.script.air",
    )

    @hook.action_enter(ground, air)
    def enter(self, fighter: Fighter, ctx: MoveContext) -> None:
        fighter.action_state.launch_armed = False

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx: MoveContext) -> bool:
        if ctx.resource(self.resource) is None or not ctx.input.just_pressed(Button.B):
            return False
        if fighter.action in (self.ground, self.air):
            return True
        if not (ctx.ground_open or ctx.air_open):
            return False
        fighter.change_action(self.ground if ctx.ground_open else self.air)
        fighter.action_frame = 1
        return True

    @hook.command_changed(0, actions=(ground, air))
    def command_changed(self, fighter: Fighter, ctx: MoveContext) -> None:
        """Mark the upstream cmd_vars[0] launch cue without inventing motion."""
        if ctx.event.value:
            fighter.action_state.launch_armed = True

    @hook.animation_end(ground, air)
    def animation_end(self, fighter: Fighter, ctx: MoveContext) -> None:
        if fighter.action == self.ground:
            fighter.change_action(Action.WAIT)
        else:
            fighter.change_action(Action.FALL)

    # The source callbacks convert these same two motion states in either
    # direction while preserving animation/state; the host applies the
    # transition at collision time.
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    @hook.validate
    def validate(self, ctx: MoveContext) -> bool:
        resource = ctx.resource(self.resource)
        if resource is None:
            return True
        # Resource presence is meaningful only when both action traces exist;
        # validation.command_trace checks row width, frame count, and values.
        return validation.command_trace(
            ctx, "neutral.script.ground", "neutral.ground"
        ) and validation.command_trace(ctx, "neutral.script.air", "neutral.air")


@fighter
class CaptainFalcon(FighterBase):
    name = "captain-falcon"
    external_ids = (0,)
    parameters = CaptainFalconParameters
    attributes = CaptainFalconParameters
    action_state = CaptainFalconActionState
    specials = SpecialMoves(
        FalconPunch(),
        ActionMove(Action.SPECIAL_S),
        ActionMove(Action.SPECIAL_HI),
        ActionMove(Action.SPECIAL_LW),
    )


__all__ = [
    "CaptainFalcon",
    "CaptainFalconActionState",
    "CaptainFalconParameters",
    "FalconPunch",
]
