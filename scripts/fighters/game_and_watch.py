"""Mr. Game & Watch's source-defined special motion states.

The native fighter owns the Chef, Judge, Fire, and Oil Panic article logic.
Those article archives are not part of the authoring API yet, so this module
keeps the fighter-side source states, input/resource gates, and finite motion
lifecycle only.  Judge accepts an optional native-provided roll and weights;
without them it uses row one as a deterministic authoring fallback.
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
    DirectionalSpecial,
    directional_match,
    fresh_special_input,
    on,
    source_phase,
    start_action,
)


class GameAndWatchActionState(ActionState):
    """Portable mirrors of the fighter-local special callback variables."""

    chef_loop_disabled: bool = False
    judge_current: int = -1
    judge_previous: int = -1
    # One-based value for the native Judge article; zero means no roll.
    judge_selected_value: int = 0
    panic_charge: int = 0
    panic_damage: float = 0.0
    panic_release_damage: float = 0.0


def _fighter_state(fighter: Any) -> GameAndWatchActionState:
    state = getattr(fighter, "action_state", None)
    if state is None:
        state = GameAndWatchActionState()
        fighter.action_state = state
    return state


def _resource_attributes(ctx: Any, path: str) -> Any:
    lookup = getattr(ctx, "resource", None)
    resource = lookup(path) if callable(lookup) else None
    return getattr(resource, "attributes", resource)


def _button_held(ctx: Any, button: Button) -> bool:
    value = getattr(ctx, "chef_b_held", None)
    if value is not None:
        return bool(value)
    held_buttons = getattr(getattr(ctx, "input", None), "held_buttons", None)
    if held_buttons is None:
        return True
    if isinstance(held_buttons, (tuple, list, set, frozenset)):
        return button in held_buttons
    # Hosts may expose the source pad mask instead of an iterable.  A
    # boolean mask is intentionally left to the host because Button values
    # are not required to be bit positions in the authoring API.
    return bool(held_buttons)


def _judge_phase(ctx: Any, retained_rows: Any = ()) -> int | None:
    """Resolve the native Judge roll when the host exposes its source data.

    ``ftGw_SpecialS_GetRandomInt`` excludes the previous two rows and rolls
    against the nine attribute weights.  The authoring API has no RNG or
    fighter-local Judge storage, so a host may provide the already-generated
    ``judge_roll`` and ``judge_previous`` values on the callback context.
    Invalid or absent optional data returns ``None`` so the caller can leave
    the input unconsumed instead of guessing a row.
    """
    lookup = getattr(ctx, "resource", None)
    resource = lookup("side.attributes") if callable(lookup) else None
    attrs = getattr(resource, "attributes", resource)
    weights = getattr(attrs, "judge_roll", None)
    roll = getattr(ctx, "judge_roll", None)
    previous = getattr(ctx, "judge_previous", ())
    if not isinstance(weights, (tuple, list)) or len(weights) != 9:
        return None
    if isinstance(roll, bool) or not isinstance(roll, int) or roll < 0:
        return None
    excluded = {
        value
        for values in (previous, retained_rows)
        for value in (values if isinstance(values, (tuple, list, set, frozenset)) else (values,))
        if isinstance(value, int) and not isinstance(value, bool) and 0 <= value < 9
    }
    if not all(isinstance(value, int) and not isinstance(value, bool) and value >= 0 for value in weights):
        return None
    total = sum(weight for index, weight in enumerate(weights) if index not in excluded)
    if total <= 0 or roll >= total:
        return None
    for index, weight in enumerate(weights):
        if index in excluded:
            continue
        if roll < weight:
            return index + 1
        roll -= weight
    return None


class _SourcePairSpecial(DirectionalSpecial):
    """Shared B entry and surface lifecycle for a two-state special."""

    ground: ClassVar[Any]
    air: ClassVar[Any]
    _ACTIVE: ClassVar[tuple[Any, Any]]

    def __init_subclass__(cls, **kwargs: Any) -> None:
        if hasattr(cls, "ground") and hasattr(cls, "air"):
            # A move with extra source phases (Oil Panic's catch/shoot rows)
            # supplies its complete active set explicitly.
            if "_ACTIVE" not in cls.__dict__:
                cls._ACTIVE = (cls.ground, cls.air)
            ground_rules = dict(cls.__dict__.get("on_ground", {}))
            air_rules = dict(cls.__dict__.get("on_air", {}))
            end_rules = dict(cls.__dict__.get("on_end", {}))
            ground_rules.setdefault(
                cls.air, Transition(cls.ground, preserve_state=True, keep_frame=True)
            )
            air_rules.setdefault(
                cls.ground, Transition(cls.air, preserve_state=True, keep_frame=True)
            )
            end_rules.setdefault(cls.ground, Transition(Action.WAIT))
            end_rules.setdefault(cls.air, Transition(Action.FALL))
            cls.on_ground = ground_rules
            cls.on_air = air_rules
            cls.on_end = end_rules
        super().__init_subclass__(**kwargs)

class Chef(NeutralSpecial, _SourcePairSpecial):
    """Chef's grounded and aerial states, including the held-B loop."""

    ground = source_phase(353)
    air = source_phase(354)

    @on.action_enter(ground, air)
    def enter(self, fighter: Any, ctx: Any) -> None:
        _fighter_state(fighter).chef_loop_disabled = False

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        """Restart Chef's source motion when its command frame allows a loop."""
        if fighter.action in self._ACTIVE:
            state = _fighter_state(fighter)
            held = _button_held(ctx, Button.B)
            if not held:
                state.chef_loop_disabled = True
            if (
                bool(getattr(ctx, "chef_loop_open", False))
                and isinstance(getattr(ctx, "chef_sausages", None), int)
                and isinstance(getattr(ctx, "chef_maximum", None), int)
                and ctx.chef_sausages < ctx.chef_maximum
                and not state.chef_loop_disabled
                and held
            ):
                start_action(fighter, fighter.action)
            return True
        if not (
            fresh_special_input(ctx, self.resource)
            and directional_match(ctx, self.root)
            and bool(ctx.ground_open or ctx.air_open)
        ):
            return False
        start_action(fighter, self.ground if ctx.ground_open else self.air)
        return True

    @on.release(Button.B)
    def release(self, fighter: Any, ctx: Any) -> bool:
        if fighter.action in self._ACTIVE:
            _fighter_state(fighter).chef_loop_disabled = True
            return True
        return False


class Judge(SideSpecial):
    """All eighteen Judge motion states, without inventing random selection."""

    ground_1 = source_phase(355)
    ground_2 = source_phase(356)
    ground_3 = source_phase(357)
    ground_4 = source_phase(358)
    ground_5 = source_phase(359)
    ground_6 = source_phase(360)
    ground_7 = source_phase(361)
    ground_8 = source_phase(362)
    ground_9 = source_phase(363)
    air_1 = source_phase(364)
    air_2 = source_phase(365)
    air_3 = source_phase(366)
    air_4 = source_phase(367)
    air_5 = source_phase(368)
    air_6 = source_phase(369)
    air_7 = source_phase(370)
    air_8 = source_phase(371)
    air_9 = source_phase(372)

    _SURFACE_PAIRS = tuple(
        zip(
            (ground_1, ground_2, ground_3, ground_4, ground_5,
             ground_6, ground_7, ground_8, ground_9),
            (air_1, air_2, air_3, air_4, air_5,
             air_6, air_7, air_8, air_9),
        )
    )
    _ACTIVE = tuple(phase for pair in _SURFACE_PAIRS for phase in pair)
    on_ground = {
        air: Transition(ground, preserve_state=True, keep_frame=True)
        for ground, air in _SURFACE_PAIRS
    }
    on_air = {
        ground: Transition(air, preserve_state=True, keep_frame=True)
        for ground, air in _SURFACE_PAIRS
    }
    on_end = {
        **{ground: Transition(Action.WAIT) for ground, _ in _SURFACE_PAIRS},
        **{air: Transition(Action.FALL) for _, air in _SURFACE_PAIRS},
    }

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        """Accept only a valid side-B edge; native code chooses the phase.

        The source-side weighted roll needs native Judge data. If it is absent,
        leave the input unconsumed rather than guessing a phase or article.
        """
        if fighter.action in self._ACTIVE:
            return True
        if not (
            fresh_special_input(ctx, self.resource)
            and directional_match(ctx, self.root)
            and bool(ctx.ground_open or ctx.air_open)
        ):
            return False

        state = _fighter_state(fighter)
        # The native fighter excludes both x222C_judgeVar1 (the last row)
        # and x2230_judgeVar2 (the row before it).  Keep host-provided
        # exclusions as well, then update the pair only after selecting the
        # new weighted row.
        phase_number = _judge_phase(ctx, (state.judge_current, state.judge_previous))
        if phase_number is None:
            return False
        state.judge_previous = state.judge_current
        state.judge_current = phase_number - 1
        state.judge_selected_value = phase_number
        phases = self._SURFACE_PAIRS[phase_number - 1]
        phase = phases[0 if ctx.ground_open else 1]
        start_action(fighter, phase)
        return True


class Fire(UpSpecial, _SourcePairSpecial):
    """Fire's grounded and aerial source entry states."""

    ground = source_phase(373)
    air = source_phase(374)

    def _transition_animation_end(self, fighter: Any, ctx: Any) -> None:
        """Match Fire Rescue's landing-attribute branch at animation end."""
        landing = None
        lookup = getattr(ctx, "resource", None)
        if callable(lookup):
            resource = lookup("up.attributes")
            attrs = getattr(resource, "attributes", resource)
            landing = getattr(attrs, "rescue_landing", None)
        if landing is not None:
            if landing == 0:
                fighter.change_action(Action.FALL)
            elif hasattr(fighter, "enter_fall_special"):
                fighter.enter_fall_special(mobility=1, landing_lag=landing)
            return
        super()._transition_animation_end(fighter, ctx)


class OilPanic(DownSpecial, _SourcePairSpecial):
    """Oil Panic's source states and full-bucket release branch."""

    ground = source_phase(375)
    ground_catch = source_phase(376)
    ground_shoot = source_phase(377)
    air = source_phase(378)
    air_catch = source_phase(379)
    air_shoot = source_phase(380)
    _ACTIVE = (ground, ground_catch, ground_shoot, air, air_catch, air_shoot)

    on_ground = {
        air: Transition(ground, preserve_state=True, keep_frame=True),
        air_catch: Transition(ground_catch, preserve_state=True, keep_frame=True),
        air_shoot: Transition(ground_shoot, preserve_state=True, keep_frame=True),
    }
    on_air = {
        ground: Transition(air, preserve_state=True, keep_frame=True),
        ground_catch: Transition(air_catch, preserve_state=True, keep_frame=True),
        ground_shoot: Transition(air_shoot, preserve_state=True, keep_frame=True),
    }
    on_end = {
        ground: Transition(Action.WAIT),
        ground_shoot: Transition(Action.WAIT),
        air: Transition(Action.FALL),
        air_shoot: Transition(Action.FALL),
    }

    @on.action_enter(ground, air)
    def enter(self, fighter: Any, ctx: Any) -> None:
        state = _fighter_state(fighter)
        state.panic_release_damage = 0.0

    @on.animation_end(ground_catch, air_catch)
    def catch_animation_end(self, fighter: Any, ctx: Any) -> None:
        """Finish the absorb animation like ``SpecialLwCatch_Anim``.

        The native callback returns to the absorb loop while the bucket is
        partial, and exits to wait/fall only once the bucket is full.  The
        generic end table cannot express that charge-dependent branch.
        """
        state = _fighter_state(fighter)
        charge = getattr(ctx, "panic_charge", state.panic_charge)
        if not isinstance(charge, int) or isinstance(charge, bool):
            charge = state.panic_charge
        else:
            state.panic_charge = charge
        if charge >= 3:
            fighter.change_action(Action.WAIT if fighter.action is self.ground_catch else Action.FALL)
            return
        target = self.ground if fighter.action is self.ground_catch else self.air
        fighter.change_action(target)
        # SpecialLwCatch_Anim uses Fighter_ChangeMotionState(..., 4, ...)
        # when returning to the absorb loop, rather than restarting frame 0.
        fighter.action_frame = 4

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        """Release a full bucket immediately, as SpecialLw_Enter does."""
        if fighter.action in self._ACTIVE:
            return True
        if not (
            fresh_special_input(ctx, self.resource)
            and directional_match(ctx, self.root)
            and bool(ctx.ground_open or ctx.air_open)
        ):
            return False
        state = _fighter_state(fighter)
        ground = bool(ctx.ground_open)
        charge = getattr(ctx, "panic_charge", state.panic_charge)
        if not isinstance(charge, int) or isinstance(charge, bool):
            charge = state.panic_charge
        else:
            state.panic_charge = charge
        if isinstance(charge, int) and not isinstance(charge, bool) and charge >= 3:
            accumulated = getattr(ctx, "panic_damage", state.panic_damage)
            if not isinstance(accumulated, (int, float)) or isinstance(accumulated, bool):
                accumulated = state.panic_damage
            attrs = _resource_attributes(ctx, "down.attributes")
            multiplier = getattr(attrs, "panic_damage_mul", None)
            addition = getattr(attrs, "panic_damage_add", None)
            if all(isinstance(value, (int, float)) and not isinstance(value, bool)
                   for value in (accumulated, multiplier, addition)):
                state.panic_release_damage = accumulated * multiplier + addition
                state.panic_charge = 0
                state.panic_damage = 0.0
            start_action(fighter, self.ground_shoot if ground else self.air_shoot)
        else:
            start_action(fighter, self.ground if ground else self.air)
        return True


class GameAndWatch(Fighter):
    action_state = GameAndWatchActionState
    specials = Fighter.specials.replace(
        neutral=Chef(), side=Judge(), up=Fire(), down=OilPanic()
    )


__all__ = ["GameAndWatch", "GameAndWatchActionState", "Chef", "Judge", "Fire", "OilPanic"]
