"""Contract tests for shared fighter authoring helpers."""

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


def _load_common():
    """Load the shared module using the standalone API test arrangement."""
    if "shared.common" in sys.modules:
        return sys.modules["shared.common"]
    shared = sys.modules.get("shared") or types.ModuleType("shared")
    spec = importlib.util.spec_from_file_location(
        "shared.common", ROOT / "scripts" / "fighters" / "common.py"
    )
    common = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules["shared"] = shared
    sys.modules["shared.common"] = common
    spec.loader.exec_module(common)
    shared.common = common
    return common


class FighterCommonTests(unittest.TestCase):
    def test_special_rules_returns_optional_dispatch_table(self):
        common = _load_common()
        specials = object()
        self.assertIs(
            common.special_rules(SimpleNamespace(rules=SimpleNamespace(specials=specials))),
            specials,
        )
        self.assertIsNone(common.special_rules(SimpleNamespace()))
        self.assertIsNone(common.special_rules(SimpleNamespace(rules=SimpleNamespace())))
        self.assertIsNone(common.special_rules(SimpleNamespace(rules=None)))

    def test_resource_attributes_support_resource_objects_and_named_paths(self):
        common = _load_common()
        attributes = object()
        resource = SimpleNamespace(attributes=attributes)
        looked_up = []

        def lookup(path=None):
            looked_up.append(path)
            if path == "special.attributes":
                return attributes
            return resource

        context = SimpleNamespace(resource=lookup)
        self.assertIs(common.resource_attributes(context, resource), attributes)
        self.assertIs(common.resource_attributes(context, "special"), attributes)
        self.assertEqual(looked_up, ["special"])

    def test_resource_attributes_uses_owned_resource_when_omitted(self):
        common = _load_common()
        attributes = object()
        owned = SimpleNamespace(attributes=attributes)
        calls = []

        def lookup(path=None):
            calls.append(path)
            return owned

        self.assertIs(common.resource_attributes(SimpleNamespace(resource=lookup)), attributes)
        self.assertEqual(calls, [None])

    def test_resource_attributes_returns_none_for_missing_or_malformed_optional_data(self):
        common = _load_common()
        self.assertIsNone(common.resource_attributes(SimpleNamespace()))
        self.assertIsNone(common.resource_attributes(SimpleNamespace(resource=lambda: None)))
        self.assertIsNone(
            common.resource_attributes(SimpleNamespace(resource=lambda path: object()), "special")
        )
        self.assertIsNone(
            common.resource_attributes(SimpleNamespace(resource=lambda path: None), "special.attributes")
        )

    def test_start_action_changes_action_and_resets_animation_frame(self):
        common = _load_common()

        class Fighter:
            action_frame = 27

            def __init__(self):
                self.changes = []

            def change_action(self, action):
                self.changes.append(action)

        fighter = Fighter()
        action = object()
        self.assertIsNone(common.start_action(fighter, action))
        self.assertEqual(fighter.changes, [action])
        self.assertEqual(fighter.action_frame, 1)

    def test_fresh_special_helpers_gate_input_and_select_open_surface(self):
        common = _load_common()

        class Input:
            def just_pressed(self, button):
                return button.name == "B"

        class Fighter:
            action_frame = 0

            def __init__(self):
                self.actions = []

            def change_action(self, action):
                self.actions.append(action)

        context = SimpleNamespace(
            input=Input(),
            ground_open=False,
            air_open=True,
            resource=lambda path: object(),
        )
        self.assertTrue(common.fresh_special_input(context, "neutral"))
        fighter = Fighter()
        air = object()
        ground = object()
        self.assertTrue(common.start_open_special(fighter, context, ground, air))
        self.assertEqual(fighter.actions, [air])
        self.assertEqual(fighter.action_frame, 1)

        context.ground_open = False
        context.air_open = False
        self.assertFalse(common.start_open_special(fighter, context, ground, air))


if __name__ == "__main__":
    unittest.main()
