# Jigglypuff special callback specification

Source: pinned `../../External/melee/src/melee/ft/kinds/ftPurin/` decomp.
The motion-state table is in `ftpurin.c`; special callbacks are split between
`ftpurinspecialn.c`, `ftpurinspecials.c`, `ftpurinspecialhi.c`, and
`ftpurinspeciallw.c`.

## Rollout (neutral special)

The native states are 346--362:

| States | Phase | Native callback family |
| --- | --- | --- |
| 346/347, 354/355 | ground/air start, facing left/right | `ftPr_SpecialNStart_Anim`, `ftPr_SpecialAirNStart_Anim` |
| 348/349, 356/357 | ground/air charge loop/full | `ftPr_SpecialNLoop_Anim`, `ftPr_SpecialNFull_Anim`, and air counterparts |
| 350, 358 | ground/air release | `ftPr_SpecialNRelease_Anim`, `ftPr_SpecialAirNChargeRelease_Anim` |
| 351, 359 | ground/air turn | `ftPr_SpecialNTurn_Anim`, `ftPr_SpecialAirNStartTurn_Anim` |
| 352/353, 360/361 | ground/air end, facing left/right | `ftPr_SpecialNEnd_Anim`, `ftPr_SpecialAirNEnd_Anim` |
| 362 | hit / landing recovery | `ftPr_SpecialNHit_Anim` and `ftPr_SpecialNHit_Coll` |

The charge state advances `specialn.x2C` by `xA8` and clamps it to `xA4`.
Releasing B enters release and seeds horizontal speed from `xC0 * (x2C-xA0)`.
Ground release uses `xB4`, `xB8`, `xC0`, `xC4`, and `xC8` for deceleration,
slope influence, and velocity limits. Air release uses `x58`, `x5C`, `x3C`,
and `x40`. These are fighter attributes, not item/article values.

### Hit capsule and scale

Rollout's C callbacks do not create the capsule geometry. The animation
command stream creates capsule slot 0. `ftPr_SpecialS_8013D8E4` then applies
the source-specific active/damage policy during release:

* Ground speed is `abs(gr_vel)`; air speed is `abs(self_vel.x)`.
* If speed is below `xCC`, slot 0 is disabled. Once speed reaches `xCC`, a
  disabled slot is re-enabled.
* While enabled, damage is the integer result of
  `x84 * (x80 + abs_speed)`, clamped to at least 1.
* `hitCapsuleToggle` increments `specialn.xC`; every `x9C` release frames it
  toggles the capsule group between 0 and 1, refreshing victim history while
  the same capsule continues rolling.
* Release and turn animation callbacks apply the four-frame scale sequence
  from `ftPr_Init_803D05C8`/`803D05D8`:

  ```text
  y: 0.65, 0.70, 0.80, 1.00
  z: 1.10, 1.35, 1.30, 1.20
  ```

  These values multiply Jigglypuff's saved base scale; after four frames the
  base scale is restored.

Wall contact in release reverses horizontal velocity and multiplies charge
and speed by `xD4`; the source also emits effect `0x406`, requests a medium
camera quake, and plays SFX `250070`. Those presentation and attribute-scaled
collision details remain host-owned in the current script runtime.

### Test vector

For a grounded release with `gr_vel = 3.0`, `xCC = 0.5`, `x80 = 1.0`, and
`x84 = 2.0`, slot 0 remains enabled and the source damage sample is:

```text
2 * (1 + abs(3)) = 8
```

At `gr_vel = 0.25` with the same attributes, the callback disables slot 0 and
emits no damage sample. With `x9C = 2`, release frames 2 and 4 toggle the
capsule group, matching the native victim-history refresh cadence.

## Sing (up special)

States 365--368 are ground/air and facing left/right. The enter callbacks set
`specialhi.x0` only when the Stadium condition in `gm_8016B1D8()` and
`grStadium_801D4FF8(player_id)` is true. During animation, if `x0` is set and
the first hit capsule is enabled, the callback changes its element to
`HitElement_Sleep`. This is a capsule-element mutation, not an article.

`ftPr_Init_8013C94C` is installed as the accessory callback on entry. It
spawns effect `1238` once at the waist joint, installs hitlag effect callbacks,
and clears itself. Ground animation end enters Wait; aerial animation end
enters Fall. Ground/air collision preserves the selected facing phase and
current frame.

## Rest (down special)

States 369--372 are ground/air and facing left/right. The enter callbacks
select the facing animation, reset command slot 0, and install
`ftPr_SpecialHi_8013CE7C`, which clears the accessory callback. Ground
animation end enters Wait; aerial animation end enters Fall. Surface changes
preserve the corresponding native phase and frame.

Rest's C callbacks contain no explicit hitbox creation or damage formula. Any
Rest hitbox is therefore part of the state animation command stream/resources,
with ordinary native hitbox activation semantics. The decomp does not expose a
Rest-specific item article or fighter-side capsule geometry in
`ftpurinspeciallw.c`.

## Articles and item ownership

There is no Jigglypuff special item article in the pinned fighter sources.
The `ftPurin` files contain no `itCreate`, article-owner, projectile, or item
state setup for neutral, side, up, or down special. The item-related functions
in `ftpurin.c` only handle generic pickup/visibility/drop behavior. The
special-specific native outputs are fighter capsules, capsule element changes,
accessory effects, scale/rotation, and SFX. The host should not invent a
Jigglypuff item article for Rollout, Sing, Pound, or Rest.

## Pound (side special)

States 363/364 use command traces `side.script.ground` and
`side.script.air`. Air command slot 0 applies the launch angle from the stick
and `STICK_ANGLE_MIN`, `STICK_ANGLE_MAX`, `MAX_LAUNCH_ANGLE`, and
`LAUNCH_SPEED`; the command is then cleared. Ground and air physics/collision
callbacks are distinct, and both preserve the corresponding source phase on a
surface change. The current script exports this command trace and launch
formula; the trace resources remain required for complete behavior.
