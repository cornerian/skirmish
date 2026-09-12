# Ledge action profile

`rules.ledge` enables static-stage ledge discovery and supplies catch ranges,
stick thresholds, hang duration, regrab cooldown and catch intangibility. Its
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

Each fighter's `ledge.snap` is optional raw per-character catch geometry
(`ftData.x44`'s `ledge_snap_x`/`ledge_snap_y`/`ledge_snap_height`, already
scaled by the character's own bone scale -- `mpColl_SetLedgeSnap`,
`ft_80081B38`/`ft_80081C88`, `ft_081B.c:59-62`). Its absence keeps the
point-distance-range test above (the original approximation this profile
started with); its presence switches that one fighter to the real per-frame
query the source runs continuously while falling
(`ft_80083090_inline`/`ft_800831CC`, `ft_081B.c:637-694`, dispatching into
`mpColl_80046904`'s `!touched_floor && CollisionFlagAir_CanGrabLedge` branch,
`mp/mpcoll.c:2516-2547`, and `ftCliffCommon_80081298` on a match). That query
(`fighter::ledge::snap_catch_left`/`snap_catch_right`, porting
`mpColl_80044164`/`mpColl_800443C4`) tests one already-known, isolated,
disconnected candidate endpoint's real asymmetric ECB-and-snap box (using the
fighter's own current ECB, `mp/mpcoll.c:1250-1368`) instead of a symmetric
point-distance range, and additionally requires: the fighter's current
facing to already match the endpoint's inward direction (only a fighter
already facing into the stage can catch its ledge -- the source never tests
`CLIFFCATCH_BOTH` from this continuous path, only from special-move
recovery's separate `ft_CheckGroundAndLedge`, which this profile does not
model); the fighter's position (not velocity) to have strictly decreased this
frame; and that no floor-end clamp already held this frame's position
(`fighter.edge_contact`). The source's own floor-database box search and its
two `mpCheckMultiple` line-of-sight-obstruction checks are not ported: since
this profile already knows which single, disconnected candidate it is
testing, that search could only ever find the candidate's own tip, with
nothing to obstruct sight to it -- `tests/oracle/ledge_snap.c` pins the
complete original functions under a forced no-obstruction scenario and
checks both the box arithmetic and this reduction against it.

Catch clears self velocity and knockback, turns inward, grants the configured
intangibility and enters `CliffCatch`. While the fighter is attached, every
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
variant physics, timeout, cooldown/regrab, checkpoints, KO cleanup, invalid
resources, and the real per-frame snap query catching a fighter that has
drifted past a ledge and rejecting one that is still ascending. Six ledge
conformance scenarios now run normally.
`ledge_differential` compares action selection and cooldown side effects with the
complete pinned original C callbacks over generated floats, booleans and integer
cooldowns, including the quick/slow percent selector.
`ledge_snap_differential` pins `mpColl_80044164`/`mpColl_800443C4` complete and
compares both their box arithmetic and their catch decision (under the forced
no-obstruction scenario above) against `fighter::ledge::snap_catch_left`/
`snap_catch_right` over generated floats.

The profile uses supplied generic action tracks. Moving or remapped collision
lines, disappearing ledges, ledge trumping, tether grabs (`ftCo_800C3A14`, a
separate caller of the same two pinned functions with its own widened
snap fudge, used only by Link's hookshot and Samus's grapple beam),
character overrides, ledge stalls, the floor-database box search and its
line-of-sight obstruction checks (see `ledge.snap`'s paragraph above), the
special-move `CLIFFCATCH_BOTH`/`ft_CheckGroundAndLedge` recovery path, and the
original intangibility-refresh policy remain unported. Authentic values and
poses must come from separately attributed native resources.
