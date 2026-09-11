# Smash attack profile

`skirmish::game::smash` ports the ordinary grounded smashes from
`ftCo_AttackS4.c`, `ftCo_AttackHi4.c` and `ftCo_AttackLw4.c`, the fresh
C-stick predicates from `ft_0DF1.c` and the smash charge state machine from
`ft_0DF0.c`: the forward smash with its High, HighSlight, Straight, LowSlight
and Low angle variants, the up smash and the down smash. Enable it with
`rules.smash` plus each fighter's `smashes` resources; `tests/support/smash.rs`
builds an invented profile. Every attack is a supplied `Attack` (poses,
hitboxes, hurtbox samples, native move identity) plus one decoded command-flag
sample per pose (`allow_interrupt`; the repeat flag is rejected), an optional
charge command (`frame`, `hold_frames`, `damage_multiplier`) decoded from the
script's smash-charge command, and, for forward smashes only, one TransN delta
per pose. Optional forward variants stand in for the source's figatree
availability checks. Locomotion parameters supply the dash-smash stick
magnitude and window and are required. The tilt profile is not required, but
without it a sub-smash stick with A has no action.

The implementation preserves these examined source branches:

- Wait, Walk, Turn, Squat, SquatWait and SquatRv chains check the forward,
  up and down smashes after the catch and before every tilt, the jab, shields,
  jumps, dashes, turns and walks. Turn evaluates the chain with its post-turn
  facing, which the up and down smashes keep. `Fighter_procInput` folds
  physical Z into the logical A press, so Z reaches a smash only where the
  chain has no catch (the down tilt's interruptible block).
- `ftCo_AttackS4_CheckInput` needs a fresh A press with `|stick.x|` at or
  beyond `dash_smash_stick_threshold` and the byte stick-X age below
  `dash_smash_window`, or `ftCo_800DF1C8`: `|cstick.x|` crossing the same
  magnitude between the previous and current samples. The chosen stick's sign
  (`x >= 0 ? +1 : -1`, so zero faces right and NaN faces left) becomes the
  facing before the angle variant is chosen as `decideAngle` does for tilts:
  High above `xB8`, HighSlight above `xBC`, Low below `xC4`, LowSlight below
  `xC0`, else Straight, skipping variants without an animation.
- `ftCo_AttackHi4_CheckInput` needs A with stick Y at or above `xCC` and the
  byte stick-Y age below the float window `xD0`, or `ftCo_800DF2D8` (previous
  C-stick Y strictly below `xCC` and current at or above it).
  `ftCo_AttackLw4_CheckInput` mirrors it with `xD4`/`xD8` and `ftCo_800DF3A8`.
- `ftCo_KneeBend_IASA` checks the catch and then
  `ftCo_AttackHi4_CheckInputNoD0` (the same test without the age window)
  before sampling the short hop, so a jump-cancelled up smash or grab leaves
  JumpSquat.
- The animation's charge command arms `SmashState_PreCharge`
  (`ftCo_800DEE84`, once per action instance, including on the entry pose).
  `ftCo_800DF0D0` runs before every input callback: a held logical A commits
  to `Charging` at animation rate zero, a released A returns to `None` or
  releases a charge. `ftCo_800DEF38` runs in the animation phase, after the
  script and before the animation callback: each charging frame counts once,
  the count clamps to `hold_frames`, and reaching it releases automatically.
  The release frame therefore counts before the input phase sees the
  release. Every motion change clears the charge (`ftCo_800DEEA8`).
- While charging the action frame holds and the frozen animation step yields
  no TransN delta; `ft_80084FA8` otherwise drives the ground speed to each
  forward-smash pose's delta times the facing. Up and down smashes use the
  ordinary ground friction.
- `ftColl_8007ABD0` prices each hitbox once when it is created:
  `ftCo_800DEEB8` scales the damage by
  `(damage_multiplier - 1) * frames / hold_frames + 1` only in the `Release`
  state, the integer stale count truncates the scaled value, and the stale
  queue applies to the scaled value. Hitboxes on or before the charge pose are
  rejected because they would keep their uncharged damage.
- `ftCo_Damage_CalcKnockback` multiplies a charging victim's knockback by
  `kb_smashcharge_mul` before the armor subtraction.
- While `allow_interrupt` is raised, every smash exposes the complete Wait
  chain (`ftCo_AttackS4_IASA` orders it as Wait does; Hi4 and Lw4 call
  `ftCo_Wait_IASA`). Smashes return to Wait after their last sample or Fall
  when the floor is gone.

Not modeled: item throws, the Ness, Peach and Game & Watch overrides, the
Pikachu/Pichu hitlag callbacks, Link and Young Link's second forward-smash hit
(`ftCo_800CECE8`, `cmd_vars[0]`), the forward smash out of an early Dash
(`ftCo_AttackS4_8008C114`), the charge shake offset (`ftCo_800DEEE8`), its
sound and graphics, the vertical scale adjustment of hitbox damage and the
crouch multiplier.

Smashes map to Slippi states 58..64 with animation indices 60..66. The charge
state, count and hold survive checkpoints. `smash_differential` compares the
complete `ftCo_AttackS4_CheckInput` with `decideFighter` and `doEnter`,
`ftCo_AttackHi4_CheckInput`, `ftCo_AttackHi4_CheckInputNoD0`,
`ftCo_AttackLw4_CheckInput`, the three fresh C-stick predicates and the
complete charge lifecycle (`ftCo_800DEE84`, `ftCo_800DEEA8`, `ftCo_800DEEB8`,
`ftCo_800DEF38`, `ftCo_800DF0D0`) with pinned C over generated sticks,
thresholds, ages, availability masks and held-button sequences.
