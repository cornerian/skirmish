# Ordinary clashes and rebound

`fighter::clank` ports the ordinary gameplay mutation of
`ftColl_8007699C`, type-3 `lbColl_80008688` victim registration and the scalar
parts of `ftCo_80099D9C`, `ftCo_Rebound_Phys` and `ftCommon_800804A0`.
`game::clank` connects these kernels to the native two-player match through an
optional, explicit `rules.clank` profile. The equal-grounded-jab conformance case
is enabled. This is the ordinary grounded non-Slash branch at upstream revision
`0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`, with experimental sampled resources.

The profile supplies response/push coefficients, hitlag cap and the Rebound
callback's floor friction multiplier. Each fighter supplies `rebound.animation_length` and complete
integer-frame `rebound.poses`; hitboxes supply separate `clank` and `rebound`
flags. Missing profiles leave older fixture flags false. No authentic character,
common-data coefficient or animation is inferred. The schema cannot encode
Catch/Inert/Slash elements, item ownership, fighter-hit disabling or bypass flags;
such native resources must not be labeled `ordinary_grounded_non_slash`.

`eligible` checks the supplied grounded state, active/clank/ground-target flags
and opponent history. Callers must supply ordinary non-Catch, non-Inert,
non-Slash fighter hits and exclude capture restrictions, fighter-hit disabling
and bypass flags. They confirm swept capsule overlap before `resolve_pair`.
The original parent scan visits each fighter pair once in entity order, with
the later fighter's hit slots outside the earlier fighter's candidate slots.
The native dispatcher uses player 1's slots outside player 0's candidates, once
per pair, before either player's shield/hurtbox checks. Both must be grounded;
airborne attacks continue through the ordinary contact path. Hitlag alone does
not exclude a hitbox from the clash scan. Full entity/capture ordering is unported.

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
offset. The match retains the contact pose through ReboundStop hitlag, clears
active hitboxes on rebound entry, then evaluates the supplied recovery poses
until Wait. Recovery completion runs before destination input dispatch. Incoming
body damage or shield stun overrides pending rebound; clank hitlag otherwise
takes precedence over dealt-damage hitlag. The native checkpoint includes the
victim slots, frozen pose, animation clock/rate and pending ground acceleration.
Rebound friction scales the already-clamped acceleration before projection when
the supplied floor multiplier is below one. General per-surface friction for
other native actions remains separate from this callback's explicit parameter.

With this profile, shield/hurt/clash contacts share victim history. The sampled
resource contract disables absent or changed slots first, then creates slots in
ascending order, copying history from surviving same-group slots. A fully absent
group is fresh when created again. The format does not express arbitrary script
command reissues or a different within-frame command order. Other groups remain
eligible after a clash; a non-rebounding hit keeps its attack action and history.

Parameters must stay within defined C integer conversion ranges; invalid numeric
inputs return errors without mutating the pair or bitmap. No common-data values
or fighter animation resources are supplied implicitly.

`tests/clank_differential.rs` compares full original-C mutations and rebound
outputs, including threshold boundaries, old responses, fractional damage,
duplicate timers, replacement order and signed zeros. `tests/clank.rs` composes
staling, clashes, hitlag and rebound using explicit synthetic coefficients. These
are kernel contracts. `tests/game_clank.rs` exercises the native pipeline, using
wholly synthetic attacks and recovery resources. Neither suite establishes whole
original-game equivalence or authentic-resource fidelity.

Rendering/audio outputs are absent. The unsupported Slash-vs-Slash sound branch
consumes shared `HSD_Randi(3)` and must retain that consumption when implemented.
The effect engine's global RNG behavior is not validated by these scalar tests.

Verification commands, counts and log hashes for this prerequisite batch are
archived in `/mnt/archive/runs/skirmish-clank-kernels-20260909/manifest.json`.
The native integration's full debug/release verification is recorded separately
in `/mnt/archive/runs/skirmish-clank-gameplay-20260909/manifest.json`.
