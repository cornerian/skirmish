# Class based fighter authoring

The native fighter authoring package is `scripts/api`. The
`scripts/api/skirmish` package is the import alias used by the fighter
sources in this repository; it re-exports the public `fighter` API.

The authoring boundary is a class declaration. A concrete `Fighter` must
provide `name`, `attributes`, and all eleven typed move groups. `SpecialMoves`
requires four moves; the ordinary groups have useful `ActionMove` defaults.
Defaults are created per group instance, so they are not shared accidentally.
Use the canonical `Action` enum when referring to engine actions:

```python
from skirmish import Action, AerialMoves, ActionMove, register
from scripts.fighters.fox import Fox


defaults = AerialMoves()
neutral_override = ActionMove(
    Action.ATTACK_AIR_N, resource="fox.aerial.neutral"
)
custom_aerials = AerialMoves(neutral=neutral_override)
assert defaults.neutral is not neutral_override


@register
class TrainingFox(Fox):
    name = "training_fox"
    aerials = custom_aerials
```

`TrainingFox` inherits Fox's canonical specials and all other default groups,
then overrides one slot with a distinct move identity. A custom fighter that
does not inherit a complete definition must construct every group explicitly.
Real definitions should use resource-backed action descriptors and move
classes.
The required fields are:

| Group | Required fields |
| --- | --- |
| `SpecialMoves` | `neutral`, `side`, `up`, `down` |
| `AerialMoves` | `neutral`, `forward`, `back`, `up`, `down` |
| `GroundedMoves` | `jab`, `rapid_jab`, `dash` |
| `TiltMoves` | `forward`, `up`, `down` |
| `SmashMoves` | `forward`, `up`, `down` |
| `GrabMoves` | `standing`, `dash`, `pummel` |
| `ThrowMoves` | `forward`, `back`, `up`, `down` |
| `DefenseMoves` | `shield`, `spot_dodge`, `roll_forward`, `roll_back`, `air_dodge` |
| `LedgeMoves` | `wait`, `getup`, `roll`, `attack`, `jump` |
| `GetupMoves` | `neutral`, `roll_forward`, `roll_back`, `attack` |
| `TauntMoves` | `taunt` |

`Move` objects are intended to be reusable immutable code and metadata. A
move's per-fighter and per-action values belong in the host supplied
`MoveContext`, `Parameters`, and `ActionState`, or in native declared state.
The immutable move/definition state is a required contract. Runtime freezing of
the entire reachable Python heap is still in progress, so this document does
not claim that all mutable globals are rejected or that full async rollback is
verified. Native declared state and checkpoints have focused coverage; module
heap checkpoint coverage remains open.

Ordinary moves use the immutable `ActionMove` dataclass. Its required
`action` field accepts an `Action`, canonical action string, or
`ActionDescriptor`; its `resource` field is optional. `ActionMove` has no
Python behavior, `run`, or timer hook: the native host handles the selected
action. A custom `Move` subclass is available for special policies and event
callbacks, but an action descriptor never implies a Python `run` method. The
native Fox ordinary moves use `ActionMove`, while Fox specials remain class
based and event-driven.

`SpecialMove` provides finite transition tables. A subclass may define
`on_end`, `on_ground`, and `on_air` mappings from source actions to
`Transition` objects. Animation completion applies `on_end`; a ground/air
change applies the matching contact table. `Transition` can preserve state or
the current frame. These callbacks are event-driven and finite; they are not
periodic update hooks.

Callbacks are attached with `on` (also exported as `hook`). They are invoked by
the native event dispatcher and may return a host command or boolean according
to the callback boundary. The supported finite hooks are:

`press`, `release`, `stick`, `availability`, `enter`, `exit`,
`animation_end`, `deadline`, `marker`, `countdown`, `command_changed`,
`before_hit`, `before_receive_hit`, `after_hit`, `after_receive_hit`,
`projectile_contact`, `landed`, `surface_contact`, `ground_air_changed`, and
`platform_drop`.

The decorators record static metadata. Depending on the hook, that metadata
can name `actions`, button masks, an animation `marker` and `track`, a
`countdown` field and `phase` (`physics` or `animation`), a command `index`, or
a numeric `deadline`. The canonical exported hook names are the values in
`fighter.events.Hook`, such as `input_pressed`, `animation_ended`,
`scheduled_deadline`, and `platform_drop_decision`. There is no required
`on_frame`, `update`, or periodic timer callback; continuous movement,
collision, hitlag, and resource timeline sampling remain native systems.

`@register` validates the class and marks it for module discovery.
`export_definition(Fox).as_dict()` produces the native linker record with the
fighter name and parameters, all eleven named `movesets`, flattened action
records, callback bindings, and deduplicated behavior records. Reusing one move
instance preserves a shared behavior identity in the exported `behaviors` and
`source_behavior` fields. Python evaluation and callback invocation remain
owned by the Pon bridge.

The current Fox source is the consolidated
[`scripts/fighters/fox.py`](../scripts/fighters/fox.py), which contains the
special policies and all ordinary `ActionMove` slots. It is the current
reference for the one-file authoring boundary; the sketches under `docs/` are
historical and are not API specifications.
