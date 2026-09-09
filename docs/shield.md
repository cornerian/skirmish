# Ordinary native shields

`game::shield` connects processed digital/analog shoulder input to GuardOn,
Guard, GuardOff and GuardSetOff. Shield contacts run the original matrix-aware
`lbColl_80006E58` narrowphase before hurtbox checks. A shrinking shield can miss
an attack that subsequently hits a hurtbox. The shield bone uses unit local
scale in the supplied pose; its explicit initial radius replaces that scale.
Parents may carry nonuniform transforms. No display object supplies gameplay
geometry or timing.

Enable the profile with `rules.shield` and each participating fighter's `shield`
attributes. The fixture `tests/fixtures/game/shield.json` shows all required
fields with **invented test values**. It is not Melee common data or a character
preset. Animation durations, the shield bone, radius and break-launch velocity
must come from native resources for a real character. These additions remain
optional for older experimental profiles.

The implementation preserves these examined source branches:

- `ftCo_Guard.c` normalizes analog strength, drains health, scales shield size,
  enforces minimum hold time and latches release. Repressing during minimum hold
  does not cancel a latched release. GuardSetOff preserves the latch and blocks
  action interrupts while its supplied animation advances at the calculated
  rate; its duration is not replaced with a rounded integer timer.
- `ftcoll.c::ftColl_80076CBC` converts stored hit damage with `getEnvDmg`, adds
  `HitCapsule::x34` shield damage, and clamps that sum before accumulating shield
  damage. Nonzero damage smaller than one becomes one. Zero integer damage does
  not invoke the shield-stun callback, even if an extra shield-damage value is
  present.
- `Fighter_ProcessHit_8006D1EC` supplies regeneration, hitlag, shield damage and
  attacker recoil. Contact damage uses already cached stale damage. Shield-only
  contacts do not run the hurt path's stale-queue insertion. Defender ground
  speed and attacker recoil are separate checkpointed values.
- `ftCo_80093240` and `ftCo_800932DC` apply horizontal SDI/ASDI along the floor
  tangent, using the shared input ages and explicit displacement coefficients.
  The original airborne attacker-recoil decay typo is retained: its small-vector
  branch clears damage-knockback Y instead of shield-recoil Y.
- `ftCo_ShieldBreak{Fly,Fall,Down,Stand}.c` launches a broken fighter, suppresses
  air control, preserves the break hurt-status branch, and transitions on landing
  and supplied animation endings. `ftCo_Furafura.c` resets shield health, computes
  the percent-dependent dizzy timer and uses `ftCommon_GrabMash` direction/button
  reductions. Neutral stick input retains its previous mash-direction bucket.

Shield health, analog strength, hold/release state, stun animation progress,
recoil vectors, dizzy timer and mash directions survive native checkpoints.
Nonfinite results fail a step atomically. Tests exercise the complete ordinary
cycle, shield pokes, zero boundaries, staling interaction, hitlag displacement
and deterministic replay. Selected complete C functions independently check
radius, drain, strength, stun/rate/push, displacement, damage conversion and mash
arithmetic. C adapters omit presentation/statistics callbacks whose results do
not feed those calculations.

This is still an experimental scheduler rather than complete Melee equivalence.
Powershield/reflect rules are rejected by requiring an explicit zero powershield
input window. Yoshi's shield, Jigglypuff's special break-death flag, electric-hit
branches, shield tilting and native shield/body animation tracks, rolls, grabs,
shield-drop input, C-stick shield jumps, full callback ordering and material
friction are not provided by this batch. Break down/up pose selection is grouped
into one lifecycle with supplied durations. Real Slippi parity also requires
the missing authentic gameplay resources and other action systems.
