# Direct Melee UI runtime

Skirmish now has a native-source path for the pinned Melee menus. The
`melee-ui-sys` crate verifies the complete upstream `mnmain.c` byte-for-byte
against revision `0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`, requires the source
checkout and its headers to be clean at that exact revision, compiles the
complete `mnmain.c` plus the exact-revision animation and joint-lookup helpers
for the host, and exposes only functions whose dependencies are already
implemented. It does not contain a second menu state-machine rewrite.

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

The development render selects only the four `MnMaAll.dat` roots loaded by
`mnMain_Scene_OnEnter`: `MenMainBack_Top`, `MenMainPanel_Top`,
`MenMainConTop_Top`, and `MenMainCursor_Top`. It retains source joint identities,
uses the original 4:3 camera, and clears to the original fog color. This fixes
the previous renderer behavior that overlaid all 57 independent archive roots.

## Current boundary

The output is a recognizable but noninteractive serialized default pose. It is
not yet the completed Melee menu. The C-owned host runtime performs the
background constructor's GObj ownership, GX/proc registration, JObj attachment,
frame-zero request, one direct process-callback invocation, and configured
render-callback invocation. It builds the exported 102-node background and
106-node panel hierarchies before calling the unchanged constructors. The
original preorder lookup resolves panel nodes 4 and 41, including their real
subtrees. The original panel process moves from the Main entry frame 0 through
the VS transition request at frame 400 to the idle request at frame 500, and
the original user-data teardown runs. Pointer-free results cross into Rust as
scene plans.

Both plans deliberately keep `pose_evaluated` false. The original functions
drive a host implementation of the relevant HSD frame-clock semantics, but the
converted scene does not yet carry runtime joint, material, or shape animation
tracks. The next step is a companion runtime manifest keyed by stable source
offsets that supplies the archive's actual hierarchy and tracks.
Content setup in `mn_8022B3A0` follows and adds cursor cloning/reparenting,
recursive visibility, SIS text, and material/shape animation masks. Unsupported
calls remain errors; success stubs are not a substitute for executing the
original UI.

The renderer-independent `skirmish::animation` module now safely decodes and
evaluates the exact FObj subset used by the Back and Panel roots. A small
provenance fixture binds one visible Back joint to its original AObj/FObj byte
ranges and asserts five sample values bit-for-bit against the pinned C runtime.
ConTop still requires the SLP opcode, material diffuse channels, and texture
color-register channels; Cursor still requires texture scale and color-register
channels. Those inputs fail explicitly until their exact consumers exist. The
decoder is not yet connected to joint posing or GPU updates, so this milestone
does not change the rendered default pose.

VS mode is implemented by the original source. The panel transition itself is
now verified through the original callback: `MENU_KIND_VS` requests frame 400
and settles into its frame-500 idle loop after 51 callback invocations. The
remaining handoff is the full input/content path: after its 20-frame entrance cooldown,
Main selection 1 enters `MENU_KIND_VS`; confirming the default Melee entry later
requests `GM_VS`. The removed translated preview's **Original scene pending**
label was only an unconnected host handoff; it was never evidence that VS mode
or its assets were unavailable.
