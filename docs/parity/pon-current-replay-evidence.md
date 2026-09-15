# Current Pon/Fox replay evidence

Run date: 2026-09-15. This is a live run against the archived v10 gameplay
pack and the independent `fox-fd.slp`; it is not copied from a baseline report.

Inputs and provenance:

- replay: `tests/fixtures/slippi/parity/fox-fd.slp`, SHA-256
  `87971fc4608e577fe5a814fc3a04dee4c0d82f99bc9a54ee16797005c5a9085e`;
- gameplay lock: `v10`, tarball SHA-256
  `e4189b85479921c1fb77d6a9bea124948e803c2c716fe6838db76a936d087e0a`,
  35,945,854 bytes;
- pairing pack: `v10-snapshot-20260913/fox-fd/match-data.bin`, SHA-256
  `04fdce420a6496f4a9f75d4dada2b207e2160ff08d080b3d0e3b6bc7b5ff229e`;
- pack manifest source: decomp `0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`,
  exporter `edd65309e21441b92cefe03e37b53e8b4226ad55`;
- Pon stdlib archive: `pon-stdlib-final-sorted.tar.gz`, SHA-256
  `5c7ce12a21f4d5ae49a8cb2c425911bc4de863f427e0ff174fb74ff7bf63639c`.

Command (with the required Cargo environment) was the real replay test:

```text
SKIRMISH_GAMEPLAY_DATA=/mnt/archive/datasets/melee/skirmish-gameplay/v10-snapshot-20260913 \
  cargo test --locked -p skirmish-cli --test real_parity -- --nocapture
```

The current Pon-configured CLI built successfully, but no replay prefix could
start. `make-initialization` failed for all nine recordings while loading the
preferred compact `fox-fd/match-data.bin`:

```text
CBOR decoding of MatchData failed
Semantic(None, "unknown field `neutral`")
```

The same live command against the sibling v10 JSON pack also failed before
initialization, with `unknown field `down`` at the pack's reported parser
location. Thus the first divergence is a startup/resource-schema failure:

| item | expected | actual |
| --- | --- | --- |
| frame | no frame (initialization never completed) | no observation |
| field | current `MatchData` schema accepted by the CLI | serialized v10 Fox data contains `neutral` in CBOR and `down` in JSON |
| outcome | independent input-to-observation prefix begins | parser rejects the archived pack |

Classification: missing/incompatible resource data (pack schema drift). This
run provides no frame-level parity claim and does not justify changing any
baseline or pack.

## Schema isolation

The rejected keys are specifically under `fighters[0].specials`: the archived
object has `character`, `down`, `neutral`, `side`, and `up`. A temporary JSON
projection that removed only `specials.down` still failed on `specials.neutral`;
removing both then failed on `specials.side`. Removing all four special
resource keys allowed `make-initialization` to complete, confirming that the
failure is at the `Specials` resource boundary rather than in smash, tilt, or
throw data (which also have direction keys named `down`). The projection was
written under `/tmp` and the source pack was not modified.

The smallest safe compatibility boundary is the `Specials` deserializer
itself: split the required `character` field from the remaining dynamic keys,
then pass every remaining subtree to `Resources::new`. This preserves all
resource values and typed attack indexing in the existing schema. Unknown keys
must remain hard errors with their source path; silently dropping a resource
would turn a startup failure into an unverified parity run. No pack conversion
or version adapter is needed for this shape.

## Post-fix live result

After the narrow `Specials` deserializer fix, the original compact v10 pack
was rerun unchanged. `make-initialization` completed for `P1`/`P4` at replay
seed `3778252302` and `next_frame = -123`. The subsequent live
`validate-replay --report` startup failed before writing a report or stepping
any frame:

```text
invalid native match data: invalid specials resource:
script returned invalid data: resource validator `move_1.validate` returned false
```

This is the current first reachable failure after pack loading. Frame is
`N/A` (zero observations); field is `specials.move_1.validate`; expected is a
valid resource-validation result (`true`), actual is Pon callback result
`false`. The replay SHA-256, v10 tarball SHA-256, pairing-pack SHA-256, source
revisions, and Pon archive SHA-256 are the hashes listed above. The failure is
classified as resource validation / unimplemented or incorrect class-script
behavior, and no baseline was changed.

The generic command-trace ABI fix accepts the archived integer-valued command
cells after JSON-to-native conversion, with focused coverage for integral,
fractional, negative, infinite, and bounded values. A subsequent full replay
rerun could not compile because an unrelated concurrent edit in
`src/game/simulation.rs` currently has an unclosed delimiter around
`update_ground_physics` (compiler locations 1968--2019 and EOF 2343). No
replay result or baseline change was inferred from that compile blocker.

Temporary ownership isolation narrowed `move_1.validate` to the archived
`fighters[0].specials.side` subtree: deleting only `specials.neutral` still
returned `move_1.validate = false`, while deleting only `specials.side` allowed
validation to proceed. That derived run reached the real replay and then
mismatched at frame 5, P1 `position.x` (`expected 0xc1edec00`, `actual
0xc1edec01`), so it is evidence of validator ownership only; it is not a
baseline candidate because the resource was deliberately removed. The live
full-pack failure remains `specials.move_1.validate` returning false, with the
side subtree intact. Narrowing the validator's internal field/value requires
inspection of the Fox class validator by the Fox-special execution owner.

## Exact side validator field

Inspection of the current generic `command_trace` host shows why the side
validator returns false. Its optional trace checks
`side.script.dash.ground` and `.air`. In the archived v10 resource,
`ground.cmd_vars[2][2]`, `ground.cmd_vars[3][2]`, and the corresponding air
rows contain the integer JSON value `1`; all other entries are `null`. The
generic `json_to_native` bridge currently converts every JSON number to
`NativeValue::F32`, so these values arrive at `command_trace` as `F32(1.0)`.
The generic command-trace predicate accepts `None` or bounded
`NativeValue::Int` only, and therefore rejects the first numeric command row.

Responsible fix boundary: the generic resource bridge/command-trace contract,
owned with lifecycle host code, should preserve integer-valued command cells
as `Int` (or explicitly accept integral finite `F32` values) while retaining
the existing bounds check `0..=0xFFFFFFFF`. This is not a Fox resource-value
error and does not justify changing the archived pack or disabling the
validator. The current full-pack run consequently has no frame observation;
the temporary side-script omission reaching frame 5 remains diagnostic only.

## Fresh artifact replay result

The CLI artifact was rebuilt with the mandated Cargo environment and verified
by Cargo's compiler-artifact record as `/tmp/skirmish-pon-target/debug/skirmish`
(canonical path `/mnt/shared/tmp/skirmish-pon-target/debug/skirmish`). Its
binary no longer contains the temporary diagnostic or the old `.is_integer`
implementation. Running that artifact against the unchanged original v10
initialization produced the first real observation divergence:

```text
outcome: mismatch
frame: 5
checked_frames: 128
port: P1
field: position.x
expected: 0xc1edec00
actual: 0xc1edec01
resources_sha256: 5dcf0815edc3c47f7f318521fbd4b4bc1fa7d5c3a5685a55f8068c5865e5286b
initialization_sha256: cf2d495cf390e17f3b700ec5181bd1cf4ff3cda67b7593e8e80abf47cb90b40e
```

This is an actual full-pack input-to-observation result; no resource subtree
was removed and no baseline was ratcheted.
