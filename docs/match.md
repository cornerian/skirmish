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
time, selected damage motion, grounded knockback scalar, tumble eligibility,
damage-surface history/lockout and tech timer, wall-jump contact timer, side,
repeat count and startup/exponent state, physical-L/R
tech ages, jump-press age, retained attack/combo/source attribution and combo
separation timers, swept hitbox centers, ECB interpolation history, stage
contacts, ledge endpoint ownership/cooldown, stocks, invincibility, match clock,
reserved RNG seed and events. This slice has no random events and consumes no
RNG draws. Checkpoints are opaque in-memory values, and restoration rejects
different resource/rule identities. Persistent
checkpoint encoding is a later versioned interface.

Clones share only immutable native resources. Each branch owns its mutable state;
stepping errors leave it unchanged. `skirmish-replay` implements the
`replay-validation` adapter around this match, so streaming expected observations
and counterfactual branches use this implementation without coupling replay to
the core simulator. The `validate-replay` command applies
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
bone-attached capsules, swept capsule intersection, walking, running-turn and jump launch
arithmetic live in `physics`. The native match schema carries all pose
samples for its jab, including startup/recovery frames. The same evaluated pose
places both hurtboxes and hitboxes; rendering is not involved. Local native
coordinates use +X forward, +Y up and +Z depth; resource imports must convert
source coordinate conventions explicitly.

The experimental scheduler first handles countdown or respawn/freeze counters,
then samples optional stage motion, updates actions and physics for both players,
samples their ECBs and resolves stage contacts in source-sized movement substeps. It evaluates contact
poses, resolves configured ordinary grounded clashes, and collects remaining
shield/hurtbox contact decisions before applying damage so both
players can trade on the same frame. Hit-group history suppresses repeated
contacts during one attack. With the [clank profile](clanks.md), active same-group
slots instead share explicit victim histories that survive hitlag and clear on
deactivation. Repeated hits retain the source combo count and start the original
grounded attacker-separation timer. Its floor-tangent displacement runs during
hitlag and selects the configured ordinary or strong distance from that count.
Pending rebound yields to incoming body damage and shield stun.
It then resolves stock losses together, checks the
clock and emits events. A timeout compares stocks, then percent; exact ties
finish as a draw. This schedule and these match policies are integration code,
not a claimed translation of the complete original callback graph.

Each fighter supplies `collision_box`: either `fixed` with explicit up/down,
front/back and angle values, or `bones` with six bone indices, source thresholds,
side-height offset and flags. These boxes are separate from hurtboxes. The stage
accepts explicit `geometry.lines` and `geometry.joints`; when absent, its compact
`floor` becomes one directed segment. Lines retain source IDs by array position,
adjacency, surface flags and joint ranges. Optional [`stage.motion`](stage-motion.md)
applies cyclic affine samples to stable line ranges, and `Match::stage_geometry`
reconstructs the collision mesh for the current checkpointed stage frame. Floor segments run left to right,
ceilings right to left, left-facing walls bottom to top and right-facing walls
top to bottom.

Movement subdivision retains the original strict six-unit threshold and ECB
growth checks: exactly twelve units takes three substeps. Grounded fighters
project onto adjacent floor segments and use their normals for ground motion.
Projection preserves the source's small separation bias. Side and ceiling
contacts project the final collision point onto the contacted surface and stop
inward velocity unless an eligible damage reflection consumes it. Platforms permit upward passage,
descending landings and intentional drops with the explicit locomotion profile.
Pass remembers and skips only its supporting line during every movement substep;
other platforms remain collidable. The next action transition clears that skip.
The drop also applies the source ten-map-callback ECB bottom lock. Full corner
adjacency remains unported. Ordinary opposing-wall and ceiling/floor squeezes,
including inward-moving surface contacts, are composed from the exact reusable
arithmetic as described in the [ECB response profile](ecb-response.md).

An active hitbox carries its previous and current world centers. New activation,
a disabled slot or a changed group resets its sweep. Hitlag still updates these
centers, preventing reuse of an old movement segment. Contacts use the source
isotropic capsule solver for hitbox clashes, including its unusual near-parallel
endpoint choice. Body [hurtbox contact](hurtbox-contact.md) and shields use the
matrix-aware `collision::shield` helper, which ports `lbColl_80006E58` and
the ordinary geometric branch of `lbColl_80007BCC`: transformed hurt volumes
retain directional radii, contact position, overlap and the original broadphase.
It accepts caller-prepared world endpoints and matrices; bone caching and forced
hits remain outside that helper. See the [shield profile](shield.md). Its C oracle
uses the SDK scalar matrix-vector routine in place of paired-single assembly,
so passing comparisons establish native scalar agreement only.
Ordinary damage and grabs accept only enabled hurtboxes; grabs also require each
capsule's explicit grabbable property. Attack resources can supply one complete
hurtbox-state sample per frame, which both paths observe from the target's current
action frame. Empty samples inherit the base states for legacy resources, while
partial samples fail validation. Legacy hurtboxes default to enabled and
grabbable.

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
Optional [`rules.damage.ground_launch`](grounded-launch.md) retains low-level
damage on the floor, projects its launch along the supporting tangent and
decays the source ground-knockback scalar. Fly-level and explicit DownDamage
launches leave the floor and apply the configured strict bounce branch. The
complete damage callback sequence remains unported.

Ordinary fighter hits select [damage direction](hit-direction.md) from the
attacker and victim X positions, including the source equal-X tie, then face the
victim toward the attacker and launch away. Throws instead negate the thrower's
facing before entering the shared transition. Prone DownDamage retains its
explicit old-facing override after calculating launch. Hitbox angle 362 instead
uses the contacted hurt capsule's midpoint and the matrix narrow phase's surface
position to choose signed angle and facing.

Optional [`rules.damage.damage_motion`](damage-motion.md) supplies the three
ordinary knockback-level thresholds, while every fighter supplies complete
ground, air and fly physics poses plus a height for each hurtbox. Selection uses
post-armor knockback, the shared hitstun scale, pre-hit ground state and the
contacted hurtbox. Sampled bones drive hurtboxes and ECBs; Damage waits for both
animation and hitstun, holding its final pose when hitstun lasts longer. When
hitstun ends before an airborne motion, Damage switches to ordinary air physics
and accepts fresh fast-fall or implemented air-action input.

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

Stale queues, hitbox damage caches and the match-wide nonzero 16-bit stale
instance sequence are checkpointed. A separate nonzero 16-bit sequence tracks
fighter action instances. Each implemented action queues the low byte of its
original motion flags: a zero identity or change from the previous identity
allocates a new ID, while matching aerial attack/landing and other shared motion
families retain it. Hits copy the source ID into the victim's retained
`last_hit_by_instance`. Native motion transitions allocate identities in the
experimental scheduler's order; the complete original callback allocation order
is not claimed. A KO clears the deceased player's stale queue and attribution,
preserving both global counters and other players' histories; match reset starts
both counters fresh.

Movement and displacement share the source stick-age timers, sampled once per
active frame including hitlag. When both optional profiles are supplied, their
axis thresholds must agree; inconsistent resources are rejected at load time.
Optional [`rules.damage.floor_response`](damage-floor.md) adds source-gated
neutral techs plus the complete configured DownBound/DownWait/DownStand recovery
chain. Its optional roll profile adds forward/backward selection and per-frame
root motion and bone poses. Optional knockdown resources add missed-tech rolls,
standing and get-up attacks with sampled bones and TransN motion, while optional
recovery rules assign invincibility to each tech and get-up action. Physical
L/R and separate A/B ages are sampled during hitlag and ordinary frames; A/B
ages reset when DownBound begins and provide its attack buffer. Damage at or
above its inclusive threshold retains tumble eligibility through DamageFall
until floor contact. DamageFall holds fast fall and the implemented air-action
inputs through hitstun, then accepts a fresh input. The evaluated hip-bone matrix
and per-fighter axis/inversion flags select checkpointed face-up or face-down
resources for the entire missed-tech suffix. Optional low-damage rules add source-gated DownDamage poses,
launch, landing and hitstun-countdown recovery without resetting DownWait. Optional
[`rules.damage.surface_response`](damage-surfaces.md) adds strict directional
wall/ceiling eligibility, normal-based reflected velocity, repeat state and
configured FlyReflectWall/FlyReflectCeiling durations with complete sampled
physics poses. Reflected actions retain ordinary gravity and drift, hold fast
fall and implemented air-action input through hitstun, then accept fresh input.
Optional
`rules.damage.surface_tech` and fighter attributes add buffered neutral/jump wall
techs, ceiling tech input motion, complete sampled physics poses and configured
recovery. Floor landing wins when multiple responses are possible in one
collision pass; wall response wins over ceiling response at a corner. Reflected
wall and ceiling actions can chain into the other surface class during the
repeat lockout. Bone-based ECBs and hurtboxes consume these poses in headless
matches.

Optional [`rules.wall_jump`](wall-jumps.md) and paired fighter resources add the
ordinary wall-jump interrupt. ECB wall contacts use fighter displacement relative
to the contacted stage line, so animated walls participate. Strict source input
windows, incapable fighters, startup freeze, repeated-height decay, sampled
physics bones, landing reset and checkpoint state are covered independently from
damage wall techs even though both use `PassiveWallJump`.

Optional [`rules.grab`](grabs.md) and per-fighter grab resources add physical-Z
standing, Dash/Run and turn-facing catch entry, distinct sampled standing/dash
grab capsules, paired pull/hold states and fresh-A pummels plus
forward/back/up/down throws. Contact selects separate grounded-low and
airborne-high captured-victim action families. Captured fighters remain
attached through explicit holder/victim bone anchors. A pummel has priority over
throw selection, applies one shared-hitlag percent event, starts the victim's
matching sampled high/low CaptureDamage pose sequence, and returns both sides independently to
hold without breaking the pair. The shared input history applies the original throw
direction priority, and each supplied release event enters the ordinary damage
pipeline. Victim weight and the per-direction independence flag drive a shared
fractional throw clock. Pair ownership, pummel-hit history, throw time and rate,
the hold timer and mash latches are
checkpointed and cleared transactionally on release or stock loss. Captured
fighters also run an explicit percent-scaled hold timer and the complete
button/stick `ftCommon_GrabMash`
transition. Expiry enters resource-driven CatchCut/CaptureCut release actions.
Attachment rises over the scaled source threshold, ground support loss switches
low captures to high, and floor contact switches high captures to low without
resetting action time.
Tether variants and capture-contact interference remain unported.

Optional [`rules.ledge`](ledges.md) and per-fighter ledge resources add static
endpoint discovery, bone-attached catch/hang poses, climb, jump, attack, escape,
drop and regrab cooldown. Optional paired quick/slow resources use the source
percent threshold for hang duration and all four ledge options. Ledge attack uses
the same swept-contact and damage pipeline as other attacks. Endpoint ownership,
selected variant, stick arming and cooldown are checkpointed; damage and stock loss
clear ownership. The [ledge profile](ledges.md)
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
coefficients, animation/event durations, crouch/turn timing, aerial jump data
and platform-drop parameters. No authentic common-data values are implied. With
that data, the scheduler supports Dash/Run/TurnRun/RunBrake, standing Turn,
Squat/SquatWait/SquatRv, tap jumps, ordinary second jumps and Pass. The
optional [dash-attack profile](dash-attack.md) adds Dash's own
early/middle/late input phases (a dash-specific forward smash and forward
roll, a dash attack with its own no-A catch buffer, and phase-gated shield
entry) on top of this same locomotion data; without it Dash's ordinary
scheduler behaviour above is unaffected. An optional
fixed five-entry `multi_jump` table supplies per-jump vertical speeds and
animation command markers, horizontal speed, air-control multipliers and the
root-joint turn used by characters with up to five aerial jumps. The first
aerial jump requires a fresh input; later jumps accept held X/Y or up only after
the current jump's marker. Root yaw changes sampled bone physics and facing flips
at the source integer halfway point.
TurnRun tests its reversed-stick boundary before braking, stores entry facing,
decelerates with the source branch, and pauses at an explicit script marker until
its scaled ground velocity reaches x0.01. RunBrake has its own explicit command
marker and carries its current action time into a later TurnRun. Jump button
history, tilt ages, consumed jumps, multijump root rotation and transition timers
are checkpointed.
`tests/fixtures/game/locomotion.json` contains invented values used by
the conformance and movement integration tests. These actions still use the
supplied static non-jab pose, with the multijump root turn applied to it;
action-specific animation resources are needed for authentic collision shapes
and timing.

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
damage/armor, integral fixed-angle launch, angle 361 and body-contact angle 362.
Aerial resources and callback limits are described in [aerials.md](aerials.md). The optional
[shield profile](shield.md) adds ordinary raise/hold/release, stun, recoil,
break, dizzy recovery, direct shield drops through one-way platforms,
C-stick shield jumps and resource-driven grounded rolls and spot dodges. The optional [grab profile](grabs.md) adds ordinary
standing, dash, pivot and shield catches with the dash-grab buffer, paired holds,
pummels with captured reaction poses, mash escape and four-direction throws. The optional [ledge
profile](ledges.md) adds static endpoint catch/hang, climb, jump, attack, escape
and drop. The optional [air-dodge profile](air-dodge.md) adds EscapeAir,
FallSpecial and the LandingFallSpecial landing. The optional [tilt
profile](tilts.md) adds forward, up and down tilts with their input chains. The optional [smash
profile](smashes.md) adds forward, up and down smashes with their charge. The
optional [jab profile](jabs.md) adds the second and third jab with their
buffered follow-up windows and the rapid jab's entry count, loop and end;
without it the jab keeps its previous chainless behaviour. The optional
[dash-attack profile](dash-attack.md) adds the Dash early/middle/late input
phases and the dash attack itself; without it Dash and Run keep their
previous behaviour. The optional [neutral-special
profile](specials.md) adds paired ground and air neutral-B actions. Inputs do not yet reproduce the full PAD-to-fighter
history. Directional specials and character-specific special state remain
unported. Some accepted stick/button
combinations consequently have no action in this experimental profile.

The optional [blast-death profile](deaths.md) adds normal directional deaths and
resource-driven star/screen phases. It does not implement the camera, effects,
audio, stat/bonus callbacks or authentic common-data values.

The [stage-motion profile](stage-motion.md) adds transformed collision samples
and exact grounded-line carry/remapping. The [ECB response profile](ecb-response.md)
adds clear-to-penetrating moving-surface contacts and ordinary opposing-surface
squeezes. Dynamic surface-kind changes and the complete connected-corner graph
remain unported.
Ledge actions currently require static marked endpoints and supplied generic
poses; ledge trumping remains unported. The optional [nudge profile](nudge.md) adds source-backed
two-leader X/Z push sampling and gameplay depth. Follower entities and the
ledge-specific backward-push map branch remain unported. The optional
[rebirth profile](rebirth.md) supplies the ordinary leader's static airborne
platform lifecycle; moving-stage offsets, Nana coordination and the full
priority action graph remain unported. The optional [damage-floor profile](damage-floor.md) supplies neutral
and directional floor techs plus resource-driven missed-tech rolls, standing and
get-up attacks. The
[damage-surface profile](damage-surfaces.md) supplies ordinary tumble
reflections and wall/ceiling techs. The [damage-motion
profile](damage-motion.md) supplies ordinary grounded, airborne and fly poses.
The [grounded launch profile](grounded-launch.md) supplies floor-relative damage
launch and per-frame grounded knockback friction.
The [damage-direction port](hit-direction.md) supplies ordinary fighter-contact
and throw launch orientation plus the prone-facing override.

Combat omits item/Slash/capture clash branches, dynamic metal/state knockback modifiers,
vulnerability/target flags, reflected-projectile motion and character-specific shield responses,
capture-specific interference and other special launch-angle behaviors. Outside supplied attack, catch,
throw, landing, escape, air-dodge and ordinary damage poses,
fighters currently use a static supplied pose; authentic walking, jumping and
other action collision requires those animation resources. The schema exposes ordinary
Euler scale inheritance but not all HSD joint flags, IK or animation scripting.
Character-specific radius/model scaling is not implemented. Tests exercise the
explicit slice; they do not certify those missing rules.
Attack resources currently accept the normal and inert detection elements;
other elemental damage and status branches remain outside the schema.

The [native-data audit](native-data.md) preserves real Fox numeric attributes,
raw jab commands, captured jab observations and Final Destination bounds with
source hashes. Those files are **incomplete resources** and are intentionally
not loaded as a playable Fox profile. Missing skeletons, genuine hurtboxes, ECB,
animation tracks and stage topology must be filled before a real matchup can
be validated. Conflicting move-data extraction conventions must be resolved
against raw commands and source arithmetic, not blended into defaults.
