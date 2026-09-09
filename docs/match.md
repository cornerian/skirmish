# Native match slice

`arena` now runs an **experimental two-player match** from reset through
countdown, per-frame movement, a bone-animated jab, contact, damage, hitlag,
knockback, stock loss, respawn and termination. The CLI's demonstration uses an
explicitly synthetic dataset, not Fox or a certified Melee matchup. Neither
builds nor execution need an ISO, DOL, emulator, renderer or audio device.

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo run --locked --bin skirmish -- demo-match > /mnt/shared/tmp/skirmish-demo.jsonl
cargo run --locked --bin skirmish -- run-match --data /path/to/native-match.json --inputs /path/to/inputs.jsonl
```

`run-match` consumes one controller pair per JSONL line, from stdin when
`--inputs` is omitted. Each controller has
`buttons` (A=256, X=1024, Y=2048, L=64, R=32), normalized `stick: [x, y]`,
optional `cstick: [x, y]` and optional processed analog `trigger` in `[0, 1]`.
Omitted new channels default to zero. Digital L/R produce full shield pressure
without replacing the stored analog value. Unsupported buttons and nonfinite or
out-of-range channels produce errors. Example:

```json
[{"buttons":256,"stick":[0.0,0.0]},{"buttons":0,"stick":[0.0,0.0]}]
```

A run can cover a bounded input scenario or an entire match. EOF ends the
scenario; inspect the final `phase` to distinguish these cases. Empty input,
malformed lines, extra input after match completion and physics failures never
produce a successful trace `end`. Outputs use the existing strict trace format,
including float bit strings, a native-resource SHA-256 identity and explicit
experimental profile. They can be passed to `compare-traces` or emitted by
`compare-binaries` adapters. Whole-match agreement with original C is **not**
established; two copies of this simulator agreeing is a determinism test.

## State and branching

`Match::step([Controller; 2])` advances one frame. `state()` exposes privileged
simulator state; it is not a player-information observation policy. `reset(seed)`
restarts the match. `checkpoint()` and `restore_checkpoint()` preserve inputs,
positions, velocities, action clocks, stick-age/jump counters, platform skip ID,
ECB bottom-lock timer, hitlag/hitstun, pending DI, elapsed damage
time, tumble eligibility, physical-L/R tech ages, swept hitbox centers, ECB
interpolation history, stage contacts, ledge endpoint ownership/cooldown, stocks,
invincibility, match clock, reserved RNG seed and events. This slice has no
random events and consumes no RNG draws. Checkpoints are opaque in-memory
values, and restoration rejects different resource/rule identities. Persistent
checkpoint encoding is a later versioned interface.

Clones share only immutable native resources. Each branch owns its mutable state;
stepping errors leave it unchanged. The crate implements `replay`'s
`FrameStepper`, so streaming expected observations and counterfactual branches
already use this match implementation. The `validate-replay` command applies
Peppi-imported inputs to real `Match::step` calls from an explicitly initialized
checkpoint. It compares position, facing, percent, stocks and airborne state;
see the [file-backed comparison contract](replays.md) for the required embedded
resources, seed, port mapping, frame label and warmup inputs. Comparison covers
the complete selected suffix and stops at the first mismatch or unsupported
condition. It does not restore hidden state from replay observations or certify
the match's unported Melee behavior. Batched Python/Gym interfaces, rewards and
coaching value estimation remain separate work.

## Physics and execution order

Bone hierarchy evaluation, local/world transforms, parent-scale compensation,
bone-attached capsules, swept capsule intersection, walking and jump launch
arithmetic live in `physics`. The native match schema carries all pose
samples for its jab, including startup/recovery frames. The same evaluated pose
places both hurtboxes and hitboxes; rendering is not involved. Local native
coordinates use +X forward, +Y up and +Z depth; resource imports must convert
source coordinate conventions explicitly.

The experimental scheduler first handles countdown or respawn/freeze counters,
then updates actions and physics for both players, samples their ECBs and resolves
static stage contacts in source-sized movement substeps. It evaluates contact
poses, resolves configured ordinary grounded clashes, and collects remaining
shield/hurtbox contact decisions before applying damage so both
players can trade on the same frame. Hit-group history suppresses repeated
contacts during one attack. With the [clank profile](clanks.md), active same-group
slots instead share explicit victim histories that survive hitlag and clear on
deactivation. Pending rebound yields to incoming body damage and shield stun.
It then resolves stock losses together, checks the
clock and emits events. A timeout compares stocks, then percent; exact ties
finish as a draw. This schedule and these match policies are integration code,
not a claimed translation of the complete original callback graph.

Each fighter supplies `collision_box`: either `fixed` with explicit up/down,
front/back and angle values, or `bones` with six bone indices, source thresholds,
side-height offset and flags. These boxes are separate from hurtboxes. The stage
accepts explicit `geometry.lines` and `geometry.joints`; when absent, its compact
`floor` becomes one directed segment. Lines retain source IDs by array position,
adjacency, surface flags and joint ranges. Floor segments run left to right,
ceilings right to left, left-facing walls bottom to top and right-facing walls
top to bottom. Current geometry is static throughout a match.

Movement subdivision retains the original strict six-unit threshold and ECB
growth checks: exactly twelve units takes three substeps. Grounded fighters
project onto adjacent floor segments and use their normals for ground motion.
Projection preserves the source's small separation bias. Side and ceiling
contacts project the final collision point onto the contacted surface and stop
inward velocity in this experimental profile. Platforms permit upward passage,
descending landings and intentional drops with the explicit locomotion profile.
Pass remembers and skips only its supporting line during every movement substep;
other platforms remain collidable. The next action transition clears that skip.
The drop also applies the source ten-map-callback ECB bottom lock. Full corner
resolution and squeeze response are unported,
although reusable squeeze arithmetic is available in `collision::ecb`.

An active hitbox carries its previous and current world centers. New activation,
a disabled slot or a changed group resets its sweep. Hitlag still updates these
centers, preventing reuse of an old movement segment. Contacts use the source
isotropic capsule solver, including its unusual near-parallel endpoint choice.
The reusable `collision::shield` helper separately ports `lbColl_80006E58` and
the ordinary geometric branch of `lbColl_80007BCC`: transformed hurt volumes
retain directional radii, contact position, overlap and the original broadphase.
It accepts caller-prepared world endpoints and matrices; bone caching, forced
hits remain outside that helper. Ordinary shield gameplay now uses it before
hurtbox contact; see the [shield profile](shield.md). Its C oracle
uses the SDK scalar matrix-vector routine in place of paired-single assembly,
so passing comparisons establish native scalar agreement only.

Damage rules explicitly supply DI limits, angle-361 coefficients and the
knockback replacement window. Optional `rules.damage.displacement` supplies
main-stick SDI thresholds, timing and distances, and the ASDI distance. Fresh
stick motions can displace a victim during positive hitlag; held input produces
one ASDI displacement at expiry, before DI. C-stick has ASDI priority when its
squared magnitude meets the minimum threshold, including equality; otherwise
ASDI uses the main stick. Positive-hitlag SDI and DI use only the main stick.
Static collision response constrains
that displacement while ordinary motion stays frozen. Attacker hitlag does not
install those damage callbacks. Zero hitlag creates no expiry callback. Optional
fighter `armor` supplies two subtraction channels and a minimum knockback;
ordinary armor subtracts the larger channel without changing percent damage.
Grounded launch projection and the complete damage callback
sequence remain unported.

Optional `rules.staling` supplies the nine original common-data penalties and
the debug bypass flag; each supported `Attack` then requires an explicit
`move_id`. The source multiplier starts at 1.0: this ordinary uncharged/unscaled
path has no fresh-move bonus. A ten-slot ring deduplicates move/instance pairs,
while damage scans only the newest nine entries, stopping at an empty slot.
Hitbox activation, a group change or a changed base damage samples its staled
damage. Continuing slots retain that value through contact and hitlag. Percent
and pending damage use the staled float; knockback's attack-damage term retains
the original integer, as in `ftColl_8007ABD0`. The frame-resource representation
does not encode arbitrary same-value damage-command reissues or item ownership.

Stale queues, hitbox damage caches and the match-wide nonzero 16-bit instance
sequence are checkpointed. Native motion transitions allocate identities in the
experimental scheduler's order; the complete original callback allocation order
is not claimed. A KO clears only the deceased player's queue, preserving the
global counter and other players' histories; match reset starts both fresh.

Movement and displacement share the source stick-age timers, sampled once per
active frame including hitlag. When both optional profiles are supplied, their
axis thresholds must agree; inconsistent resources are rejected at load time.
Optional [`rules.damage.floor_response`](damage-floor.md) adds source-gated
neutral techs plus the complete configured DownBound/DownWait/DownStand recovery
chain. Physical-L/R ages are sampled during hitlag and ordinary frames. Damage
at or above its inclusive threshold retains tumble eligibility through
DamageFall until floor contact. Tech rolls, wall/ceiling responses, bounces,
get-up choices and action-specific poses remain separate work.

Optional [`rules.grab`](grabs.md) and per-fighter grab resources add physical-Z
catch entry, sampled bone-attached grab capsules, paired pull/hold states and
forward/back/up/down throws. Captured fighters remain attached through explicit
holder/victim bone anchors. The shared input history applies the original throw
direction priority, and each supplied release event enters the ordinary damage
pipeline. Pair ownership is checkpointed and cleared transactionally on release
or stock loss. Pummels, mash escape, dash/tether variants, throw staling and
capture-contact interference remain unported.

Optional [`rules.ledge`](ledges.md) and per-fighter ledge resources add static
endpoint discovery, bone-attached catch/hang poses, climb, jump, attack, escape,
drop and regrab cooldown. Ledge attack uses the same swept-contact and damage
pipeline as other attacks. Endpoint ownership, stick arming and cooldown are
checkpointed; damage and stock loss clear ownership. The [ledge profile](ledges.md)
documents its explicit resource schema and remaining dynamic-stage and
character-specific limits.

Optional [`fighters[].special`](specials.md) supplies paired sampled ground and
air neutral-special animations. Fresh neutral B dispatch, action priority,
terrain conversion, completion and bone-attached combat all run in the headless
match. The [neutral-special profile](specials.md) records the exact source input
boundary and the character-specific effects and directional specials that are
still pending.

Optional [`rules.rebirth`](rebirth.md) replaces the compatibility grounded
respawn with an airborne entry, exact remaining-frame travel to a supplied
static platform, an invulnerable wait, and input/timeout release to `Fall`.
Both player slots, action/timer state, post-release invincibility and checkpoint
branches are covered by the native match tests. The [rebirth profile](rebirth.md)
records the exact C arithmetic boundaries and remaining dynamic-stage, follower
and action-dispatch gaps.

## Explicit movement data

`fighters[].locomotion` supplies thresholds, stick-age windows, dash/run
coefficients, animation/event durations, crouch/turn timing, ordinary aerial
jump multipliers and platform-drop parameters. No authentic common-data values
are implied. With that data, the scheduler supports Dash/Run/RunBrake, standing
Turn, Squat/SquatWait/SquatRv, tap jumps, ordinary second jumps and Pass. Jump
button history, tilt ages, consumed jumps and transition timers are checkpointed.
`tests/fixtures/game/locomotion.json` contains invented values used by
the conformance and movement integration tests. These actions still use the
supplied static non-jab pose; action-specific animation resources are needed
for authentic collision shapes and timing.

The older synthetic demo omits the optional locomotion, displacement and armor
data and retains its original limited behavior. The conformance scenarios
explicitly enable the parameters needed for each behavior. Omitting a profile
does not select Melee defaults or certify compatibility.

`rules.top_ko_min_knockback` supplies the common-data upward-knockback threshold.
Airborne fighters crossing the top survive when their knockback is at or below
it, even if self velocity carries them higher. Grounded fighters crossing the
top still lose a stock. Side and bottom crossings remain unconditional in the
ordinary supported branch. Omission retains the older demo's synthetic rule
that every boundary crossing loses a stock. Optional [`rules.death`](deaths.md)
adds directional death actions plus deterministic top star/screen selection,
camera-relative model motion and delayed stock loss. Scripted/player death
exemptions and ice-damage entry remain unported. The complete KO callback order
is also unported: this scheduler checks after contacts, while the original
eligibility callback runs earlier in fighter update.

## Explicit coverage limits

The match supports the supplied movement profiles, airborne drift/fast fall,
a jab, all five ordinary aerials with landing/autocancel/L-cancel, ordinary
damage/armor, integral fixed-angle launch and angle 361. Aerial resources and
callback limits are described in [aerials.md](aerials.md). The optional
[shield profile](shield.md) adds ordinary raise/hold/release, stun, recoil,
break and dizzy recovery. The optional [grab profile](grabs.md) adds ordinary
standing catches, paired holds and four-direction throws. The optional [ledge
profile](ledges.md) adds static endpoint catch/hang, climb, jump, attack, escape
and drop. The optional [neutral-special profile](specials.md) adds paired ground
and air neutral-B actions. Inputs do not yet reproduce the full PAD-to-fighter
history. Directional specials, character-specific special state, running turns
and character multijumps remain unported. Some accepted stick/button
combinations consequently have no action in this experimental profile.

The optional [blast-death profile](deaths.md) adds normal directional deaths and
resource-driven star/screen phases. It does not implement the camera, effects,
audio, stat/bonus callbacks or authentic common-data values.

Moving stages/remapping and full ECB corner/squeeze response remain unported.
Ledge actions currently require static marked endpoints and supplied generic
poses; percent-dependent variants and ledge trumping remain unported. The optional [nudge profile](nudge.md) adds source-backed
two-leader X/Z push sampling and gameplay depth. Follower entities and the
ledge-specific backward-push map branch remain unported. The optional
[rebirth profile](rebirth.md) supplies the ordinary leader's static airborne
platform lifecycle; moving-stage offsets, Nana coordination and the full
priority action graph remain unported. The optional [damage-floor profile](damage-floor.md) supplies neutral
tech and knockdown recovery; bounces, tech rolls and wall/ceiling techs remain
unported.

Combat omits item/Slash/capture clash branches, dynamic metal/state knockback modifiers,
vulnerability/target flags, powershield/reflect and character-specific shield responses,
grab escapes/pummels and other special launch-angle behaviors. Outside supplied attack, catch,
throw, and landing poses,
fighters currently use a static supplied pose; authentic walking, jumping and
damage collision require those animation resources. The schema exposes ordinary
Euler scale inheritance but not all HSD joint flags, IK or animation scripting.
Character-specific radius/model scaling is not implemented. Tests exercise the
explicit slice; they do not certify those missing rules.

The [native-data audit](native-data.md) preserves real Fox numeric attributes,
raw jab commands, captured jab observations and Final Destination bounds with
source hashes. Those files are **incomplete resources** and are intentionally
not loaded as a playable Fox profile. Missing skeletons, genuine hurtboxes, ECB,
animation tracks and stage topology must be filled before a real matchup can
be validated. Conflicting move-data extraction conventions must be resolved
against raw commands and source arithmetic, not blended into defaults.
