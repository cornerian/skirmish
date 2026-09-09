# Native menu navigation

This crate implements controller input and branch-menu navigation independently
of graphics or a console runtime. `MenuState` supports the ten input callbacks
in the pinned `src/melee/mn/mnmain.c`: main, one-player, versus, trophies,
options, data, regular match, stadium, special versus, and records. Labels are
native presentation text; original menu and selection indices are retained.

The source revision is
`0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`, as recorded in the workspace's
`upstream.lock.json`. All builds and tests run natively without an ISO, DOL,
emulator, or GameCube runtime.

```rust
use menus::{Menu, MenuState, Unlocks, input};

let mut menu = MenuState::at(Menu::Main, 1, Unlocks::default()).unwrap();
let action = menu.step(input::CONFIRM); // enters VS. Mode
for _ in 0..5 { menu.step(0); }         // original transition cooldown
let request = menu.step(input::CONFIRM); // requests the VS scene
assert!(menu.snapshot().pending.is_some());
```

`new` uses the original scene-entry cooldown of 20 input polls. `at` constructs
an already-entered branch at a validated available selection, with no cooldown.
Confirm and back transitions consume five subsequent polls, including polls
containing input; input is not buffered. Confirm takes priority over back, up,
and down. Up wins over down. Selection wraps and skips locked/hidden entries.

`step_for_port` records the first confirming controller where the original
callback writes the single-player port. Its port argument must be in `0..4`.
Save-data predicates for All-Star and Sound Test are explicit `Unlocks` inputs.
`Menu::is_available` implements the original predicate, whose C comment says
"locked" even though true means available. The predicate itself does not check
index bounds; `MenuState::at` does.

Selecting a leaf emits `Action::Requested(Destination)` and suspends the branch.
Scene requests preserve the original game-mode numeric values. Panel requests
identify the corresponding unported initializer. The caller is responsible for
opening that destination. `resume` is an explicit native adapter operation that
returns to the originating branch and selection with a five-poll cooldown. It
does not run or complete a leaf screen.

Navigation coverage in `src/melee/mn/mnmain.c`:

| Original code | Preserved behavior |
| --- | --- |
| `mn_80229938`, `mn_80229A04` | Availability and visible-index counting |
| `x2_dec`, `x2_inc`, `decrement_selection`, `increment_selection` | Wrapped navigation with unavailable slots skipped |
| `mn_8022C4F4` | Special versus navigation and scene requests |
| `mn_8022C7CC` | Stadium navigation, scene requests, and Multi-Man panel request |
| `mn_8022CA54` | Records navigation and panel requests |
| `mn_8022CC28` | Regular-match navigation and single-player scene requests |
| `mn_8022CE6C` | Data navigation and records/panel transitions |
| `mn_8022D104` | Options navigation and panel requests |
| `mn_8022D34C` | Trophy navigation and scene requests |
| `mn_8022D594` | Versus navigation and scene/panel transitions |
| `mn_8022D7F4` | One-player navigation and scene/panel transitions |
| `mn_8022DB10` | Main navigation, submenu entry, and title exit request |
| `mnMain_Scene_OnEnter` | Initial 20-poll input lockout and menu selection only |
| `mn_80229894` | Five-poll branch-return lockout and parent selection only |

These callbacks' scalar navigation state and destination requests are ported.
GObj creation/destruction, animation, artwork, text archives, lighting, sound,
scene lifecycle, inactivity handling, save storage, and all requested panels and
scenes remain outside this translation. The source branches for one-player and
data only set their five-poll confirm cooldown when entering another branch;
their panel initializers own subsequent timing. Snapshots preserve that
distinction before handoff. Character select, stage select, custom-rule editing,
and starting a faithful game from a menu are not implemented here.

From the workspace root in xonsh, run the native navigation tests with:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo test --locked -p menus
```

Use `/tmp/skirmish-target` when shared scratch storage is unavailable. The
workspace's `c-oracle` checks compare covered scalar behavior with host-compiled
upstream C; this does not establish PowerPC or full-game equivalence.
