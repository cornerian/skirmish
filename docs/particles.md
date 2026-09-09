# Procedural WESL particles

Particle presentation uses the native renderer's window and headless paths.
The caller supplies current world position, half size, rotation, linear RGBA,
normalized age and a visual seed. It retains ownership of emission, attachment,
velocity, collision, pause, removal and gameplay RNG. The preview's two-second
loop is synthetic and is not Melee's particle simulation.

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo run --locked -p renderer --bin skirmish-renderer -- --particle smoke --no-audio
cargo run --locked -p renderer --bin skirmish-renderer -- --particle smoke --particle-age 0.35 --headless /tmp/smoke.png
cargo test --locked -p renderer --lib gpu_particles -- --ignored --nocapture
```

`WindowRenderer::set_particles` and `render_particles_headless` accept the same
samples. Empty input clears the particles. Batches above 8,192 or containing
nonfinite values, invalid sizes or colors are rejected before replacing the
previous batch. Samples outside their open lifetime interval are invisible.

## Rendering and cost

Six GPU-generated vertices form each camera-facing quad. One 56-byte record per
live particle is uploaded into a persistent 448 KiB instance buffer. CPU staging
vectors reserve the capacity once. Sorting is allocation-free and back to front
by view depth with an input-index tiebreaker. Smoke uses one instanced draw,
premultiplied alpha and depth comparison without depth writes. There are no
textures, compute dispatches, geometry shaders, baked frames or per-particle GPU
allocations. Noise is capped at three bands and attenuated above the pixel
footprint. Transparent scene meshes currently draw before particles: their
mutual transparency ordering is not solved by the particle sort.

This is a bounded rendering design, not a measured frame-time guarantee. GPU
cost depends on projected area and overlap; overlapping large quads still cost
fill rate. The preview permits checking distant and enlarged effects by zooming.

## Effect coverage

| Effect | Source evidence | Redesign | Status |
| --- | --- | --- | --- |
| Smoke puff | skirmish-assets `effects/smoke/turbulent_smoke_puff`, EfCoData offsets 232800 and 1021600 | Expanding lobed density, seeded filtered turbulence, lit gray body and lifetime fade | Implemented; runtime effect IDs unverified |
| Fire | EfCaData particle frames at offsets 0xcf00, 0xdf00, 0xef00, visually inspected in the existing decoded export | Tapered silhouette, upward noise advection, hot core, cool edge and dissipating tip | Implemented redesign; emitter timing remains caller-owned |
| Glow | EfCaData offset 0x9c0 and aliases in particle_sources.jsonl | Radial core, halo and filtered rays with additive light | Implemented visual redesign |
| Embers | files/EfCaData.dat offset 0x1a00; full aliases in particle_sources.jsonl | Analytic embers appearance and lifetime fade | Implemented visual redesign |
| Sparkles | files/EfCaData.dat offset 0x4a20; full aliases in particle_sources.jsonl | Analytic sparkles appearance and lifetime fade | Implemented visual redesign |
| Impact | files/EfCaData.dat offset 0x4e60; full aliases in particle_sources.jsonl | Analytic impact appearance and lifetime fade | Implemented visual redesign |
| Vortex | files/EfCoData.dat offset 0x11d00; full aliases in particle_sources.jsonl | Analytic vortex appearance and lifetime fade | Implemented visual redesign |
| Lightning | files/EfCoData.dat offset 0x15d00; full aliases in particle_sources.jsonl | Analytic lightning appearance and lifetime fade | Implemented visual redesign |
| Shard | files/EfCoData.dat offset 0x17d20; full aliases in particle_sources.jsonl | Analytic shard appearance and lifetime fade | Implemented visual redesign |
| Shockwave | files/EfCoData.dat offset 0x20d60; full aliases in particle_sources.jsonl | Analytic shockwave appearance and lifetime fade | Implemented visual redesign |
| Energy Burst | files/EfCoData.dat offset 0x48da0; full aliases in particle_sources.jsonl | Analytic energy burst appearance and lifetime fade | Implemented visual redesign |
| Glint | files/EfCoData.dat offset 0x511e0; full aliases in particle_sources.jsonl | Analytic glint appearance and lifetime fade | Implemented visual redesign |
| Arc | files/EfCoData.dat offset 0x521e0; full aliases in particle_sources.jsonl | Analytic arc appearance and lifetime fade | Implemented visual redesign |
| Star | files/EfCoData.dat offset 0x531e0; full aliases in particle_sources.jsonl | Analytic star appearance and lifetime fade | Implemented visual redesign |
| Explosion | files/EfCoData.dat offset 0x8cb40; full aliases in particle_sources.jsonl | Analytic explosion appearance and lifetime fade | Implemented visual redesign |
| Snowflake | files/EfCoData.dat offset 0xa93c0; full aliases in particle_sources.jsonl | Analytic snowflake appearance and lifetime fade | Implemented visual redesign |
| Streak | files/EfCoData.dat offset 0xb40c0; full aliases in particle_sources.jsonl | Analytic streak appearance and lifetime fade | Implemented visual redesign |
| Bubble | files/EfCoData.dat offset 0xda560; full aliases in particle_sources.jsonl | Analytic bubble appearance and lifetime fade | Implemented visual redesign |

The source texture SHA-256 values for those two aliases are
`f43fd5bcaff5ddb9bc5647a73fc76dca39bc4eee4db1e89c6b51233d36aae80f` and
`41dc2940848018561251f720e446e5d10af3cd64203f978125c59f85f72088b3`.
The asset catalog identifies these visually as smoke-like; it does not identify
their runtime names. This shader intentionally creates new wisps. The asset
project's original approximation metrics do not validate this new shader.

The source catalog `crates/renderer/particle_sources.jsonl` retains 337 unique
frames and all 593 aliases from the existing decoded export, visually classified
into 28 families. `particle_catalog::effect_for_texture` resolves a decoded RGBA
hash to an implemented shader family at material-load time. An unimplemented
family or unknown texture returns `None`. The catalog contains source identities
and visual classifications, not copied textures or original runtime names.

Remaining families are pending. Melee's source distinguishes
generator effects, animated mesh effects and composite spawns. Direct generator
ranges alone are not a verified inventory of actual bank entries. Do not label a
generic shader or unknown ID as a completed reconstruction. The native runtime
has not yet translated the HSD emitter bytecode, and this renderer does not
substitute for that translation. Builds and checks require no disc or emulator.

The offline audit command is
`bun crates/renderer/particle_inventory.ts /path/to/melee /tmp/particle-inventory.json`.
At the pinned revision it finds 579 selected API references and 339 distinct
literal dispatch/generator/bank keys. These are source references, not 339
distinct particle appearances: data-driven entries remain outside that count.
The audit preserves dynamic expressions and source locations for follow-up.
