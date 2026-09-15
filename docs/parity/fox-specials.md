# Fox special parity matrix

This is an audit of the Fox special callbacks in the pinned upstream source,
not a claim that the class based authoring layer is complete.  The upstream
revision is `0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`, recorded in
[`upstream.lock.json`](../../upstream.lock.json).  The authoritative source
files are under `/mnt/shared/Projects/Code/External/melee/src/melee/ft/kinds/ftFox/`.

The four move classes currently live in [`scripts/fighters/fox.py`](../../scripts/fighters/fox.py).
They describe the observable action policy through the provisional
`fighter` API.  The C callbacks below also include cosmetic item, GFX, and
item collision callbacks so that omissions are visible; those callbacks are
not silently treated as fighter behavior.

## Requirement-to-oracle matrix

| Move and phases | Upstream callback surface | Observable requirements | Current authoring coverage | Evidence and remaining gap |
|---|---|---|---|---|
| Blaster: ground/air Start, Loop, End | `ftfoxspecialn.c`: `ftFx_SpecialN_Enter`, `ftFx_SpecialAirN_Enter`, `ftFx_SpecialNStart_Anim/IASA/Phys/Coll`, `ftFx_SpecialNLoop_Anim/IASA/Phys/Coll`, `ftFx_SpecialNEnd_Anim/IASA/Phys/Coll`, and air counterparts (lines 255–562); `ftFox_SpecialN_CheckLoopInput` in `inlines.h` | Start/Loop/End transitions; fresh-B repeat arming; command variable fire timing; ground entry velocity reset; air landing/ordinary fall; laser spawn position, angle, speed, lifetime and hitboxes | `NeutralSpecial` has `action_enter`, B input, animation-end, command-trace, and validation hooks; `command_changed` emits a projectile | C snapshot/oracle: `tests/oracle/original/ftfoxspecialn.c`, `tests/oracle/ftfoxspecialn.functions.json`; integration: `tests/game_fox_neutral_special.rs`, differential: `tests/fox_neutral_special_differential.rs`; fixture: `tests/fixtures/game/fox-neutral-special.json`. Cosmetic blaster lifecycle (`ftFx_SpecialN_SpawnBlaster`, `RemoveBlaster`, `GetBlasterAction`, `CheckBlasterAction`, `CheckRemoveBlaster`) and `itfoxblaster.c` are intentionally unmodeled because the item is hitbox-free. The pinned C shot path resolves `FtPart_RThumbNb` (source part ID 35) through `FighterPartsTable.part_to_joint`, applies local offset `[0, 1.2325000762939453, 4.263599872589111]`, then zeros world Z. The current Python path uses the live ECB vertical midpoint as a muzzle approximation. The v10 pack does provide animated 73-bone Start/Loop/End poses and their command traces, but it omits `part_to_joint`/semantic joint mapping and the host has no generic current-pose joint-world-position transform query. The resource exporter owns that mapping; Skirmish should consume the exported contract rather than duplicate a `PlCo.dat` parser. |
| Illusion: ground/air Start, Dash, End | `ftfoxspecials.c`: Start entry/Anim/IASA/Phys/Coll and ground↔air callbacks (88–246); Dash `Anim/IASA/Phys/Coll` (261–420); End `Anim/IASA/Phys/Coll` and entries (467–607) | Side-stick fresh-B gate and reversal; start gravity/velocity policy; dash shortening; fixed dash movement; ground/air conversion; wall/floor clamp and aerial landing; ghost item spawn/removal | `SideSpecial` covers input, action enter, animation end, landed, ground-air changed, and validation | C oracle adapters and differential: `tests/oracle/original/fox_specials.c`, `tests/fox_side_special_differential.rs`; integration: `tests/game_fox_side_special.rs`; fixture: `tests/fixtures/game/fox-side-special.json`. `ftFx_SpecialS_CreateGFX`, ghost position/item state (`itfoxillusion.c`), and ghost removal are not represented by the fighter API; the ghost has no fighter hitbox in this port. |
| Fire Fox: Hold, HoldAir, Travel, AirTravel, Landing, Fall, Bound | `ftfoxspecialhi.c`: Hold physics/collision/conversion (110–208), Travel animation/IASA/Phys/Coll and conversion (215–483), Air entry (483–529), Landing/Fall/Bound callbacks (538–771) | Charge timing and launch input; charge effects; launch angle and speed; travel duration; continuous travel hit; surface redirection and bound; landing/fall exits; air/ground conversion | `UpSpecial` covers input, action entry, hold animation end, travel deadline, terminal animation end, bound marker, landed, ground-air changed, surface contact, and validation | C oracle: `tests/oracle/original/ftfoxspecialhi.c`, `tests/oracle/ftfoxspecialhi.functions.json`; differential: `tests/fox_up_special_differential.rs`; integration: `tests/game_fox_up_special.rs`; fixture: `tests/fixtures/game/fox-up-special.json`. GFX (`CreateLaunchGFX`, `CreateChargeGFX`) and model rotation are presentation-only. The launch/continuous hitboxes are resource data, exercised with explicit pack-derived helpers in `game_fox_up_special.rs`; they are not values invented in this matrix. |
| Reflector: ground/air Start, Loop, Turn, Hit, End | `ftfoxspeciallw.c`: `SetVars/Enter`; Start callbacks (125–256); Loop (257–448); Turn and checks (450–660); Hit and checks (662–860); End (860–981) | held-B/release lag; start hit; ground jump/platform drop; aerial jump; turn threshold and immediate facing flip; reflect state; projectile-contact Hit entry; phase exits and ground↔air conversion; air drift/gravity | `DownSpecial` covers action enter/exit, B press/release, stick change, jump and availability, animation/deadline hooks, projectile contact, landing, ground-air changed, platform drop, and validation | C snapshot/oracle: `tests/oracle/original/ftfoxspeciallw.c`, `tests/oracle/ftfoxspeciallw.functions.json`; differential: `tests/fox_down_special_differential.rs`; integration: `tests/game_fox_down_special.rs`; reflection integration: `tests/game_fox_neutral_special_reflect.rs`; fixture: `tests/fixtures/game/fox-down-special.json`. `Hit` is reachable only through a projectile-contact event; actual projectile/item collision is not available in the current native runtime. Reflect bubble geometry and GFX (`CreateLoopGFX`, `CreateStartGFX`, `CreateReflectGFX`) therefore remain resource/presentation metadata, with `reflecting` as the observable fighter state. |

## Callback categories not represented as move events

Every special file also contains no-op/common callbacks for ordinary physics
and collision.  They are represented by the host lifecycle around a move and
must not be added as arbitrary Python hooks: `Phys`, `Coll`,
`GroundToAir`, and `AirToGround` are phase metadata plus engine boundaries.
Likewise, `Create*GFX` callbacks are presentation effects, while the Blaster
gun and Illusion ghost are item-owned cosmetic state.

The upstream neutral file additionally contains throw-time gun handling in
`ftFx_Throw_Anim` (lines 567–695).  It is outside the four special move
classes and has no gameplay hitbox.  The item sources `itfoxlaser.c`,
`itfoxblaster.c`, and `itfoxillusion.c` are separate item behaviors; their
projectile travel/contact semantics belong to the projectile subsystem and
must be supplied as native resources/events before a Python move can claim
full item parity.

## Supported fixtures

The current independent fixtures are the four JSON resources named in the
matrix and the corresponding `tests/support/fox_*_special.rs` loaders.  The
function-level C oracles are built only when the `c-oracle` feature is
enabled.  The real replay corpus is listed in
[`tests/fixtures/slippi/parity/recordings.json`](../../tests/fixtures/slippi/parity/recordings.json);
those recordings exercise shared action observations and only establish
parity where their exported special resources contain the required data.
Missing resource data, item events, or a fixture for a branch is a coverage
gap, not a passing parity result.

## Class API contract for the next implementation

`Move` instances must remain immutable descriptors.  Per-fighter mutable
state belongs in `Fighter.action_state(action)` (or an equivalent host-owned
state object), and callbacks receive a copied `MoveContext`/event payload.
The four special classes need these event boundaries: input pressed/released,
stick changed, action entered/exited, animation ended, scheduled marker or
countdown, command trace changed, landed, surface contact, ground/air changed,
platform-drop decision, and projectile contact.  A move must be able to select
an action descriptor, preserve or reset action state explicitly, update native
velocity/motion through bounded commands, emit a projectile, and request
fall/landing/Wait exits.  Hitboxes, reflect descriptions, projectile
attributes, and animation command traces remain validated resource data.  Exact
joint-attached effects additionally require a declarative semantic joint or
socket reference plus a host operation that resolves its world transform from
the already-selected animated pose at the command-frame boundary; the exact
wire/API name is pending design.  A raw guessed bone index or ECB/height
heuristic is insufficient for Fox's Blaster origin, including its explicit
world-Z zeroing behavior.

This contract is intentionally narrower than exposing the upstream `HSD_GObj`
or item structs.  It is sufficient for the four fighter policies above while
leaving cosmetic item state and unavailable projectile collisions explicit.
