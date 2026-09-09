# Native menu branches

The `menus` crate implements navigation for Main, 1-P Mode, VS. Mode, Trophies,
Options, Data, Regular Match, Stadium, Special Melee and Records. It retains the
pinned `mnmain.c` branch selection indices, wrapping, hidden/unlocked selection
rules, input priority, transition cooldowns, and scene/panel requests. Its owned
state has no renderer, platform, resource-file, or controller-device dependency.
The [crate README](../crates/menus/README.md) records function coverage and limits.

## Graphical menu preview

The `renderer` crate presents all ten branches in an SDL3 window through wgpu
and WESL. From the repository root in `xonsh --no-rc`:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo run --locked -p renderer --bin skirmish-renderer -- --menus
```

Use arrows to navigate, Enter/Space/`Z` to confirm, and Escape/Backspace/`X` to
go back. Back on the Main menu exits; `Q` quits from either view. F1 switches
between menu and static scene preview on the same wgpu surface. Without
`--menus`, the executable starts with its existing scene preview. Supply
`--all-star` and `--sound-test` for those caller-owned unlock flags.

Controllers navigate with the D-pad or left stick. Physical South or Start
confirms; East or West goes back. The SDL South position maps to HSD A, while
East and West map to HSD B for menus. West covers the GameCube controller's
physical B button. These menu bindings are separate from gameplay calibration.
Up to four controller ports retain their assignments as other devices connect
or disconnect. The left stick engages a direction at half travel and releases
it by quarter travel. This hysteresis is
the graphical host's menu-input policy, not a translation of original analog
controller processing or gameplay calibration.

The graphical host has one SDL event consumer. It dispatches events returned
by polling and timed waits, while `ControllerHub::with_sdl` shares the context
without consuming the queue. Keyboard and window events therefore remain
available to the application. Menu repeats and cooldowns advance at a fixed
60 Hz independently of rendering frequency. After a stall, the clock permits
at most eight catch-up ticks, discards excess whole ticks, and keeps fractional
time. Hidden or minimized windows pause the clock.

Selecting an unimplemented leaf shows a "Screen unavailable" panel naming the
selection and explaining how to return. Back resumes the originating branch
and selection with its translated cooldown. Holding Confirm or Back does not
produce another press on return. This preview does not start a match, implement
destination screens, or reproduce original menu artwork and animations. It does
not delete files for an erase-data request.

Capture the UI without initializing SDL or audio:

```xonsh
cargo run --locked -p renderer --bin skirmish-renderer -- --menus --headless /mnt/shared/tmp/skirmish-menu-preview.png
```

The destination directory must exist. A graphics adapter is still required.
Window presentation supports Vulkan, Metal, and DX12; headless rendering also
supports GLES. The build uses installed SDL3 and, on Linux, ALSA development
files and `pkg-config`. See the [renderer README](../crates/renderer/README.md)
for setup, graphics smoke checks, and the private surface ownership boundary.

## Terminal preview

From the repository root in xonsh:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo run --locked --bin skirmish -- menus
```

Type `up` or `down`, then press Enter. An empty line, `enter`, `a`, or `start`
confirms; `back` or `b` returns; `quit` exits. The terminal adapter completes
transition cooldowns between discrete commands. For example, `down`, `enter`
opens VS. Mode; another `enter` requests Melee. `back` returns to VS. Mode and
another `back` restores its selection on the Main menu. EOF exits cleanly.

Use `--all-star` and `--sound-test` to supply those unlock flags. These flags are
caller-owned state; this preview does not read or write a saved game.

This is a terminal navigation preview. It does not display the game's graphical
menus, start a faithful match, or implement the destination screens. Selecting
Custom Rules, Sound, character/match scenes, or other leaves produces a typed
request and suspends branch input until the caller resumes. The terminal shows
this boundary and permits Back. No erase-data request deletes files.

## Controller frames and reproducible traces

```xonsh
cargo run --locked --bin skirmish -- run-menus --inputs /path/to/menu-inputs.jsonl
```

Without `--inputs`, JSONL is read from stdin. Each input line is exactly one
adapter tick. A normal line supplies the held digital buttons for all four ports:

```json
{"kind":"controllers","held":[0,0,0,0]}
```

The input contract is HSD digital button bits, including any already determined
stick directions. It is not raw analog controller bytes. Relevant bits are:

| Button | Decimal bit value |
| --- | ---: |
| D-pad left / right / down / up | 1 / 2 / 4 / 8 |
| R / L | 32 / 64 |
| A / B / X / Y | 256 / 512 / 1024 / 2048 |
| Start | 4096 |
| Digital stick up / down / left / right | 65536 / 131072 / 262144 / 524288 |

Start with 20 idle frames to consume the original menu entrance cooldown. Then
Down `[4,0,0,0]`, release `[0,0,0,0]`, and A `[256,0,0,0]` enter VS. Mode. Supply
five idle frames before a fresh A or Start press to request Melee. The adapter
derives triggered buttons and the original accelerated directional repeats.
Holding A through a transition does not generate another confirmation.

The explicit native adapter command below returns from a pending destination
to its originating branch/selection with a five-tick cooldown, releasing the
adapter's held buttons. It is rejected when no destination is pending:

```json
{"kind":"resume"}
```

Output uses the existing semantic trace schema: a Header identifies the pinned
revision, initial menu and unlocks; Frames record inputs, state and actions; End
records the frame count. The trace can be passed to `compare-traces` or used with
`compare-binaries`. Empty/malformed input fails without an End record. The adapter
limits a scenario to one million ticks. It does not consume real time or simulate
the requested destination while it is pending.

## Validation

The workspace tests exercise terminal and JSONL journeys, held confirmation,
resume behavior, input rejection, branch wrapping, hidden/unlocked options and
requests. The `c-oracle` feature compares the translated scalar behavior against
verbatim selected functions from pinned C snapshots, using documented host shims.
The renderer's device-independent tests additionally exercise input through
visible menu rows and leaf return, unlock filtering, fixed ticks across display
rates, bounded catch-up, and menu geometry at landscape and portrait sizes.
Controller polling hardware, the host's analog stick thresholds, graphics,
audio, animation callbacks, leaf screen internals and whole-scene scheduling
remain outside the original-C comparison. GPU/window checks and audible playback
require separate verification.

With a graphics adapter available, run the explicitly ignored menu capture
checks for visible input-to-image changes and capture without SDL video:

```xonsh
cargo test --locked -p renderer --test menu_capture -- --ignored --nocapture
```

The [renderer verification commands](../crates/renderer/README.md#verification)
also include the scene capture and SDL window smoke checks.
