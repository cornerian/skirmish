# Skirmish renderer

Native rendering and audio presentation live in Skirmish's feature-gated root
module. The `skirmish::renderer` module and `skirmish-renderer` executable
provide a static scene preview and the development host for the original Melee
menu source. One data-driven menu flow serves every input adapter; there is no
separately translated preview UI.

The graphics path uses wgpu 30.0.1, with WESL 0.4.4 compiling the mesh and
lighting shaders at build time. SDL3 0.20 owns the window and events; CPAL
0.18.2 owns native audio output. Headless users can leave the `renderer`
feature disabled to avoid the graphics and audio stack.

## Run

Use stable Rust and Cargo from the workspace root. Install SDL3; on Linux the
build also needs ALSA development files and `pkg-config`. Window presentation
requires a Vulkan, Metal, or DX12 adapter and driver. Headless rendering also
permits GLES. In `xonsh --no-rc`:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo run --locked --features renderer --bin skirmish-renderer
```

The default window shows the procedural scene. `--scene` loads a
`skirmish-visual-v1` JSON export. Arrows orbit, `+`/`-` zoom, `R` resets the
camera, Space plays a quiet synthetic cue, Escape exits, and `Q` quits.
`--no-audio` prevents audio-device initialization.

```xonsh
cargo run --locked --features renderer --bin skirmish-renderer -- \
  --scene /path/to/scene.json
```

Capture one frame without opening SDL or an audio device:

```xonsh
cargo run --locked --features renderer --bin skirmish-renderer -- \
  --headless /mnt/shared/tmp/skirmish-renderer-preview.png \
  --width 257 --height 193
```

For direct Melee UI development, load the converted `MnMaAll.dat` scene through
the four roots chosen by the original scene source:

```xonsh
cargo run --locked --features melee-ui-source --bin skirmish-renderer -- \
  --melee-menu-assets /path/to/MnMaAll-scene.json \
  --no-audio --width 1280 --height 720
```

This executes the connected original background and panel source slice over the
archive topology and runs the source-backed Main behavior at 60 fixed ticks per
second. Arrows/stick navigate, A/Enter/Space confirms, B/Escape backs out, Start
confirms, and mouse hover/left-click use the rendered 4:3 viewport. Full
content/cursor construction, visible animation application, original audio, and
destination instantiation remain under development; see
[direct UI status](melee-ui.md). No recreated menu or "unavailable/pending"
screen is drawn over this path.

For a static capture of the current serialized pose, add `--headless OUTPUT.png`
and use a 640×480 output. Headless mode does not run the interactive session.

Run a short window smoke check that exits after three presented frames:

```xonsh
cargo run --locked --features renderer --bin skirmish-renderer -- \
  --frames 3 --no-audio
```

`--frames` is for window mode and cannot be combined with `--headless`.

## Architecture and current limits

- `scene` validates native JSON mesh attributes and indices, converts source
  triangle winding, loads referenced PNGs, and collects export/material
  warnings.
- `renderer` uploads the scene and shares the depth-tested draw path between an
  SDL3 surface and PNG capture. It resizes its depth target, suspends at zero
  size, and retries recoverable surface events.
- `melee` selects the original `MnMaAll.dat` roots and invokes the source bridge.
- `controls` converts keyboard and SDL controller state into independently
  repeated digital sources and maps pointer events through authored-space hit
  regions. `menu_host` merges them into one canonical command frame before the
  renderer-independent menu runtime advances.
- `audio` retains the CPAL stream and passes commands through a preallocated,
  wait-free SPSC ring buffer.

The private `platform` module contains one documented unsafe surface-creation
call. Its owner stays on the SDL thread and drops the surface before the window.
Public renderer APIs expose no surface or acquired frame.

The current material path is a textured Lambert approximation. GX TEV stages,
alpha tests, custom blending/depth state, source lighting, runtime animation,
and skinning are not yet complete. Texture transforms, coordinate generation,
LOD, wrapping, and filtering are approximated; extra texture stages, line
primitives, and point primitives are omitted.

## Verification

```xonsh
cargo fmt --all --check
cargo test --locked --workspace
cargo test --locked --workspace --features c-oracle
cargo test --locked --workspace --features melee-ui-source
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

Graphics checks are ignored in ordinary runs because they need an adapter and,
for window tests, a compositor:

```xonsh
cargo test --locked --features renderer --lib \
  renderer::gpu::tests::gpu_capture_draws_geometry_and_unpads_rows \
  -- --ignored --nocapture
cargo test --locked --features renderer --test renderer_cli \
  window_smoke_exits_after_presenting_the_requested_frames \
  -- --ignored --nocapture
```

Keep generated PNGs outside the repository: disposable previews belong in
shared temporary storage (or `/tmp` as a sandbox fallback), while durable runs
belong under `/mnt/archive/runs` with provenance.
