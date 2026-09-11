# Dash-attack profile

`skirmish::game::dash` ports the Dash-phase input dispatch from `ftCo_Dash.c`,
the shared dash-attack/shield arms of `ftCo_Run.c`'s `ftCo_Run_IASA`, and the
dash attack itself from `ftCo_AttackDash.c`. Enable it with `rules.dash` plus
each fighter's `dash_attack` resource; `tests/support/dash.rs` builds an
invented profile. `rules.dash` requires `rules.grab.shield_grab` (the shared
middle-phase limit and AttackDash catch buffer) and every fighter's
`locomotion` parameters (the shared dash-smash stick threshold/window and the
run transition). The dash attack is a supplied `Attack` (poses, hitboxes,
hurtbox samples, native move identity 5) plus one decoded command-flag sample
per pose (`allow_interrupt`; the repeat flag is rejected) and an optional
per-pose TransN `root_translations` delta, exactly like a forward smash.
Without `rules.dash`, Dash and Run keep their previous behaviour (existing
`grab`/`shield`/`locomotion` tests are unaffected); `Match::new` still works
with a fully absent profile.

When `rules.dash` is present, `game::dash::update_actions` is the single place
Dash, Run and AttackDash frames are dispatched from: `simulation::update_actions`
calls it before `grab::update_actions`. When it consumes a frame (any of the
per-phase checks below, a jump-squat entry or a run/turn-run/RunBrake
transition fired), the older, still-present Dash/Run arms in `grab.rs`,
`shield.rs` and `locomotion.rs` are never reached for that frame. When it
does not (nothing above fired), it returns `false` and those older arms run
next over the same, already-checked input, redundantly but harmlessly
reaching the same "nothing happened" result. They remain exactly the
pre-existing behaviour when `rules.dash` is `None`.

The implementation preserves these examined source branches:

- `ftCo_Dash_IASA`'s `frame = fp->cur_anim_frame` (the pre-increment
  `action_frame`) selects the phase every frame: **early**
  (`fp->mv.co.dash.x4 != 0 && frame <= x44`), **middle**
  (`frame <= x4C`, which is also every frame of a Turn-entered dash, since
  `dash.x4` is 0 there and the early guard never passes), otherwise **late**.
  `dash.x4` (`locomotion::State.dash_from_input`) is 1 when
  `ftCo_Dash_CheckInput` enters Dash (`try_dash`'s same-direction branch,
  reused for the late re-dash) and 0 when Turn's own smash completion enters
  it (`ftCo_Turn.c:133`, `start_dash(f, p, false)`).
- Every phase first checks `ftCo_800D8A38` (a fresh logical A press with a
  held logical shoulder starts CatchDash; `Fighter_procInput` folds physical
  Z into both bits), reusing the same predicate `grab::update_actions`'s own
  Dash/Run arm already encoded.
- **Early**: `ftCo_AttackS4_8008C114` (`smash::select_dash`): a fresh A press
  with the facing-relative stick at or beyond the dash-smash threshold, with
  **no stick-age window** (unlike the ordinary Wait-chain smash), keeps the
  current facing; the C-stick crossing the same threshold behaves like the
  ordinary check and can flip facing. The resulting angle picks a forward
  variant exactly as `smash::select` does. If that does not fire and
  `frame <= x48`, `ftCo_80099264`: a held logical shoulder (no stick check)
  starts the ordinary forward roll directly (`escape::dash_forward_roll`,
  `ftCo_800992A8(EscapeF, false)`), skipping `try_roll`'s stick-based
  selection. Neither check runs past `x48`, even though the frame can still
  be inside the early phase. Both transitions fall through to the shared
  `x54` friction tail (below) on the same frame, immediately after entering.
- **Middle**: `ftCo_AttackDash_CheckInput` on a fresh logical A enters
  AttackDash and `ftCo_AttackDash_SetMv0` arms `fighter.dash.grab_buffer` to
  `x68` (`rules.grab.shield_grab.dash_buffer_frames`); this transition
  `return`s and does not reach the friction tail. Otherwise, when the stick
  opposes the current facing, `ftCo_Dash_CheckInput` reused as the dash-back
  predicate (`locomotion::try_dash`, already gated by the caller's direction
  check so it can only choose the smash-Turn branch) enters Turn. Otherwise
  `ftCo_80091AD8`: a fresh L/R press inside the powershield window opens
  GuardReflect, or a held shoulder with remaining shield health opens
  GuardOn; either way `grab::shield_entry_buffer`'s existing
  `action_frame > dash_buffer_frame_limit` gate keeps the catch buffer at
  zero here (`guard.x20` is written by the source but never read, so it is
  not modeled). The dash-back Turn and the shield entry both fall through to
  the friction tail; the AttackDash entry does not.
- **Late**: `ftCo_Dash_CheckInput` (`try_dash`, both directions this time):
  the same direction restarts Dash (`dash_from_input = true`, `action_frame`
  resets to 1 on the next observed frame); the opposite direction enters a
  smash Turn. Otherwise `ftCo_80091A4C`/`ftCo_80091B9C`: the same
  fresh-press/held-shoulder shield entry as the middle phase, but now
  `shield_entry_buffer` arms the catch buffer to `x68`. Both the re-dash/Turn
  and the shield entry fall through to the friction tail.
- **The shared `x54` friction tail** (`transition_friction`, applied by
  [`apply_transition_friction`] immediately after each transition listed
  above, on the same frame): `fp->gr_vel += -(fp->gr_vel * x54) * friction_mul`
  (`ft_GetGroundFrictionMultiplier`, the stage/metal multiplier, is fixed at
  1.0 in this profile). Reading `ftCo_Dash_IASA` literally, every phase
  branch that falls out of its `if`/`else` instead of `return`ing reaches
  this same block: the early forward smash and roll, the middle dash-back
  Turn and shield entry, and the late re-dash/Turn and shield entry all fall
  through; the catch, the AttackDash entry, block_42's jump-squat entry and
  run transition, and "nothing fired" all `return` instead and never reach
  it. A neutral special entered from Dash also falls through to it
  (`ftCo_SpecialS_CheckInput` is the first check of both the early and
  middle phases); `simulation::update_actions` applies the tail there
  itself, since `special::update_actions` runs after `dash::update_actions`
  returned (having found nothing to do) for that frame. Because
  `ftCo_Dash_Enter` computes `dash.x0` (`locomotion::dash_initial_delta`)
  from `gr_vel` *before* the tail runs, a late-phase re-dash's first
  observed `ground_velocity` is `dash_initial_velocity * facing -
  gr_vel_before * x54`, not simply the ordinary entry formula.
- **block_42** (reached from any phase when nothing above fired, or when
  the fall-through checks above evaluated false): the existing jump-squat
  entry (`locomotion::jump_input`, already shared by
  Wait/Walk/Squat/Turn/RunBrake, modeling `fn_800CAF78`'s relaxed tap-jump/
  X-Y check) and, for Dash, the existing
  `action_frame >= dash_run_frame && stick * facing >= run_threshold`
  transition to Run (`fn_800CA5F0`, gated on the script's run flag
  `cmd_vars[0]`, which the animation's set-cmd-var command writes,
  `ftaction.c:462`, and which `dash_run_frame` models directly rather than
  a raw script flag); for Run, the existing turn-run/RunBrake entry. In the
  pinned source, block_42 is guarded by a taunt check (`ftCo_800DE9D8`,
  D-pad up, entering AppealS via `ftCo_800DEA28`, `ftCo_AppealS.c:35,43`)
  that this port does not model: `if (!ftCo_800DE9D8(gobj)) { ...; return; }`
  always returns once entered (via one of its `RETURN_IF`s, or the
  unconditional `return` at the end when nothing fired), so the friction
  tail after this block is reached in the pinned source only past a taunt
  entry. Since taunts are never modeled here, jump, the run transition and
  "nothing fired" all end the frame the same way the pinned source's
  non-taunt path does: without touching `ground_velocity`.
- `ftCo_Run_IASA` reuses the same catch, AttackDash-entry (unconditionally
  armed, matching `ftCo_80091B90(x410)`'s no-op discard of its own argument)
  and shield-entry (always buffered) checks, then falls back to the existing
  jump/turn-run/RunBrake dispatch exactly as Run already did.
- `ftCo_AttackDash_CheckInput`/`decideFighter`/`doEnter`: entry clears
  `allow_interrupt` and the catch buffer (`simulation::enter` resets
  `fighter.dash` on every motion change, matching `mv.co.attackdash.x0 = 0`);
  the caller then arms it to `x68`.
- `ftCo_AttackDash_IASA` every frame: `ftCo_800D8AE0` (`attack_dash_grab`):
  a held logical shoulder with a nonzero buffer starts CatchDash **without
  needing A**, otherwise the buffer counts down while nonzero. If that did
  not fire and the pose's `allow_interrupt` flag is raised, the complete
  Wait chain opens exactly as a smash's does (`dash::interruptible` is
  folded into `tilt::interrupt_chain` alongside `smash::interruptible`, so
  the existing generic grab/shield/tilt/jump/dash dispatchers all become
  reachable without a second copy of that machinery).
- `ftCo_AttackDash_Anim`: end of poses returns to Wait, or Fall airborne.
- `ftCo_AttackDash_Phys` (`ft_80085030` without its TransN branch, since that
  branch's target is supplied directly as a resource): per-pose TransN root
  motion when `root_translations` is supplied, otherwise
  `apply_friction(ground_velocity, x50 * ground_friction)`
  (`ftCommon_ApplyFrictionGround`: one frame of deceleration toward zero,
  clamped so it cannot overshoot past zero).

Deviations from a literal reading of the pinned control flow, reported per
`AGENTS.md`:

- **The `x54` friction tail's only unmodeled trigger is the taunt.**
  `ftCo_800DE9D8` (called first in `block_42`) is the taunt check, not a
  jump predicate: it is `ftCo_800DE9B8`
  (`fp->input.pressed_buttons & HSD_PAD_DPADUP`, `ftCo_AppealS.c:35`)
  entering AppealS through `ftCo_800DEA28` when pressed
  (`ftCo_AppealS.c:43`). Reading `block_42` literally:
  `if (!ftCo_800DE9D8(gobj)) { RETURN_IF(fn_800CAF78(gobj));
  RETURN_IF(!fp->cmd_vars[0]); RETURN_IF(fn_800CA5F0(gobj)); return; }`
  always returns once entered (via one of the `RETURN_IF`s or the
  unconditional `return` at the end), so `block_42` reaches the tail only
  when a taunt was just entered; every *other* fall-through path to the
  tail (the early forward smash/roll, the middle dash-back Turn/shield
  entry, the late re-dash/Turn/shield entry, and a neutral special from
  Dash) is outside `block_42` and does not depend on this check at all, so
  this port applies the tail on those transitions directly
  (`dash::apply_transition_friction`, called from `update_dash_or_run` and
  from `simulation::update_actions` for the special case). Taunts
  themselves are unmodeled (D-pad input is not read at all in this
  profile), so `block_42` alone never reaches the tail here: a frame where
  literally nothing fired (not even jump or the run transition) leaves
  `ground_velocity` untouched, exactly as `rules.dash = None` would
  (`tests/game_dash.rs`'s
  `an_idle_dash_frame_matches_ground_velocity_with_rules_dash_none`, and
  `tests/dash_differential.rs`'s `dash_frame_boundary_leaves_gr_vel_unchanged`
  against the pinned C with the taunt check scripted false). The fall-through
  transitions are checked against the pinned `ftCo_Dash_IASA`'s traced call
  order in `tests/dash_differential.rs`'s `dash_frame_matches_call_order`,
  and against the Rust implementation's exact arithmetic in
  `tests/game_dash.rs`'s
  `late_phase_redash_ground_velocity_reflects_the_transition_friction_tail`
  and `late_phase_guard_on_ground_velocity_reflects_the_transition_friction_tail`.
- `cmd_vars[0]` inside that same block is the script's run flag: the
  animation's set-cmd-var command writes it (`ftaction.c:462`). This port
  models that scripted transition directly as the existing
  `action_frame >= dash_run_frame` threshold rather than decoding the raw
  command, and `fn_800CAF78` (the relaxed tap-jump/X-Y check ahead of it) as
  the existing shared `locomotion::jump_input(relaxed = true)` call already
  used by every other grounded action's jump-squat entry.
- Item branches in both `ftCo_Dash_IASA` and `ftCo_AttackDash_CheckInput`
  are unmodeled (as in every other ground-attack profile). `SpecialS` itself
  runs earlier in the existing dispatch (`special::update_actions`, unaffected
  by this module), but `simulation::update_actions` still applies the
  friction tail when it fires from a Dash frame, since that is the first
  check of both the early and middle phases in the pinned source.

Not modeled: the Kirby neutral-special override (`ftKb_SpecialN_800F1F68`),
`RunDirect`, item throws, the taunt (`AppealS`, D-pad up, the only unmodeled
trigger of the `x54` friction tail), and `guard.x20` (written by
`ftCo_80091AD8` but never read anywhere in the pinned source).

The dash attack maps to Slippi state 50 with animation index 52 (move
identity 5, action-instance identity 5); `crates/skirmish-replay/src/
observation.rs`'s `44..=64 => state + 2` animation range now covers it
directly. The catch buffer survives checkpoints. `dash_differential`
traces `ftCo_Dash_IASA`'s complete dispatch order (`dash_frame_matches_call_order`,
against a Rust mirror of the exact source control flow) and separately
compares `ftCo_Dash_CheckInput`, `ftCo_800D8AE0`'s catch buffer and
`ftCommon_ApplyFrictionGround` (already pinned by the `locomotion_common`
and `physics` adapters, which `apply_friction` mirrors, exercised here for
AttackDash's `x50` deceleration) with pinned C over generated sticks, ages,
buffers, answers and boundary frames; `tests/game_dash.rs` covers the same
dispatch end to end against the Rust implementation, including the exact
`ground_velocity` arithmetic the friction tail produces on a re-dash and a
shield entry. `crates/cli/tests/replay_match.rs` matches a dash attack
entered from Run and a late-phase re-dash against their own Peppi-written
bytes and detects a removed press at its first affected frame.
