# Fox/Falco neutral special (Blaster) -- design note

Pinned decomp rev `0bac93a5`. This batch replaces Fox's neutral special from
the shared single-phase `game::specials::neutral` shell (currently a
placeholder: any character whose data supplies a `neutral` resource gets a
plain ground/air pose pair with no projectile) with his own dedicated
Start/Loop/End state machine plus the minimal generic projectile system it
needs. Falco shares this exact source file and reuses the same Rust module
with his own attributes, gated only at the observation layer, matching the
side/down specials' own precedent (`docs/fox-side-special.md`,
`docs/fox-down-special.md`).

Sources: `src/melee/ft/kinds/ftFox/ftfoxspecialn.c` (whole file, 695 lines;
`ftfoxspecialn.h` for signatures), `ftFox/inlines.h:7-14`
(`ftFox_SpecialN_CheckLoopInput`), `ftFox/types.h:79-89` (`ftFox_DatAttrs`'s
Blaster fields), `melee/it/kinds/itfoxlaser.c`/`.h` (the laser item's own
state table and Logic94 callbacks), `melee/it/kinds/itlgunray.c` (the
sibling "L Gun" ray item; not fired by this move, read only to confirm which
pieces of the ray motion/reflection math in `melee/it/kinds/inlines.h` are
shared plumbing rather than laser-specific), `melee/it/kinds/inlines.h:139-224`
(`Item_UpdateRayAnimation`/`Item_BounceRayOffShield`/
`Item_ResetRayAfterReflection`/`Item_InitRaySpawnFields`/
`Item_InitRaySpawnPosition`, the generic "ray" projectile helpers this
design treats as the model for `game::projectile`), `melee/it/itCharItems.h:265-276`
(`FoxLaserAttr`), `melee/it/types.h:80-127,189-330` (`ItemAttr`, `Article`,
`Item`, confirming where a laser's actual damage/knockback hitbox data
lives -- see "Where the hitbox numbers live" below), `melee/ft/ft_081B.c:1027-1033`
(`ft_80083F88`, the grounded Coll handler) and its own `ftCo_AirCatchHit_Coll`
sibling (declaration only found via `ft_081B.h`; body read from
`ft_081B.c`), `melee/it/kinds/itfoxblaster.c` (the cosmetic hand-held gun
model item, confirmed hitbox-free and out of scope -- see below).

The gun model, cosmetic and out of scope
-----------------------------------------

`ftFx_SpecialN_SpawnBlaster`/`ftFx_SpecialN_RemoveBlaster`/
`fp->u.fx.x222C_blasterGObj` track a *second*, purely visual item -- the
holstered/drawn blaster model attached to Fox's hand
(`da->x20_FOX_BLASTER_GUN_ITKIND`, `it_802AE8A8`) -- entirely separate from
the fired laser (`da->x1C_FOX_BLASTER_SHOT_ITKIND`). Its own `Coll`
callbacks (`itFoxblaster_UnkMotion{8,9,10}_Coll`, `itfoxblaster.c:813-878`)
all unconditionally `return false`: no hitbox table, no `SetAllHitboxes`
call, nothing that can touch a fighter. `ftFx_Throw_Anim` (the same file's
tail) reuses this cosmetic gun item during Fox/Falco's own grab throws
(`ftCo_MS_ThrowB/Hi/Lw`) so the model stays visible/correctly positioned
through a throw, with no additional gameplay effect. Resolved the same way
the side special's ghost item was: **confirmed hitbox-free, entirely
unmodeled**, along with its SFX bookkeeping (`it_802ADDD0`/`it_802AE538`/
`it_802AE608`), its own per-fighter-action item-state table
(`it_803F6E68`), and `ftFx_SpecialN_GetBlasterAction`/
`ftFx_SpecialN_CheckBlasterAction`/`ftFx_SpecialN_CheckRemoveBlaster` (all
three exist only to drive this cosmetic item's own Anim callback).

Fox/Falco phases (`ftfoxspecialn.c`)
-------------------------------------

- **Entry** (`ftFx_SpecialN_Enter`/`ftFx_SpecialAirN_Enter`): common
  `ftCommon_8007D7FC` (ground only, zeroes `gr_vel`/`self_vel` -- matches
  the existing `simulation::enter` reset plus an explicit ground-velocity
  zero this move's own `update_actions` performs), `cmd_vars[0..3]` and
  `isBlasterLoop` all cleared, then `ftFox_SpecialN_SpawnBlaster` (the
  cosmetic gun, unmodeled per above). No stick check of its own: dispatch
  priority (`characters::fox::MOVES` = `[side, up, neutral, down]`) already
  excludes any input the side/up/down branches would have claimed first; the
  genuine gate is the shared, exact-boundary `fighter::special::
  neutral_input` (`ftCo_800D67C4`, unchanged, still used by this move's own
  entry exactly as the retired shell used it -- see "What moves, what
  doesn't" below).
- **Start** (`ftFx_SpecialNStart_Anim`/`_IASA`/`_Phys`/`_Coll`): plays a
  fixed-length pose (ground: `ft_80084F3C`, ordinary ground friction; air:
  `ft_80084DB0`, ordinary air friction/gravity -- both the fighter's plain
  common physics, no dedicated Start attribute, matching the side special's
  own Start-phase precedent). On the last frame, transitions to Loop with
  `Ft_MF_SkipAttackCount | Ft_MF_SkipModel | Ft_MF_KeepGfx` and immediately
  installs `accessory4_cb = ftFx_SpecialN_CreateBlasterShot` (the fire
  callback) and resets the cosmetic gun's own held-state. `_IASA` calls
  `ftFox_SpecialN_CheckLoopInput` every Start frame (`inlines.h:7-14`): a
  **fresh** B press (`pressed_buttons & HSD_PAD_B`, not held) while
  `cmd_vars[0] != 0` sets `isBlasterLoop = true`. Nothing in this file ever
  sets `cmd_vars[0]` to a nonzero value during Start (it is zeroed at Entry
  and never reassigned here); the flag is animation-script-driven data this
  decomp does not expose (see "Fire timing and the repeat window" below).
- **Loop** (`ftFx_SpecialNLoop_Anim`/`_IASA`/`_Phys`/`_Coll`): the same
  physics as Start. On the last frame: if `isBlasterLoop` is true, loops
  back into Loop itself (`ftFox_SpecialN_BeginLoopTransition` +
  `ftFox_SpecialN_FinishLoopTransition`, which resets `isBlasterLoop` to
  `false` and reinstalls the fire callback for the new cycle); else enters
  End. Either way, if `cmd_vars[2] != 0` this frame, the shot actually fires
  (`ftFox_SpecialN_PrepareBlasterShot` + `ftFox_SpecialN_FireBlasterShot`,
  see "Fire timing" below) -- **the Loop `Anim` callback fires the shot
  itself, on the same frame it decides whether to repeat**, not on a
  separate accessory-callback frame elsewhere in the clip; `accessory4_cb`
  is how the *source's* generic per-frame command dispatcher invokes
  `ftFx_SpecialN_CreateBlasterShot`, but every actual call site in this file
  that matters for gameplay re-derives the same `cmd_vars[2] != 0` firing
  gate directly inside the Anim callback that also handles the loop
  decision. `_IASA` re-checks `CheckLoopInput` every Loop frame too, so a
  fresh B press mid-Loop (not just at the exact repeat frame) can still
  arm the next cycle's repeat.
- **End** (`ftFx_SpecialNEnd_Anim`/`_IASA`/`_Phys`/`_Coll`): plays a fixed
  pose with no further fire/loop checks (`_IASA` is a no-op). Ground:
  `ftFox_SpecialN_UpdateEndAnimation` unhides the gun (`cmd_vars[1]`-driven
  visibility, unmodeled) and, once the pose's frames run out,
  `ftFox_SpecialN_RemoveBlasterNULL` (destroy the cosmetic gun item,
  unmodeled) then `ft_8008A2BC` -- **direct re-entry to `Wait`, no landing
  lag of its own** (this function is already a captured dependency in the
  side special's own oracle harness, see `docs/fox-side-special.md`'s
  "Oracle" section: "Wait re-entry"). Air: same clip-end check, but exits
  through `da->x18_FOX_BLASTER_LANDING_LAG`: `0` -> `ftCo_Fall_Enter`
  (ordinary Fall, no lag at all); nonzero ->
  `ftCo_80096900(gobj, 1, 0, true, 1, x18)` (`FallSpecial`, mobility `1`,
  `allow_interrupt = true`, landing lag `x18` -- the same `ftCo_80096900`
  helper the side special's own End-air exit already uses through
  `specials::helpers::enter_fall_special`).

Ground/air conversion is asymmetric, and different from every other Fox
special modeled so far
------------------------------------------------------------------------

- **Ground Coll, all three phases** is `ft_80083F88`
  (`ft_081B.c:1027-1033`): `if (ft_80082708(gobj) == GA_Ground) { ftCo_Fall_
  Enter(gobj); }` -- i.e. leaving the ground from *any* grounded Blaster
  phase enters **ordinary `Fall`**, not `SpecialAirNStart/Loop/End`. Unlike
  the side special (whose Start/Dash phases *do* convert symmetrically, and
  only End breaks the pattern) or the down special (whose every phase
  converts), Blaster's grounded phases have **no** ground-to-air conversion
  of their own at all: walking off a platform mid-Blaster cancels the move
  outright. This move's `transfer_ground_air` therefore has **no ground ->
  air arm**, letting the existing generic fallback (`!specials::
  transfer_ground_air(f, false) && ... { simulation::enter(f, Action::
  Fall) }`) produce this for free, exactly like the side special's own End
  phase already relies on for its one asymmetric case.
- **Air Coll, all three phases** is `ftCo_AirCatchHit_Coll` (`ft_081B.c`,
  found only by grep since it has no header comment): re-samples ground
  contact (`mpColl_800471F8`) and, if contact is detected and `ft_80081A00`
  (an in-progress-ledge-grab guard, not a ledge-catch attempt) does not
  suppress it, either `ft_8008A2BC` (direct `Wait` re-entry, no lag) when
  `self_vel.y > ftCo_800D0EC8(fp)` (a fast-fall/landing velocity threshold)
  or `ftCo_Landing_Enter_Basic` (the ordinary landing-lag entry) otherwise.
  **No ledge-catch scan of any kind** -- unlike every other Fox aerial
  special modeled so far, `SpecialAirNStart/Loop/End` are not
  ledge-catchable. This is the ground-contact-triggered landing path,
  distinct from End-air's own natural-clip-end exit above (which never
  touches the ground and uses `x18`'s landing lag instead); the two are
  different triggers with different landing rules, both real.
- Because ground air-conversion never happens for this move, `fox_neutral::
  Move::transfer_ground_air` only implements Start<->Loop<->End's air<->
  ground arm (an aerial phase reaching the ground converts to its grounded
  counterpart at the same frame, preserving `is_loop`); the reverse
  direction is absent by design, matching the citation above.

Fire timing and the repeat window (approximated, flagged)
-----------------------------------------------------------

Two pieces of this move's timing are script-driven data this pinned C
snapshot does not expose (no figatree/subaction command stream is checked
into the decomp; only DAT-resource data, owned by the separate
`skirmish-assets` project, would confirm them):

- **Which Loop frame fires the shot.** `cmd_vars[2]` is set to a nonzero
  value by an animation-embedded command at some point in the Loop clip
  (mirroring exactly how a fighter's own attack hitboxes are commands
  embedded in *that* attack's clip -- this project's existing `Attack::
  frames[i].hitboxes` resource already models that same class of
  data for ordinary attacks). Absent the decoded command stream, this port
  fires the shot on Loop's own entry frame (frame 0) of every cycle --
  reproducing one shot per Loop cycle at the correct cadence and total
  count, but not necessarily the exact mid-clip frame the source's own
  accessory callback lands on. Flagged here and in
  `characters::fox::neutral`'s own module doc for a follow-up once the
  export below supplies real per-frame command data.
- **The `cmd_vars[0]` repeat-arming window.** Zeroed at Entry, read by
  `CheckLoopInput` every Start and Loop frame, but this file never assigns
  it a nonzero value itself (also animation-script data). Since Melee's own
  observable Blaster behavior requires *re-pressing* B each shot (holding B
  down without releasing does not keep firing -- `pressed_buttons`, not
  `held`, is what `CheckLoopInput` reads) and nothing in this file ever
  arms it during Start, this port approximates `cmd_vars[0]` as **false
  throughout Start, true throughout Loop**: a fresh B press at any point
  during a Loop cycle arms that cycle's repeat; a fresh press during Start
  does not. This reproduces the known mash-to-repeat behavior without
  claiming the exact scripted window.

Where the hitbox numbers live (export requirement)
-----------------------------------------------------

Unlike every fighter attack this codebase already exports, **the laser's
own damage/angle/knockback hitbox is not represented anywhere in the pinned
C decomp at all.** Melee's item system stores a fired item's active hitbox
capsules in `Item::x5D4_hitboxes[4]` (`melee/it/types.h:315-320`, the same
`HitCapsule` shape a fighter's own `x5D4`-equivalent attack hitboxes use),
populated by the laser Article's own animation-command stream (`ItemStateDesc::
xC_script`, `melee/it/types.h:141-153`) exactly the way a fighter attack's
hitboxes are populated by that attack's own command stream -- i.e. this is
real per-frame data, but it lives in `ItCo.dat`'s (or the equivalent
character-item data file's) opaque script bytes, not in any C source file.
`Article::x0_common_attr` (`ItemAttr`, `melee/it/types.h:80-127`) is
*physical* item data (spin speed, fall speed, item ECBs, grab range, destroy
GFX/SFX) with no damage/knockback fields at all, and `Article::
x4_specialAttributes` (`FoxLaserAttr`, `melee/it/itCharItems.h:265-276`) is
lifetime (`+0`, confirmed) plus a runtime-only visual scale (`+4`) and seven
more unlabeled floats (`+8`, `+C`, `+10`, `+14`, `+18`, `+1C`, `+20`, `+24`)
whose purpose this batch could not determine from source alone (candidates:
travel speed if it differs from `ftFox_DatAttrs.x14_FOX_BLASTER_VEL`, a
visual max-scale value consumed by `Item_UpdateRayAnimation`'s own
`max_scale` parameter, wall-bounce/absorb multipliers -- none confirmed).

### Export requirements (sent to the exporter as its own message, repeated here)

The exporter needs to add, for Fox (`It_Kind_Fox_Laser_Shot`) and Falco
(`It_Kind_Falco_Laser_Shot`) laser items and their owning fighters:

1. **`ftFox_DatAttrs` Blaster fields** (`ftFox/types.h:79-89`, offsets
   relative to the struct's own base already located by the existing
   `ext_attr` export): `x10_FOX_BLASTER_ANGLE` (f32, radians),
   `x14_FOX_BLASTER_VEL` (f32), `x18_FOX_BLASTER_LANDING_LAG` (f32),
   `x1C_FOX_BLASTER_SHOT_ITKIND` (i32 enum, expected constant per
   character -- confirms which Article table entry to read next). The gun
   item kind (`x20`) is cosmetic-only and can be skipped.
2. **The laser Article's `FoxLaserAttr`** (`itCharItems.h:265-276`,
   `Article::x4_specialAttributes` for the item kind from (1)): all ten
   floats verbatim, labeled by offset if their purpose cannot be inferred;
   `lifetime` (`+0`) is the one field this batch can already interpret with
   confidence.
3. **The laser's own hitbox**, read the same way the exporter already reads
   a fighter attack's per-frame hitbox commands, but for the *item's*
   two-state animation (`it_803F67D0`: state 0 = right-facing, state 1 =
   left-facing, both looping the same single pose): damage, launch angle,
   knockback growth/base/weight-independent, hitlag multiplier, shield
   damage, hitbox size and its bone-local offset (`ItemModelDesc::
   x0_joint`-relative, or world-relative if the command stream places it
   in world space directly -- please record which). This is the field this
   batch most needs and could not obtain without guessing, which the task
   brief forbids.
4. **Muzzle spawn geometry**: confirm whether the laser's own `item->pos`
   (drawn/hit position) is the fighter's `FtPart_RThumbNb` hold-joint bone
   (offset `x=0, y=1.2325000762939453, z=4.263599872589111`, per
   `ftFox_SpecialN_GetHoldJoint`) or `ftLib_80086990`'s own fighter-position
   query (reached via `it_8026BB68`, called from `Item_InitRaySpawnPosition`
   for the item's *current* position -- the hold joint is used only for
   `spawn.prev_pos`, the ray-cast anchor for next frame's terrain check).
   This port spawns at the hold joint directly, as a documented
   simplification; if the exporter can dump `ftLib_80086990`'s actual
   behavior (or a captured in-game position sample), that would resolve it.
5. **Whichever Slippi item-frame fields peppi already exposes**
   (`type`/`state`/`position`/`velocity`/`owner`, see "Slippi observation
   plan" below) for a short real Blaster recording, to cross-check this
   port's own motion model against real game output the same way the
   fighter-side batches already cross-check against real replays.

This section was also sent verbatim (abbreviated) to the coordinating
conversation as its own message once drafted, per the task brief, so the
exporter could start in parallel with this batch's own implementation work.

The generic projectile system (`src/game/projectile.rs`)
-----------------------------------------------------------

Scope, deliberately minimal (per the task brief: spawn, per-frame motion,
lifetime/despawn, hurtbox/shield/reflect collision producing the ordinary
damage pipeline with the item's own knockback, no item pickup):

- **Spawn**: `Projectile { kind, owner: usize, position: [f32; 3], angle,
  speed, facing_dir, lifetime_remaining, hitbox, staling identity }`. One
  active projectile at a time per owner mirrors the source's own single
  `fp->u.fx.x222C_blasterGObj`-style bookkeeping closely enough for this
  move (Blaster never spawns a second laser before the first's lifetime
  ends in practice, since the loop cadence is slower than the laser's own
  travel-and-lifetime window); a `Vec<Projectile>` is kept rather than a
  fixed slot so the system stays generic for a future move that does need
  concurrent instances.
- **Motion**: `velocity = speed * (cos(angle), sin(angle))`,
  `position += velocity` every frame the projectile is not itself in
  hitlag (projectiles have no hitlag of their own in the source clips
  read), mirroring `Item_UpdateRayAnimation`'s own vector recompute (the
  visual scale-growth term is drawing-only and unmodeled).
- **Lifetime**: a plain per-frame countdown (`FoxLaserAttr.lifetime`,
  frames), despawning at zero, mirroring `it_80275158`.
- **Terrain despawn**: approximated as leaving the stage's own outer
  bounding box, rather than the source's true swept ray-vs-terrain-line
  cast (`it_8026E9A4`/`it_8029C4D4`). A real stage-line raycast would reuse
  this codebase's existing `collision::stage::Stage` line intersection, but
  wiring a projectile through that geometry (which today only fighters'
  own ECB/foot collision uses) is out of this batch's scope; flagged as a
  simplification, not silently dropped.
- **Hurtbox collision**: builds a world-space `fighter::combat::Capsule`
  from the projectile's previous and current position this frame (a swept
  segment, exactly mirroring how this codebase already sweeps a fighter's
  own bone-attached hitboxes frame to frame in `hitboxes::update_tracks`,
  and why a fast-moving projectile needs a segment rather than a single
  point at all), then reuses `collision::shield::capsule_matrix` against
  each of the target's hurtboxes -- the exact same primitive
  `simulation::advance`'s own fighter-vs-fighter hit loop already calls --
  gated by the same eligibility checks that loop already applies
  (invincibility, intangibility, `body_state.accepts_contact()`, grab
  captor, shield-break action, Respawn/Eliminated, rebirth invulnerability,
  `death::owns_action`). A confirmed hit calls `damage::apply_hit` directly
  with a synthesized `Hitbox` from the laser's own resource data and
  `HitDirection::FighterContact`, so every existing damage/knockback/
  hitlag/hitstun/DI/staling rule the fighter-vs-fighter path already
  implements applies unchanged; the projectile despawns after one hit (no
  piercing -- consistent with `itFoxLaser_Logic94_Absorbed`/`_Clanked`
  both returning `true`, "this item is now spent").
- **Shield collision**: reuses `shield::geometry` + `collision::shield::
  capsule_matrix` (the same call `simulation::advance`'s own fighter loop
  makes for a fighter's melee hitbox against a shield) to detect contact
  with an active shield. On contact, mirrors `Item_BounceRayOffShield`
  (`lbVector_Mirror` against the shield's own contact normal, then
  `angle = atan2(vel.y, vel.x)`): the laser survives, reverses off the
  shield, and keeps its own owner (so it can still hit its original owner
  back if the shield-holder is a second projectile-eligible target in a
  future multi-projectile match, though today's two-player-only pipeline
  makes that moot). `itFoxLaser_Logic94_HitShield`'s own separate,
  simpler "destroy on shield contact" callback exists in the same logic
  table; this batch could not confirm which of `ShieldBounced`/`HitShield`
  the source actually dispatches to for a live shield vs. a different
  circumstance (post-hitstun shield stagger, a broken shield, etc.) without
  deeper `item.c` archaeology than this batch's time budget allowed, so
  only the bounce behavior is modeled; flagged as an open question, not a
  silent guess presented as fact.
- **Reflector collision**: if the projectile's swept capsule intersects a
  fighter whose `fighter.shield.reflecting` bit is set (the *existing*
  field `characters::fox::down`'s own Reflector already sets on every Loop/
  Turn/Hit entry, previously documented as having "no effect in this
  engine -- there are no projectiles to reflect", see `docs/
  fox-down-special.md`), the laser flips `owner` to that fighter, reverses
  its facing/angle by `pi` and resets its visual scale
  (`itFoxLaser_Logic94_Reflected`/`Item_ResetRayAfterReflection`), and
  keeps flying. **This is the first gameplay effect the `reflecting` bit
  has ever had in this codebase.**
- **Staling**: the laser stales as `FtMoveId_SpecialN` (a single fixed move
  identity shared by every laser regardless of which Blaster phase fired
  it, matching the task brief); this port allocates one `staling::Entry`
  per spawned projectile from the owner's own `state.attack_instances`
  counter at spawn time, exactly like a fighter's own attack instance is
  allocated once per distinct attack use.
- **No item pickup, no clank-vs-item, no absorption** (no absorbing
  character exists in this codebase): all confirmed out of scope by the
  task brief and left unmodeled.

Resource and state shape
---------------------------

- `characters::Specials::Fox.neutral` changes type from the shared shell's
  `specials::neutral::Parameters` to this move's own
  `characters::fox::neutral::NeutralSpecial { neutral_thresholds: [f32; 2],
  start: Phase, loop_phase: Phase, end: Phase, attributes: Attributes,
  laser: Laser }` (`Phase` reused from `characters::fox::side::Phase`,
  matching `down.rs`'s own precedent of reusing that shape rather than
  redeclaring an identical struct). `Attributes { angle, speed,
  landing_lag }` (`x10`/`x14`/`x18`). `Laser { lifetime, hitbox:
  data::Hitbox }` reuses the existing fighter `Hitbox` shape verbatim for
  the projectile's own single capsule, rather than inventing a parallel
  type, since every field it needs (`damage`, `angle_degrees`, `growth`,
  `fixed`, `base`, `shield_damage`, `center`, `radius`) already exists
  there and `bone`/`clank`/`rebound`/`element`/`group` are simply unused by
  a projectile (`bone: 0`, `group: 0`, rest at their defaults).
- `Fighter.fox_neutral_special: characters::fox::neutral::State { is_loop:
  bool, pending_shot: bool }` -- `is_loop` mirrors `isBlasterLoop`;
  `pending_shot` is this port's own signal from the per-fighter `SpecialMove`
  dispatch (which cannot itself reach `State::projectiles`) up to
  `simulation::advance`'s own per-frame loop (which can), read and cleared
  once per frame immediately after the ordinary fighter update-actions/
  animation pass, matching "item logic runs after fighters."
- `State.projectiles: Vec<projectile::Projectile>`, included in checkpoints
  like every other match-state field (`State` already derives `Clone` +
  `Serialize`).
- New `Action` variants: `SpecialNStart`, `SpecialNLoop`, `SpecialNEnd`,
  `SpecialAirNStart`, `SpecialAirNLoop`, `SpecialAirNEnd`, replacing the
  shell's single `SpecialN`/`SpecialAirN` pair everywhere in this codebase
  (see "What else changes" below).
- New `Event` variants for observation/replay-debugging convenience:
  `ProjectileSpawned { owner, kind }`, `ProjectileHit { owner, victim }`,
  `ProjectileReflected { owner }`, `ProjectileDespawned { owner }`.

What moves, what doesn't (retiring the shared shell for Fox)
----------------------------------------------------------------

`characters::Specials` has exactly one variant (`Fox`) today; the shared
`game::specials::neutral` shell's own end-to-end tests (`tests/
game_special.rs`, `tests/support/special.rs`) exercised it *through* Fox,
since no other character exists to attach it to. Once Fox has his own
dedicated Blaster, nothing in the resource graph can reach the shared
shell's `SpecialMove` impl at all -- keeping it would be untested,
unreachable code with no way to verify it still compiles correctly against
its own trait. This batch retires `src/game/specials/neutral.rs` and its
two test files outright (not merely stops calling it), consistent with
"refactor freely" and the project's existing practice of not keeping dead
code around "for later." A future second character that only needs a
plain single-phase neutral special can re-add an equivalent small shell
trivially; `fighter::special::neutral_input` (the actual retained decomp
boundary, independent of the shell) is untouched and still covered by its
own existing oracle differential (`special_differential.rs`), since Fox's
new dedicated module calls that same pure function directly.

What else changes (mechanical, not this move's own behavior)
-------------------------------------------------------------

- `Action::SpecialN`/`SpecialAirN` were also reused as a convenient "some
  neutral-special action" stand-in by unrelated feature tests
  (`tests/game_damage_surface.rs`, `tests/game_wall_jump.rs`) checking what
  a fresh, centered-stick B press produces after hitstun/wall-tech/damage-
  fall clears. Those assertions describe genuine dispatch behavior (not an
  arbitrary sentinel), so they are updated to the real post-hitstun result,
  `Action::SpecialNStart`/`SpecialAirNStart`.
- `fighter::action_instance::motion_identity`'s `SpecialN | SpecialAirN =>
  17` (the shared low-byte "table startup identity", `x2070.x2073`) becomes
  all six new variants mapping to the same `17`, preserving one continuous
  Slippi `instance_id` across an entire Start->Loop->...->End sequence,
  matching the source's own single identity for the whole
  `ftFx_SpecialNIndex` family.
- `crates/skirmify-replay::observation::action_state`'s match arm and its
  own unit test gain the six new states at Slippi 341..346 (see "Slippi
  ids" below) in place of the old 341/344 pair.
- `crates/cli/tests/replay_match.rs`'s
  `physical_b_drives_file_backed_neutral_special_and_detects_its_removal`
  previously borrowed the shared shell plus Fox's own jab hitboxes as a
  convenient "does neutral-B dispatch and hit at all" stand-in; it is
  rewritten against this move's own real fixture (a laser that travels and
  hits the second fighter after a short delay, not an instant melee
  hitbox), keeping the same "removing the B press changes the recorded
  bytes" regression shape.

Slippi ids
-------------

`ftFx_MS_SpecialSStart == ftCo_MS_Count + 6` was already confirmed
(`docs/fox-side-special.md`); since Fox's own character-specific block
begins at `ftFx_MS_SpecialNStart` (`ftFox/forward.h`'s own declaration
order lists Neutral before Side), `ftCo_MS_Count == 341` and:

| Action | Slippi state | Animation index |
|---|---|---|
| `SpecialNStart` | 341 (confirmed, previously mismodeled as plain `SpecialN`) | 295 (confirmed) |
| `SpecialNLoop` | 342 | 296 (extrapolated, -46 offset) |
| `SpecialNEnd` | 343 | 297 (extrapolated) |
| `SpecialAirNStart` | 344 (confirmed, previously mismodeled as plain `SpecialAirN`) | 298 (confirmed) |
| `SpecialAirNLoop` | 345 | 299 (extrapolated) |
| `SpecialAirNEnd` | 346 | 300 (extrapolated) |

The animation indices continue the same unverified -46 constant-offset
extrapolation the side/up/down batches already flagged (no figatree table
exists in the pinned decomp for character-specific motion states); this is
the same open question those docs raised, not a new one.

Real-replay cross-check (item 5 of the export requirements, done directly)
------------------------------------------------------------------------------

Dumped `FOX_LASER`/`FOX_BLASTER` item frames (py-slippi, `Game(path).frames[i].items`)
from `/mnt/archive/datasets/melee/slippi-public-dataset-v3.7/data/FOX/batch_00/
18_24_36 [H2O] Fox + Fox (FD).slp` (no `owner` field in this py-slippi
version's parsed `Item`; correlated by proximity/timing instead). Confirms,
independent of source-reading:

- **Speed**: a grounded neutral shot's `velocity` is exactly `(7.0, 0.0)`
  every frame of its flight (`x14_FOX_BLASTER_VEL == 7.0`).
- **Motion**: `position` advances by exactly `velocity` every frame with no
  drift or scaling, confirming the plain `position += velocity` model.
- **Lifetime**: first-observed `timer` is `34.0`, decrementing by exactly
  `1.0` every frame -- consistent with a 35-frame lifetime and matching
  `FoxLaserAttr`'s own `// [35]` comment on `lifetime` exactly.
  the item's `damage` field jumps from unset to `3` on the frame it
  disappears early (before its timer reaches zero), and the corresponding
  fighter's own `post.damage` (percent) increases by `3.0` the same frame:
  **the laser's own hit damage is 3%**, a real, confirmed value (matches
  Melee community knowledge of Blaster's damage, now independently
  verified from a real replay rather than assumed).
- **On hit**: the item disappears from the frame's item list outright (no
  post-hit "spent" sub-state observed) -- consistent with "no piercing,
  destroyed after one hit."
- **Open question, not resolved**: several later shots in the same
  replay have non-axis-aligned velocities (e.g. `(0.414, 6.988)`,
  `(1.761, 6.775)`) rather than the fixed-per-facing angle
  `ftFox_SpecialN_PrepareBlasterShot` (`fp->facing_dir == 1.0F ? x10 : PI -
  x10`) would predict from source alone. This batch could not locate the
  actual code path that varies the shot angle (Melee community knowledge
  describes an up/down-stick "Blaster angling" tech; the cited source
  function shows no stick read at all, so either a different function
  computes the real angle or this batch mis-traced the call graph). Flagged
  as an open discrepancy between the read source and observed real
  behavior; **this implementation models the fixed-per-facing angle only**,
  matching what was actually read in the decomp, not the observed angling.
  The synthetic fixture below uses the confirmed `speed = 7.0`,
  `lifetime = 35`, `damage = 3` values from this replay rather than
  invented numbers; every other hitbox field (angle/growth/base/
  weight-independent knockback/hitlag/shield damage/size) remains
  genuinely invented pending the exporter, and is flagged as such at its
  point of use.

Slippi observation plan for items
-------------------------------------

`docs/replays.md` currently states items are not compared. Slippi/peppi
expose, per item-frame: `id` (a per-item spawn-order identifier), `type`
(the external item kind, e.g. Fox's laser), `state` (an item-internal
sub-state byte), `position`/`velocity` (both f32 pairs), `damage_taken`,
`expiration_timer`, `spawn_id`, `missile_type`/`turnip_type`/`is_launched`/
`charge_power` (character-specific unions, irrelevant here), and `owner`
(added in a later Slippi version; `-1` when absent/unowned). This batch
adds an `observation::items` query returning, per active `Projectile`:
`kind` (mapped to peppi's own external item-type id the same way
`characters::fox::slippi_ids` maps action states), `position`, `velocity`
(derived from `angle`/`speed`), and `owner` (this engine's own 0/1 player
index, mapped the same way action-state owner attribution already works
elsewhere). `state`/`expiration_timer`/`spawn_id`/`id` are not modeled this
batch (no multi-instance id allocation scheme exists yet for a system that,
today, only ever has Blaster's own single-projectile-per-owner shape) and
are left as an explicit gap for whichever future batch needs concurrent
same-owner projectiles or true item ids. `crates/cli`'s own replay
comparison harness is *not* extended to diff these fields against a real
Slippi file's item block in this batch -- that requires the file-backed
comparison pipeline (`docs/replays.md`) to grow an item-aware diff pass
mirroring its existing per-fighter-post diff, which is a second, separable
piece of work; this batch's own self-recorded regression instead asserts
the *native* `Event`/`Projectile` state directly (spawn, travel, hit),
which this port fully controls and can assert exactly, and documents the
real-Slippi-item-diff work as the concrete next step in `docs/replays.md`.

Tests
--------

`fighter::characters::fox::neutral` (pure arithmetic, if any is added
beyond what already exists) and `game::projectile` unit-test motion,
lifetime countdown, the shield-bounce reflection formula and the
Reflector hand-off in isolation from the full `Match` pipeline.
`tests/game_fox_neutral_special.rs` covers: strict-threshold entry
matching the retired shell's own conformance (both slots, competing
inputs, release/repress rearming -- migrated from `tests/game_special.rs`
rather than dropped), Start -> Loop -> End with and without a repeat press,
the laser actually spawning and traveling, a real hit applying damage/
knockback/hitstun through the ordinary pipeline, staling across repeated
hits, a shield bounce, a Reflector hand-off (owner flips, the reflected
laser can hit its original owner), lifetime despawn, the asymmetric ground-
leaves-to-Fall / air-lands-to-Wait-or-Landing conversions, the Slippi ids,
and a checkpoint round trip including in-flight projectiles.

## Known gaps and deviations (see inline citations above for detail)

- The exact Loop frame the shot fires on, and the exact `cmd_vars[0]`
  repeat-arming window, are both animation-script data this decomp does
  not expose; approximated as "Loop's own entry frame" and "true
  throughout Loop, false throughout Start" respectively.
- The laser's own damage/angle/knockback hitbox values do not exist in the
  pinned C decomp at all (they are the item's own per-frame animation
  command data); this batch's fixture is a synthetic, hand-built value set
  pending the export requirements above, clearly not claimed as the real
  game's numbers.
- Terrain/wall despawn is approximated as leaving the stage's outer
  bounding box, not a true swept ray-vs-terrain-line cast.
- `ShieldBounced` vs `HitShield`'s exact dispatch conditions in the source
  are not fully disambiguated; only the bounce behavior is modeled.
- The real-Slippi-item-field replay diff (as opposed to this batch's own
  native self-recorded regression) is scoped out, with the concrete next
  step recorded in `docs/replays.md`.
- The muzzle spawn position uses the hold-joint bone directly rather than
  confirming `ftLib_80086990`'s own transform.
