import importlib.util
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).parents[3]
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))

from fighter import Action, DonkeyKongAttribute, export_definition


def _module():
    spec = importlib.util.spec_from_file_location(
        "donkey_kong_test", ROOT / "scripts" / "fighters" / "donkey_kong.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    # This fixture deliberately uses a synthetic module name, so supply the
    # explicit identity that a canonical source filename normally provides.
    module.DonkeyKong.name = "donkey-kong"
    module.DonkeyKong.external_ids = (1,)
    return module


class _Input:
    stick = (0.0, 1.0)

    def just_pressed(self, button):
        return True


class _Fighter:
    action = None
    grounded = True
    action_frame = 0

    def __init__(self):
        self.changes = []
        self.max_jumps_calls = 0
        self.velocity = (0.0, 0.0)
        self.ground_velocity = 0.0
        self.facing = 1.0
        self.action_state = SimpleNamespace(command=(7, 6, 5, 4))

    def has_complete_animation(self, state):
        return state in (381, 382)

    def special_attribute(self, attribute):
        return {
            DonkeyKongAttribute.SPECIAL_HI_GROUNDED_HORIZONTAL_VELOCITY: 1.0,
            DonkeyKongAttribute.SPECIAL_HI_AERIAL_HORIZONTAL_VELOCITY: 0.75,
            DonkeyKongAttribute.SPECIAL_HI_GROUNDED_MOBILITY: 0.1,
            DonkeyKongAttribute.SPECIAL_HI_AERIAL_MOBILITY: 0.2,
            DonkeyKongAttribute.SPECIAL_HI_AERIAL_GRAVITY: 0.8,
            DonkeyKongAttribute.SPECIAL_HI_AERIAL_VERTICAL_VELOCITY: 2.0,
            DonkeyKongAttribute.SPECIAL_HI_LANDING_LAG: 20.0,
        }.get(attribute)

    def set_velocity(self, x, y):
        self.velocity = (x, y)

    def max_jumps(self):
        self.max_jumps_calls += 1

    def change_action(self, action, **kwargs):
        self.changes.append((action, kwargs))
        self.action = action

    def enter_fall_special(self, **kwargs):
        self.changes.append(("fall_special", kwargs))


class DonkeyKongTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.module = _module()
        cls.move = cls.module.SpinningKong()

    def test_source_phase_ids_are_custom_and_stable(self):
        definition = export_definition(self.module.DonkeyKong).as_dict()
        actions = definition["actions"]
        self.assertEqual(
            actions["special.up.ground"]["action"],
            "Action.Source.1:381",
        )
        self.assertEqual(
            actions["special.up.ground"]["slippi_state"],
            381,
        )
        self.assertEqual(actions["special.up.ground"]["animation_loop"], False)
        behavior_id = definition["movesets"]["specials"]["up"]
        behavior = next(item for item in definition["behaviors"] if item["id"] == behavior_id)
        self.assertEqual(behavior["resource"], "specials.special_attributes")
        self.assertIsNone(behavior["validate"])
        callbacks = [item["callback"].rsplit(".", 1)[-1] for item in behavior["callbacks"]]
        self.assertLess(callbacks.index("surface_change"), callbacks.index("_transition_ground_air"))

    def test_spinning_kong_declares_source_stick_steering(self):
        actions = export_definition(self.module.DonkeyKong).as_dict()["actions"]
        ground = actions["special.up.ground"]["motion"]["kwargs"]["ground"][0]
        air_operations = actions["special.up.air"]["motion"]["kwargs"]["air"]
        gravity, air = air_operations
        self.assertEqual(gravity["callee"], "motion.gravity_multiplier")
        self.assertEqual(gravity["kwargs"]["index"], 0)
        self.assertEqual(gravity["kwargs"]["value"], 0)
        self.assertEqual(gravity["kwargs"]["multiplier"]["args"][0], {"layout": 3, "field_id": 1})
        self.assertEqual(ground["callee"], "motion.stick_steering")
        self.assertEqual(air["callee"], "motion.stick_steering")
        self.assertEqual(ground["kwargs"]["threshold"], 0.0)
        self.assertEqual(air["kwargs"]["threshold"], 0.0)
        self.assertEqual(ground["kwargs"]["acceleration"]["args"][0], {"layout": 3, "field_id": 4})
        self.assertEqual(ground["kwargs"]["target"]["args"][0], {"layout": 3, "field_id": 2})
        self.assertEqual(air["kwargs"]["acceleration"]["args"][0], {"layout": 3, "field_id": 5})
        self.assertEqual(air["kwargs"]["target"]["args"][0], {"layout": 3, "field_id": 3})

    def test_up_b_enters_ground_or_air_phase_only_with_upward_direction(self):
        fighter = _Fighter()
        context = SimpleNamespace(
            input=_Input(),
            ground_open=True,
            air_open=False,
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5)),
        )
        self.assertTrue(self.move.input_pressed(fighter, context))
        self.assertEqual(fighter.changes[0][0], self.move.ground)

        context.input.stick = (0.0, -1.0)
        fighter = _Fighter()
        self.assertFalse(self.move.input_pressed(fighter, context))

    def test_ground_phase_ends_to_wait(self):
        fighter = _Fighter()
        fighter.action = self.move.ground
        self.move._transition_animation_end(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.changes[0][0], Action.WAIT)

    def test_fresh_air_entry_uses_grounded_horizontal_limit(self):
        fighter = _Fighter()
        fighter.velocity = (1.5, -0.25)
        context = SimpleNamespace(
            input=_Input(),
            ground_open=False,
            air_open=True,
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5)),
        )
        self.assertTrue(self.move.input_pressed(fighter, context))
        self.assertEqual(fighter.velocity, (1.0, 2.0))

    def test_fresh_ground_entry_clears_vertical_velocity(self):
        fighter = _Fighter()
        fighter.ground_velocity = 1.5
        context = SimpleNamespace(
            input=_Input(),
            ground_open=True,
            air_open=False,
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5)),
        )
        self.assertTrue(self.move.input_pressed(fighter, context))
        self.assertEqual(fighter.ground_velocity, 1.0)
        self.assertEqual(fighter.velocity, (1.0, 0.0))

    def test_repeated_b_does_not_reapply_entry_velocity(self):
        fighter = _Fighter()
        context = SimpleNamespace(
            input=_Input(),
            ground_open=True,
            air_open=False,
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5)),
        )
        self.assertTrue(self.move.input_pressed(fighter, context))
        fighter.ground_velocity = 0.25
        fighter.velocity = (0.25, 3.0)
        self.assertTrue(self.move.input_pressed(fighter, context))
        self.assertEqual(fighter.velocity, (0.25, 3.0))
        self.assertEqual(fighter.max_jumps_calls, 1)

    def test_fresh_entry_clears_all_native_command_slots(self):
        fighter = _Fighter()
        context = SimpleNamespace(
            input=_Input(),
            ground_open=True,
            air_open=False,
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5)),
        )
        self.assertTrue(self.move.input_pressed(fighter, context))
        self.assertEqual(fighter.action_state.command, (0, 0, 0, 0))

    def test_entry_requires_the_complete_special_attribute_block(self):
        fighter = _Fighter()
        original = fighter.special_attribute
        fighter.special_attribute = lambda attribute: (
            None
            if attribute == DonkeyKongAttribute.SPECIAL_HI_AERIAL_GRAVITY
            else original(attribute)
        )
        context = SimpleNamespace(
            input=_Input(),
            ground_open=False,
            air_open=True,
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5)),
        )
        self.assertFalse(self.move.input_pressed(fighter, context))

    def test_entry_requires_declared_special_resource_when_host_resolves_resources(self):
        fighter = _Fighter()
        context = SimpleNamespace(
            input=_Input(),
            ground_open=True,
            air_open=False,
            resource=lambda path: None,
            rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5)),
        )
        self.assertFalse(self.move.input_pressed(fighter, context))

    def test_fresh_ground_and_air_entries_reset_jumps(self):
        for ground_open, air_open in ((True, False), (False, True)):
            fighter = _Fighter()
            context = SimpleNamespace(
                input=_Input(),
                ground_open=ground_open,
                air_open=air_open,
                rules=SimpleNamespace(specials=SimpleNamespace(vertical_threshold=0.5)),
            )
            self.assertTrue(self.move.input_pressed(fighter, context))
            self.assertEqual(fighter.max_jumps_calls, 1)

    def test_surface_change_clamps_ground_to_air_once(self):
        fighter = _Fighter()
        fighter.action = self.move.ground
        fighter.velocity = (2.5, -0.5)
        self.move.surface_change(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.velocity, (0.75, -0.5))
        self.assertEqual(fighter.max_jumps_calls, 0)

    def test_surface_change_clamps_air_to_ground_once(self):
        fighter = _Fighter()
        fighter.action = self.move.air
        fighter.ground_velocity = -2.5
        self.move.surface_change(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.ground_velocity, -1.0)
        self.assertEqual(fighter.max_jumps_calls, 0)

    def test_air_animation_end_uses_landing_lag_for_fall_special(self):
        fighter = _Fighter()
        fighter.action = self.move.air
        self.move.end_air(fighter, SimpleNamespace())
        self.assertEqual(fighter.changes[0][0], "fall_special")
        self.assertEqual(fighter.max_jumps_calls, 1)

    def test_missing_landing_lag_fails_closed(self):
        fighter = _Fighter()
        fighter.action = self.move.air
        fighter.special_attribute = lambda attribute: None
        self.move.end_air(fighter, SimpleNamespace())
        self.assertEqual(fighter.changes, [])


if __name__ == "__main__":
    unittest.main()
