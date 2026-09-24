"""Yoshi's source-defined special motion families.

The fighter callbacks in ``ftYoshi`` branch on egg, tongue, and star article
state. Those article archives are not part of the authoring package, so the
script keeps their native motion states and lifecycle transitions while the
directional resource gate prevents a partial article implementation from
pretending to be complete.
"""

from skirmish import (
    Action,
    Button,
    Fighter,
    NeutralSpecial,
    SideSpecial,
    UpSpecial,
    DownSpecial,
    SpecialRoot,
    Transition,
    directional_b_input,
    directional_b_reserved,
    fresh_special_input,
    frame_preserving_surface_pairs,
    hook,
    source_phase,
    start_action,
)


class _YoshiSpecial:
    """Resource-gated B dispatch shared by Yoshi's four source moves."""

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx) -> bool:
        if fighter.action in self._ACTIVE:
            active_handler = getattr(self, "_on_active_input", None)
            if active_handler is not None:
                active_handler(fighter)
            return True
        if not fresh_special_input(ctx, self.resource):
            return False
        if self.root is SpecialRoot.NEUTRAL:
            allowed = not directional_b_reserved(ctx)
        elif self.root is SpecialRoot.SIDE:
            allowed = directional_b_input(ctx, self.resource, 0, "side_stick_threshold")
        else:
            allowed = directional_b_input(
                ctx,
                self.resource,
                1,
                "vertical_threshold",
                direction=1 if self.root is SpecialRoot.UP else -1,
            )
        if allowed is not True:
            return False
        if not (ctx.ground_open or ctx.air_open):
            return False
        start_action(fighter, self.ground if ctx.ground_open else self.air)
        return True


class EggLay(NeutralSpecial, _YoshiSpecial):
    """Neutral special phases; article capture/lay branches stay gated."""

    ground = source_phase(346)
    ground_tongue = source_phase(347)
    ground_egg = source_phase(348)
    ground_swallow = source_phase(349)
    ground_end = source_phase(350)
    air = source_phase(351)
    air_tongue = source_phase(352)
    air_egg = source_phase(353)
    air_swallow = source_phase(354)
    air_end = source_phase(355)
    _ACTIVE = (
        ground,
        ground_tongue,
        ground_egg,
        ground_swallow,
        ground_end,
        air,
        air_tongue,
        air_egg,
        air_swallow,
        air_end,
    )
    on_end = {
        ground: Transition(Action.WAIT),
        ground_tongue: Transition(Action.WAIT),
        ground_egg: Transition(Action.WAIT),
        ground_swallow: Transition(Action.WAIT),
        ground_end: Transition(Action.WAIT),
        air: Transition(Action.FALL),
        air_tongue: Transition(Action.FALL),
        air_egg: Transition(Action.FALL),
        air_swallow: Transition(Action.FALL),
        air_end: Transition(Action.FALL),
    }
    # The source motion table has one ground and one air state for each of
    # the five neutral-special phases (346..350 and 351..355).  Keep these
    # pairs explicit: the middle phases are selected by article/victim
    # command variables in the decomp and must still preserve their phase
    # when a ground/air collision changes contact.
    on_ground, on_air = frame_preserving_surface_pairs(
        ground, ground_tongue, air, air_tongue
    )
    for ground_phase, air_phase in (
        (ground_egg, air_egg),
        (ground_swallow, air_swallow),
        (ground_end, air_end),
    ):
        on_ground[air_phase] = Transition(ground_phase, preserve_state=True, keep_frame=True)
        on_air[ground_phase] = Transition(air_phase, preserve_state=True, keep_frame=True)


class EggRoll(SideSpecial, _YoshiSpecial):
    """Egg Roll's ground and aerial source state machines."""

    ground = source_phase(360)
    ground_start = source_phase(356)
    ground_loop = source_phase(357)
    ground_turn = source_phase(358)
    ground_end = source_phase(359)
    air = ground
    air_loop = source_phase(361)
    air_turn = source_phase(362)
    air_landing = source_phase(363)
    _ACTIVE = (
        ground,
        ground_start,
        ground_loop,
        ground_turn,
        ground_end,
        air_loop,
        air_turn,
        air_landing,
    )
    on_end = {
        ground: Transition(air_loop),
        air_loop: Transition(air_turn),
        air_turn: Transition(air_landing),
        air_landing: Transition(Action.FALL),
        ground_start: Transition(ground_loop),
        ground_loop: Transition(ground_turn),
        ground_turn: Transition(ground_end),
        ground_end: Transition(Action.WAIT),
    }
    on_ground = {
        ground: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_loop: Transition(ground_loop, preserve_state=True, keep_frame=True),
        air_turn: Transition(ground_turn, preserve_state=True, keep_frame=True),
        air_landing: Transition(Action.WAIT),
    }
    on_air = {
        ground_start: Transition(ground, preserve_state=True, keep_frame=True),
        ground_loop: Transition(air_loop, preserve_state=True, keep_frame=True),
        ground_turn: Transition(air_turn, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_landing, preserve_state=True, keep_frame=True),
    }

    def _on_active_input(self, fighter: Fighter) -> None:
        # ftYs_SpecialAirSLoop_{0,1}_IASA and Loop_{2,3}_IASA use B as the
        # release edge.  The native helper preserves the current animation
        # frame while selecting the ground end or aerial landing phase.
        if fighter.action in (self.ground_loop, self.ground_turn):
            fighter.change_action(self.ground_end, preserve_state=True, keep_frame=True)
        elif fighter.action in (self.air_loop, self.air_turn):
            fighter.change_action(self.air_landing, preserve_state=True, keep_frame=True)


class EggToss(UpSpecial, _YoshiSpecial):
    """Egg Toss source phases without synthesizing the egg article."""

    ground = source_phase(364)
    air = source_phase(365)
    _ACTIVE = (ground, air)
    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground, on_air = frame_preserving_surface_pairs(ground, ground, air, air)


class GroundPound(DownSpecial, _YoshiSpecial):
    """Ground Pound and its native landing state."""

    ground = source_phase(366)
    landing = source_phase(367)
    air = source_phase(368)
    _ACTIVE = (ground, landing, air)
    on_end = {
        ground: Transition(air),
        landing: Transition(Action.WAIT),
        # ftYs_SpecialAirLw_Anim only arms the descent command when its
        # animation expires; the source keeps the airborne pound active until
        # collision resolves it.  Falling here would skip the collision path
        # and lose the landing/star behavior.
    }
    on_ground = {air: Transition(landing, preserve_state=True, keep_frame=True)}
    on_air = frame_preserving_surface_pairs(ground, ground, air, air)[1]


class Yoshi(Fighter):
    specials = Fighter.specials.replace(
        neutral=EggLay(),
        side=EggRoll(),
        up=EggToss(),
        down=GroundPound(),
    )


__all__ = ["Yoshi", "EggLay", "EggRoll", "EggToss", "GroundPound"]
