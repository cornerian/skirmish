# Luigi article and special parity

This specification records the source behavior that the Luigi script must
preserve at the native boundary. The source paths below are relative to the
pinned `External/melee` checkout.

## Fireball article

| Property | Source behavior |
| --- | --- |
| Article identity | `It_Kind_Luigi_Fire`; the authoring `ArticleId.LUIGI_FIRE` is wire ID `105`. |
| Fighter entry | `ftLuigi/ftluigispecialn.c`, `ftLg_SpecialN_Enter` and `ftLg_SpecialAirN_Enter`, motion states 341 and 342. |
| Spawn callback | `ftluigispecialn.c`, `ftLg_SpecialN_FireSpawn`; the article is spawned at the fighter's `L1ST_NB` part with the current facing. |
| Spawn setup | `it/kinds/itluigifireball.c`, `it_802C01AC` and `it_802C027C`; velocity is `(attrs.x0 * facing, 0, 0)`, and the article lifetime is initialized from `attrs.x4`. |
| Article motion | `itLuigifireball_UnkMotion0_Anim` decrements the lifetime; `UnkMotion0_Phys` applies falling physics; `UnkMotion0_Coll` destroys on collision below `attrs.xC` speed and otherwise emits the Luigi fire effect `1288`. |
| Contact callbacks | `itLuigiFireball_Logic89_DmgDealt`, `Clanked`, `HitShield`, and `Absorbed` return true; `Reflected` delegates to `it_80273030`; `ShieldBounced` delegates to `itColl_BounceOffShield`. |

The normal `HitShield` callback therefore consumes the fireball. The separate
`ShieldBounced` callback is a distinct source event; the typed host preserves
that distinction and leaves its bounce handoff unsupported. `Reflected` is
also gated until the typed article attributes expose the source `xC70` speed
multiplier; the current Luigi resource therefore uses no reflection policy.

### Test vector

For a fireball spawned at `(0, 0, 0)` with facing `-1`, article attributes
`x0 = 1.5`, `x4 = 30`, and `xC = 0.5`, the expected initial article state is
velocity `(-1.5, 0, 0)` and lifetime `30`. A collision with speed `1.5`
keeps the article alive and emits effect `1288`; a collision with speed `0.5`
or below destroys it.

## Green Missile

`ftLuigi/ftluigi.c` registers states 343–354:

| States | Native phases and source callbacks |
| --- | --- |
| 343/349 | Grounded/aerial start; `ftLg_SpecialSStart_Anim`, friction physics, and ground/air collision conversion. |
| 344/350 | Grounded/aerial charge; release in `ftLg_SpecialSHold_IASA` or `ftLg_SpecialAirSHold_IASA`; charge auto-launches after `xC_LUIGI_GREENMISSILE_MAX_CHARGE_FRAMES`. |
| 347/353 | Normal launch; charge-scaled hit damage and transition to flight only after command slot 0 is raised by the launch animation. |
| 348/354 | Misfire launch; selected by `HSD_Randi(x44_LUIGI_GREENMISSILE_MISFIRE_CHANCE)` and using the same command-gated flight transition. |
| 345/351 | Flight; grounded S2 has empty animation and collision callbacks, while `ftLg_SpecialAirS2_Anim` enters end and aerial landing or wall contact enters grounded end. The native flight setup always selects aerial S2 (351). |
| 346/352 | Grounded/aerial end; grounded end exits to wait, aerial end exits to fall. |

`ftLg_SpecialS_Anim` and `ftLg_SpecialAirS_Anim` poll command slot 0 every
animation tick, so launch remains active when the animation ends before the
native launch cue. `ftLg_SpecialSFly_Enter` then clears command slot 0 and selects velocity from
either the charge fields (`x24`, `x28`, `x2C`) or misfire fields (`x48`, `x4C`).
Flight gravity and horizontal deceleration use `x30`, `x34`, `x3C`, and
`x40`; end physics uses `x3C` and `x40`. Damage uses
`chargeFrames * x14 + x10` while misfire is false.

## Cyclone

`ftLuigi/ftluigispeciallw.c` registers states 357 and 358. Entry clears
command slots 0–2 and initializes grounded momentum state. The grounded and
aerial physics callbacks use `x74`, `x78`, `x7C`, `x80`, and `x84`; command
slot 2 plus held B performs the aerial tap using `x8C` and `x90`. Aerial
animation consumes command slot 1 into the persistent `x222C_cycloneCharge`
marker before choosing fall or landing lag `x94`. Collision also resets
command slot 2 and applies the source fall-speed and horizontal clamps.

## Current host boundary

The fighter script declares states, directional entry, source transitions,
command resets, Green Missile release, and Cyclone command-slot charge/tap
branches. Native article spawning, article collision/contact callbacks,
Green Missile RNG and attribute-scaled velocity/damage, and Cyclone held-B
physics with character attributes still require host support. These effects
must remain native-owned until the resource and callback interfaces expose
the corresponding article and Luigi attribute data.

## Timing audit

The four move families retain their source timing at the script boundary:

* Fireball 341/342 exits to wait or fall when animation ends. The source IASA
  callbacks observe command slot 0 and may process regular input, but the
  shared script host does not model that generic IASA polling path.
* Green Missile 343–354 advances start to charge on animation completion,
  launches on B release or the native charge cap, and waits for command slot 0
  before entering aerial flight. Its unused ground S2 row has no terminal
  animation or contact transition; leaving grounded end goes directly to fall.
* Super Jump Punch 355/356 runs its source fall-special landing path at
  animation completion; the declared wait/fall targets are the host's
  no-landing-lag fallback.
* Cyclone 357/358 consumes aerial command slot 1 before its terminal callback,
  and command slot 2 plus held B performs the tap physics branch. Ground/air
  contact preserves the current frame, while terminal ground/air exits use
  wait/fall when no native landing-lag descriptor is available.
