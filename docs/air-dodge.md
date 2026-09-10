# Air dodge profile

`game::escape_air` ports `ftCo_EscapeAir.c` with its `ftCo_FallSpecial.c`
continuation and the shared `ftCo_Landing.c` special landing. Enable it with
`rules.escape_air` plus each fighter's `escape_air` resources; the fixture
`tests/fixtures/game/escape-air.json` shows the schema with **invented test
values**. Locomotion parameters are required because FallSpecial consumes every
jump. Without the profile a physical L/R press in the air does nothing rather
than guessing the common values.

Resources are physics samples, not presentation. Each EscapeAir sample carries
bone poses, the scripted fighter-wide `x1988` collision state and the script's
skip-decay flag. The special landing supplies its `x2EC` animation end and
integer-frame poses. FallSpecial uses the supplied bind pose, as Fall does;
its FallSpecialF/B drift variants are not selected.

The implementation preserves these examined source branches:

- `ftCo_80099A58` runs in the Jump, JumpAerial, Fall and Pass input chains, the
  tumble list and the wall-tech list, after neutral specials and before aerial
  attacks and double jumps. It needs a fresh physical L or R press; analog
  pressure and held shoulders do not qualify.
- `ftCo_80099A9C`'s `inlineA0` sets both self-velocity axes to
  `escapeair_force` along `atan2f(stick.y, stick.x)`, or to zero while both
  stick axes sit strictly inside `escapeair_deadzone`. The ordinary motion
  change clears fast fall and runs sample 0's script before contacts.
- `ftCo_EscapeAir_Phys` multiplies both axes by `escapeair_decay` without
  gravity or drift until the script raises its skip-decay flag; afterwards the
  ordinary fall, fast-fall and drift physics apply.
- When the samples end, `ftCo_80096900(1, 1, false, x340, x344)` enters
  FallSpecial with `Ft_MF_KeepFastFall` and `ftCommon_UseAllJumps`. Its
  `xC = 1` branch is the ordinary fall physics, so the `x340` mobility scale is
  not consumed. FallSpecial offers no modeled action; jumps are exhausted and
  aerial attacks are absent from its chain.
- `ftCo_80096CC8` lets a FallSpecial fighter fall through one-way platforms
  while the main stick is at or below `x25C`; solid floors always land.
- `ftCo_80099D70` and `ftCo_80096D28` with `x10` set enter LandingFallSpecial
  from either state through `ftCo_LandingFallSpecial_Enter(false, x344)`, so
  the special landing plays at `(0.1 + x2EC) / x344`, ignores input, keeps the
  landing self-velocity as ground speed under ordinary friction (the
  wavedash), and returns to Wait when its rate crosses `x2EC`.

The `x334` item timer, `ftCo_80095328`/`ftCo_800D705C` item interrupts, the
Link/Samus tether catch, the parasol, the FallSpecial `x10 = 0` Wait landing of
other entries, the `ftCommon_8007D60C` grounded entry and the FallSpecialF/B
pose variants are not modeled. `escape_air_differential` compares the complete
`ftCo_80099A58`, `inlineA0`/`ftCo_80099A9C`, `ftCo_EscapeAir_Phys` and
`ftCo_80096CC8` bodies with pinned C; launch angles use host `libm`, so
generated cases allow the documented rounding tolerance while boundary cases
match bit for bit. The sampled body state, skip-decay flag, landing rate and
elapsed landing animation survive native checkpoints.
