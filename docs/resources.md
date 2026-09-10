# Resource integration

Resource conversion is handled by the separate `skirmish-assets` project. It
owns converted export organization, vector conversion where appropriate, and future
procedural artwork such as smoke and particles. Skirmish's current priority is
native headless gameplay and physics. These are consumer requirements, not a
final export schema or a request to rearrange the resource project's directories.
The current experimental `MatchData` fixture format is not the full asset format.
The [canonical asset tree](asset-tree.md) specifies categories, current consumers,
and completeness requirements, with an exhaustive linked source inventory.
The independent extraction crate can install original disc files; conversion
remains separate from that storage step.

## Gameplay and presentation

| Resource | Consumer | Required behavior |
| --- | --- | --- |
| Fighter/common attributes and action command streams | Gameplay | Preserve numeric precision, command fields, scheduling and source revision |
| Bone hierarchy, bind transforms and gameplay animation tracks | Physics | Preserve bone identities, transform order, frame timing and interpolation semantics |
| Hitboxes, hurtboxes, ECB and stage collision topology | Physics | Preserve attachments, dimensions, flags, connectivity and gameplay timing |
| Vector artwork, materials, textures, decorative geometry | Presentation | May evolve independently of collision data |
| Decorative smoke, particles and shader effects | Presentation | Consume simulation events/state without affecting gameplay or its RNG |
| Projectiles, hazards or effects that cause gameplay changes | Gameplay and physics, with optional presentation | Their collisions, lifetimes and other gameplay changes must execute headless |

Vectorizing an image or simplifying a visual mesh does not define a replacement
for collision geometry. Physics needs the original numeric geometry and bone
motion semantics even when the renderer displays a different representation.
Visual effects may use their own randomness; disabling them must leave gameplay
state, event ordering and gameplay RNG consumption unchanged.

## Export information needed by consumers

Exports should identify resources independently of directory placement. A
versioned manifest can resolve stable resource IDs to paths relative to the
bundle root, allowing the producing task to organize directories sensibly.
Skirmish must not depend on a particular developer's checkout path or an ISO/DOL.
The independent `extraction::AssetBundle` API can install and resolve exact disc
identifiers with path containment and integrity checks. It does not implement a
converted gameplay/visual bundle resolver; that integration remains outstanding.

For gameplay resources, retain:

- Source game revision, provenance, content hashes and explicit coverage gaps.
- Coordinate axes, handedness, units, scale conventions and transform order.
- Bone ID mappings, parent relationships, local versus world transforms, bind
  transforms and joint flags affecting gameplay poses.
- Frame origin, tick rate, animation rate/interpolation rules and the relationship
  between action commands and pose samples. Preserve original tracks/commands
  when a converted representation has not yet been validated.
- Complete per-frame hurtbox eligibility samples whenever an action command
  changes enabled, disabled or intangible state; do not emit partial arrays.
- Original numeric precision and relevant packed command fields; presentation
  conversion must not silently round or reinterpret gameplay data.

Missing resources stay explicit. Existing synthetic fixtures exercise the match
pipeline until complete exports can replace them and pass integration checks.
The [earlier native-data audit](native-data.md) remains useful as a small source
of decoder regressions; it is not a competing resource extraction pipeline.

When usable exports arrive, integrate one fighter pair and stage first, resolve
their gameplay resource references, and validate bone poses, contacts and action
timing before treating that bundle as a faithful matchup. Visual resources can
be connected later without becoming a headless runtime dependency.

## Exact presentation visual exports

The `skirmish-visual-v1` scene schema carries an additive contract for exact
presentation identity. A visual export that opts in must satisfy all of it;
the loader and `renderer::presentation::VisualPresentationBinding` reject
partial metadata instead of guessing, and never fall back to bare offsets or
filenames for exact bindings.

| Field | Requirement |
| --- | --- |
| `resources[].id` | Archive ID including its extension (`MnMaAll.dat`). |
| `resources[].sha256` | Lowercase SHA-256 of the complete original DAT. It must equal the presentation manifest's `resource.sha256`. |
| `resources[].offset_spaces` | `{"joints", "materials", "textures"}`, each `data_section` or `file`, stating the coordinate space of every descriptor identity below. The manifest's `visual_offsets` must agree; a mismatch is a bind error. |
| mesh `joint` | Owning JObj descriptor offset in the joints space. |
| mesh `resource_id`, `dobj_index` | The owning archive and the zero-based ordinal of the DObj in that JObj's `next` list. One JObj offset belongs to exactly one resource. |
| mesh `geometry_space` | `joint_local` when positions are relative to the owning joint (rigid parts), `world` when the exporter baked the joint chain (envelope-skinned parts). Required for every exact mesh; `joint_local` also requires the owning joint's complete `flags`/`local`/`world`/`inverse_bind` pose. |
| material `material_offset` | MObj descriptor offset in the materials space, plus the existing `render_mode` and pixel-engine fields. |
| material `textures[i].tobj_offset`, `tobj_index` | TObj descriptor offset in the textures space; `tobj_index` must equal the stage ordinal `i`. |

The consumer joins these occurrences to a `skirmish-presentation-v1` manifest
bound over the same archive bytes. Sampled joint visibility and material color
reach exact GPU draw occurrences. Native joint-local transforms are routed only
for `joint_local` draws; world-baked draws retain them as
`BakedWorldGeometry`, joints without draws as `UnmappedJointLocal`, and
joint-local draws as `UnsupportedJointLocal` until the driver composes native
local deltas into world matrices for the renderer's per-draw joint transform.
Texture image, UV, and TEV register updates remain unsupported.

`tests/presentation_binding.rs` exercises the whole chain with a pinned
`MnMaAll.dat` rotation curve in a synthetic archive. Its blocked acceptance
test binds the real export from `MNMAALL_DAT` and `MNMAALL_SCENE` against
`tests/fixtures/melee-ui/mnmaall-visual-contract.json`.

The resource project's current MnMaAll scene declares none of `resources`,
`resource_id`, `dobj_index`, `tobj_index`, `offset_spaces`, or
`geometry_space`, and writes every part in world space, so the interactive
host cannot build exact bindings from it yet. Its reference report already
carries `dobj_offset`, `material_offset`, `tobj_offset`, `local_positions`,
`matrix_indices`, and the source hash, so the producer patch is narrow:
`src/mesh.rs` (`Part` gains the DObj ordinal and its vertex space; `Mesh`
gains the source hash and offset spaces), `src/visual.rs`
(`Mesh::write_visual_scene_scales` emits the fields above and joint-local
positions for single-matrix parts), `tools/model_recipes.py` (carries the
ordinal, hash, and spaces into recipes), and the regenerated
`assets/ui/menus/mnmaall/models/model.rs`. That repository is not modified
from here without explicit authorization.
