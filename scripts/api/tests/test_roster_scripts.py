"""Integration coverage for the checked-in roster fighter declarations.

These tests deliberately execute each source file in a fresh synthetic Python
package.  That mirrors source-bundle module isolation and avoids relying on a
process-global registration table (or on Pon) for discovery.
"""

from __future__ import annotations

import importlib
import importlib.util
import sys
import types
import unittest
import uuid
from pathlib import Path


ROOT = Path(__file__).parents[3]
FIGHTERS = ROOT / "scripts" / "fighters"
API = ROOT / "scripts" / "api"
if str(API) not in sys.path:
    sys.path.insert(0, str(API))

from fighter import ActionMove, Fighter, export_definition, resolve_identity


ROSTER_MODULES = {
    "bowser",
    "donkey_kong",
    "dr_mario",
    "game_and_watch",
    "ganondorf",
    "ice_climbers",
    "jigglypuff",
    "kirby",
    "link",
    "luigi",
    "mario",
    "marth",
    "mewtwo",
    "ness",
    "peach",
    "pichu",
    "pikachu",
    "roy",
    "samus",
    "sheik",
    "yoshi",
    "young_link",
    "zelda",
}

EXPECTED_MODULES = ROSTER_MODULES | {"captain", "falco", "fox"}
MOVESET_GROUPS = {
    "specials",
    "aerials",
    "grounded",
    "tilts",
    "smashes",
    "grabs",
    "throws",
    "defense",
    "ledge",
    "getup",
    "taunt",
}
SPECIAL_ROOTS = ("neutral", "side", "up", "down")


def _fresh_source_package() -> tuple[str, types.ModuleType]:
    """Create an import package whose modules cannot reuse prior fixtures."""
    package_name = f"_roster_source_{uuid.uuid4().hex}"
    package = types.ModuleType(package_name)
    package.__path__ = [str(FIGHTERS)]
    package.__package__ = package_name
    spec = importlib.util.spec_from_file_location(
        package_name, FIGHTERS / "__init__.py", submodule_search_locations=[str(FIGHTERS)]
    )
    package.__spec__ = spec
    sys.modules[package_name] = package
    return package_name, package


def _load_all_fighters() -> tuple[dict[str, types.ModuleType], str]:
    package_name, _ = _fresh_source_package()
    modules: dict[str, types.ModuleType] = {}
    try:
        for path in sorted(FIGHTERS.glob("*.py")):
            modules[path.stem] = importlib.import_module(f"{package_name}.{path.stem}")
        return modules, package_name
    except Exception:
        _drop_source_package(package_name)
        raise


def _drop_source_package(package_name: str) -> None:
    for module_name in tuple(sys.modules):
        if module_name == package_name or module_name.startswith(f"{package_name}."):
            del sys.modules[module_name]


def _fighter_class(module: types.ModuleType) -> type[Fighter]:
    candidates = [
        value
        for value in vars(module).values()
        if isinstance(value, type)
        and issubclass(value, Fighter)
        and value is not Fighter
        and value.__module__ == module.__name__
        and not value.__dict__.get("__abstract__", False)
    ]
    if len(candidates) != 1:
        raise AssertionError(
            f"{module.__name__} should expose one concrete fighter; "
            f"found {[candidate.__name__ for candidate in candidates]}"
        )
    resolve_identity(candidates[0])
    return candidates[0]


def _assert_string_dict_keys(testcase: unittest.TestCase, value: object, path: str) -> None:
    if isinstance(value, dict):
        for key, nested in value.items():
            testcase.assertIsInstance(key, str, path)
            _assert_string_dict_keys(testcase, nested, f"{path}[{key!r}]")
    elif isinstance(value, (list, tuple)):
        for index, nested in enumerate(value):
            _assert_string_dict_keys(testcase, nested, f"{path}[{index}]")


class RosterScriptTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.modules, cls.package_name = _load_all_fighters()
        cls.fighters = {
            module_name: _fighter_class(module)
            for module_name, module in cls.modules.items()
        }

    @classmethod
    def tearDownClass(cls) -> None:
        _drop_source_package(cls.package_name)

    def test_all_roster_sources_declare_one_concrete_fighter(self):
        self.assertEqual(set(self.modules), EXPECTED_MODULES)
        self.assertEqual(len(self.fighters), 26)

        names = [fighter.name for fighter in self.fighters.values()]
        self.assertEqual(len(set(names)), 26)
        self.assertTrue(all(isinstance(name, str) and name for name in names))

    def test_registered_definitions_cover_css_external_ids_exactly_once(self):
        definitions = []
        for module_name, fighter in self.fighters.items():
            definition = export_definition(fighter).as_dict()
            _assert_string_dict_keys(self, definition, module_name)
            definitions.append(definition)
        external_ids = [definition["external_ids"] for definition in definitions]
        self.assertTrue(all(len(ids) == 1 for ids in external_ids))
        self.assertEqual(
            [ids[0] for ids in external_ids],
            list(dict.fromkeys(ids[0] for ids in external_ids)),
        )
        self.assertEqual({ids[0] for ids in external_ids}, set(range(26)))

    def test_roster_derived_definitions_have_default_special_behaviors(self):
        behavior_roots = {}
        for module_name in ROSTER_MODULES:
            fighter = self.fighters[module_name]
            definition = export_definition(fighter)
            exported = definition.as_dict()

            self.assertEqual(set(exported["movesets"]), MOVESET_GROUPS, module_name)

            specials = getattr(fighter, "specials")
            moves = [getattr(specials, root) for root in SPECIAL_ROOTS]
            self.assertEqual(len({id(move) for move in moves}), 4, module_name)
            self.assertTrue(
                all(not isinstance(move, ActionMove) for move in moves),
                module_name,
            )

            for root, move in zip(SPECIAL_ROOTS, moves):
                # The standard policy stores the resource root on the move;
                # use its public value rather than depending on a concrete
                # enum class in this integration test.
                self.assertEqual(getattr(getattr(move, "root"), "value", getattr(move, "root", None)), root)
                input_events = [event for event in move.events() if event.hook.value == "input_pressed"]
                self.assertEqual(len(input_events), 1, f"{module_name}.{root}")
                self.assertTrue(callable(getattr(move, input_events[0].callback, None)))
                behavior_roots.setdefault(root, set()).add(type(move))

                behavior_id = exported["movesets"]["specials"][root]
                behavior = next(item for item in exported["behaviors"] if item["id"] == behavior_id)
                callbacks = behavior["callbacks"]
                self.assertTrue(
                    any(callback["hook"] == "input_pressed" for callback in callbacks),
                    f"{module_name}.{root} exported no input callback",
                )

        self.assertEqual(set(behavior_roots), set(SPECIAL_ROOTS))


if __name__ == "__main__":
    unittest.main()
