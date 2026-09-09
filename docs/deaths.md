# Blast deaths

Optional `rules.death` replaces the compatibility immediate respawn with
explicit death actions. Side and bottom crossings select `DeadLeft`,
`DeadRight` or `DeadDown`; a forced ordinary top death selects `DeadUp`. These
four paths publish `DeathStarted` and `Knockout` together, then retain their
configured action for `normal_frames` before entering `Respawn`.

An eligible top crossing otherwise consumes exactly one HSD RNG draw. The
inclusive `screen_chance_percent` selects a screen death unless
`camera_disables_screen` is set; every other result selects a star death. Star
death waits, travels toward the supplied camera height and depth over an exact
number of frames, then loses the stock and waits before respawn. Screen death
runs its supplied camera-relative approach, changes to the hit-camera action,
holds, falls under supplied gravity and terminal velocity, then records the
stock loss and finish delay. Death actions ignore input, pushing, stage contact,
grabs and attacks. Their timers, phase, model offset, hidden flag and RNG state
are checkpointed and serialized for headless analysis.

`fighter::death::select` retains the complete comparison and call order from
`ftCo_800D3158`. `death_differential` compiles that full pinned C function with
host callbacks that only record the selected branch. Generated cases compare
the five early exclusions, right/left/top/bottom priority, strict top gates,
ordinary/ice star and screen variants, arbitrary chance values and the exact
post-call HSD seed. This boundary needs no ISO, DOL or emulator.

The phase resources in `rules.death` are explicit native data. The integration
fixture uses invented values; authentic common data still needs extraction and
provenance. The screen approach uses the configured frame progress because the
original animation-frame source is not present yet. Ice damage does not yet
enter the ice death variants in the match scheduler. Player/script death
exemptions, camera implementation, effects, sound, bonus/stat callbacks and the
original callback position within the full fighter update remain pending.
