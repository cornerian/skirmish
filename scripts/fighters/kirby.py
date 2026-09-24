"""Kirby's source-defined specials.

The native Kirby neutral special branches into victim capture, Copy ability,
and spit-star/item state. Those branches belong to the native fighter/item
system, so this script keeps their source motion identities without trying to
recreate dynamic copied specials or article behavior.
"""

from skirmish import (
    Action,
    Button,
    DownSpecial,
    Fighter,
    NeutralSpecial,
    SideSpecial,
    SpecialRoot,
    Transition,
    UpSpecial,
    directional_b_input,
    directional_b_reserved,
    fresh_special_input,
    frame_preserving_surface_pairs,
    hook,
    source_phase,
    start_action,
)


class _KirbySpecial:
    """Shared resource gate for Kirby's four native B entry states."""

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx) -> bool:
        if fighter.action in self._ACTIVE:
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
        if allowed is not True or not (ctx.ground_open or ctx.air_open):
            return False
        start_action(fighter, self.ground if ctx.ground_open else self.air)
        return True


class Inhale(NeutralSpecial, _KirbySpecial):
    """Inhale/copy shell; victim and copied-action branches stay native-only."""

    ground = source_phase(353)
    ground_loop = source_phase(354, animation_loop=True)
    ground_end = source_phase(355)
    ground_capture = source_phase(356)
    ground_capture_wait = source_phase(357)
    ground_eat = source_phase(358)
    ground_eat_wait = source_phase(359, animation_loop=True)
    ground_eat_walk_slow = source_phase(360)
    ground_eat_walk_middle = source_phase(361)
    ground_eat_walk_fast = source_phase(362)
    ground_eat_turn = source_phase(363)
    ground_eat_jump = source_phase(364)
    ground_eat_jump2 = source_phase(365)
    ground_eat_landing = source_phase(366)
    ground_drink = source_phase(367)
    ground_drink_end = source_phase(368)
    ground_spit = source_phase(369)
    ground_spit_end = source_phase(370)

    air = source_phase(371)
    air_loop = source_phase(372, animation_loop=True)
    air_end = source_phase(373)
    air_capture = source_phase(374)
    air_capture_wait = source_phase(375)
    air_eat = source_phase(376)
    air_eat_fall = source_phase(377)
    air_drink = source_phase(378)
    air_drink_end = source_phase(379)
    air_spit = source_phase(380)
    air_spit_end = source_phase(381)
    air_eat_turn = source_phase(382)

    _ACTIVE = (
        ground, ground_loop, ground_end, ground_capture, ground_capture_wait,
        ground_eat, ground_eat_wait, ground_eat_walk_slow,
        ground_eat_walk_middle, ground_eat_walk_fast, ground_eat_turn,
        ground_eat_jump, ground_eat_jump2, ground_eat_landing, ground_drink,
        ground_drink_end, ground_spit, ground_spit_end, air, air_loop, air_end,
        air_capture, air_capture_wait, air_eat, air_eat_fall, air_drink,
        air_drink_end, air_spit, air_spit_end, air_eat_turn,
    )

    @hook.input_released(Button.B)
    def release(self, fighter: Fighter, ctx) -> bool:
        """The source loop leaves only when the swallow button is released."""
        destination = {
            self.ground_loop: self.ground_end,
            self.air_loop: self.air_end,
        }.get(fighter.action)
        if destination is None:
            return False
        fighter.change_action(destination)
        return True

    on_end = {
        ground: Transition(ground_loop),
        ground_end: Transition(Action.WAIT),
        air: Transition(air_loop),
        air_end: Transition(Action.FALL),
    }
    on_ground, on_air = frame_preserving_surface_pairs(
        ground, ground, air, air
    )
    _ground, _air = frame_preserving_surface_pairs(
        ground_loop, ground_loop, air_loop, air_loop
    )
    on_ground.update(_ground)
    on_air.update(_air)
    # Capture, eat, drink, and spit each have paired source motions.  The
    # victim/item callbacks remain native-owned, while surface changes still
    # preserve the active source motion and animation frame.
    _ground, _air = frame_preserving_surface_pairs(
        ground_capture, ground_capture, air_capture_wait, air_capture_wait
    )
    on_ground.update(_ground)
    on_air.update(_air)
    _ground, _air = frame_preserving_surface_pairs(
        ground_capture_wait, ground_capture_wait, air_capture, air_capture
    )
    on_ground.update(_ground)
    on_air.update(_air)
    _ground, _air = frame_preserving_surface_pairs(
        ground_eat, ground_eat, air_eat, air_eat
    )
    on_ground.update(_ground)
    on_air.update(_air)
    _ground, _air = frame_preserving_surface_pairs(
        ground_eat_wait, ground_eat_wait, air_eat_fall, air_eat_fall
    )
    on_ground.update(_ground)
    on_air.update(_air)
    _ground, _air = frame_preserving_surface_pairs(
        ground_drink, ground_drink, air_drink_end, air_drink_end
    )
    on_ground.update(_ground)
    on_air.update(_air)
    _ground, _air = frame_preserving_surface_pairs(
        ground_drink_end, ground_drink_end, air_drink, air_drink
    )
    on_ground.update(_ground)
    on_air.update(_air)
    _ground, _air = frame_preserving_surface_pairs(
        ground_spit, ground_spit, air_spit_end, air_spit_end
    )
    on_ground.update(_ground)
    on_air.update(_air)
    _ground, _air = frame_preserving_surface_pairs(
        ground_spit_end, ground_spit_end, air_spit, air_spit
    )
    on_ground.update(_ground)
    on_air.update(_air)
    _ground, _air = frame_preserving_surface_pairs(
        ground_eat_turn, ground_eat_turn, air_eat_turn, air_eat_turn
    )
    on_ground.update(_ground)
    on_air.update(_air)
    _ground, _air = frame_preserving_surface_pairs(
        ground_end, ground_end, air_end, air_end
    )
    on_ground.update(_ground)
    on_air.update(_air)


class Hammer(SideSpecial, _KirbySpecial):
    """Hammer's ground and aerial source states."""

    ground = source_phase(383)
    air = source_phase(384)
    _ACTIVE = (ground, air)

    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    on_ground, on_air = frame_preserving_surface_pairs(ground, ground, air, air)


class FinalCutter(UpSpecial, _KirbySpecial):
    """Final Cutter's four source phases on each surface."""

    ground_start = source_phase(385)
    ground_rise = source_phase(386)
    ground_fall = source_phase(387)
    ground_end = source_phase(388)
    air_start = source_phase(389)
    air_rise = source_phase(390)
    air_fall = source_phase(391)
    air_end = source_phase(392)
    ground, air = ground_start, air_start
    _ACTIVE = (
        ground_start, ground_rise, ground_fall, ground_end,
        air_start, air_rise, air_fall, air_end,
    )

    @hook.stick(actions=(ground_start, air_start))
    def reverse(self, fighter: Fighter, ctx) -> bool:
        """Mirror ftKb_SpecialHi[1] IASAs' one-shot reverse input."""
        if fighter.action not in (self.ground_start.action, self.air_start.action):
            return False
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", (0, 0, 0, 0))
        if len(command) < 4 or command[3]:
            return False
        stick = getattr(getattr(ctx, "input", None), "stick", (0.0, 0.0))
        threshold = getattr(
            getattr(getattr(ctx, "rules", None), "specials", None),
            "reverse_upb_stick_range", 0.5,
        )
        if abs(stick[0]) <= threshold or stick[0] * fighter.facing >= 0:
            return False
        fighter.facing = -fighter.facing
        state.command = (command[0], command[1], command[2], 1)
        return True

    on_end = {
        # The native ground callback enters the aerial travel phases after
        # launch; the collision callback owns the optional grounded finish.
        ground_start: Transition(air_rise),
        ground_rise: Transition(air_fall),
        ground_end: Transition(Action.WAIT),
        air_start: Transition(air_rise),
        air_rise: Transition(air_fall),
        air_end: Transition(Action.FALL),
    }
    on_ground, on_air = frame_preserving_surface_pairs(
        ground_start, ground_rise, air_start, air_rise
    )
    _ground, _air = frame_preserving_surface_pairs(
        ground_fall, ground_end, air_fall, air_end
    )
    on_ground.update(_ground)
    on_air.update(_air)


class Stone(DownSpecial, _KirbySpecial):
    """Stone's entry, held, and release phases."""

    ground_start = source_phase(393)
    ground_hold = source_phase(394, animation_loop=True)
    ground_end = source_phase(395)
    air_start = source_phase(396)
    air_hold = source_phase(397, animation_loop=True)
    air_end = source_phase(398)
    ground, air = ground_start, air_start
    _ACTIVE = (ground_start, ground_hold, ground_end, air_start, air_hold, air_end)

    on_end = {
        ground_start: Transition(ground_hold),
        ground_end: Transition(Action.WAIT),
        air_start: Transition(air_hold),
        air_end: Transition(Action.FALL),
    }
    on_ground, on_air = frame_preserving_surface_pairs(
        ground_start, ground_hold, air_start, air_hold
    )
    _ground, _air = frame_preserving_surface_pairs(
        ground_end, ground_end, air_end, air_end
    )
    on_ground.update(_ground)
    on_air.update(_air)


class Kirby(Fighter):
    specials = Fighter.specials.replace(
        neutral=Inhale(),
        side=Hammer(),
        up=FinalCutter(),
        down=Stone(),
    )


__all__ = ["Kirby", "Inhale", "Hammer", "FinalCutter", "Stone"]
