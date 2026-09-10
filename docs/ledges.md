# Ledge action profile

`rules.ledge` enables static-stage ledge discovery and supplies catch ranges,
stick thresholds, hang duration, regrab cooldown and catch invincibility. Its
optional `slow` table supplies the source percent threshold and slow hang
duration. Every fighter then supplies `fighters[].ledge`: a bone anchor, complete physics poses
and root offsets for catch, wait, climb, jump, attack and escape. A paired
optional `slow` set replaces all four option motions, including the ledge-attack
hitboxes. Omitting both
keeps older synthetic match resources compatible; supplying only one side is a
resource error.

An eligible endpoint belongs to an enabled, visible floor line whose material
flags include `LEDGE`. The endpoint must have no connected neighbor and no other
fighter may own the same line/side pair. An airborne fighter in a catchable
action can acquire it while stationary or descending, within the configured
bone-anchor bounds, unless the main stick is held at or below the down threshold
or the checkpointed cooldown is active. Candidate traversal and equal-distance
ties use stable line, side and player order.

Catch clears self velocity and knockback, turns inward, grants the configured
invincibility and enters `CliffCatch`. While the fighter is attached, every
frame solves the fighter root so the selected bone point reaches the endpoint
plus the supplied action-frame offset. This makes the collision pose, hurtboxes
and ledge-attack hitboxes headless physics inputs; no renderer participates.

`CliffWait` uses attack, shoulder and jump-button priority before its stick
regions. A neutral stick sample arms the climb/drop gate. The retained
`ftCo_8009AAFC` rule allows a main-stick climb toward or above the stage, rejects
the C-stick in the climb region, and accepts either stick in the drop region.
Climb, attack and escape follow their supplied root-motion tracks and finish on
the supporting floor. Jump detaches on an explicit frame and applies its inward
and upward launch vector. Attack contact uses the ordinary swept-hitbox, shield,
damage, staling and hitlag pipeline. Damage, drop, jump release, timeout and KO
clear endpoint ownership; drop and release install the configured cooldown.

The source's strict percent comparison selects and checkpoints one complete
quick or slow set. The selected samples drive attachment, jump release and
velocity, hurtboxes and attack contact.

`game_ledge` covers both endpoint directions, eligibility and connectivity,
stable occupancy, attachment, input priority, action completion, jump launch,
attack damage, damage release, quick/slow percent boundaries and timers, complete
variant physics, timeout, cooldown/regrab, checkpoints, KO cleanup and invalid
resources. Six ledge conformance scenarios now run normally.
`ledge_differential` compares action selection and cooldown side effects with the
complete pinned original C callbacks over generated floats, booleans and integer
cooldowns, including the quick/slow percent selector.

The profile uses supplied generic action tracks. Moving or remapped collision
lines, disappearing ledges, ledge trumping, tether grabs,
character overrides, ledge stalls, wall jumps, complete collision-environment
flags and the original invincibility-refresh policy remain unported. Authentic
values and poses must come from separately attributed native resources.
