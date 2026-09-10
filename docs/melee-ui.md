# Direct Melee UI runtime

Skirmish now has a native-source path for the pinned Melee menus. The
`melee-ui-sys` crate verifies the complete upstream `mnmain.c` byte-for-byte
against revision `0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`, requires the source
checkout and its headers to be clean at that exact revision, compiles the source
for the host, and exposes only functions whose dependencies are already
implemented. It does not contain a second menu state-machine rewrite.

Set `SKIRMISH_MELEE_SOURCE` to a pinned doldecomp/melee checkout, or keep it at
the repository's documented `../../External/melee` location. The
`melee-ui-source` feature currently executes original scalar menu functions and
is the expansion point for the original GObj/JObj scene setup:

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
not yet the completed Melee menu. The converted scene does not carry runtime
joint, material, or shape animation descriptors, and the source runtime does
not yet have native implementations of the HSD object scheduler, archive symbol
loader, JObj operations, or SIS text bridge.

The next complete source slice is `mnMain_Scene_OnEnter` followed by
`mn_80229B2C`, `mn_80229DC0`, and `mn_8022B3A0`. The host boundary must implement
real operations for loading a symbol, attaching animation descriptors,
requesting/evaluating an animation frame, cloning/reparenting joints, changing
visibility, and draining evaluated draws. Unsupported calls must remain errors;
success stubs are not a substitute for executing the original UI.

VS mode is implemented by the original source. After its 20-frame entrance
cooldown, Main selection 1 enters `MENU_KIND_VS`; confirming the default Melee
entry later requests `GM_VS`. The temporary host panel says **Original scene
pending** because that scene-scheduler handoff is not connected yet, not because
VS or its assets are unavailable.
