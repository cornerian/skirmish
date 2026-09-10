# Skirmish renderer

Native rendering and audio presentation in Skirmish's feature-gated root module.
The `skirmish::renderer` module and `skirmish-renderer` executable provide a static scene
preview and interactive navigation for all ten translated menu branches. They
do not yet reproduce game rendering, original menu artwork, or animation.
The built-in procedural scene runs without external game resources, an ISO,
DOL, emulator, or GameCube runtime.

The graphics path uses [wgpu 30.0.1](https://docs.rs/wgpu/30.0.1/wgpu/), with
[WESL 0.4.4](https://docs.rs/wesl/0.4.4/wesl/) compiling modular shaders at build
time. [SDL3 0.20](https://docs.rs/sdl3/0.20.0/sdl3/) owns the window, events, and
controller input; [CPAL 0.18.2](https://docs.rs/cpal/0.18.2/cpal/) owns native
audio output. Menus and the scene preview share one wgpu surface.
The workspace lockfile records the resolved dependencies. Headless users can
leave the `renderer` feature disabled to avoid the graphics and audio stack.

## Run

Use stable Rust and Cargo from the Skirmish workspace root. Install SDL3; the
crate uses the system library with the Rust binding's `raw-window-handle`
feature. On Linux the build also needs ALSA development files and `pkg-config`.
Window presentation requires a Vulkan, Metal, or DX12 graphics adapter and
driver. Headless rendering additionally permits GLES. In `xonsh --no-rc`:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo run --locked --features renderer --bin skirmish-renderer
```

Use `/tmp/skirmish-target` when the shared temporary directory is unavailable.
The default window opens the main menu, including **Import Game Assets**.
This native screen provides automatic ISO search, an OS file picker, drag-and-drop,
progress, cancellation, and persistent original-file storage. See the
[import guide](asset-import.md) for coverage and storage details.
An explicit `--scene` starts in the static scene unless `--menus` is also supplied.
The scene supports arrows to orbit, `+`/`-`
to zoom, `R` to reset the camera, Space to play a quiet synthetic cue, and Escape
to exit. Press F1 to switch between scene and menus using the same window and
graphics surface. `Q` quits from either view. `--no-audio` prevents audio device
initialization; an unavailable audio device produces a warning and leaves the
visual preview usable.

Start directly in the menu:

```xonsh
cargo run --locked --features renderer --bin skirmish-renderer -- --menus
```

Use arrows to navigate, Enter/Space/`Z` to confirm, and Escape/Backspace/`X` to
go back. Back from the Main menu exits. Controllers use the D-pad or left stick
to navigate, physical South or Start to confirm, and East or West to go back.
South maps to HSD A; East and West map to HSD B for menus. West includes the
GameCube controller's physical B button. These are menu bindings; gameplay
calibration remains separate.
Add `--all-star` and `--sound-test` to expose those unlocked entries. These flags
do not read or write saved games.

The menu presents Main, 1-P Mode, VS. Mode, Trophies, Options, Data, Regular
Match, Stadium, Special Melee, and Records. Unconnected leaf destinations show
an explanatory "Original scene pending" panel; Back returns to the originating selection. Matches,
settings panels, trophies, and other destination screens remain unimplemented.
No erase-data request deletes files. See [menu behavior](menus.md).

Load a `skirmish-visual-v1` JSON export. PNG paths may be absolute or relative to
the scene file, including sibling directories. Replace `/path/to/scene.json`
with the export's actual location:

```xonsh
cargo run --locked --features renderer --bin skirmish-renderer -- --scene /path/to/scene.json
```

Capture one frame without initializing SDL or audio:

```xonsh
cargo run --locked --features renderer --bin skirmish-renderer -- --headless /mnt/shared/tmp/skirmish-renderer-preview.png --width 257 --height 193
cargo run --locked --features renderer --bin skirmish-renderer -- --menus --headless /mnt/shared/tmp/skirmish-menu-preview.png
```

For the direct Melee UI development path, load a converted `MnMaAll.dat` scene
through its four original source-selected roots and authored camera:

```xonsh
cargo run --locked --features melee-ui-source --bin skirmish-renderer -- --melee-menu-assets /path/to/MnMaAll-scene.json --headless /mnt/shared/tmp/melee-main-default-pose.png --width 640 --height 480
```

This is a serialized default-pose milestone, not a claim that the original JObj
animations and scheduler are complete. See the [direct UI status](melee-ui.md).
Direct Melee asset mode never draws or toggles the translated Skirmish menu
overlay: `--menus` is rejected with `--melee-menu-assets`, and F1 remains on the
direct scene. The separate asset-import screen can still be opened and returns
to the direct scene when closed.

The second command captures the menu UI through the same WESL draw path used
in the window. Add `--scene /path/to/scene.json` to capture an export. Headless
rendering still requires a real or software graphics adapter. Output dimensions must be
between 1 and 8192 and fit the adapter's limits. The destination directory must
already exist. Keep generated PNGs outside the repository: shared temporary
storage for disposable previews, or `/mnt/archive/runs` with provenance for
durable captures. `/tmp` is the sandbox fallback for temporary previews.

Run a short window smoke check that exits after three presented frames:

```xonsh
cargo run --locked --features renderer --bin skirmish-renderer -- --frames 3 --no-audio
cargo run --locked --features renderer --bin skirmish-renderer -- --menus --frames 3 --no-audio
```

`--frames` is for window mode and cannot be combined with `--headless`.

## Architecture and current limits

- `scene` validates the native JSON mesh attributes and indices, converts source
  triangle winding, loads referenced PNGs, and collects export/material warnings.
  It supports diffuse color, vertex color/alpha selection, the first texture
  stage using UV0, source visibility and culling, and normals. Missing normals
  are generated from the triangles with a warning.
- `renderer` uploads the static scene and shares the depth-tested draw path
  between an SDL3 window surface and PNG capture. The window resizes its depth
  target, suspends drawing at zero size, and retries recoverable surface events.
  Capture removes GPU row padding before writing RGBA PNG data.
- `main` owns the sole SDL event pump and dispatches both polled events and the
  event returned by a timed wait. `ControllerHub::with_sdl` shares its context
  without pumping or draining the queue, keeping window and keyboard events
  available to this application loop.
- `menu` adapts the `menus` crate's state to visible rows and destination panels.
  Its integer clock advances at 60 Hz independently of presentation frequency.
  Catch-up is limited to eight ticks after a stall; excess whole ticks are
  discarded while fractional time is retained. Hidden or minimized windows
  pause the clock.
- `controls` maps keyboard and up to four stable controller ports to HSD buttons.
  Confirm and Back are edge triggered; directions use the translated repeat
  rules. Left-stick directions engage at half travel and release by quarter
  travel. These thresholds are a host menu policy, without claiming original
  analog-controller processing or gameplay calibration.
- `ui` builds menu and text geometry for the same surface or headless target.
  the root `build.rs` links `src/renderer/shaders/mesh.wesl`,
  `src/renderer/shaders/lighting.wesl`, and `src/renderer/shaders/ui.wesl`
  into WGSL in Cargo's `OUT_DIR`; the executable embeds the output. Shader
  compilation needs no separate WESL CLI step.
- `audio` retains the CPAL stream and passes commands through a preallocated,
  wait-free SPSC ring buffer. The mixer limits playback to eight 180 ms
  procedural voices. Its callback does not allocate, lock, perform file IO, or
  log; stream errors are counted atomically for the application to report.

The private `platform` module contains one documented unsafe surface creation
call. Its owner stays on the SDL thread and drops the surface, then its graphics
instance, then the window; the window retains the SDL video subsystem and
display connection. Public renderer APIs expose no surface or acquired frame.
There are no unsafe `Send`/`Sync` implementations for SDL objects, and the crate
retains `unsafe_code = "deny"` outside this constructor. Windowed GLES is excluded
because wgpu requires an owned, thread-safe display provider that SDL does not
supply; the separate headless path retains GLES support.

The current material path is a textured Lambert approximation. GX TEV stages,
alpha tests, custom blending/depth state, source lighting, animation, and
skinning are unsupported. Texture transforms, coordinate generation, LOD,
wrapping and filtering are approximated; extra texture stages, line primitives,
and point primitives are omitted. The loader emits explicit warnings for these
limits and carries export warnings forward. It does not modify simulation
state or replace the asset exporter.

## Verification

Run the workspace's required checks from its root, with `CARGO_TARGET_DIR` set
as above:

```xonsh
cargo fmt --all --check
cargo test --locked --workspace
cargo test --locked --workspace --features c-oracle
cargo test --locked --workspace --features renderer
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

Renderer tests cover scene validation, material/texture loading, shader
validation, audio mixing without a device, and CLI errors without a display.
Menu tests exercise input through visible selection and leaf return, unlock
filtering, timing across rendering rates, bounded catch-up, and geometry at
landscape and portrait sizes.
The graphics checks are explicitly ignored in ordinary test runs because they
need graphics services. Run them separately when those services are available:

```xonsh
cargo test --locked --features renderer --lib renderer::gpu::tests::gpu_capture_draws_geometry_and_unpads_rows -- --ignored --nocapture
cargo test --locked --features renderer --test renderer_menu_capture -- --ignored --nocapture
cargo test --locked --features renderer --test renderer_cli window_smoke_exits_after_presenting_the_requested_frames -- --ignored --nocapture
```

The capture checks need a graphics adapter; menu capture verifies visible
selection/branch changes and output without SDL video initialization. The window
check additionally needs a compositor. These checks do not verify audible
playback or faithful game presentation. Record their results separately from
the device-independent suite.

SDL3/menu verification on 2026-09-09 is archived at
`/mnt/archive/runs/skirmish-sdl-menus-20260909-v1`. The run passed formatting,
Clippy, 305 workspace tests, 444 tests with `c-oracle`, and all five separately
enabled graphics tests. The ordinary suites each ignored 34 tests, including
those five graphics tests and 29 incomplete conformance scenarios. The archive
contains `run.json` with commands and source hashes, `menu.png`, and Wayland
captures of navigation, a destination request and return, F1 scene switching,
and resizing on the RTX 4090. Physical controller and audio hardware were not
exercised. This run's build cache is `/mnt/shared/tmp/skirmish-sdl-menus-target`;
it was moved from `/tmp` when the temporary RAM disk filled during testing.

The following archive is historical evidence for the original winit scene
renderer, before the SDL3 host and graphical menus. It does not validate the
current migration. That initial verification run on 2026-09-09 is stored outside
Git at
`/mnt/archive/runs/skirmish-renderer-20260909-v1`. `run.json` records commands,
source hashes, inputs, versions and results; the directory contains `demo.png`
and `fox.png`. The Fox input was the existing native export at
`/mnt/archive/runs/melee-visual-fidelity-20260909/generated/fox/scene.json`.
It was rendered with `--width 960 --height 640` using the same headless command
above. GPU capture and three-frame Wayland presentation passed on an NVIDIA
GeForce RTX 4090. Offline audio tests passed; the desktop exposed no audio output
sink, so that run did not verify physical playback. The application warned and
continued presenting successfully.
