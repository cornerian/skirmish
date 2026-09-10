# Ordinary wall-jump profile

`rules.wall_jump` supplies the common contact-input window, away-stick boundary,
horizontal tilt age window, startup timer and repeated-jump vertical multiplier.
Each `fighters[].wall_jump` profile records whether that fighter can wall jump,
its minimum wall-relative approach speed, launch velocities and every sampled
`PassiveWallJump` bone pose. The optional pair keeps legacy fixtures compatible;
supplying common rules or fighter data alone is rejected.

After ECB collision, a left-side contact records wall side `-1` and a right-side
contact records `+1`, matching `ftWallJump_8008169C`. The arming comparison uses
the absolute difference between the fighter's attempted X displacement and the
contacted line point's frame displacement. This lets an inward-moving wall arm a
stationary fighter. The approach, timer and tilt-age comparisons are strict;
away-stick displacement is inclusive. Losing wall contact disables the timer.

A successful interrupt enters the same `PassiveWallJump` motion used by a
wall-tech jump, faces away from the surface, clears motion and freezes for the
configured startup callbacks. Launch multiplies the fighter's vertical speed by
`vertical_velocity_base.powf(previous_wall_jumps)` and uses the complete sampled
bone track for headless hurtbox and bone-based ECB physics. The count saturates
at 255 and landing resets it. All interrupt, startup and repeat fields are part
of `Fighter`, so checkpoints and counterfactual replay branches retain them.
After startup, the action dispatches the implemented airborne neutral-special,
aerial-attack and available aerial-jump branches. The timer blocks those
interrupts, and aerial selection has priority over a simultaneous jump.

`wall_jump_differential` compares arbitrary binary32 inputs and every mutated
field against the complete pinned original interrupt. `passive_wall_launch_differential`
runs the complete original animation callback and compares both launch velocity
components bit for bit. `game_wall_jump` covers both wall directions, moving
walls, startup, repeat decay, landing reset, incapable fighters, sampled motion,
post-startup aerial interrupts, resource rejection and checkpoint replay.

Authentic per-fighter values and pose tracks still need extraction. Wall cling,
wall damage/bounce action coverage and character-specific collision callbacks
remain part of the wider action translation.
