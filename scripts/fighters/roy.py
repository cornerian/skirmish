"""Roy's source-state special declarations."""

import math

from skirmish import Button, Fighter, source_phase
from fighter.emblem_family import (
    EmblemDownSpecial,
    EmblemNeutralSpecial,
    EmblemSideSpecial,
    EmblemUpSpecial,
)
from fighter.events import on
from fighter.helpers import resource_attributes


class FlareBlade(EmblemNeutralSpecial):
    """Roy's Flare Blade phases from ``ftMars_Init_MotionStateTable``."""

    ground_start = source_phase(341)
    ground_loop = source_phase(342, animation_loop=True)
    ground_end0 = source_phase(343)
    ground_end1 = source_phase(344)
    air_start = source_phase(345)
    air_loop = source_phase(346, animation_loop=True)
    air_end0 = source_phase(347)
    air_end1 = source_phase(348)



class DoubleEdgeDance(EmblemSideSpecial):
    """Roy's four stage Dancing Blade phase tree (ground and air)."""

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

    @staticmethod
    def _ab_pressed(ctx) -> bool:
        input_state = getattr(ctx, "input", None)
        query = getattr(input_state, "just_pressed", None)
        if not callable(query):
            return False
        return bool(query(Button.A) and query(Button.B))

    @on.input_pressed(Button.A, Button.B, require_all_buttons=True)
    def input_pressed(self, fighter, ctx) -> bool:
        if fighter.action in self._active_actions():
            return self.choose_phase(fighter, ctx) if self._ab_pressed(ctx) else False
        return super().input_pressed(fighter, ctx)

    def choose_phase(self, fighter, ctx) -> bool:
        if fighter.action not in self._active_actions() or not self._ab_pressed(ctx):
            return False
        state = getattr(fighter, "action_state", None)
        command = getattr(state, "command", (0, 0, 0, 0))
        if not isinstance(command, (tuple, list)) or len(command) != 4:
            return False
        if not command[0]:
            state.command = (command[0], 1, command[2], command[3])
            return True
        if command[1]:
            return False
        stage = {
            self.ground_start: "ground_2_", self.air_start: "air_2_",
            self.ground_2_up: "ground_3_", self.ground_2_down: "ground_3_",
            self.air_2_up: "air_3_", self.air_2_down: "air_3_",
            self.ground_3_up: "ground_4_", self.ground_3_neutral: "ground_4_",
            self.ground_3_down: "ground_4_", self.air_3_up: "air_4_",
            self.air_3_neutral: "air_4_", self.air_3_down: "air_4_",
        }.get(fighter.action)
        if stage is None:
            return False
        rules = getattr(getattr(ctx, "rules", None), "specials", None)
        threshold = getattr(rules, "vertical_threshold", 0.5)
        y = getattr(getattr(ctx, "input", None), "stick", (0.0, 0.0))[1]
        if fighter.action in (self.ground_start, self.air_start):
            choice = "up" if y > threshold else "down"
        elif y > threshold:
            choice = "up"
        elif y < -threshold:
            choice = "down"
        else:
            choice = "neutral"
        target = getattr(self, stage + choice)
        state.command = (0, 0, command[2], command[3])
        fighter.change_action(target)
        return True



class Blazer(EmblemUpSpecial):
    """Roy's ground and aerial Blazer states."""

    ground = source_phase(367)
    air = source_phase(368)

    @on.stick(actions=(ground, air))
    def steer(self, fighter, ctx) -> bool:
        """Mirror the shared Mars Dolphin Slash horizontal steering callback."""
        attributes = resource_attributes(ctx, self.resource)
        command = getattr(getattr(fighter, "action_state", None), "command", (0, 0, 0, 0))
        if attributes is None or command[0]:
            return False
        threshold = getattr(attributes, "specialhi_facing_threshold", getattr(attributes, "x34", None))
        maximum = getattr(attributes, "specialhi_angle_limit", getattr(attributes, "x38", None))
        stick = getattr(getattr(ctx, "input", None), "stick", (0.0, 0.0))
        try:
            horizontal = float(stick[0])
        except (IndexError, KeyError, TypeError, ValueError):
            return False
        if threshold is None or maximum is None or abs(horizontal) <= threshold:
            return False
        denominator = 1.0 - threshold
        if denominator <= 0.0:
            return False
        angle = float(maximum) * (abs(horizontal) - threshold) / denominator
        angle = math.radians(angle if horizontal < 0.0 else -angle)
        previous = float(getattr(fighter, "lstick_angle", 0.0))
        if abs(angle) <= abs(previous):
            return False
        fighter.lstick_angle = angle
        return True

    @on.animation_end(ground, air)
    def enter_fall_special(self, fighter, ctx) -> bool:
        attributes = resource_attributes(ctx, self.resource)
        mobility = getattr(attributes, "specialhi_freefall_air_spd_mul", None)
        landing_lag = getattr(attributes, "specialhi_landing_lag", None)
        if mobility is None or landing_lag is None:
            return False
        enter = getattr(fighter, "enter_fall_special", None)
        if not callable(enter):
            return False
        enter(mobility=mobility, landing_lag=landing_lag)
        return True


class Counter(EmblemDownSpecial):
    """Roy's Counter start and hit states."""

    ground = source_phase(369)
    ground_hit = source_phase(370)
    air = source_phase(371)
    air_hit = source_phase(372)

    @on.command_changed(1, actions=(ground, air))
    def command_changed(self, fighter, ctx) -> bool:
        """Arm the shared Counter hit phase when its native cue fires."""
        event = getattr(ctx, "event", None)
        if getattr(event, "value", 0) != 1:
            return False
        descriptor = getattr(ctx, "shield_descriptor", None)
        if descriptor is None:
            descriptor = getattr(ctx, "counter_shield", None)
        register = getattr(fighter, "register_counter_shield", None)
        if descriptor is None or not callable(register):
            return False
        register(descriptor)
        # Native contact, not this command cue, enters ground_hit/air_hit.
        return True


class Roy(Fighter):
    specials = Fighter.specials.replace(
        neutral=FlareBlade(),
        side=DoubleEdgeDance(),
        up=Blazer(),
        down=Counter(),
    )


__all__ = ["Roy", "FlareBlade", "DoubleEdgeDance", "Blazer", "Counter"]
