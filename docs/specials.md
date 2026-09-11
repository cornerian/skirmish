# Neutral specials

`game::specials::neutral` (framework wiring, `src/game/specials/neutral.rs`)
implements the shared shell described below; `game::specials` (`src/game/
specials/mod.rs`) owns the dispatch every move -- this one and Fox's side
special -- shares. `fighters[].specials`' `neutral` entry (a per-character
`Specials` variant, `game::characters::Specials`) supplies paired ground and
air physics animations for a fighter's neutral special. Each animation is the
same sampled `Attack` resource used by jabs, aerials and ledge attacks: every
frame owns a complete bone pose and up to four bone-attached hitboxes. The
pair must have equal frame counts so
terrain conversion can retain the current animation frame.

A fresh physical B press selects the action only when both main-stick axes are
strictly inside `neutral_thresholds`. Grounded locomotion states enter
`SpecialN`; ordinary jump/fall states enter `SpecialAirN`. Special selection
and released wall-tech actions enter `SpecialAirN`. Airborne Damage, DamageFall
and reflected wall/ceiling actions also enter it after hitstun reaches zero. Special selection
precedes catch, shield, ordinary attack and aerial dispatch. Frozen wall tech or
active hitstun blocks the action, and a B press held through release must be
rearmed. Holding B cannot restart the move after it finishes; returning through
a released frame rearms the input edge.

Both variants use the ordinary ground or air physics path. Walking off an edge
changes to `SpecialAirN`, and landing changes to `SpecialN`, retaining the frame
while installing the paired pose. Animation completion returns to `Wait` on the
ground or `Fall` in the air. Hitboxes use the shared sweep, clank, staling,
shield and damage pipeline. All action, input history, hitbox tracking and
animation state is included in match checkpoints.

`fighter::special::neutral_input` is an exact boundary retained from
`ftCo_800D67C4`. `special_differential` compiles that complete function from its
pinned C snapshot and compares arbitrary button, stick and threshold values.
`game_special` covers both player slots, strict thresholds, competing-input
priority, release/repress behavior, sampled combat contact, air physics,
landing conversion, checkpoints and invalid resources. Both neutral-special
conformance scenarios now run normally.

This profile provides the shared neutral-B action shell. Authentic character
callbacks still need character-specific resources and state for projectiles,
charge storage, reflectors, capture, transformation, RNG use and other effects.
Up and down specials and direct special-to-special interrupt rules remain
separate work. Omitting the profile leaves B accepted but without an action.

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
