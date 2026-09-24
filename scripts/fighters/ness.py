"""Ness's source-defined special motion states.

The fighter-side callbacks are described here, while PK Flash, PK Fire, and
PK Thunder articles remain host resources. Their item archives are not part
of the authoring package, so these moves deliberately stop at the native
fighter states and never invent projectile behavior. The transition tables
mirror the animation callbacks in ``ftnessspecialn.c``, ``ftnessspecials.c``,
``ftnessspecialhi.c``, and ``ftnessspeciallw.c``; article creation, collision
steering, and landing lag remain host-owned until those resources are
available.
"""

from skirmish import (
    Action,
    Button,
    DownSpecial,
    Fighter,
    NeutralSpecial,
    SideSpecial,
    Transition,
    UpSpecial,
    directional_match,
    fresh_special_input,
    hook,
    resource_attributes,
    source_phase,
    start_action,
)


def _phase(state: int, *, loop: bool = False):
    return source_phase(state, animation_loop=loop)


class _NessSpecial:
    """Shared source-motion availability gate for Ness's B specials."""

    _ACTIVE = ()

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx) -> bool:
        if fighter.action in self._ACTIVE:
            return True
        if not fresh_special_input(ctx, self.resource):
            return False
        if not directional_match(ctx, self.root):
            return False
        grounded = bool(ctx.ground_open)
        if not (grounded or ctx.air_open):
            return False
        phase = self.ground if grounded else self.air
        animation = getattr(fighter, "has_complete_animation", None)
        if callable(animation):
            state = dict(phase.metadata)["slippi_state"]
            if not animation(state):
                return False
        start_action(fighter, phase)
        return True


class PKFlash(NeutralSpecial, _NessSpecial):
    ground_start = _phase(348)
    ground_hold = _phase(349, loop=True)
    ground_release = _phase(350)
    ground_end = _phase(351)
    air_start = _phase(352)
    air_hold = _phase(353, loop=True)
    air_release = _phase(354)
    air_end = _phase(355)
    ground = ground_start
    air = air_start
    _ACTIVE = (
        ground_start, ground_hold, ground_release,
        ground_end, air_start, air_hold, air_release, air_end,
    )

    @hook.input_released(Button.B)
    def release(self, fighter: Fighter, ctx) -> bool:
        destination = {
            self.ground_hold: self.ground_release,
            self.air_hold: self.air_release,
        }.get(fighter.action)
        if destination is None:
            return False
        fighter.change_action(destination)
        return True

    on_end = {
        ground_start: Transition(ground_hold),
        air_start: Transition(air_hold),
        ground_release: Transition(ground_end),
        air_release: Transition(air_end),
        ground_end: Transition(Action.WAIT),
        air_end: Transition(Action.FALL),
    }
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_hold: Transition(ground_hold, preserve_state=True, keep_frame=True),
        air_release: Transition(ground_release, preserve_state=True, keep_frame=True),
        air_end: Transition(ground_end, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_hold: Transition(air_hold, preserve_state=True, keep_frame=True),
        ground_release: Transition(air_release, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_end, preserve_state=True, keep_frame=True),
    }


class PKFire(SideSpecial, _NessSpecial):
    ground = _phase(356)
    air = _phase(357)
    _ACTIVE = (ground, air)

    @hook.landed(actions=(air,))
    def landing(self, fighter: Fighter, ctx) -> bool:
        """Enter the source aerial PK Fire landing-fall-special state."""
        enter = getattr(fighter, "enter_landing_special", None)
        if not callable(enter):
            return False
        attributes = resource_attributes(ctx, self.resource)
        if attributes is None:
            return False
        escape_air = ctx.resource("escape_air")
        animation_end = getattr(escape_air, "landing_animation_end", None)
        if animation_end is None:
            return False
        landing_lag = getattr(
            attributes,
            "pkfire_landing_lag",
            getattr(attributes, "specials_landing_lag", getattr(attributes, "x38", None)),
        )
        if landing_lag is None:
            return False
        # ``ftCo_LandingFallSpecial_Enter(false, x38)`` maps to the native
        # landing helper: false means the resulting landing state cannot be
        # interrupted, and the helper derives its animation rate from the
        # common escape-air landing end frame.
        enter(animation_end, landing_lag)
        return True

    @hook.ground_air_changed(ground)
    def ground_to_air(self, fighter: Fighter, ctx) -> bool:
        """Match ``ftNs_SpecialS_Coll`` falling off the ground."""
        if ctx.grounded:
            return False
        fighter.change_action(Action.FALL)
        return True

    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground = {}
    on_air = {}


class PKThunder(UpSpecial, _NessSpecial):
    ground_start = _phase(358)
    ground_hold = _phase(359, loop=True)
    ground_end = _phase(360)
    ground_launch = _phase(361)
    air_start = _phase(362)
    air_hold = _phase(363, loop=True)
    air_end = _phase(364)
    air_launch = _phase(365)
    air_rebound = _phase(366)
    ground = ground_start
    air = air_start
    _ACTIVE = (
        ground_start, ground_hold, ground_end, ground_launch,
        air_start, air_hold, air_end, air_launch, air_rebound,
    )

    on_end = {
        ground_start: Transition(ground_hold),
        air_start: Transition(air_hold),
        ground_launch: Transition(ground_end),
        ground_end: Transition(Action.WAIT),
    }
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_hold: Transition(ground_hold, preserve_state=True, keep_frame=True),
        air_end: Transition(ground_end, preserve_state=True, keep_frame=True),
        # ``ftNs_SpecialAirHi_Coll`` uses AirToGroundStateChange to enter
        # the grounded PK Thunder 2 motion when its launch path meets a
        # valid floor.
        air_launch: Transition(ground_launch, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_hold: Transition(air_hold, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_end, preserve_state=True, keep_frame=True),
        # ``ftNs_SpecialHi_Coll`` changes the grounded launch motion to the
        # aerial launch motion when Ness loses the floor.
        ground_launch: Transition(air_launch, preserve_state=True, keep_frame=True),
    }

    @hook.animation_end(air_end, air_launch, air_rebound)
    def enter_fall_special(self, fighter: Fighter, ctx) -> bool:
        """Match the source PK Thunder 2 FallSpecial exit.

        The pinned ``ftnessspecialhi.c`` callbacks for states 364, 365, and
        366 use ``x70_PK_THUNDER_2_LANDING_LAG``: zero enters ordinary fall,
        while a nonzero value enters FallSpecial with full aerial mobility.
        The source also supplies ``x6C_PK_THUNDER_2_FREEFALL_ANIM_BLEND``;
        the current typed ``enter_fall_special`` host contract accepts only
        mobility and landing lag, so that blend remains host-owned and is not
        fabricated here.  The native host performs the source jump-budget and
        ground-velocity reset while entering FallSpecial.
        """
        attributes = resource_attributes(ctx, self.resource)
        landing_lag = getattr(
            attributes,
            "pkthunder2_landing_lag",
            getattr(attributes, "specialhi_landing_lag", getattr(attributes, "x70", None)),
        )
        if landing_lag is None:
            fighter.change_action(Action.FALL)
            return True
        if landing_lag == 0:
            fighter.change_action(Action.FALL)
            return True
        enter = getattr(fighter, "enter_fall_special", None)
        if not callable(enter):
            fighter.change_action(Action.FALL)
            return True
        enter(mobility=1, landing_lag=landing_lag)
        return True


class PSIMagnet(DownSpecial, _NessSpecial):
    ground_start = _phase(367)
    ground_hold = _phase(368, loop=True)
    ground_hit = _phase(369)
    ground_end = _phase(370)
    ground_turn = _phase(371)
    air_start = _phase(372)
    air_hold = _phase(373, loop=True)
    air_hit = _phase(374)
    air_end = _phase(375)
    air_turn = _phase(376)
    ground = ground_start
    air = air_start
    _ACTIVE = (
        ground_start, ground_hold, ground_hit, ground_end, ground_turn,
        air_start, air_hold, air_hit, air_end, air_turn,
    )

    _RELEASE_TARGETS = {
        ground_hold: ground_end,
        ground_hit: ground_end,
        ground_turn: ground_end,
        air_hold: air_end,
        air_hit: air_end,
        air_turn: air_end,
    }

    @hook.input_released(Button.B)
    def release(self, fighter: Fighter, ctx) -> bool:
        destination = self._RELEASE_TARGETS.get(fighter.action)
        if destination is None:
            return False
        fighter.change_action(destination)
        return True

    on_end = {
        ground_start: Transition(ground_hold),
        air_start: Transition(air_hold),
        ground_hit: Transition(ground_hold),
        air_hit: Transition(air_hold),
        ground_turn: Transition(ground_hold),
        air_turn: Transition(air_hold),
        ground_end: Transition(Action.WAIT),
        air_end: Transition(Action.FALL),
    }
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_hold: Transition(ground_hold, preserve_state=True, keep_frame=True),
        air_hit: Transition(ground_hit, preserve_state=True, keep_frame=True),
        air_end: Transition(ground_end, preserve_state=True, keep_frame=True),
        air_turn: Transition(ground_turn, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_hold: Transition(air_hold, preserve_state=True, keep_frame=True),
        ground_hit: Transition(air_hit, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_end, preserve_state=True, keep_frame=True),
        ground_turn: Transition(air_turn, preserve_state=True, keep_frame=True),
    }


class Ness(Fighter):
    specials = Fighter.specials.replace(
        neutral=PKFlash(), side=PKFire(), up=PKThunder(), down=PSIMagnet()
    )


__all__ = ["Ness", "PKFlash", "PKFire", "PKThunder", "PSIMagnet"]
