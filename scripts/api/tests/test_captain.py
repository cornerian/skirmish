"""Focused contract tests for the partial Captain Falcon authoring module."""

import importlib.util
import sys
import types
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))

from fighter import Action, Button


def _load_captain():
    # Shared modules are normally supplied by AssetStore; provide the same
    # namespace for this CPython-only API contract test.
    if "fighters.captain" in sys.modules:
        return sys.modules["fighters.captain"].CaptainFalcon
    shared = types.ModuleType("shared")
    spec = importlib.util.spec_from_file_location(
        "shared.common", ROOT / "scripts" / "fighters" / "common.py"
    )
    common = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules["shared"] = shared
    sys.modules["shared.common"] = common
    spec.loader.exec_module(common)
    shared.common = common
    from fighters.captain import CaptainFalcon

    return CaptainFalcon


class CaptainFalconTests(unittest.TestCase):
    class Input:
        def __init__(self, pressed=(), stick=(0.0, 0.0)):
            self.pressed = set(pressed)
            self.stick = stick

        def just_pressed(self, button):
            return button in self.pressed

    class Fighter:
        def __init__(self, action, *, grounded=False):
            captain = _load_captain()
            self.action = action
            self.action_state = captain.__dict__["action_state"]()
            self.action_frame = 0
            self.grounded = grounded
            self.ground_velocity = 3.0
            self.velocity = [3.0, 4.0]
            self.changes = []

        def change_action(self, action, **kwargs):
            self.changes.append((action, kwargs))
            self.action = action

    @staticmethod
    def context(*, resource_value=object(), input_value=None, ground_open=False,
                air_open=False, event_value=None, grounded=False):
        resource = resource_value
        return SimpleNamespace(
            input=input_value or CaptainFalconTests.Input(),
            ground_open=ground_open,
            air_open=air_open,
            grounded=grounded,
            event=SimpleNamespace(value=event_value),
            resource=lambda _path: resource,
        )

    def test_b_entry_requires_resource_and_starts_ground_or_air_action(self):
        captain = _load_captain()
        move = captain.specials.neutral
        ground = self.Fighter(None)
        context = self.context(input_value=self.Input((Button.B,)), ground_open=True)
        self.assertTrue(move.input_pressed(ground, context))
        self.assertEqual(ground.action, move.ground)
        self.assertEqual(ground.action_frame, 1)

        air = self.Fighter(None)
        context = self.context(input_value=self.Input((Button.B,)), air_open=True)
        self.assertTrue(move.input_pressed(air, context))
        self.assertEqual(air.action, move.air)
        self.assertEqual(air.action_frame, 1)

        blocked = self.Fighter(None)
        context = self.context(input_value=self.Input((Button.B,)))
        self.assertFalse(move.input_pressed(blocked, context))
        missing = self.Fighter(None)
        context = self.context(resource_value=None, input_value=self.Input((Button.B,)), ground_open=True)
        self.assertFalse(move.input_pressed(missing, context))

    def test_terminal_animation_exits_ground_to_wait_and_air_to_fall(self):
        captain = _load_captain()
        move = captain.specials.neutral
        ground = self.Fighter(move.ground)
        move.animation_end(ground, self.context())
        self.assertEqual(ground.changes, [(Action.WAIT, {})])
        air = self.Fighter(move.air)
        move.animation_end(air, self.context())
        self.assertEqual(air.changes, [(Action.FALL, {})])

    def test_surface_transitions_preserve_state_and_frame(self):
        captain = _load_captain()
        move = captain.specials.neutral
        air = self.Fighter(move.air)
        move._transition_ground_air(air, SimpleNamespace(grounded=True))
        self.assertEqual(air.changes, [(move.ground, {"preserve_state": True, "keep_frame": True})])
        ground = self.Fighter(move.ground)
        move._transition_ground_air(ground, SimpleNamespace(grounded=False))
        self.assertEqual(ground.changes, [(move.air, {"preserve_state": True, "keep_frame": True})])

    def test_command_cue_is_air_only_and_ignores_zero(self):
        captain = _load_captain()
        move = captain.specials.neutral
        air = self.Fighter(move.air)
        move.command_changed(air, self.context(event_value=1))
        self.assertTrue(air.action_state.launch_armed)
        ground = self.Fighter(move.ground)
        move.command_changed(ground, self.context(event_value=1))
        self.assertFalse(ground.action_state.launch_armed)
        move.command_changed(air, self.context(event_value=0))
        self.assertTrue(air.action_state.launch_armed)

    def test_enter_clears_pending_command_and_validation_requires_both_traces(self):
        captain = _load_captain()
        move = captain.specials.neutral
        fighter = self.Fighter(move.ground)
        fighter.action_state.launch_armed = True
        move.enter(fighter, self.context())
        self.assertFalse(fighter.action_state.launch_armed)

        class ValidContext:
            def resource(self, path):
                return SimpleNamespace(cmd_vars=[[0, 0, 0, 0]], allow_interrupt=[False])

            def frames(self, path):
                return 1

            def array_length(self, path, index=None):
                return 4 if index is not None and path.endswith("cmd_vars") else 1

        self.assertTrue(move.validate(ValidContext()))
        self.assertTrue(move.validate(self.context(resource_value=None)))

        class InvalidContext:
            def resource(self, path):
                return object()

        self.assertFalse(move.validate(InvalidContext()))

    def test_exports_identity_and_resource_backed_punch(self):
        from fighter.api import export_definition

        captain = _load_captain()
        exported = export_definition(captain).as_dict()
        self.assertEqual(exported["name"], "captain-falcon")
        self.assertEqual(exported["external_ids"], [0])
        neutral = exported["movesets"]["specials"]["neutral"]
        self.assertEqual(neutral, "move_0")
        behavior = next(item for item in exported["behaviors"] if item["id"] == neutral)
        self.assertEqual(behavior["resource"], "neutral")
        self.assertEqual(exported["actions"]["special.neutral.ground"]["slippi_state"], 347)
        self.assertEqual(exported["actions"]["special.neutral.air"]["slippi_state"], 348)

    def test_inherits_all_standard_groups_without_duplicate_declarations(self):
        captain = _load_captain()
        fighter_base = captain.__mro__[1]
        self.assertEqual(fighter_base.__name__, "FighterBase")
        for group in ("aerials", "grounded", "tilts", "smashes", "grabs", "throws", "defense", "ledge", "getup", "taunt"):
            self.assertIs(getattr(captain, group), getattr(fighter_base, group))


if __name__ == "__main__":
    unittest.main()
