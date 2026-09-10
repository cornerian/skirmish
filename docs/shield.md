# Native shields

`game::shield` connects processed digital/analog shoulder input to GuardOn,
Guard, GuardOff, GuardSetOff and GuardReflect. `game::escape` adds the ordinary
grounded evasions EscapeF, EscapeB and EscapeN entered from those states. Shield contacts run the original matrix-aware
`lbColl_80006E58` narrowphase before hurtbox checks. A shrinking shield can miss
an attack that subsequently hits a hurtbox. The shield bone uses unit local
scale in the supplied pose; its explicit initial radius replaces that scale.
Parents may carry nonuniform transforms. No display object supplies gameplay
geometry or timing.

Enable the profile with `rules.shield` and each participating fighter's `shield`
attributes. Optional `rules.escape` plus each fighter's `escape` motions enable
rolls and spot dodges; `tests/fixtures/game/escape.json` shows that schema with
**invented test values** and requires the locomotion parameters that supply the
shared stick ages. The fixture `tests/fixtures/game/shield.json` shows all
required shield fields with **invented test values**. It is not Melee common data or a character
preset. Animation durations, the shield bone, radius and break-launch velocity
must come from native resources for a real character. These additions remain
optional for older experimental profiles.

The implementation preserves these examined source branches:

- `ftCo_Guard.c` normalizes analog strength, drains health, scales shield size,
  enforces minimum hold time and latches release. Repressing during minimum hold
  does not cancel a latched release. GuardSetOff preserves the latch and blocks
  action interrupts while its supplied animation advances at the calculated
  rate; its duration is not replaced with a rounded integer timer.
- `ftCo_80091A4C`, `ftCo_80093694` and the GuardReflect callbacks enter a
  powershield only for a fresh physical L/R press inside both trigger and
  GuardOn windows. The independent `x2A4` reflector and `x2B4` damage-immunity
  timers retain the source's active-at-zero boundary and freeze in hitlag.
  Releasing the trigger is latched while the reflector remains active.
- `ftcoll.c::ftColl_80076CBC` converts stored hit damage with `getEnvDmg`, adds
  `HitCapsule::x34` shield damage, and clamps that sum before accumulating shield
  damage. Nonzero damage smaller than one becomes one. Zero integer damage does
  not invoke the shield-stun callback, even if an extra shield-damage value is
  present.
  A powershield contact skips shield-health loss and the ordinary shield effect,
  while retaining hitlag, stun, attacker recoil and the source's unmultiplied
  defender push branch.
- The fighter-contact scan distinguishes normal hitboxes from
  `HitElement_Inert`. An inert overlap sets the one-frame `x221C_b5` shield-touch
  signal without health loss, stun, hitlag, body damage or hit-group history.
  This is gameplay state used by character callbacks, rather than a rendering
  effect.
- `Fighter_ProcessHit_8006D1EC` supplies regeneration, hitlag, shield damage and
  attacker recoil. Contact damage uses already cached stale damage. Shield-only
  contacts do not run the hurt path's stale-queue insertion. Defender ground
  speed and attacker recoil are separate checkpointed values.
- `ftCo_80093240` and `ftCo_800932DC` apply horizontal SDI/ASDI along the floor
  tangent, using the shared input ages and explicit displacement coefficients.
  The original airborne attacker-recoil decay typo is retained: its small-vector
  branch clears damage-knockback Y instead of shield-recoil Y.
- `ftCo_8009A080` and `ftCo_80099F1C` route held digital or processed analog
  shield plus a fresh downward main-stick tilt directly into Pass on a one-way
  platform. The transition reuses the ordinary floor-line skip, falling physics
  and ECB lock. A shield entered on the current scheduler frame cannot also
  drop until its next action callback.
- `ftCo_800CB024` extends the ordinary jump dispatcher with
  `ftCo_800DF910`'s inclusive upward C-stick threshold. Main-stick tap jump and
  fresh X/Y retain priority. C-stick input does not require a fresh excursion,
  and its release during JumpSquat selects the native short-hop branch.
- `ftCo_Escape.c` supplies the grounded evasions. Every guard IASA chain checks
  `ftCo_8009980C` (spot dodge) before `ftCo_8009917C` (roll), and both precede
  the jump and platform-drop dispatchers; GuardOff offers only the spot dodge
  and the jump. A roll needs a fresh main-stick magnitude at or beyond `x31C`
  inside the `x320` age window, or otherwise a held horizontal C-stick through
  `ftCo_800DF8B0`; the selected axis value times facing chooses EscapeF (zero
  products included) or EscapeB. A spot dodge needs a fresh downward main
  stick at or below `x314` inside `x318`, or a held downward C-stick through
  `ftCo_800DF8E8`. Entry uses an ordinary `Fighter_ChangeMotionState`, so the
  active-shield state ends and health regenerates from the entry frame's
  `Fighter_ProcessHit` onward; the transition also clears the `x2218` reflect
  bit and `x221C_b3` entry latch while the `x221C_b2` immunity window stays
  frozen, and `x221D_b5` disables overlap nudges against the evading fighter. `ftCo_Escape_Phys` runs `ft_80085030`, converting each
  sample's TransN delta into the exact ground velocity before ordinary friction
  would apply, while `ftCo_EscapeN_Phys` keeps ordinary friction. Both
  collision callbacks use the ordinary ground path, so leaving the floor enters
  Fall. The animation callbacks return to Wait after the last supplied sample;
  the roll also clears ground velocity. The unread `x324` copy, the item-throw
  IASA, and the Samus/Yoshi entry branches are not modeled.
- Escape samples carry the scripted fighter-wide `Fighter::x1988` collision
  state (`body_state`). Intangible or invincible samples reject hits and grabs
  for that frame only; ordinary transitions reset the state, and Slippi's
  hurtbox byte reports it ahead of the timed counters. The invincible branch's
  no-damage contact registration is not reproduced.
- `ftCo_ShieldBreak{Fly,Fall,Down,Stand}.c` launches a broken fighter, suppresses
  air control, preserves the break hurt-status branch, and transitions on landing
  and supplied animation endings. `ftCo_Furafura.c` resets shield health, computes
  the percent-dependent dizzy timer and uses `ftCommon_GrabMash` direction/button
  reductions. Neutral stick input retains its previous mash-direction bucket.

Shield health, analog strength, hold/release state, powershield flags and timers,
stun animation progress, recoil vectors, dizzy timer, mash directions and the
scripted body state survive native checkpoints.
Nonfinite results fail a step atomically. Tests exercise the complete ordinary
cycle, shield pokes, zero boundaries, staling interaction, hitlag displacement
and deterministic replay. Selected complete C functions independently check
radius, drain, strength, powershield-window ticking, ordinary/powershield
stun-rate/push, displacement, shield-drop input, damage conversion and mash arithmetic. The complete roll and spot-dodge dispatchers and both escape
C-stick predicates are compared with pinned C over arbitrary bit patterns. C adapters omit presentation/statistics callbacks whose results do
not feed those calculations.

This is still an experimental scheduler rather than complete Melee equivalence.
Reflected-projectile motion is not yet simulated even though the fighter's
reflector-active window is represented. Yoshi's shield, Jigglypuff's special break-death flag, electric-hit
branches, shield tilting and native shield/body animation tracks, shield grabs,
full callback ordering and material friction are not provided by this batch.
Break down/up pose selection is grouped
into one lifecycle with supplied durations. Real Slippi parity also requires
the missing authentic gameplay resources and other action systems.
