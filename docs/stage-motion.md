# Stage motion

Optional `stage.motion` turns explicit collision geometry into a deterministic
sampled stage. Each track names a nonempty stable line range and a cyclic list
of absolute affine transforms. Frame zero is the initial pose. The motion clock
pauses during the match countdown and advances once before each playing frame;
`State.stage.frame` checkpoints that clock. `Match::stage_geometry` reconstructs
the current lines and conservative joint bounds from immutable resources, so a
headless rollout can inspect the collision mesh without rendering state.

A supported fighter retains its collision-line ID. After ordinary self motion,
and during hitlag when still grounded, the scheduler remaps the fighter position
from that line's previous endpoints to its current endpoints. This is the
`mpGetSpeed` call site in `Fighter_procUpdate`. `collision::stage::remap_point`
preserves the complete `mpRemap2d` mixed f32/f64 evaluation, endpoint clamping
and degenerate-line branch. `stage_differential` compares arbitrary binary32
inputs with the pinned original C body. Airborne fighters stop inheriting the
line motion and can land against the current transformed mesh.

Collision response also compares the previous and current sampled planes. An
inward-moving wall or ceiling can constrain the ECB, and a rising solid floor
can land an otherwise stationary airborne fighter. Opposing walls invoke
horizontal squeeze; simultaneous ceiling/floor response invokes grounded
vertical squeeze. A one-way floor cannot catch an upward fighter, and motion
along a plane does not capture a fighter already behind it. See the
[ECB response profile](ecb-response.md).

Transforms must keep every line finite, bounded and directed according to its
declared surface kind. Tracks cannot overlap. The native fixture uses invented
samples; authentic stage tracks must arrive with resource provenance. This
profile does not yet reclassify dynamic lines after rotation, run stage-object
callbacks or reproduce the complete connected-corner adjacency walk. Those
behaviors need more source translation plus independent stage traces.
