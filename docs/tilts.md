# Tilt attack profile

`game::tilt` ports `ftCo_AttackS3.c`, `ftCo_AttackHi3.c` and
`ftCo_AttackLw3.c`: the forward tilt with its High, HighSlight, Straight,
LowSlight and Low angle variants, the up tilt and the down tilt with its
buffered repeat. Enable it with `rules.tilt` plus each fighter's `tilts`
resources; `tests/support/tilt.rs` builds an invented profile. Every attack is
a supplied `Attack` (poses, hitboxes, hurtbox samples, native move identity)
plus one decoded command-flag sample per pose: `allow_interrupt` and, for the
down tilt only, the script's repeat flag. Optional forward variants stand in
for the source's figatree availability checks. Locomotion parameters are
required for the input chains. Smashes, dash attacks, item branches, the
Landing interrupt window and the Game & Watch down-tilt override are not
modeled.

The implementation preserves these examined source branches:

- Wait, Walk, Turn, Squat, SquatWait and SquatRv chains check the forward tilt,
  up tilt, down tilt and then the jab, after catches and before shields, jumps,
  dashes, turns and walks. Turn evaluates the chain with its post-turn facing.
- `ftCo_AttackS3_CheckInput` needs a fresh A press, facing-relative stick X at
  or beyond `x98` and `ftCo_GetLStickAngle` (`atan2f(y, |x|)`) strictly inside
  `x20_radians`; `decideAngle` picks High above `x9C`, HighSlight above `xA0`,
  Low below `xA8`, LowSlight below `xA4`, else Straight, skipping variants
  without an animation. `ftCo_AttackHi3_CheckInput` needs stick Y at or above
  `attackhi3_stick_threshold_y` and an angle strictly above the limit;
  `ftCo_AttackLw3_CheckInput` needs stick Y at or below `xB0` and an angle
  strictly below its negation.
- Tilt physics is ordinary ground friction and the ordinary floor collision,
  so leaving a floor enters Fall. Forward and up tilts return to Wait after
  their last sample; the down tilt enters SquatWait through `ftCo_800D638C`.
- While `allow_interrupt` is raised, forward and up tilts expose the complete
  Wait chain; the down tilt exposes `ftCo_AttackLw3_IASA`: forward and up
  tilts, `checkPadA`, then the down tilt, jab, jump, dash, squat, turn and
  walk, without shields, catches or specials.
- `checkPadA` runs on every down-tilt frame: a fresh A press restarts the tilt
  once the script's repeat flag is raised and otherwise arms `attacklw3.x0`;
  the animation callback restarts the tilt when the flag rises while a press
  is buffered.
- The down tilt enters with `Ft_MF_SkipAttackCount`, keeping the previous
  action instance, and installs the deferred `x21EC` callback: leaving it (or
  repeating it) runs `ft_800892A0` and `ft_80089824`, which restart the stale
  attack instance after the ordinary `ft_800890D0` identity change and
  allocate two action instances with identity 0 before the ordinary
  motion-change accounting.
- `ftCo_Catch_CheckInput` from the same states now accepts a fresh logical A
  press with the logical shoulder held (physical Z or A with a shoulder), and
  `ftCo_800D8A38` does the same from Dash and Run; SquatWait and SquatRv join
  the catch states.

Tilts map to Slippi states 51..57 with animation indices 53..59. The buffered
repeat flag, stale identities and action instances survive checkpoints.
`tilt_differential` compares the complete `ftCo_AttackS3_CheckInput` with
`decideAngle`, `ftCo_AttackHi3_CheckInput`, `ftCo_AttackLw3_CheckInput` and
`checkPadA` with pinned C over generated sticks, thresholds and availability
masks, reusing the already pinned `ftCo_GetLStickAngle` body.
