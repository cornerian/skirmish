"""Bowser's source-backed special motion states.

The native ``ftKoopa`` table supplies the phases and surface lifecycle.  The
Flame Breath article and side-special victim/item callbacks are deliberately
not authored here: those depend on the separate item/capture archives, so
entry remains gated by the corresponding fighter resource.
"""

from __future__ import annotations

from typing import Any

from skirmish import (
    Action,
    ActionState,
    Button,
    DownSpecial,
    Fighter,
    NeutralSpecial,
    SpecialMove,
    SpecialRoot,
    SideSpecial,
    Transition,
    UpSpecial,
    directional_b_input,
    directional_b_reserved,
    fresh_special_input,
    hook,
    source_phase,
    start_open_special,
)


class BowserActionState(ActionState):
    """Typed mirror of the native Klaw B edge latch."""

    klaw_b_held: bool = False


def _button_held(ctx: Any, button: Button) -> bool:
    """Read the host's held-button view with the portable default.

    ``ftKp_SpecialSWait_IASA`` only re-enters the hit motion while B remains
    held.  Older portable contexts do not expose a held mask, and their
    input-pressed event already means the button is down, so preserve that
    behavior as the fallback.
    """
    held = getattr(getattr(ctx, "input", None), "held_buttons", None)
    if held is None:
        return True
    if isinstance(held, (tuple, list, set, frozenset)):
        return button in held
    if isinstance(held, int) and button is Button.B:
        return bool(held & 0x200)
    return bool(held)


class _KoopaSpecial(SpecialMove):
    """Shared resource gate and directional B dispatch for Bowser specials."""

    _ACTIVE: tuple[Any, ...] = ()
    _ENTRY: tuple[Any, Any]

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        # Native IASA consumes B while any source phase is active.  The
        # separate article/capture effects are unavailable, but the move
        # itself must still be entered only with its native resource present.
        if fighter.action in self._ACTIVE:
            return True
        if self.root is SpecialRoot.NEUTRAL and not fresh_special_input(ctx, self.resource):
            return False
        if not self._direction_matches(ctx):
            return False
        return start_open_special(fighter, ctx, *self._ENTRY)

    def _direction_matches(self, ctx: Any) -> bool:
        if self.root is SpecialRoot.NEUTRAL:
            return not directional_b_reserved(ctx)
        if self.root is SpecialRoot.SIDE:
            return directional_b_input(ctx, self.resource, 0, "side_stick_threshold") is True
        if self.root is SpecialRoot.UP:
            return directional_b_input(
                ctx, self.resource, 1, "vertical_threshold", direction=1
            ) is True
        return directional_b_input(
            ctx, self.resource, 1, "vertical_threshold", direction=-1
        ) is True


class FlameBreath(NeutralSpecial, _KoopaSpecial):
    """Native states 341–346; Flame Breath's item article is not embedded."""

    ground_start = source_phase(341)
    ground_loop = source_phase(342, animation_loop=True)
    ground_end = source_phase(343)
    air_start = source_phase(344)
    air_loop = source_phase(345, animation_loop=True)
    air_end = source_phase(346)
    _ENTRY = (ground_start, air_start)
    _ACTIVE = (ground_start, ground_loop, ground_end, air_start, air_loop, air_end)

    on_end = {
        ground_start: Transition(ground_loop),
        ground_end: Transition(Action.WAIT),
        air_start: Transition(air_loop),
        air_end: Transition(Action.FALL),
    }
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_loop: Transition(ground_loop, preserve_state=True, keep_frame=True),
        air_end: Transition(ground_end, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_loop: Transition(air_loop, preserve_state=True, keep_frame=True),
        ground_end: Transition(air_end, preserve_state=True, keep_frame=True),
    }

    @hook.input_released(Button.B)
    def release(self, fighter: Any, ctx: Any) -> None:
        if fighter.action in (self.ground_start, self.ground_loop):
            fighter.change_action(self.ground_end)
        elif fighter.action in (self.air_start, self.air_loop):
            fighter.change_action(self.air_end)


class KoopaKlaw(SideSpecial, _KoopaSpecial):
    """Native states 347–358; capture/victim callbacks remain omitted."""

    ground_start = source_phase(347)
    ground_hit = source_phase(348)
    ground_hold = source_phase(349)
    ground_wait = source_phase(350, animation_loop=True)
    ground_end_forward = source_phase(351)
    ground_end_back = source_phase(352)
    air_start = source_phase(353)
    air_hit = source_phase(354)
    air_hold = source_phase(355)
    air_wait = source_phase(356, animation_loop=True)
    air_end_forward = source_phase(357)
    air_end_back = source_phase(358)
    _ENTRY = (ground_start, air_start)
    _ACTIVE = (
        ground_start, ground_hit, ground_hold, ground_wait,
        ground_end_forward, ground_end_back,
        air_start, air_hit, air_hold, air_wait,
        air_end_forward, air_end_back,
    )
    _TURN_PHASES = (ground_hit, ground_wait, air_hit, air_wait)

    @hook.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        state = getattr(fighter, "action_state", None)
        if state is not None and fighter.action in (
            self.ground_hit, self.air_hit, self.ground_wait, self.air_wait,
        ):
            state.klaw_b_held = True
        if fighter.action in (self.ground_wait, self.air_wait):
            return self.hold_capture(fighter, ctx)
        return super().input_pressed(fighter, ctx)

    @hook.action_enter(ground_start, air_start)
    def clear_latched_b(self, fighter: Any, ctx: Any) -> None:
        state = getattr(fighter, "action_state", None)
        if state is not None:
            state.klaw_b_held = False

    # The native hit/hold branch is selected by capture callbacks.  Those
    # callbacks require the victim archive and therefore cannot be represented
    # by this fighter-only package.  A missed start still follows the native
    # start animation's terminal path (wait/fall); the hit and hold states stay
    # available for a host that supplies the capture callback.
    on_end = {
        ground_start: Transition(Action.WAIT),
        ground_hit: Transition(ground_wait),
        ground_end_forward: Transition(Action.WAIT),
        ground_end_back: Transition(Action.WAIT),
        air_start: Transition(Action.FALL),
        air_end_forward: Transition(Action.FALL),
        air_end_back: Transition(Action.FALL),
    }

    @hook.before_hit(actions=(ground_start, air_start))
    def capture_contact(self, fighter: Any, ctx: Any) -> None:
        """Enter the source hit phase when the claw contact is confirmed.

        ``ftKp_SpecialS_8013302C``/``801330E4`` are invoked by the native
        capture contact path.  The victim hand-off remains host-owned, but the
        fighter motion transition is representable through ``before_hit``.
        A contact that does not establish a victim then falls through to the
        native wait phase at animation end.
        """
        if fighter.action == self.ground_start:
            fighter.change_action(self.ground_hit)
        elif fighter.action == self.air_start:
            fighter.change_action(self.air_hit)

    @hook.animation_end(air_hit)
    def finish_hit(self, fighter: Any, ctx: Any) -> None:
        """Keep a held capture latched when the hit animation completes."""
        state = getattr(fighter, "action_state", None)
        latched = bool(getattr(state, "klaw_b_held", False))
        if latched:
            target = self.air_hold
        else:
            target = self.air_wait
        if state is not None:
            state.klaw_b_held = False
        fighter.change_action(target)

    def hold_capture(self, fighter: Any, ctx: Any) -> bool:
        """Re-enter the source hold motion when capture B remains held.

        The native wait IASA callback consumes a held B by selecting state
        349/355 (the hit hold motion), while a released B leaves the wait
        state available for directional throw selection.  The previous
        fighter-only script consumed the event but never performed this
        observable transition.
        """
        target = {
            self.ground_wait: self.ground_hold,
            self.air_wait: self.air_hold,
        }.get(fighter.action)
        if target is None or not _button_held(ctx, Button.B):
            return fighter.action in self._ACTIVE
        state = getattr(fighter, "action_state", None)
        if state is not None:
            # Native inlineA0 clears mv.kp.specials.b_held on re-entry.
            state.klaw_b_held = False
        fighter.change_action(target)
        return True

    @hook.stick_changed(actions=_TURN_PHASES)
    def choose_throw_direction(self, fighter: Any, ctx: Any) -> bool:
        """Match ``ftKp_SpecialS{,Air}Hit/Wait_IASA`` direction selection."""
        if fighter.action not in self._TURN_PHASES:
            return False
        stick = getattr(getattr(ctx, "input", None), "stick", (0.0, 0.0))
        rules = getattr(ctx, "rules", None)
        specials = getattr(rules, "specials", None)
        threshold = getattr(specials, "side_stick_threshold", None)
        if threshold is None or abs(stick[0]) < threshold:
            return False
        # The native routine compares the stick direction in world space to
        # the fighter's facing direction: +1 selects forward, -1 back.
        forward = stick[0] * getattr(fighter, "facing", 1.0) > 0.0
        grounded = fighter.action in (self.ground_hit, self.ground_wait)
        if grounded:
            target = self.ground_end_forward if forward else self.ground_end_back
        else:
            target = self.air_end_forward if forward else self.air_end_back
        fighter.change_action(target, preserve_state=True, keep_frame=True)
        return True
    on_ground = {
        air_start: Transition(ground_start, preserve_state=True, keep_frame=True),
        air_hit: Transition(ground_hit, preserve_state=True, keep_frame=True),
        air_hold: Transition(ground_hold, preserve_state=True, keep_frame=True),
        air_wait: Transition(ground_wait, preserve_state=True, keep_frame=True),
        air_end_forward: Transition(ground_end_forward, preserve_state=True, keep_frame=True),
        air_end_back: Transition(ground_end_back, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground_start: Transition(air_start, preserve_state=True, keep_frame=True),
        ground_hit: Transition(air_hit, preserve_state=True, keep_frame=True),
        ground_hold: Transition(air_hold, preserve_state=True, keep_frame=True),
        ground_wait: Transition(air_wait, preserve_state=True, keep_frame=True),
        ground_end_forward: Transition(air_end_forward, preserve_state=True, keep_frame=True),
        ground_end_back: Transition(air_end_back, preserve_state=True, keep_frame=True),
    }


class WhirlingFortress(UpSpecial, _KoopaSpecial):
    """Native states 359–360; armor/effect callbacks remain native-gated."""

    ground = source_phase(359)
    air = source_phase(360)
    _ENTRY = (ground, air)
    _ACTIVE = (ground, air)

    on_end = {ground: Transition(Action.WAIT), air: Transition(Action.FALL)}
    # ftKp_SpecialAirHi_Coll converts back to the ground motion state when
    # the rising/falling fortress reaches a platform.
    on_ground = {air: Transition(ground, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}


class Bomb(DownSpecial, _KoopaSpecial):
    """Native states 361–363; the ground-pound effect remains resource-gated."""

    ground = source_phase(361)
    air = source_phase(362)
    landing = source_phase(363)
    _ENTRY = (ground, air)
    _ACTIVE = (ground, air, landing)

    on_end = {
        ground: Transition(air),
        landing: Transition(Action.FALL),
    }
    on_ground = {air: Transition(landing, preserve_state=True, keep_frame=True)}
    on_air = {ground: Transition(air, preserve_state=True, keep_frame=True)}

    def _transition_animation_end(self, fighter: Any, ctx: Any) -> None:
        """Enter the aerial pound at the native launch frame.

        ``ftKp_SpecialLw_Anim`` calls ``ftKp_SpecialLw_80134988`` when the
        ground pound ends.  That helper changes to motion 362 at animation
        frame 30, rather than restarting the aerial motion at frame zero.
        The generic transition descriptor has no frame target, but the
        portable fighter proxy exposes ``action_frame`` for this source
        callback.
        """
        if fighter.action == self.ground:
            fighter.change_action(self.air)
            fighter.action_frame = 30
            return
        if fighter.action == self.air:
            # ftKp_SpecialAirLw_Anim sets its landing command flag when the
            # aerial motion ends, but deliberately stays in motion 362.  The
            # collision callback consumes that flag on a later ground check;
            # falling here would skip the native landing phase entirely.
            return
        super()._transition_animation_end(fighter, ctx)


class Bowser(Fighter):
    action_state = BowserActionState
    specials = Fighter.specials.replace(
        neutral=FlameBreath(),
        side=KoopaKlaw(),
        up=WhirlingFortress(),
        down=Bomb(),
    )


__all__ = [
    "Bowser",
    "BowserActionState",
    "FlameBreath",
    "KoopaKlaw",
    "WhirlingFortress",
    "Bomb",
]
