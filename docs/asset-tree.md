# Canonical asset tree

This is the asset organization contract for Skirmish's importer and consumers.
Every imported output must have a stable ID, source file, source offset or symbol,
category, relative path, content hash, and decoding status in the bundle manifest.
Copying a DAT container does not count as extracting its constituent assets.

The [complete source tree](asset-source-tree.md) explicitly lists all **1,209**
game filesystem files from the supported USA 1.02 image. It includes the original
DAT/USD archives, sound banks, music streams, movies, text, and other files.
File presence in that inventory does not imply use by the current native game.

## Assets used by the current native game

These are the built-in assets used by the graphical game and native demo today:

```text
runtime/
├── graphics/
│   ├── checker-cube           # Scene::demo(), 6 authored quad faces
│   ├── floor                  # Scene::demo(), authored floor quad
│   └── procedural-checker     # Scene::demo(), generated 8×8 RGBA texture
├── shaders/
│   ├── mesh.wesl              # mesh vertex/fragment entry points
│   └── lighting.wesl          # shared lighting functions
├── audio/
│   └── navigation-cue         # AudioOutput/Mixer, synthesized 180 ms cue
└── gameplay/
    └── integration-match.json # bundled synthetic data used by demo-match
```

Implementations: [scene assets](../src/renderer/scene.rs),
[audio cue](../src/renderer/audio.rs), [shaders](../src/renderer/shaders),
and [demo match data](../tests/fixtures/game/integration-match.json).
Build-time linked WGSL is derived from the listed WESL sources, not an additional
authored asset. The direct Melee UI path consumes original menu exports separately.

The renderer additionally consumes the user-selected `skirmish-visual-v1` scene
JSON and its referenced PNG textures. `run-match` consumes the selected native
`MatchData` JSON. `validate-replay` consumes initialization JSON (including native
match data) and a Slippi replay. These are user-selected inputs, so their exact
filenames are recorded by the selected bundle/run, not invented here.

The focused Fox/stage resources under [native-data fixtures](../tests/fixtures/native-data)
are regression evidence, not complete runtime fighters or stages: `sources.json`,
`fox-native-subset.json`, `fox-jab1-captured.csv`, `fox-jab1-processed.json`, and
`final-destination-bounds.json`. Replay/oracle test inputs are test data and are
inventoried separately in their fixture manifests.

## Decoded import layout

The target bundle groups payloads by resource type. Archive IDs preserve original
case and include the extension (`PlFxNr.dat`, not a guessed character identity).
Within an archive, descriptor offsets and public symbols are stable source IDs.
Human-friendly aliases such as `characters/fox/normal` belong in the manifest;
they must not replace the source identities or imply that unknown assets are known.

```text
<bundle>/
├── manifest.json
├── asset-tree.json                         # exhaustive decoded asset IDs/paths
├── models/<archive-id>/
│   ├── scene.json                          # skirmish-visual-v1, renderer input
│   ├── mesh_<offset>.json                  # positions, topology, normals, UVs, colors
│   └── skeleton.json                      # bone IDs, parents, flags, SRT/bind transforms
├── materials/<archive-id>/
│   └── material_<offset>.json              # colors, texture links, GX render metadata
├── images/<archive-id>/
│   └── image_<offset>_palette_<offset>_mip_<level>.png
├── audio/
│   ├── music/<source-id>/stream.wav         # decoded HPS stream
│   ├── banks/<source-id>/sound_<index>.wav  # decoded SSM subsong
│   └── <source-id>/metadata.json           # sample rate, channels, loop points, IDs
├── animations/<archive-id>/
│   ├── skeletal_<symbol>.json              # original keys, interpolation, frame timing
│   ├── material_<symbol>.json              # material/color/texture animation tracks
│   └── visibility_<symbol>.json            # display/visibility animation tracks
├── gameplay/<archive-id>/
│   ├── attributes_<symbol>.json            # common/fighter/item parameters, exact bits
│   ├── actions_<symbol>.json               # action tables and command streams
│   └── effects_<symbol>.json               # gameplay-affecting effect/projectile data
├── collision/<archive-id>/
│   ├── stage_<symbol>.json                 # vertices, edges, connectivity, flags
│   └── fighter_<symbol>.json               # hurtboxes, ECB, bone attachments
├── fonts/<source-id>/
│   ├── glyphs.png                         # decoded glyph atlas
│   └── glyphs.json                        # character map, dimensions, metrics
├── text/<source-id>/strings.json           # text bytes, encoding, decoded strings
├── movies/<source-id>/                     # original stream + metadata until decoded
├── archive-data/<archive-id>/
│   ├── archive.json                       # header, public/extern symbols, relocation graph
│   └── data.bin                           # preserved untyped data, exact source bytes
├── unresolved/<source-id>/report.json      # unsupported types/ranges and reasons
└── disc/files/                            # complete original source hierarchy
```

This tree is a **contract**, not a statement that every decoder already exists.
The independent extraction crate can install `disc/files` and its manifest.
The offline resource pipeline already exports image PNGs, static model scenes,
material references, and reconstructed fonts. Its existing catalogs under
`skirmish-assets/catalog` enumerate those outputs and aliases; their presence
does not imply that the extraction crate already produces them.

Audio is not assumed to live inside every DAT. The supported image uses separate
HPS music files and SSM sound banks, and these must also be decoded and indexed.
DAT/USD archives are heterogeneous: a single archive can contribute to several
categories. An archive's name must never be used as proof of its internal types.

## Inventory and completeness rules

- `asset-tree.json` must list **every emitted file**, its stable ID, category,
  relative path, source archive/file, descriptor/symbol/subsong ID, and hash.
- Source metadata must retain coordinate conventions, winding, precision, bone
  identity, timing, palettes/mips, audio loop points, and cross-resource links.
- Shared payloads may be deduplicated, but every source alias must remain listed.
- Classify entries as `decoded`, `preserved-untyped`, or `unsupported`; include
  reasons and byte ranges for the latter two. No silently dropped archive blocks.
- `archive-data` preserves the full relocation/symbol graph and raw data section.
  Raw preservation alone is not a semantic decoder for animations/gameplay data.
- Models/images/materials serve presentation; skeleton animation, collision,
  action commands, and gameplay effects must remain available headlessly.
- Counts and source hashes must reconcile with the complete source tree. Report
  decoded coverage independently of whether the current game consumes an asset.
- An import must not report “all assets decoded” while unresolved entries remain.

When a consumer or decoder is added, update this document, the bundle inventory,
and the relevant category's format/coverage documentation in the same change.
