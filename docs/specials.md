# Neutral specials

Fox's neutral special (Blaster) is `game::characters::fox::neutral`
(`src/game/characters/fox/neutral.rs`), covered in full in
`docs/fox-neutral-special.md`: a Start/Loop/End state machine, repeatable
while B is pressed, that fires a generic fired-projectile
(`game::projectile`, `src/game/projectile.rs`) laser through the ordinary
damage/shield/reflect pipeline. `fighter::special::neutral_input` (below)
is the shared entry gate every character's own neutral special uses,
Fox's included.

The generic single-phase shared shell this codebase previously used as a
placeholder for any character's neutral special (`game::specials::neutral`)
is retired: with Fox now on his own dedicated Blaster implementation,
`characters::Specials` has no remaining variant that could reach it, so it
was removed outright rather than kept as untested, unreachable code (see
`docs/fox-neutral-special.md`'s "What moves, what doesn't" section). A
future second character that only needs a plain single-phase neutral
special can re-add an equivalent small shell.

`fighter::special::neutral_input` is an exact boundary retained from
`ftCo_800D67C4`: a fresh B press with both main-stick axes strictly inside
`neutral_thresholds`. `special_differential` compiles that complete
function from its pinned C snapshot and compares arbitrary button, stick
and threshold values -- unaffected by the shell's retirement, since Fox's
own dedicated module calls this same pure function directly.

# Projectiles

`game::projectile` (`src/game/projectile.rs`) is the minimal generic
fired-projectile system Blaster's laser needs: spawn, per-frame motion,
lifetime/despawn, and hurtbox/shield/reflect collision producing the
ordinary damage pipeline with the item's own knockback. `State.projectiles`
holds every in-flight instance, included in checkpoints. See
`docs/fox-neutral-special.md` for the full design, citations and known
gaps (the fire-timing approximation, the terrain-despawn simplification,
and the shield/Reflector interaction citations, including the first real
gameplay effect the Reflector's own `down::Reflect` geometry and
`fighter.shield.reflecting` bit have had in this codebase).

# Fox/Falco side special (Illusion/Phantasm)

`game::characters::fox::side` (`src/game/characters/fox/side.rs`) implements
this move against the same `game::specials` framework the neutral shell
above uses; the pure arithmetic lives in `fighter::characters::fox`
(`src/fighter/characters/fox.rs`). `fighters[].specials`' `side` entry (the
same per-character `Specials::Fox` variant the neutral entry above shares)
and `rules.specials` cover Fox's (and, by shared code, Falco's) side
special: `SpecialSStart/SpecialS/SpecialSEnd` on the
ground, `SpecialAirSStart/SpecialAirS/SpecialAirSEnd` in the air. It checks
its own fresh-B-plus-stick-threshold input ahead of the neutral branch
above, in every chain that reaches it (`ftCo_SpecialS_CheckInput` is the
source's own first check there too), turns the fighter, blends ground
velocity toward zero by a fighter-specific retention, then plays a Start
pose set, a TransN-driven root-motion dash (direct velocity set from the
animation, not accumulated) shortenable by a second B press, and a fixed-
speed End pose set that returns to Wait on the ground or exits into
`FallSpecial` with a scaled air-drift mobility and landing lag in the air.
See `docs/fox-side-special.md` for the complete per-phase citation, the
resolved ghost-item hitbox question and the deviations found from this
batch's own design note while implementing it.

# Fox/Falco down special (Reflector)

`game::characters::fox::down` (`src/game/characters/fox/down.rs`)
implements this move against the same framework, checked after the side
special and the neutral shell above (the source's own grounded chain
checks SpecialS, SpecialHi (unmodeled), SpecialN, then SpecialLw in that
fixed order). `fighters[].specials`' `down` entry covers a five-phase
Start/Loop/Turn/Hit/End state machine: `SpecialLwStart/SpecialLw/
SpecialLwTurn/SpecialLwHit/SpecialLwEnd` on the ground,
`SpecialAirLwStart/SpecialAirLw/SpecialAirLwTurn/SpecialAirLwHit/
SpecialAirLwEnd` in the air. Holding B keeps the Loop pose active; a
release only exits once a fixed release-lag countdown (started at Start
entry, ticking every frame from Loop onward) has also elapsed. Loop's own
IASA reverses into Turn on a stick past the ordinary standing-turn
threshold (an immediate facing flip, unlike the visual-only model
rotation), else jump-cancels on the ground or aerial-jumps in the air; a
platform drop from Start or Loop keeps the move, converting straight to
the aerial variant. The Hit phase exists only for a projectile reflect
Skirmish has no projectiles to trigger, so it is reachable through tests
only. See `docs/fox-down-special.md` for the complete per-phase citation
and what remains unmodeled.
