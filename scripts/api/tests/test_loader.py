import sys
import unittest
from types import SimpleNamespace


API = __import__("pathlib").Path(__file__).parents[1]
if str(API) not in sys.path:
    sys.path.insert(0, str(API))

from fighter import Fighter, Move, MoveContext
from skirmish._loader import _plain, discover, dispatch, move_args


class _CallbackMove(Move):
    async def run(self, action: MoveContext) -> None:
        return None


def _fighter_class(module_name="loader_fixture"):
    return type(
        "FixtureFighter",
        (Fighter,),
        {"__module__": module_name, "name": "fixture", "external_ids": (999,)},
    )


class LoaderTests(unittest.TestCase):
    def test_discover_requires_one_local_concrete_fighter(self):
        fighter = _fighter_class()
        namespace = {"__name__": "loader_fixture", "Fighter": Fighter, "FixtureFighter": fighter}
        self.assertIs(discover(namespace), fighter)

        foreign = _fighter_class("other_module")
        self.assertIs(discover({**namespace, "Foreign": foreign}), fighter)

        with self.assertRaisesRegex(ValueError, "exactly one"):
            discover({"__name__": "loader_fixture", "A": fighter, "B": _fighter_class()})

    def test_discover_rejects_malformed_canonical_module_metadata(self):
        fighter = _fighter_class()
        with self.assertRaisesRegex(ValueError, "canonical fighter module"):
            discover({
                "__name__": "loader_fixture",
                "__skirmish_canonical_module__": 7,
                "FixtureFighter": fighter,
            })

    def test_dispatch_wraps_arguments_and_rejects_boolean_or_out_of_range_slots(self):
        seen = []

        def callback(value):
            seen.append(value)
            return {"result": value}

        bundle = {"callbacks": (callback,)}
        self.assertEqual(dispatch(bundle, 0, ({"nested": [1]},)), {"result": {"nested": [1]}})
        self.assertEqual(seen, [{"nested": [1]}])
        for index in (True, -1, 1, "0"):
            with self.assertRaises(ValueError):
                dispatch(bundle, index, ())

    def test_move_args_returns_selected_move_and_fresh_context(self):
        first, second = _CallbackMove(), _CallbackMove()
        bundle = {"moves": (first, second)}
        fighter_descriptor = {"__skirmish_native__": True, "token": 3, "kind": "Fighter", "path": "fighter"}
        action_descriptor = {"action": "special.neutral"}
        move, context = move_args(bundle, 1, fighter_descriptor, action_descriptor)
        self.assertIs(move, second)
        self.assertEqual(context.action, action_descriptor)
        self.assertEqual(context.fighter.kind, "Fighter")
        for index in (True, -1, 2, 1.0):
            with self.assertRaises(ValueError):
                move_args(bundle, index, fighter_descriptor, action_descriptor)

    def test_plain_recursively_produces_wire_values(self):
        value = {1: (2.5, {False: "nested"}), "enum": SimpleNamespace(as_dict=lambda: {"x": 3})}
        self.assertEqual(_plain(value), {
            "1": [2.5, {"False": "nested"}],
            "enum": {"x": 3},
        })


if __name__ == "__main__":
    unittest.main()
