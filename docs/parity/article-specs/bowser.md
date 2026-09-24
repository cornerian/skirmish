# Bowser Flame Article Contract

This is a source-backed article contract for Bowser's neutral special.  It
records the pinned decomp behavior that a native article host must expose; it
does not claim that the current fighter-only Python script implements the
article.

The authoritative files are:

- `../../../../External/melee/src/melee/it/kinds/itkoopaflame.c`
- `../../../../External/melee/src/melee/it/kinds/itkoopaflame.h`
- `../../../../External/melee/src/melee/ft/kinds/ftKoopa/ftkoopaspecialn.c`
- `../../../../External/melee/src/melee/ft/kinds/ftKoopa/ftkoopa.c`

## Spawn and state

`ftKp_SpecialLw_80134ACC` calls `itKoopaFlame_Spawn` from the flame-breath
IASA callback.  The spawn receives the mouth position, owner facing, the
owner's current flame count flag, a GFX selector, and owner-scaled speed and
size values.  `itKoopaFlame_Spawn` then:

- assigns the owner as both parent references and starts at the supplied
  position;
- initializes lifetime from article attributes (`x0_lifetime = 28.0`), with
  hitbox lifetime `x4_hitbox_lifetime = 20.0`;
- computes random speed in `[1.9, 2.2]` times the owner base speed;
- computes random launch angle in `[2.1816616, 2.5307274]` radians, negating it
  for left facing;
- initializes direction to the owner's facing and starts the GFX latch and
  frame counter at zero.

The article has one state in `ItemStateTable_KoopaFlame`: animation
`itKoopaFlame_UnkMotion0_Anim`, physics `itKoopaFlame_UnkMotion0_Phys`, and
collision `itKoopaFlame_UnkMotion0_Coll`.

`itKoopaFlame_UnkMotion0_Phys` writes velocity as
`speed * (sin(angle), cos(angle))` every frame.  The animation callback scales
the active hitbox once from its original scale, applies the owner scale, emits
the first effect only once (`efSync_Spawn(1243 + gfx)` for a Koopa flame), and
destroys the article when the frame counter becomes greater than 20.

## Surface and article callbacks

`itKoopaFlame_UnkMotion0_Coll` expands the article ECB to 3.0 on every side,
updates environment collision flags, and calls
`itKoopaFlame_Update_Direction` followed by `itKoopaFlame_Update_Angle` when a
floor, ceiling, or wall is touched.  Direction adds the contacted surface
normal and normalizes.  Angle steering uses the signed wrapped difference
between velocity and direction, with the source's `0.02 * abs(delta)/pi`
small-angle factor and `0.5 * abs(delta)/pi` wide-angle factor.

The item callbacks in `itKoopaFlame_Logic111_*` have these observable results:

| Callback | Source behavior |
| --- | --- |
| `DmgDealt` | Returns `false`; the flame remains alive. |
| `Reflected` | Adds pi to angle, clamps it, flips facing, and negates velocity; returns `false`. |
| `Clanked` | Returns `false`; the flame remains alive. |
| `Absorbed` | Destroys all article effects and returns `true` so the item is consumed. |
| `ShieldBounced` | Mirrors velocity against the stored collision normal, zeros Z, recomputes angle with `atan2(y, x)`, and returns `false`. |
| `HitShield` | Returns `false`; the flame remains alive. |
| `EvtUnk` | Calls `it_8026B894` with the referenced article object. |

`ftKp_Init_OnLoad` registers the Koopa flame item kind with the fighter item
archive (`it_8026B3F8(items[0], It_Kind_Koopa_Flame)`) and enables the fighter's
article bit.  The decomp does not provide a separate Bowser death callback
that sweeps flames; owner and effect cleanup therefore belongs to the item
engine's parent/owner lifecycle plus the article callbacks above.

## Exact deterministic test vector

Use a Koopa flame with article attributes exactly as above, owner attributes
`ftKp_SpecialLw_80134DE0 = 10` and `ftKp_SpecialLw_80134E1C = 20`, spawn inputs
`base_speed = 10`, `scale = 20`, `facing = +1`, and force both calls to
`HSD_Randf()` to return `0.5`.  Set `gfx = 0`, position `(3, 4, 0)`, and no
surface collision.

The expected post-spawn values are:

```text
x38_base_speed = 1.0
x3C_scale      = 1.0
x28_speed      = 2.05
x24_angle      = 2.3561945...  (pi * 3/4)
x0_pos         = (3, 4, 0)
xC_direction   = (1, 0, 0)
```

The first physics callback must set approximately
`x40_vel = (1.4495689, -1.4495689, 0)`, from `2.05 * (sin(angle), cos(angle))`.
The first animation callback must emit effect ID `1243`, scale the active
hitbox by `1.0`, and advance the frame counter from 0 to 1.  With no further
callbacks, the article is destroyed after the counter advances past 20.

## Current host gaps

The current `scripts/fighters/bowser.py` exports only fighter motion phases
341–346 for Flame Breath and deliberately has no article emission or item
collision callbacks.  The portable fighter API has no article archive handle,
owner-linked item object, article ECB/surface-normal state, deterministic
article RNG stream, or effect cleanup operation.  Implementing the flame in
Python without those seams would fabricate behavior and would not satisfy the
source contract above.

## Fighter phase audit

The fighter script covers the four native special families and their source
phase ranges:

| Family | Source phases | Scripted timing |
| --- | --- | --- |
| Flame Breath | 341–346 (`ftkoopaspecialn.c`) | Start enters the loop, B release enters 343/346, and ground/air changes preserve the current frame. |
| Koopa Klaw | 347–358 (`ftkoopaspecials.c`) | Start contact enters hit; the ground hit path reaches wait 350, while the aerial hit path enters 355 when B is latched and otherwise reaches wait 356; directional input selects forward/back throw ends. |
| Whirling Fortress | 359–360 (`ftkoopaspecialhi.c`) | Ground and air phases convert on contact and finish to wait/fall. Native armor, effects, and velocity callbacks remain outside the fighter-only API. |
| Bowser Bomb | 361–363 (`ftkoopaspeciallw.c`) | Ground pound changes to aerial phase, landing selects 363, and landing animation finishes in fall. The native aerial entry seeks animation frame 30, which the generic transition descriptor cannot encode. |

The Klaw hold check follows `ftKp_SpecialSWait_IASA`: native code tests
`held_buttons & HSD_PAD_B`, where `HSD_PAD_B` is `1 << 9`. The portable
adapter tests bit `0x200` for integer button masks, and its hit animation-end
callback consumes the typed `BowserActionState.klaw_b_held` latch without
sampling a potentially changed current mask.
