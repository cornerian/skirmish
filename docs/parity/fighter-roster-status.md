# Fighter roster parity status

This is a source backed inventory of the 26 checked in CSS fighters. “Authored
coverage” means the Python declaration currently exports native motion/action
identities and the input, animation, command, and ground/air callbacks that
the portable API can represent. It does not mean that the fighter is
behaviorally complete. Article archives, capture objects, companion fighters,
and native collision callbacks remain host work unless the row says otherwise.

| Fighter | Authored motion and command coverage | Major native only gaps | Pinned source anchors |
| --- | --- | --- | --- |
| Captain Falcon | Special action descriptors, command traces, family entry/rebound callbacks | Falcon Dive victim capture, throw/capture object lifecycle, native hit geometry | `ftCaptain/ftcaptainspecials.c`, `ftCaptain/ftcaptainspecialhi.c` |
| Donkey Kong | Spinning Kong ground/air motion profiles, typed attributes, entry and surface callbacks; Giant Punch charge/release state | Giant Punch hand hitbox callback, cargo/grab state | `ftDonkey/ftdonkeyspecialhi.c`, `ftDonkey/ftdonkeyspecialn.c` |
| Fox | Blaster, Illusion, Fire Fox, Shine phases; command state, launch, steering, landing, article emission | Reflector/item handoff details, full item archive effects and native collision/absorption/clank paths | `ftFox/ftfoxspecialn.c`, `ftfoxspecials.c`, `ftfoxspecialhi.c`, `ftfoxspeciallw.c`; `itFoxLaser/itfoxlaser.c` |
| Game & Watch | Chef, Judge, Fire, Oil Panic source phases and optional Judge data routing | Chef food, Judge article/effect selection, Fire parachute, Oil Panic bucket/reflection storage | `ftGameWatch/ftgw*.c`, `itGameWatch*.c` |
| Kirby | Four source special phase graphs and directional input gates | Copy capture/ability replacement, spit/star and copied-special dispatch, article ownership | `ftKirby/ftkirbyspecial*.c` |
| Bowser | Special phase graph, resource gates, finite surface/animation lifecycle | Flame article, side-special victim capture and throw/item callbacks | `ftKoopa/ftkoopaspecial*.c`, `itKoopaFlame/itkoopaflame.c` |
| Link | Boomerang, bow, bomb, and Spin Attack motion/command phases; release and held-item branches | Arrow/boomerang/bomb item physics, pickup/return/ownership, hit and reflection callbacks | `ftLink/ftlinkspecial*.c`, `itLink*.c` |
| Luigi | Fireball article phase, Green Missile charge/misfire graph, Cyclone phases | Fireball, missile, misfire effects, cyclone launch and native item callbacks | `ftLuigi/ftluigispecial*.c`, `itLuigiFire/itluigifireball.c` |
| Mario | Fireball article phase plus Cape, Super Jump Punch, and Mario Tornado motion lifecycles | Fireball/cape effects, Mario Tornado movement/effects and native article callbacks | `ftMario/ftmariospecial*.c`, `itMarioFire/itmariofireball.c` |
| Marth | Shield Breaker, Dancing Blade branch phases, Dolphin Slash, Counter phases | Sword hitbox/command branches and counter grab/reflection details | `ftMars/ftmarsspecial*.c` |
| Mewtwo | Shadow Ball, Confusion, Teleport, Disable phase/state declarations and local latches | Shadow Ball charge article, Confusion reflect/grab object, Disable collision article | `ftMewtwo/ftmewtwospecial*.c`, `itMewtwo*.c` |
| Ness | PK Flash charge/release, PK Fire, PK Thunder, PSI Magnet phase graphs and release gates | PK articles, Thunder steering/trail, PSI Magnet absorption/reflection and effects | `ftNess/ftnessspecial*.c`, `itNess*.c` |
| Peach | Peach Bomber, Parasol, Toad, and turnip motion states; side wall branch and neutral hit command branch | Turnip weighted/item lifecycle, Toad/spore, parasol article/fall, Bomber explosion and ownership | `ftPeach/ftpeachspecial*.c`, `ftPeach/ftpeachfloat*.c`, `itPeachTurnip/itpeachturnip.c`, `itPeachToad/itpeachtoad.c`, `itPeachToadSpore/itpeachtoadspore.c`, `itPeachExplode/itpeachexplode.c`, `itPeachParasol/itpeachparasol.c` |
| Pikachu | Thunder Jolt, Quick Attack, Skull Bash, Thunder source phases and electric-family movement callbacks | Jolt/Thunder articles, trail/target steering, native effect and collision callbacks | `ftPikachu/ftpikachuspecial*.c`, `itPikachu*.c` |
| Ice Climbers | Popo special phases, surface lifecycle, typed partner-aware declarations | Nana synchronization, ice/blizzard articles, Belay partner/capture and rope state | `ftPp/ftpopspecial*.c`, `ftNana/ftnanaspecial*.c`, `itClimbers*.c` |
| Jigglypuff | Rollout charge/release/turn graph, Sing, Pound launch motion, Rest and facing phases | Rollout hit capsule/scale and wall callbacks, Sing effect, Rest hitbox | `ftPurin/ftpurinspecial*.c`, `itPurin*.c` |
| Samus | Charge Shot charge/release, Missile, Bomb, Screw Attack phases; typed attributes and motion profiles | Charge Shot/Missile/Bomb article state, homing/turn clamps, bomb and grapple ownership | `ftSamus/ftsamusspecial*.c`, `itSamusCharge/itsamuschargeshot.c`, `itSamusMissile/itsamusmissile.c` |
| Yoshi | Egg Lay/Throw, Egg Roll, Egg Toss, Yoshi Bomb/Ground Pound phase graphs and resource gates | Egg/tongue/star articles, tongue capture, egg collision, and Bomb/Ground Pound effects | `ftYoshi/ftyoshispecial*.c`, `itYoshi*.c` |
| Zelda | Nayru, Din, and Transform source phase graphs; hold/release and surface transitions | Din article steering/explosion, transformation/cross-character replacement, native effects | `ftZelda/ftzeldaspecial*.c`, `itZeldaDinFire/itzeldadinfire.c` |
| Sheik | Needles charge/release, Chain, Vanish, Transform phase graphs and release gates | Needle/chain articles, charge bookkeeping, chain collision and vanish effects | `ftSeak/ftseakspecial*.c`, `itSeak*.c` |
| Falco | Shared Fox special graph with Falco identity, parameters, and laser article kind | Falco-specific article archive/effects and shared native item collision details | `ftFalco/ftfalco.c`, shared `ftFox/ftfoxspecial*.c`, `itFoxLaser/itfoxlaser.c` |
| Young Link | Link-family Fire Arrow, Boomerang, Bomb, Spin Attack phases; empty/held/release branches | Young Link article physics, pickup/return/ownership, native collision callbacks | `ftCLink/ftclinkspecial*.c`, `itLink*.c` |
| Dr. Mario | Megavitamin article phase plus Mario-family side/up/down source lifecycle | Vitamin article, cape effects, Mario-family native article/effect callbacks | `ftDrMario/ftdrmariospecial*.c`, `itDrMarioVitamin/itdrmariopill.c` |
| Roy | Mars-family Flare Blade, Dancing Blade, Dolphin Slash, Counter phase graph | Sword hitbox/command branches and counter reflection/capture details | `ftMars/ftmarsspecial*.c` |
| Pichu | Pikachu-family electric special graph plus Pichu identity/wall-jump metadata | Pichu electric articles, self-damage/effects, native collision and item callbacks | `ftPichu/ftpichuspecial*.c`, shared `ftPikachu/ftpikachuspecial*.c`, `itPichu*.c` |
| Ganondorf | Captain-family Warlock Punch, Gerudo Dragon, Wizard's Foot, Dark Dive phases with Ganon data | Dark Dive victim capture/throw, flame/effect and native hitbox callbacks | `ftGanon/ftganonspecial*.c`, shared `ftCaptain/ftcaptainspecial*.c` |

## Current evidence boundary

At main commit `24ad570`, the Python suite had 409 tests. At main commit
`9776cf0`, it has 446 tests after the corrected Rollout and Stone scaffolds
plus the reviewed fighter audits. The merged
source-backed callbacks cover additional command resets, release gates,
ground/air and terminal transitions, charge state, counter routing, and
article emission seams across the roster. The tests establish declaration
shape, source-state identity, callback registration, selected transition
branches, resource validation, and a limited set of native-host seams.

The remaining parity work is concentrated in native and article behavior:
article archives and ownership, capture and companion-object lifecycles,
hitbox and collision geometry, reflection and clank rules, effects, and
fighter-specific movement or steering. The suite does not establish
whole-match behavioral parity, frame-by-frame equivalence to the decomp,
article archive equivalence, or replay equivalence for every fighter. The
matrix remains a worklist and coverage inventory; no row is complete merely
because its source module exports or its focused tests pass.

The pinned source revision is recorded in `upstream.lock.json`; from this
repository, the checkout is at `../../../External/melee` (or can be located
through the project inventory). Function-level examples that have
already informed the portable declarations include Peach's
`ftPe_SpecialSStart_Anim`, `ftPe_SpecialN_Anim`, Fox's
`ftFox_SpecialN_Anim`, and the shared item callbacks in
`it/kinds/inlines.h` (`Item_UpdateRayAnimation`,
`Item_BounceRayOffShield`, and `Item_ResetRayAfterReflection`).
