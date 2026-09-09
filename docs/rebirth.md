# Rebirth platforms

`rules.rebirth` enables an explicit headless stock-loss lifecycle. Each player
has an entry position and a platform target, followed by configured travel and
wait durations and a stick-release threshold. Omission preserves the older
grounded respawn behavior for existing resources.

After the ordinary respawn delay, the fighter is reset at its entry position in
`Rebirth`, airborne and facing inward. Each frame recomputes velocity from its
current position, target and remaining travel callbacks. The last travel step
therefore reaches the supplied target exactly for ordinary finite resources.
`RebirthWait` holds that static position until its duration ends or a button,
trigger, main-stick or C-stick input requests release. Release enters `Fall` and
retains `rules.respawn_invincibility_frames` of protection.

Both rebirth actions ignore gravity, stage contact response, fighter pushing,
grabs, attacks and blast boundaries. Bone transforms and collision sampling
still run, so the headless state exposes the same pose-backed geometry shape as
other actions. The complete state, timers, previous controller input and
invincibility counter live in `game::State`, so checkpoints and counterfactual
suffixes reproduce the lifecycle without renderer state or global objects.

`fighter::rebirth` retains the two leader velocity expressions from
`ftCo_Rebirth_Phys` and `ftCo_RebirthWait_Phys` separately. Their multiply order
is intentionally different. `rebirth_differential` compiles both complete
functions from the pinned original C snapshot and compares the Rust results by
bits over generated binary32 inputs and all positive signed frame counts.

`game_rebirth` covers both player slots, reset state and one-frame event timing,
every travel step, exact arrival, timeout and all supported release input kinds,
post-platform invincibility, real overlapping attack suppression, checkpoint
replay and malformed resources. `conformance_stage` now runs its stock-loss
rebirth scenario normally.

The current resource describes a static target directly. Moving-stage spawn
offsets, platform collision callbacks and accessory object motion are not yet
represented. Nana's leader/follower velocity-copy branch and coordinated
release are also pending follower simulation. The generic input release policy
does not yet dispatch every original priority action directly from
`RebirthWait`; it establishes deterministic platform exit for the implemented
action set.
