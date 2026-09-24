"""Regression coverage for Yoshi's source-defined Egg Roll hold phases."""

from scripts.fighters.yoshi import EggRoll


def test_egg_roll_native_hold_phases_are_animation_loops():
    """The decomp freezes Roll loop motion until release/collision callbacks."""
    assert EggRoll.ground_loop.animation_loop is True
    assert EggRoll.ground_turn.animation_loop is True
    assert EggRoll.air_loop.animation_loop is True
    assert EggRoll.air_turn.animation_loop is True


def test_egg_roll_release_still_selects_terminal_phase():
    """Loop metadata must not change the explicit B release handoff."""
    assert EggRoll.ground_loop in EggRoll._ACTIVE
    assert EggRoll.ground_turn in EggRoll._ACTIVE
    assert EggRoll.air_loop in EggRoll._ACTIVE
    assert EggRoll.air_turn in EggRoll._ACTIVE
    assert EggRoll.on_end[EggRoll.ground_loop].target == EggRoll.ground_turn
    assert EggRoll.on_end[EggRoll.air_loop].target == EggRoll.air_turn
