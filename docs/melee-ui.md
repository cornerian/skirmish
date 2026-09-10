# Direct Melee UI runtime

Skirmish now has a native-source path for the pinned Melee menus. The
`melee-ui-sys` crate verifies the complete upstream `mnmain.c` byte-for-byte
against revision `0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`, requires the source
checkout and its headers to be clean at that exact revision, compiles the
complete `mnmain.c` plus the exact-revision animation and joint-lookup helpers
for the host, and exposes only functions whose dependencies are already
implemented. It does not contain a second menu state-machine rewrite.

Main- and Versus-menu behavior is represented once as validated declarative
Rust data and a small renderer-independent state machine. The pinned C remains
its behavioral oracle: item order, descriptions, authored selection ranges,
vertical wrap, input priority, entrance and action lockouts, cue IDs,
destination IDs, and Back behavior are covered directly. This is the
extensible replacement for hard-coded source control flow, not a parallel
preview menu.

Set `SKIRMISH_MELEE_SOURCE` to a pinned doldecomp/melee checkout, or keep it at
the repository's documented `../../External/melee` location. The
`melee-ui-source` feature executes original scalar menu functions and the exact
`mn_80229B2C` background constructor and `mn_80229DC0` panel constructor with
its original `fn_80229BF4` process callback:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo test --locked -p melee-ui-sys --features source
cargo run --locked --features melee-ui-source --bin skirmish-renderer -- \
  --melee-menu-assets /path/to/MnMaAll-scene.json \
  --headless /mnt/shared/tmp/melee-main-default-pose.png \
  --width 640 --height 480
```

When that checkout is absent, the feature emits a build warning and keeps the
serialized default-pose preview usable; it does not claim that original source
was executed. This keeps `--all-features` validation portable while preserving
an explicit distinction between the source-backed and preview-only paths.

The development asset loader selects only the four `MnMaAll.dat` symbols loaded
by `mnMain_Scene_OnEnter`: `MenMainBack_Top`, `MenMainPanel_Top`,
`MenMainConTop_Top`, and `MenMainCursor_Top`. It retains source joint identities,
uses the original 4:3 camera, and clears to the original fog color. The current
serialized preview renders all four exports as roots, which is not yet the
source composition: `MenMainCursor_Top` is a prototype that `mn_8022B3A0`
clones five times and reparents beneath ConTop slots.

## Current boundary

The SDL window runs a persistent 60 Hz fixed-tick menu session. It starts in
Main and resolves Main-to-Versus and Versus-to-Main handoffs without rebuilding
the host input adapters. Keyboard, hot-plug SDL controllers, and mouse all enter
the same canonical command queue. Directional holds use Melee's independent
20/8/4/2-frame repeat schedule. Menu handoffs preserve keyboard and controller
edge state, inherit the source-global action cooldown, and re-hit-test the
stationary pointer against the destination menu.

Mouse motion and left-click are transformed from window units through the
renderer's current high-density 4:3 viewport into the authored 640×480 canvas.
Clicks emit the same focus-then-confirm commands used by every other adapter.
Main and Versus share the source-derived five-slot geometry; their
non-overlapping regions are replaceable presentation data derived from the
frame-5 visible label quads. Melee itself had no mouse contract, so the
eight-pixel tolerance is a host extension.

Each selected menu cue initializes or restarts a renderer-independent authored
frame clock. The first presentation tick samples the range start, subsequent
fixed and catch-up ticks advance at unit rate, and looped clips wrap at the
source end boundary before displaying the terminal integer frame. This clock is
connected to menu effects, but not yet to decoded asset tracks.

The rendered output is still a recognizable serialized default pose, so
selection changes and destination requests are currently visible in host
diagnostics rather than pixels. This is not yet the completed Melee menu. The
C-owned host runtime performs the
background constructor's GObj ownership, GX/proc registration, JObj attachment,
frame-zero request, one direct process-callback invocation, and configured
render-callback invocation. It builds the exported 102-node background and
106-node panel hierarchies before calling the unchanged constructors. The
original preorder lookup resolves panel nodes 4 and 41, including their real
subtrees. The original panel process moves from the Main entry frame 0 through
the VS transition request at frame 400 to the idle request at frame 500, and
the original user-data teardown runs. Pointer-free results cross into Rust as
scene plans.

Both C scene plans deliberately keep `pose_evaluated` false. The converted
scene still lacks the animation target graph and alternate texture tables
needed to bind archive tracks to runtime objects.

The generic presentation-instance layer models independently mutable joint
hierarchy and SRT-or-matrix state, composed HSD world matrices, effective
branch visibility, material state, and texture-animation state under stable
source identities and distinct runtime identities. Clones therefore retain their source provenance without sharing
mutable state. The layer consumes sampled channel values but does not decode an
asset schema, schedule playback, upload GPU state, or implement skinning. The
menu host does not yet construct these instances from `MnMaAll` or synchronize
them with the renderer.

A companion animation manifest keyed by stable source offsets must still supply
the hierarchy, track-to-owner bindings, material and texture identities, and
ordered alternate-image tables. Content construction must then express
`mn_8022B3A0`'s cursor cloning and reparenting, SIS text, and animation masks
against those instances. Main-to-Versus and Versus-to-Main are connected
internal destinations; scene exits plus the Special, Rules, and Name submenus
remain unresolved. The three original menu sounds are also not connected.
Unsupported calls remain errors; diagnostic action requests and success stubs
are not a substitute for executing the original UI.

The renderer-independent `skirmish::animation` module safely decodes and
evaluates the exact FObj subset used by the Back and Panel roots. Its coverage
fixture binds all 133 AObjs and 276 FObjs in those slices and checks 1,349
samples against the pinned C oracle. A smaller provenance fixture also binds
one visible Back mesh to its original joint and byte ranges.

The decoder recognizes every scalar channel used by the audited Main-selection
ConTop subtree: joint translation, scale, and branch visibility; material
diffuse RGB and alpha; and texture image, U/V translation, blend, Konst alpha,
and TEV0 alpha. That subtree uses interpolation opcodes 1–4 and does not require
SLP. Cursor texture scale and its additional color-register channels remain
explicitly unsupported.

The GPU path retains serialized-hidden geometry plus source MObj, first-stage
TObj, and render-mode metadata without rebuilding geometry or textures. Source
render mode fixes each draw in its OPA, TEXEDGE, or XLU pass while material
alpha animates, and authored traversal order remains stable within each pass.
Visibility uses an exact export-joint/optional-instance selector; material
color instead requires the concrete source MObj identity, so one joint cannot
accidentally broadcast a material track across unrelated materials. Vertex-owned
color channels remain identity factors when a material update arrives.

These exported offsets are still resource-local and cannot distinguish runtime
clones, so they are a transitional renderer boundary rather than canonical
scene-instance identities. The exact join is specified in the
[visual export contract](resources.md#exact-presentation-visual-exports):
declared resource hash and offset spaces, JObj/DObj/MObj/TObj occurrence
ordinals, and a per-draw `geometry_space`. `VisualPresentationBinding`
cross-checks that declaration against the manifest and routes sampled
visibility and material color to exact draws. Each scene instance composes
HSD world matrices from its joint locals (the original `HSD_MtxSRT` and
`C_MTXConcat` ports, honoring `JOBJ_CLASSICAL_SCALE`, with user-defined
matrices taken verbatim), and every composed `JointWorld` delta is routed to
the GPU's per-draw joint transform when the joint's draws are joint-local.
World-baked draws retain it as `BakedWorldGeometry` and draw-less joints as
`UnmappedJointWorld`. Billboards, quaternion rotation, constraints, and
skinning are not modeled; the manifest reports the one billboard joint in the
panel hierarchy as `UnmodeledJointFlags`. Texture image switches and the
animated TObj translation/scale reach the first sampled stage through the
original `MakeTextureMtx` port (checked against the pinned C) with the
export's GX wrap modes, when the current image resolves to an exported
texture; the label frames the menu switches through TexAnim tables are not
in the current export, so those deltas are retained as
`UnresolvedTextureImage`. Later texture stages, blend, konst, and TEV0
registers are still not rendered.

The resource project's current MnMaAll export carries none of that metadata
and bakes every part into world space, so the interactive host still advances
a plain `AnimationPlayback` and cannot construct exact bindings. Joint posing,
skinning, texture selection, UV transforms, texture color registers, complete
PE state, and additional texture stages are not GPU-bound. Consequently the
rendered menu remains the serialized default pose.

Versus behavior is a connected declarative menu, not an unavailable screen.
Its Melee, Tournament, Special, Rules, and Name entries carry the source
descriptions `0x8E`–`0x92`, label frames 40–48, selection ranges 700–949 with
their twenty-frame loop offsets, cue IDs, destinations, and Back behavior.
Confirming Versus in Main enters this runtime with Melee selected and the
inherited five-frame cooldown; Back restores Main with Versus selected. The
same five-slot mouse policy works through the handoff without leaking the click
or re-triggering a held button.

Independently, the original panel callback verifies that `MENU_KIND_VS`
requests frame 400 and settles into its frame-500 idle loop after 51 callback
invocations. VS content construction, paired transition clips, SIS text, and
the external `GM_VS`/Tournament and submenu destinations remain unconnected, so
the current pixels do not yet change when the behavioral handoff occurs.
