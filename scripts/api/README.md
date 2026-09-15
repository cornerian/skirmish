# Skirmish fighter API

`skirmish-fighter-api` is the pure Python authoring surface for Skirmish
fighters. It ships the public `skirmish` package and the implementation
package named `fighter`; both packages include type metadata (`py.typed`) so
editors and type checkers can inspect the declarations.

The package targets API edition `0.1` and requires Python 3.11 or newer. The
package version is `0.1.0`; keep an authoring module and its host on the same
API edition because exported definition and callback names are part of the
host contract. Install the wheel with `uv`, or add this directory to a local
editor environment while authoring.

## Runtime boundary

Fighter modules are evaluated by Skirmish's embedded Pon host. The Python
objects in `skirmish._native` are host proxies and only become operational
when that host provides the private `_skirmish_native` module. Operations that
read or mutate fighter state, call host methods, or dispatch exported
callbacks are therefore host-only. Importing and exporting declarations works
in an ordinary Python editor environment, which is useful for completion,
static checking, and validation.

This distribution does not add a Python interpreter dependency to the game,
and it does not promise that every CPython build can execute inside Pon. The
supported runtime is the embedded host shipped with the matching Skirmish
build. Do not use host-only operations in tooling that runs outside that host.

Release builds carry Pon's pure-Python standard library beside the executable
under `resources/pon-stdlib/`. The host verifies the archive and its
`identity.json` manifest before loading fighter modules, so the release works
from any working directory without relying on `PON_STDLIB_PATH`. Explicit
`--pon-stdlib` and `--pon-stdlib-sha256` flags remain available for verified
development overrides.

Build and install with the repository's `uv` workflow:

```xonsh
uv build --wheel
uv pip install dist/skirmish_fighter_api-0.1.0-py3-none-any.whl
```

The wheel is pure Python and contains no native gameplay dependency.

## Authoring model

Declare a `Fighter` class with its name, attributes, and eleven typed move
groups. Ordinary groups such as `AerialMoves()` and `GroundedMoves()` provide
fresh canonical `ActionMove` defaults. `SpecialMoves` is the exception: it
requires `neutral`, `side`, `up`, and `down` explicitly.

Use `Action` enum members for canonical engine actions. An `ActionMove` may
also carry an `ActionDescriptor` and optional resource identity; it describes
the selected native action and does not implement a Python `run` method.
Reusing one move instance in multiple slots is supported and preserved in the
exported behavior identity. To override one default while retaining the rest,
construct only that slot:

```python
from scripts.fighters.fox import Fox
from skirmish import Action, ActionMove, AerialMoves, register


@register
class TrainingFox(Fox):
    name = "training_fox"
    aerials = AerialMoves(
        neutral=ActionMove(Action.ATTACK_AIR_N, resource="fox.aerial.neutral")
    )
```

Special policies subclass `SpecialMove`. Their finite `on_end`, `on_ground`,
and `on_air` mappings use `Transition` objects to move between action phases;
the host invokes these through animation and ground/air events. Fox's
`FoxActionState.command` is declared as a fixed four integer tuple. Runtime
state is host-owned, and this SDK does not claim full heap freezing or full
fighter parity.

For a disposable install smoke test, the repository verification command is:

```xonsh
uv venv /tmp/skirmish-fighter-env
uv pip install --python /tmp/skirmish-fighter-env/bin/python --no-index dist/skirmish_fighter_api-0.1.0-py3-none-any.whl
/tmp/skirmish-fighter-env/bin/python -c "import skirmish, fighter; print('ok')"
```

The wheel check also confirms that `tests/`, `__pycache__/`, and bytecode files
are absent.
