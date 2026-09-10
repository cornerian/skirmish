# Resource integration

Resource conversion is handled by the separate `skirmish-assets` project. It
owns converted export organization, vector conversion where appropriate, and future
procedural artwork such as smoke and particles. Skirmish's current priority is
native headless gameplay and physics. These are consumer requirements, not a
final export schema or a request to rearrange the resource project's directories.
The current experimental `MatchData` fixture format is not the full asset format.
The [canonical asset tree](asset-tree.md) specifies categories, current consumers,
and completeness requirements, with an exhaustive linked source inventory.
The optional in-game importer installs original disc files; conversion remains
separate from that player-facing installation step.

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
Skirmish must not depend on a particular developer's checkout path, an extraction
tool at runtime, or an ISO/DOL. The optional [in-game importer](asset-import.md)
now installs original files, and `assets::AssetBundle` resolves their exact disc
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
