"""Small source-backed behavior shared by Marth and Roy.

The native ``ftEmblem`` entry uses the Mars callbacks for both fighters.  The
two scripts therefore share the finite surface transitions and the entry
input gate, while retaining their own phase declarations and resource data.
 The finite A+B phase chooser and Counter command activation are modeled here
 as source callbacks; the fighter modules only declare phase identities.
"""

from __future__ import annotations

from typing import Any, ClassVar

from .actions import Action
from .events import on
from .helpers import fresh_special_input, start_open_special
from .standard import DownSpecial, NeutralSpecial, SideSpecial, UpSpecial, directional_match
from .transitions import SpecialMove, Transition
from .compat import Button, source_phase


class _EmblemFamilySpecial(SpecialMove):
    """Common B entry and source-state surface plumbing."""

    _entry_names: ClassVar[tuple[str, str]] = ("ground", "air")
    _surface_names: ClassVar[tuple[tuple[str, str], ...]] = ()

    def __init_subclass__(cls, **kwargs: Any) -> None:
        cls._configure_source_rules()
        super().__init_subclass__(**kwargs)

    @classmethod
    def _configure_source_rules(cls) -> None:
        ground, air = cls._entry_names
        if not all(hasattr(cls, name) for name in (ground, air)):
            return

        # Every ftMars/ftEmblem special entry has the same ground/air
        # availability gate.  Subclasses may add their own phase-specific
        # terminal rules after these source declarations.
        cls.on_ground = {getattr(cls, a): Transition(getattr(cls, g), preserve_state=True, keep_frame=True)
                         for g, a in cls._surface_names}
        cls.on_air = {getattr(cls, g): Transition(getattr(cls, a), preserve_state=True, keep_frame=True)
                      for g, a in cls._surface_names}

    @classmethod
    def _entry_actions(cls) -> tuple[Any, Any]:
        return tuple(getattr(cls, name) for name in cls._entry_names)  # type: ignore[return-value]

    @classmethod
    def _active_actions(cls) -> tuple[Any, ...]:
        names = getattr(cls, "_active_names", cls._entry_names)
        return tuple(getattr(cls, name) for name in names)

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        """Enter the declared source entry state on a fresh, valid B input."""
        if fighter.action in type(self)._active_actions():
            return True
        if not fresh_special_input(ctx, self.resource):
            return False
        if not directional_match(ctx, self.root):
            return False
        ground, air = type(self)._entry_actions()
        return start_open_special(fighter, ctx, ground, air)


class EmblemNeutralSpecial(NeutralSpecial, _EmblemFamilySpecial):
    """Shield Breaker entry, charge surface changes, and release phases."""

    # ftMars uses this same source table for Marth and Roy.  Keep the numeric
    # states in the family API; fighter modules only choose the move's native
    # name (ShieldBreaker vs. FlareBlade).
    ground_start = source_phase(341)
    ground_loop = source_phase(342)
    ground_end0 = source_phase(343)
    ground_end1 = source_phase(344)
    air_start = source_phase(345)
    air_loop = source_phase(346)
    air_end0 = source_phase(347)
    air_end1 = source_phase(348)

    _entry_names = ("ground_start", "air_start")
    _active_names = ("ground_start", "ground_loop", "air_start", "air_loop")
    _surface_names = (
        ("ground_start", "air_start"),
        ("ground_loop", "air_loop"),
        ("ground_end0", "air_end0"),
        ("ground_end1", "air_end1"),
    )

    def __init_subclass__(cls, **kwargs: Any) -> None:
        required = (*cls._entry_names, "ground_loop", "air_loop",
                    "ground_end0", "ground_end1", "air_end0", "air_end1")
        if all(hasattr(cls, name) for name in required):
            cls.on_end = {
                cls.ground_start: Transition(cls.ground_loop),
                cls.air_start: Transition(cls.air_loop),
                cls.ground_end0: Transition(Action.WAIT),
                cls.ground_end1: Transition(Action.WAIT),
                cls.air_end0: Transition(Action.FALL),
                cls.air_end1: Transition(Action.FALL),
            }
        super().__init_subclass__(**kwargs)

    @on.input_released(Button.B)
    def release(self, fighter: Any, ctx: Any) -> bool:
        """Release exits the charge loop through the uncharged end state.

        The native timer can also select the fully charged end state.  That
        timer depends on fighter attributes and remains a concrete resource
        concern; this callback covers only the common observable release path.
        """
        if fighter.action == self.ground_loop:
            fighter.change_action(self.ground_end0)
            return True
        if fighter.action == self.air_loop:
            fighter.change_action(self.air_end0)
            return True
        return False

    @on.command_changed(0, actions=(ground_loop, air_loop))
    def fully_charged(self, fighter: Any, ctx: Any) -> bool:
        """Enter the native fully charged end phase from either loop.

        ``ftMs_SpecialN`` sets command variable zero when the charge timer
        expires, then selects End1.  Button release selects End0 instead.
        """
        if getattr(getattr(ctx, "event", None), "value", 0) != 1:
            return False
        if fighter.action == self.ground_loop:
            fighter.change_action(self.ground_end1)
            return True
        if fighter.action == self.air_loop:
            fighter.change_action(self.air_end1)
            return True
        return False


class EmblemSideSpecial(SideSpecial, _EmblemFamilySpecial):
    """Dancing Blade entry, A+B phase selection, and surface pairing."""

    ground_start = source_phase(349)
    ground_2_up = source_phase(350)
    ground_2_down = source_phase(351)
    ground_3_up = source_phase(352)
    ground_3_neutral = source_phase(353)
    ground_3_down = source_phase(354)
    ground_4_up = source_phase(355)
    ground_4_neutral = source_phase(356)
    ground_4_down = source_phase(357)
    air_start = source_phase(358)
    air_2_up = source_phase(359)
    air_2_down = source_phase(360)
    air_3_up = source_phase(361)
    air_3_neutral = source_phase(362)
    air_3_down = source_phase(363)
    air_4_up = source_phase(364)
    air_4_neutral = source_phase(365)
    air_4_down = source_phase(366)

    _entry_names = ("ground_start", "air_start")
    _active_names = (
        "ground_start", "ground_2_up", "ground_2_down", "ground_3_up",
        "ground_3_neutral", "ground_3_down", "ground_4_up",
        "ground_4_neutral", "ground_4_down", "air_start", "air_2_up",
        "air_2_down", "air_3_up", "air_3_neutral", "air_3_down",
        "air_4_up", "air_4_neutral", "air_4_down",
    )
    _surface_names = (
        ("ground_start", "air_start"),
        ("ground_2_up", "air_2_up"),
        ("ground_2_down", "air_2_down"),
        ("ground_3_up", "air_3_up"),
        ("ground_3_neutral", "air_3_neutral"),
        ("ground_3_down", "air_3_down"),
        ("ground_4_up", "air_4_up"),
        ("ground_4_neutral", "air_4_neutral"),
        ("ground_4_down", "air_4_down"),
    )

    @staticmethod
    def _ab_pressed(ctx: Any) -> bool:
        input_state = getattr(ctx, "input", None)
        query = getattr(input_state, "just_pressed", None)
        if callable(query):
            return bool(query(Button.A) or query(Button.B))
        buttons = getattr(input_state, "pressed_buttons", 0)
        return bool(buttons & 0x300)

    def _phase_choice(self, ctx: Any) -> str:
        rules = getattr(getattr(ctx, "rules", None), "specials", None)
        threshold = getattr(rules, "vertical_threshold", 0.5)
        y = getattr(getattr(ctx, "input", None), "stick", (0.0, 0.0))[1]
        if y >= threshold:
            return "up"
        if y <= -threshold:
            return "down"
        return "neutral"

    @on.input_pressed(Button.A, Button.B)
    def input_pressed(self, fighter: Any, ctx: Any) -> bool:
        # The shared B hook also receives A+B button masks.  Keep phase
        # selection on that single serialized input callback so roster
        # definitions do not register duplicate input routes.
        if fighter.action in self._active_actions() and self._ab_pressed(ctx):
            return self.choose_phase(fighter, ctx)
        return super().input_pressed(fighter, ctx)

    def choose_phase(self, fighter: Any, ctx: Any) -> bool:
        """Mirror ftMars's command-gated A+B phase tree."""
        if fighter.action not in self._active_actions() or not self._ab_pressed(ctx):
            return False
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", (0, 0, 0, 0))
        if len(command) != 4:
            return False
        # Native IASA first arms cmd_vars[1], then consumes a later A+B once
        # the animation command (cmd_vars[0]) has arrived.
        if not command[0]:
            state.command = (1, command[1], command[2], command[3])
            return True
        if command[1]:
            return False
        stage = {
            self.ground_start: ("ground_2_", None), self.air_start: ("air_2_", None),
            self.ground_2_up: ("ground_3_", None), self.ground_2_down: ("ground_3_", None),
            self.air_2_up: ("air_3_", None), self.air_2_down: ("air_3_", None),
            self.ground_3_up: ("ground_4_", None), self.ground_3_neutral: ("ground_4_", None),
            self.ground_3_down: ("ground_4_", None), self.air_3_up: ("air_4_", None),
            self.air_3_neutral: ("air_4_", None), self.air_3_down: ("air_4_", None),
        }.get(fighter.action)
        if stage is None:
            return False
        prefix, forced = stage
        if forced is None and fighter.action in (self.ground_start, self.air_start):
            rules = getattr(getattr(ctx, "rules", None), "specials", None)
            threshold = getattr(rules, "vertical_threshold", 0.5)
            y = getattr(getattr(ctx, "input", None), "stick", (0.0, 0.0))[1]
            choice = "up" if y >= threshold else "down"
        else:
            choice = forced or self._phase_choice(ctx)
        target = getattr(self, prefix + choice)
        state.command = (0, 0, command[2], command[3])
        fighter.change_action(target)
        return True


class EmblemUpSpecial(UpSpecial, _EmblemFamilySpecial):
    """Dolphin Slash entry; landing/fall-special attributes stay native."""

    ground = source_phase(367)
    air = source_phase(368)

    _entry_names = ("ground", "air")
    _surface_names = (("ground", "air"),)


class EmblemDownSpecial(DownSpecial, _EmblemFamilySpecial):
    """Counter entry, hit phases, and source-preserving surface changes."""

    ground = source_phase(369)
    ground_hit = source_phase(370)
    air = source_phase(371)
    air_hit = source_phase(372)

    _entry_names = ("ground", "air")
    _surface_names = (
        ("ground", "air"),
        ("ground_hit", "air_hit"),
    )

    def __init_subclass__(cls, **kwargs: Any) -> None:
        required = (*cls._entry_names, "ground_hit", "air_hit")
        if all(hasattr(cls, name) for name in required):
            cls.on_end = {
                cls.ground: Transition(Action.WAIT),
                cls.ground_hit: Transition(Action.WAIT),
                cls.air: Transition(Action.FALL),
                cls.air_hit: Transition(Action.FALL),
            }
        super().__init_subclass__(**kwargs)

    @on.command_changed(1, actions=(ground, air))
    def command_changed(self, fighter: Any, ctx: Any) -> bool:
        """Enter ftMars's hit phase when command variable 1 is armed."""
        value = getattr(getattr(ctx, "event", None), "value", 0)
        if value != 1:
            return False
        if fighter.action == self.ground:
            fighter.change_action(self.ground_hit)
            return True
        if fighter.action == self.air:
            fighter.change_action(self.air_hit)
            return True
        return False

__all__ = [
    "EmblemNeutralSpecial",
    "EmblemSideSpecial",
    "EmblemUpSpecial",
    "EmblemDownSpecial",
]
