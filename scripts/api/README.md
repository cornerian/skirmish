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

`fighter` is the canonical implementation package. `skirmish` is the stable
authoring facade, and the older `skirmish.api` and `skirmish.events` module
paths remain compatibility aliases for source bundles. They resolve to the
same descriptor classes and decorator singletons, so a module can use either
facade without creating a second registration or callback system.

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

A canonical roster module declares exactly one concrete `Fighter` subclass.
The module filename supplies its roster identity, so normal fighter scripts do
not repeat a `name`, `external_ids`, `roster`, or registration decorator. The
loader discovers the local subclass and exports it after validation:

```python
from skirmish import Action, ActionMove, AerialMoves, Fighter


class Fox(Fighter):
    aerials = AerialMoves(
        neutral=ActionMove(Action.ATTACK_AIR_N)
    )
```

`Fighter` supplies fresh defaults for all eleven move groups, including four
resource-gated `OpenSpecial` defaults. Override only the group or slot that a
fighter implements. `SpecialMoves.replace(...)` is the compact way to retain
the other special defaults:

```python
from skirmish import ArticleId, B0ArticleSpecial, Fighter, source_phase


class Fireball(B0ArticleSpecial):
    article_id = ArticleId.MARIO_FIRE
    ground_state = 343
    air_state = 344
    ground = source_phase(343, animation=295)
    air = source_phase(344, animation=296)


class Mario(Fighter):
    specials = Fighter.specials.replace(neutral=Fireball())
```

Source motion states and callback filters are declared together. The exporter
rewrites source references for the owning fighter identity, including callback
action filters and transition targets:

```python
from skirmish import Action, Fighter, SpecialMove, on, source_phase


class SpecialN(SpecialMove):
    ground = source_phase(341)

    @on.animation_end(ground)
    def finish(self, fighter, context):
        fighter.change_action(Action.WAIT)


class FighterScript(Fighter):
    specials = Fighter.specials.replace(neutral=SpecialN())
```

The exported action is namespaced as `Source.<fighter-id>:341`; the callback
filter uses the same identity. This keeps shared move instances safe when the
same behavior is reused by multiple fighter declarations.

`NeutralSpecial`, `SideSpecial`, `UpSpecial`, and `DownSpecial` provide only
the canonical special resource root; complex moves still declare their own
actions and behavior. Use `Action` enum members for canonical engine actions.
An `ActionMove` may also carry an `ActionDescriptor` and optional resource
identity; ordinary move geometry and attributes remain native action data,
not resource-string metadata. A resource is useful only when a custom move
intentionally gates behavior on a named native resource. `ActionMove` does
not implement a Python `run` method. Reusing one move instance in multiple
slots is supported and preserved in the exported behavior identity.

`FighterBase` is the strict declaration-only base for noncanonical external
definitions that intentionally provide every move group themselves. Such
definitions may provide explicit identity (and may use the compatibility
registration API); this is not part of a normal roster module. Runtime state is
host-owned, and this SDK does not claim full heap freezing or full fighter
parity.

For a disposable install smoke test, the repository verification command is:

```xonsh
uv venv /tmp/skirmish-fighter-env
uv pip install --python /tmp/skirmish-fighter-env/bin/python --no-index dist/skirmish_fighter_api-0.1.0-py3-none-any.whl
/tmp/skirmish-fighter-env/bin/python -c "import skirmish, fighter; print('ok')"
```

The wheel check also confirms that `tests/`, `__pycache__/`, and bytecode files
are absent.
