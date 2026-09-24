"""Focused contracts for the Python/native fighter object bridge."""

import unittest

import skirmish._native as native
from fighter import Action, SourceAction, action, source_action


class NativeBridgeTests(unittest.TestCase):
    def setUp(self):
        self.previous = (native._get, native._set, native._call, native._call_named)

    def tearDown(self):
        native._get, native._set, native._call, native._call_named = self.previous

    def test_native_methods_use_scoped_path_and_unwrap_arguments(self):
        calls = []

        native._get = lambda token, path: None
        native._set = lambda token, path, value: calls.append(("set", token, path, value))
        native._call = lambda token, path, args: calls.append(("call", token, path, args)) or {"ok": True}
        native._call_named = (
            lambda token, path, args, kwargs:
            calls.append(("named", token, path, args, kwargs)) or None
        )

        fighter = native.NativeObject(17, "fighter", "fighter")
        self.assertEqual(fighter.enter_landing_special(1.5), {"ok": True})
        fighter.spawn_article(
            source_action(381), action(Action.SPECIAL_N_START), angle=0.25
        )
        self.assertEqual(calls, [
            ("call", 17, "fighter.enter_landing_special", [1.5]),
            ("named", 17, "fighter.spawn_article", ["Source.381", "special_n_start"], {"angle": 0.25}),
        ])

    def test_wrap_preserves_nested_native_identity_and_binary32_scalars(self):
        value = native.wrap({
            "fighter": {"__skirmish_native__": True, "token": 9, "kind": "fighter", "path": "fighter"},
            "nested": [{"value": 1.0}],
        }, token=3)
        self.assertIsInstance(value["fighter"], native.NativeObject)
        self.assertEqual((value["fighter"].token, value["fighter"].path), (9, "fighter"))
        self.assertEqual(float(value["nested"][0]["value"]), 1.0)
        self.assertEqual(type(value["nested"][0]["value"]).__name__, "_F32")

    def test_action_descriptor_and_source_values_keep_wire_spelling(self):
        self.assertEqual(native.unwrap(action(Action.SPECIAL_N_START)), "special_n_start")
        self.assertEqual(native.unwrap(SourceAction(381)), "Source.381")
        self.assertEqual(native.unwrap(source_action(382)), "Source.382")

    def test_schema_installation_adds_mutable_state_descriptors(self):
        native.install_native_properties(("charge",), ("phase",))
        values = {"fighter.state.charge": 4, "fighter.action_state.phase": "catch"}
        writes = []
        native._get = lambda token, path: (
            {"__skirmish_native__": True, "token": token, "kind": "state", "path": path}
            if path in ("fighter.state", "fighter.action_state")
            else values[path]
        )
        native._set = lambda token, path, value: writes.append((token, path, value))
        fighter = native.NativeObject(4, "fighter", "fighter")
        self.assertEqual(fighter.state.charge, 4)
        fighter.state.charge = 5
        self.assertEqual(writes, [(4, "fighter.state.charge", 5)])
        self.assertEqual(fighter.action_state.phase, "catch")

    def test_native_indices_validate_type_and_bounds(self):
        native._get = lambda token, path: 2 if path.endswith(".length") else path
        values = native.NativeObject(1, "array", "array")
        self.assertEqual(values[-1], "array[1]")
        with self.assertRaises(IndexError):
            values[2]
        with self.assertRaises(TypeError):
            values["1"]


if __name__ == "__main__":
    unittest.main()
