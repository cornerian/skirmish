"""Popo's source-defined special motion graph.

The native Ice Climbers implementation routes several branches through Nana
and through the ice, blizzard, and belay articles. This declaration keeps the
lead fighter's source states and ordinary ground/air lifecycle visible while
leaving companion synchronization and article ownership to the native host.
"""

from __future__ import annotations

from typing import Any

from skirmish import (
    Action,
    DirectionalSpecial,
    DownSpecial,
    Fighter,
    NeutralSpecial,
    SideSpecial,
    Transition,
    UpSpecial,
    MoveContext,
    frame_preserving_surface_pairs,
    on,
    source_phase,
)


class IceShot(NeutralSpecial, DirectionalSpecial):
    """Ice Shot's fighter phases; the ice article remains native-owned."""

    ground = source_phase(341)
    air = source_phase(342)
    _ACTIVE = (ground, air)

    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground, on_air = frame_preserving_surface_pairs(ground, ground, air, air)


class SquallHammer(SideSpecial, DirectionalSpecial):
    """Squall Hammer's four Popo motion rows (ftPp special S1/S2).

    The source collision callbacks rebound wall velocity, switch S1/S2 to
    their aerial rows, and synchronize Nana's attached pose. Those operations
    need collision normals, article hitlag state, and the companion object;
    they remain native-host responsibilities here.
    """

    ground_start = source_phase(343)
    ground_partner = source_phase(344)
    air_start = source_phase(345)
    air_partner = source_phase(346)
    # Public roots select the first source row; Nana/article code can enter
    # the partner rows directly through the native host.
    ground, air = ground_start, air_start
    _ACTIVE = (ground_start, ground_partner, air_start, air_partner)

    on_end = {
        ground_start: Transition(Action.WAIT),
        ground_partner: Transition(Action.WAIT),
        air_start: Transition(Action.FALL),
        air_partner: Transition(Action.FALL),
    }
    on_ground, on_air = frame_preserving_surface_pairs(
        ground_start, ground_partner, air_start, air_partner
    )

    @on.surface_contact()
    def wall_rebound(self, fighter: Any, ctx: MoveContext) -> bool:
        """Advance Squall Hammer's S1 row after a source wall collision.

        ``ftPp_SpecialS1_Coll`` selects the S2 animation on wall contact;
        native collision code owns the normal and rebound velocity.  A floor
        contact must not consume this transition.
        """
        wall = getattr(ctx, "wall", None)
        if wall is None:
            wall = getattr(ctx, "wall_contact", False)
        if not wall:
            return False
        current = getattr(fighter.action, "action", fighter.action)
        ground_start = getattr(self.ground_start, "action", self.ground_start)
        air_start = getattr(self.air_start, "action", self.air_start)
        if current == ground_start:
            target = self.ground_partner
        elif current == air_start:
            target = self.air_partner
        else:
            return False
        fighter.change_action(target, preserve_state=True, keep_frame=True)
        return True


class Belay(UpSpecial, DirectionalSpecial):
    """Belay's ten Popo rows; companion selection remains native-owned.

    The ``*_start_1`` rows are the source fallback branch used when Nana is
    unavailable. The Python API cannot inspect the companion controller, so
    it exports both branches and leaves that selection to the native host.
    Rope creation, launch velocity, wall/ceiling collision, and Nana's
    teleport/throw callbacks likewise remain native-only.
    """

    ground_start_0 = source_phase(347)
    ground_throw_0 = source_phase(348)
    ground_throw_2 = source_phase(349)
    ground_start_1 = source_phase(350)
    ground_throw_1 = source_phase(351)
    air_start_0 = source_phase(352)
    air_throw_0 = source_phase(353)
    air_throw_2 = source_phase(354)
    air_start_1 = source_phase(355)
    air_throw_1 = source_phase(356)
    ground, air = ground_start_0, air_start_0
    # Compatibility aliases for the earlier generic names.
    ground_throw = ground_throw_0
    ground_launch = ground_throw_2
    ground_fallback = ground_start_1
    ground_fallback_throw = ground_throw_1
    air_throw = air_throw_0
    air_launch = air_throw_2
    air_fallback = air_start_1
    air_fallback_throw = air_throw_1
    _ACTIVE = (
        ground_start_0, ground_throw_0, ground_throw_2, ground_start_1,
        ground_throw_1, air_start_0, air_throw_0, air_throw_2, air_start_1,
        air_throw_1,
    )

    on_end = {
        ground_start_0: Transition(ground_throw_0),
        ground_throw_0: Transition(Action.WAIT),
        ground_throw_2: Transition(Action.WAIT),
        ground_start_1: Transition(ground_throw_1),
        ground_throw_1: Transition(Action.WAIT),
        air_start_0: Transition(air_throw_0),
        air_throw_0: Transition(Action.FALL),
        air_throw_2: Transition(Action.FALL),
        air_start_1: Transition(air_throw_1),
        air_throw_1: Transition(Action.FALL),
    }
    on_ground, on_air = frame_preserving_surface_pairs(
        ground_start_0, ground_throw_0, air_start_0, air_throw_0
    )
    fallback_ground, fallback_air = frame_preserving_surface_pairs(
        ground_throw_2, ground_start_1, air_throw_2, air_start_1
    )
    on_ground.update(fallback_ground)
    on_air.update(fallback_air)
    final_ground, final_air = frame_preserving_surface_pairs(
        ground_throw_1, ground_throw_1, air_throw_1, air_throw_1
    )
    on_ground.update(final_ground)
    on_air.update(final_air)

    @on.command_changed(2, actions=(ground_start_0, air_start_0))
    def partner_fallback(self, fighter: Any, ctx: MoveContext) -> None:
        """Enter the source's no-Nana start branch when the host reports it.

        ``ftPp_SpecialHiStart_{0,Air}_Anim`` checks command 2 and then calls
        the fallback motion only when Nana is out of range.  Partner range is
        native state, so an absent field leaves this callback inert rather
        than guessing from the command value alone.
        """
        event = getattr(ctx, "event", None)
        if not getattr(event, "value", 0) or getattr(ctx, "partner_available", None) is not False:
            return
        current = getattr(fighter.action, "action", fighter.action)
        ground_start = getattr(self.ground_start_0, "action", self.ground_start_0)
        target = self.ground_start_1 if current == ground_start else self.air_start_1
        fighter.change_action(target, preserve_state=True, keep_frame=True)

    @on.command_changed(1, actions=(ground_throw_0, air_throw_0))
    def partner_launch(self, fighter: Any, ctx: MoveContext) -> None:
        """Enter AirHiThrow2 when native Nana launch state is observed."""
        event = getattr(ctx, "event", None)
        if not getattr(event, "value", 0) or getattr(ctx, "partner_launching", None) is not True:
            return
        # ftPp_SpecialHi_8012280C always selects motion state 354, including
        # when the command arrived on the ground throw row.
        fighter.change_action(self.air_throw_2)


class Blizzard(DownSpecial, DirectionalSpecial):
    """Blizzard's ground/air fighter phases; the blizzard article is native."""

    ground = source_phase(357)
    air = source_phase(358)
    _ACTIVE = (ground, air)

    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground, on_air = frame_preserving_surface_pairs(ground, ground, air, air)


class IceClimbers(Fighter):
    specials = Fighter.specials.replace(
        neutral=IceShot(),
        side=SquallHammer(),
        up=Belay(),
        down=Blizzard(),
    )


__all__ = ["IceClimbers", "IceShot", "SquallHammer", "Belay", "Blizzard"]
