# Gameplay data export: Fox on Final Destination (design note)

Goal: an exported, headless gameplay resource set for Fox versus Fox on Final
Destination that Skirmish's `validate-replay` can run against a real Slippi
recording, so the first divergent frame becomes the parity backlog.

Authorized by the user on 2026-09-11 ("Go ahead and export"). The exporter
lives in `skirmish-assets` (the resource-conversion owner per Skirmish's
`docs/resources.md`); Skirmish consumes its output. Never require the ISO at
Skirmish build or test time; the export runs once and lands in the archive.

## Inputs (all present)
- Disc files from the verified USA 1.02 import at
  `/mnt/archive/runs/melee-assets-20260909/disc/files/`: `PlCo.dat` (common
  fighter data), `PlFx.dat` (Fox fighter data: attributes, subactions,
  hurtboxes, model part table), `PlFxAJ.dat` (Fox animation figatrees),
  `PlFxNr.dat` (Fox skeleton: JObj tree), `GrNLa.dat` (Final Destination:
  collision, spawns, blast zones). Cite the import's manifest hashes.
- Pinned decomp `/mnt/shared/Projects/Code/External/melee` rev
  0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9 for every on-disk layout:
  `sysdolphin/baselib/archive.h` (HSD_Archive header, relocations, public
  symbols), `melee/ft/types.h` (`ftData`, `ftCo_DatAttrs` at +0 sized 0x184,
  `ftData_x8`, hurtbox init, `Fighter_WaitAnimData`, `ftCommonData`),
  `melee/ft/ftdata.c` (how the runtime resolves subactions, figatrees,
  hurtboxes and part tables from the archive), `melee/lb/lbanim.h/.c` and
  `sysdolphin/baselib/{fobj,aobj,figatree}.c` (FigaTree, FigaTrack, FObj
  keyframe streams and interpolation), `melee/ft/ftaction.c` (the script
  command table: opcode widths and fields), `melee/mp/{types.h,mplib.c}` and
  `melee/gr/*` (map collision lines, ledges, spawn points, blast zones),
  `melee/ft/kinds/ftFox/types.h` (Fox special attributes).
- Independent references already checked into Skirmish for cross-checks:
  `tests/fixtures/native-data/fox-native-subset.json` (44 attributes with
  exact binary32 bits and the 15 raw Attack11 script events),
  `fox-jab1-captured.csv` (libmelee's captured jab hitbox positions, frames
  1-17), `final-destination-bounds.json` (blast bounds, edge x, ledge x).
- The target replay: `/mnt/archive/datasets/melee/slippi-public-dataset-v3.7/
  data/FOX/batch_00/15_01_20 Fox + Fox (FD).slp` (Slippi 2.0.1, stage 32,
  ports P1 and P4, 4673 frames, sha256
  87971fc4608e577fe5a814fc3a04dee4c0d82f99bc9a54ee16797005c5a9085e).

## Output
`/mnt/archive/runs/skirmish-fox-fd-export-20260911/` containing
`match-data.json` (Skirmish `MatchData`: two Fox fighters plus the stage and
common rules), a sidecar `fox-gameplay.json` with every decoded structure by
name and offset (attributes by decomp field name and bit pattern, subaction
table, script events per subaction, hurtboxes, part table, figatree index),
`fd-stage.json` (raw decoded map data), and `manifest.json` (source files
with sha256 and offsets, decomp revision, exporter version, decoding status
per category). Skirmish then builds the `validate-replay` initialization
from `match-data.json` plus the replay (seed, ports, stocks, warm-up).

## Decoding plan (exact, cite each layout)
1. HSD archive reader: header, data block, relocation table (big-endian
   u32 offsets to pointer fields), public symbol table (name -> offset).
   Resolve pointers by adding the data base; never follow unrelocated
   fields.
2. `PlCo.dat` -> `ftCommonData` (public symbol name from `ftdata.c`/
   `ftcommon.c` loading code): map every field Skirmish already names in its
   rules (dash/walk thresholds, tilt/smash radians, escape windows, jab
   windows, knockback/hitlag/hitstun constants, kb_squat_mul,
   kb_smashcharge_mul, x44/x48/x4C/x54/x68 dash values, teeter values,
   x1FC/x21C/x218/x220/x224 special thresholds, powershield window, etc.)
   by the decomp offset, and keep the whole struct by offset in the sidecar.
3. `PlFx.dat` -> `ftData` (public symbol `ftDataFox`; confirm in
   `ftdata.c`): attributes (`ftCo_DatAttrs`, bit-exact; must equal the 44
   fixture values), Fox special attributes (`ext_attr`), part table (bone
   ids for TransN, XRotN, the shield bone, ECB bones), hurtbox table
   (`ftHurtboxInit` entries: bone, height kind, grabbable, offsets, scale),
   ECB description, the subaction table (per entry: figatree symbol name,
   animation data offset/size into `PlFxAJ.dat`, script pointer, flags),
   the wait-animation table (`ftData.xC` with weights) and the model part
   descriptors.
4. `PlFxNr.dat` -> skeleton: the root JObj (public symbol from the model
   loading code) traversed child/next with `HSD_JointDesc` flags, rotation
   (Euler, radians, XYZ order as HSD builds matrices), scale, translation ->
   Skirmish `bones` (parent index, translation, rotation, scale,
   classical_scale flag from the JObj flags). Preserve the bone order the
   runtime uses for part indices.
5. `PlFxAJ.dat` figatrees: FigaTree {type, flags, frames, nodes, tracks};
   nodes count tracks per joint in joint order; FigaTrack {start frame,
   length, data offset, data type, value/tangent formats, flags}; decode the
   FObj keyframe stream exactly as `HSD_FObjInterpretAnim`/`fobj.c` does
   (opcode nibbles, run lengths, fraction-bit quantized values and
   tangents, interpolation kinds: none, constant, linear, spline with
   tangents, slerp for rotations if used) and sample each joint's local
   transform at integer frames 0..frames (the animation system evaluates at
   the current frame after advancing; document the exact sampling
   convention and verify against the jab capture). Emit per-frame poses
   for every subaction Skirmish consumes (every action the current
   `MatchData` schema holds: jab, tilts, smashes, aerials, dash attack,
   escapes, air dodge, ledge, throws, damage, downs, techs, taunts, the
   Fox specials, walks/runs/jumps/falls where the schema takes pose sets)
   and keep the rest in the sidecar as track data for later.
6. Scripts (`ftaction.c` command table): decode every opcode with its
   width and fields; produce per-frame samples for the flags Skirmish
   models (`allow_interrupt`, jab combo/rapid, smash charge command with
   its frame/hold/multiplier, hurtbox state and airborne-state commands,
   throw flags, clear-hitboxes, cmd-var sets, TransN/loop flags) and the
   hitbox create/clear events (bone, offset, size, damage, angle, base and
   growth knockback, shield damage, element, group/id, clank/rebound
   flags) turned into per-frame active hitbox sets. The 15 raw Attack11
   events must round-trip the fixture exactly.
7. `GrNLa.dat` map data: the collision line/vertex tables with line kinds
   (floor, ceiling, left/right wall), material and platform flags,
   prev/next connectivity, ledge markers, the spawn points (P1..P4 and
   respawn platforms) and blast boundaries/camera bounds from the ground
   parameters -> Skirmish `StageGeometry` plus `stage.floor`, `blast`,
   `spawns`. The bounds must match the FD fixture (blast −246/246/188/−140,
   edges ±85.5657, ledge centres ±88.4735) within binary32.
8. Assemble `MatchData` for Fox versus Fox on FD, validate it with
   Skirmish's `Match::new` (run the Skirmish CLI or a small Rust check
   against the consumer crate; if a field Skirmish requires cannot be
   decoded yet, leave that optional profile out and say so in the
   manifest's decoding status rather than inventing values).

## Verification (all local, no ISO, no emulator)
- Unit tests on synthetic archives for the HSD reader, FObj decoding
  (including every interpolation kind and fraction-bit format), the script
  decoder (opcode widths) and the map decoder.
- Fixture cross-checks: the 44 attributes bit-exact, the 15 Attack11
  events byte-exact, the FD bounds, and the jab hitbox world positions on
  frames 1-17 computed through the exported bones and poses (tolerance a
  few binary32 ulps after documenting Skirmish's bone-matrix convention).
- The exported `match-data.json` loads in Skirmish and runs a scripted
  match without validation errors.
- Then the real-file comparison (Skirmish side) reports its first
  divergent frame; record it in the export manifest.

## Skirmish side (separate task, same day)
- `skirmish-cli make-initialization --match-data <json> --replay <slp>
  --output <json>`: read the game start (random seed, ports, stocks,
  characters, stage id) and produce the `Initialization` JSON (`ports`,
  `seed`, `next_frame` = the first frame, `warmup` empty unless the
  importer already models the pre-frame countdown), rejecting mismatched
  characters/stage.
- A `validate-replay` run over that initialization and an `#[ignore]`d
  test that runs it when `SKIRMISH_FOX_FD_EXPORT` points at the export
  directory, asserting only that the report is produced and printing the
  first divergent frame (parity is measured, not asserted).
- Rename the self-recorded replay tests/docs wording so they are not
  mistaken for parity evidence.

## Durable location for tests (user request, 2026-09-11)
The exporter is generic over `Pl*.dat`/`Gr*.dat`. Milestone 1 exports Fox
and Final Destination; the same command then exports every character and
stage into the durable dataset directory
`/mnt/archive/datasets/melee/skirmish-gameplay/<export-version>/` (with the
complete disc extraction staying at
`/mnt/archive/runs/melee-assets-20260909/disc/files`). Skirmish tests find
it through `SKIRMISH_GAMEPLAY_DATA=/mnt/archive/datasets/melee/skirmish-gameplay/<export-version>`
and, when the variable or directory is absent, skip with a printed message
rather than fail (so the repository never needs the ISO). Never copy the
export into either git repository.

## Compact binary pack: `match-data.bin` (2026-09-12)

`<pairing>/match-data.json` is `skirmish::game::data::MatchData` in plain
JSON and, for `fox-fd` in gameplay export v6
(`/mnt/archive/datasets/melee/skirmish-gameplay/v6-snapshot-20260911/`), is
379,662,106 bytes (~380 MB) -- most of a CI job's wall time against this
export goes to parsing that file. JSON stays the exporter's source of
truth (never regenerated from the binary form, always hand-inspectable);
`skirmish pack convert <json> <bin>` (`crates/cli/src/pack.rs`) adds a
lossless binary sibling, `match-data.bin`, that every loader accepting a
`--match-data` path (`make-initialization`, the `real_parity*` tests' data
discovery, `skirmish_cli::pack::discover_match_data`) prefers automatically
when present, falling back to `.json` unchanged. `skirmish pack verify
<json> <bin>` decodes both independently and asserts they produce the
exact same `MatchData` value -- this is what the packaging step below runs
before trusting a converted pack.

**Format.** An 18-byte header (magic `SKPK`, a format version, a copy of
`MatchData.schema` for a cheap sanity check, and the payload length)
followed by the `MatchData` value encoded as CBOR (RFC 8949, via the
`ciborium` crate) using `MatchData`'s own, unmodified `Serialize`/
`Deserialize` derive -- see `crates/cli/src/pack.rs`'s module doc for why
CBOR, not `bincode`/`postcard`: `MatchData` contains internally tagged
enums and a `#[serde(flatten)]` field, which require a self-describing
format (`deserialize_any` support) that `bincode`/`postcard` explicitly do
not provide, and several `Option` fields use `skip_serializing_if`, which
silently desyncs a positional binary encoding the moment a field is
omitted. CBOR keeps map keys and tags on the wire like JSON does, so the
existing derive round-trips unmodified, while still writing every number
as fixed-width binary (an `f32` is 4 raw IEEE-754 bytes, never re-parsed
decimal text) with no text-syntax overhead.

**Measured (fox-fd, gameplay export v6, this machine, release build unless
noted; a shared, noisy machine, so treat the load-time numbers as
directional):**

| | `match-data.json` | `match-data.bin` |
|---|---|---|
| Size | 379,662,106 bytes | 62,931,104 bytes (16.6%, 6.03x smaller) |
| In-process decode into `MatchData` (interleaved runs, min of 9) | ~0.4-1.3 s | ~0.5-0.6 s, consistently 1.2-1.4x faster |
| `make-initialization` + `validate-replay` wall time | ~1.8 s | ~1.8 s, unchanged |

The end-to-end `make-initialization`/`validate-replay` pipeline time is
essentially unchanged because its dominant cost is elsewhere: `make-
initialization` still writes a full `Initialization` (JSON, embedding the
entire `MatchData` again) that `validate-replay` reads back, and neither of
those steps changed format in this batch -- only the `--match-data` input
itself did. The `.bin` sibling's win is the ~6x smaller file (less to
store, less to download in CI) and a real but modest decode-time
improvement, not a change to the whole pipeline's cost.

**Packaging.** `tools/package_gameplay_export.py --skirmish-cli <built
skirmish binary>` now converts every pairing's `match-data.json` to
`match-data.bin` (into a temporary staging directory; the source export
directory itself is never modified) and packages only `match-data.bin`,
each pairing's own `manifest.json`, and the top-level `manifest.json`/
`rules.json`/`stages/*.json` -- the top-level duplicate `match-data.json`
(historically also written at the export root, identical to a pairing's
own copy) and any other large sidecar dump are dropped, since nothing
downstream reads them. `.github/workflows/system-tests.yml`'s extraction
step is unchanged: it just untars whatever the lock file points at, and
`real_parity*`'s data discovery already prefers `.bin` over `.json`,
whichever the tarball contains.

