# Native data available for a first matchup

Full resource extraction is now owned by the separate `melee-assets` task.
The fixtures below remain focused regression evidence; this task should consume
that project's native exports rather than duplicate its extraction work. See
the [consumer requirements](resources.md) for gameplay/presentation boundaries.

The small [source fixtures](../tests/fixtures/native-data/sources.json) contain
Fox movement attributes, raw jab commands, captured jab hitbox positions, and
Final Destination boundary constants. They can feed native decoders and focused
tests immediately. They are incomplete resources and do not establish an accurate
Fox match. All checked-in fixtures are text; building, running, and testing needs
no external executable, disc image, emulator, or network download.

| Fixture | Usable content | Coverage limit |
| --- | --- | --- |
| `fox-native-subset.json` | 44 attributes mapped by byte offset to the pinned upstream `ftCo_DatAttrs`, with original names, numeric values and exact binary32 representations; all 15 raw events for `Attack11` | No skeleton, bind pose, animation transforms, hurtbox capsules or environment collision box |
| `fox-jab1-captured.csv` | Exact libmelee rows for character 1, action 44, frames 1–17; two active hitboxes on frames 2–3 | Captured positions relative to the fighter, not bone-local offsets or arbitrary pose evaluation |
| `fox-jab1-processed.json` | Publisher's complete processed `jab1` entry | Retained for discrepancy tests; parsed scales and timing are not authoritative |
| `final-destination-bounds.json` | Four blast boundaries, standing edge x and ledge-hanging center x | No floor height, collision vertices, segment connectivity, walls or ledge rules |

The Fox dump is published by
[pfirsich's data service](https://melee.theshoemaker.de/dat-dumps/Fox.json).
The extracted attributes include weight 75, gravity approximately 0.23,
terminal velocity approximately 2.8, maximum run velocity approximately 2.2,
and two total jumps. The fixture preserves the original precision; use its
`f32:` bit strings when bit identity matters. Field names come from
[the pinned upstream layout](https://github.com/doldecomp/melee/blob/0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9/src/melee/ft/types.h#L686),
because some publisher labels differ or are unknown.

The captured data and stage constants are pinned to libmelee commit
`1da979657122facd0750ea99cf6858255e198326`. Its
[capture code](https://github.com/altf4/libmelee/blob/1da979657122facd0750ea99cf6858255e198326/melee/framedata.py#L935)
subtracts fighter position from hitbox coordinates. Its character `size` value
is a circular target approximation, so it is deliberately excluded from the
native hurtbox data. Its
[stage table](https://github.com/altf4/libmelee/blob/1da979657122facd0750ea99cf6858255e198326/melee/stages.py)
gives Final Destination blast boundaries of left −246, right 246, upper 188,
lower −140. Standing edge x is approximately 85.5657 and hanging center x is
approximately 88.4735, with symmetric negative values. These observations do
not define a complete stage collision mesh; missing geometry is explicitly null.

Three differences must be resolved before combining these sources into a
validated action:

- The publisher's
  [event decoder](https://github.com/pfirsich/meleeDat2Json/blob/master/meleedat2json/events.py)
  divides raw size and offsets by 255. The pinned
  [upstream action handler](https://github.com/doldecomp/melee/blob/0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9/src/melee/ft/ftaction.c#L323)
  multiplies by the binary32 constant `0.003906f`. For jab's raw size 852,
  these produce approximately 3.34117647 and 3.32791185 respectively. The
  latter agrees with libmelee's captured radius after binary32 conversion.
  Preserve raw commands and apply upstream semantics, including its coordinate
  mapping from the command's z offset to the runtime bone offset's x axis.
- The publisher's raw command enables IASA at frame 16; libmelee's CSV first
  reports IASA on frame 8. Both report active hitboxes on frames 2–3. The
  interruptibility disagreement remains unresolved.
- The publisher reports animation `numFrames` 18 and processed `totalFrames`
  17. These are distinct source conventions, not a proven common frame clock.

The manifest records source URLs, available revisions, retrieval metadata,
whole-source hashes, extraction selections, fixture hashes, conflicts, and
coverage gaps. Publisher JSON is content-pinned by SHA-256 because its URLs
are unversioned. The original libmelee LGPL license is retained. The inspected
publisher pages/repositories specify no dataset license; the fixtures contain
small attributed numerical excerpts and command records, not character models
or animation assets. The historical `PlFx.dat` label is source metadata only.

A complete native Fox resource still needs verified bone parents and bind
transforms, animation tracks for every supported state, hurtbox capsules and
bone attachments, environment collision boxes, and complete relevant action
scripts. A complete stage needs its collision topology and ledge metadata.
The source game revision and observation timing must also agree. Replay
validation additionally starts from a complete native simulator checkpoint;
these partial resources do not reconstruct hidden state. Experimental match
fixtures should remain explicitly identified until these gaps are closed.
