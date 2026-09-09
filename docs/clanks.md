# Clash and rebound kernels

`fighter::clank` ports the ordinary gameplay mutation of
`ftColl_8007699C`, type-3 `lbColl_80008688` victim registration and the scalar
parts of `ftCo_80099D9C`, `ftCo_Rebound_Phys` and `ftCommon_800804A0`.
The native match does not yet dispatch these kernels. Its simultaneous-jab
conformance case remains explicitly unimplemented.

`eligible` checks the supplied grounded state, active/clank/ground-target flags
and opponent history. Callers must supply ordinary non-Catch, non-Inert,
non-Slash fighter hits and exclude capture restrictions, fighter-hit disabling
and bypass flags. They confirm swept capsule overlap before `resolve_pair`.
The original parent scan visits each fighter pair once in entity order, with
the later fighter's hit slots outside the earlier fighter's candidate slots.
The kernel deliberately does not reproduce that whole collision scheduler.

`resolve_pair` receives the caller's existing pending responses and second-side
candidate bitmap. It compares truncated cached, already-staled damage using the
explicit common-data `damage_gap`. A damage difference exactly at that threshold
leaves the stronger side unsuppressed. Suppressed groups register the opponent
on every active same-group slot. Registration fills the first empty victim slot,
then uses the original twelve-entry replacement ring. Duplicate registration
preserves its timer and cursor. A clash does not write the stale-move queue.

The second-side mutation occurs first. The return value reports first-side
suppression and tells the caller whether to stop that slot scan. Rebound is
independent of clank eligibility. Only a strictly greater integer response
damage replaces pending hitlag/rebound data; nonzero fractional damage below one
produces a response damage of one, while the priority comparison still uses zero.
Zero damage and equal or weaker subsequent responses retain old outputs.

`rebound` accepts positive duration, explicit animation length, push coefficients
and surface friction multiplier. It returns the animation rate, raw impulse and
ground acceleration. The source enters ReboundStop after collision response
priority, waits through hitlag, then enters Rebound at the next animation callback.
The raw impulse controls the first friction callback; zero, including negative
zero, takes the friction branch immediately. Ground acceleration is added to
ground velocity after self-velocity projection, so it is not an immediate position
offset. Animation poses, callback ordering, action cleanup and checkpoints of the
complete match remain responsibilities of the future integration.

Parameters must stay within defined C integer conversion ranges; invalid numeric
inputs return errors without mutating the pair or bitmap. No common-data values
or fighter animation resources are supplied implicitly.

`tests/clank_differential.rs` compares full original-C mutations and rebound
outputs, including threshold boundaries, old responses, fractional damage,
duplicate timers, replacement order and signed zeros. `tests/clank.rs` composes
staling, clashes, hitlag and rebound using explicit synthetic coefficients. These
are kernel contracts, not whole-match or authentic-resource fidelity claims.

Rendering/audio outputs are absent. The unsupported Slash-vs-Slash sound branch
consumes shared `HSD_Randi(3)` and must retain that consumption when implemented.
The effect engine's global RNG behavior is not validated by these scalar tests.

Verification commands, counts and log hashes for this prerequisite batch are
archived in `/mnt/archive/runs/skirmish-clank-kernels-20260909/manifest.json`.
