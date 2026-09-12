# Local validation provenance

The 2026-09-13 Falco Laser batch wires `Specials::Falco.neutral` (gameplay
export v10's `fighters/falco.json` carries it for the first time, same
schema as Fox's own `characters::fox::neutral::NeutralSpecial`). The task
brief for this batch expected a distinct `itfalcolaser.c`, by analogy with
the side special's own Fox/Falco ghost item; reading the pinned decomp
disproves this: `melee/it/it_3F2F.c`'s own per-item-kind logic table gives
`It_Kind_Fox_Laser`/`It_Kind_Falco_Laser` byte-identical stanzas (same
`it_803F67D0` state table, same seven `itFoxLaser_Logic94_*` callbacks),
and no `itfalcolaser.c` exists anywhere in `src/melee`. Falco's laser is
the same C item as Fox's, spawned with a different `Item_Kind` constant
purely to reach his own `Article` data and a cosmetic SFX pick
(`ftFx_SpecialN_FireBlasterShot`'s only `FTKIND_FALCO` branch); this is now
a pinned, automated finding, not prose: `tests/
falco_laser_table_differential.rs` extracts both stanzas from a newly
pinned whole-file snapshot (`tests/oracle/original/it_3F2F.c`,
`sources.json`) and asserts they match verbatim, plus a `c-oracle`-gated
known-values check that the already-pinned generic laser-spawn function
reproduces Falco's own exported attributes exactly. `game::projectile::
ProjectileKind` gains a `FalcoLaser` variant purely as an observation
label (`characters::fox::neutral::drain_pending_shot` now checks the
firing fighter's own `Specials` variant); every function in `game::
projectile` was already generic over `kind` and needed no change.

Falco's own laser data is genuinely different from Fox's in exactly the
fields the existing resource shape already has: slower (`speed = 5.0` vs.
`7.0`), a much longer lifetime (`100` vs. `35` frames), and -- the one real
gameplay difference -- nonzero knockback (`growth: 100, fixed: 5` on all
four hitboxes, vs. Fox's all-zero), matching Melee community knowledge
that Falco's laser flinches harder than Fox's pure-flinch shot; the fourth
hitbox is also a uniform size/reach rather than Fox's own widened one.
Cross-checked against a real recording independent of the exporter
(`19_39_37 Falco + Fox (DL).slp`, Slippi `3.9.0`, `slippi-public-dataset-
v3.7/data/FALCO/batch_00`, py-slippi in a scratch `uv venv`): 16 `FALCO_
LASER` (`sid.Item.FALCO_LASER == 55`) spawn instances all confirm speed
`5.0`, plain `position += velocity` motion, and a `100`-frame lifetime
(first-observed `timer == 99.0`, decrementing by exactly `1.0`); one
instance reproduces the same oblique-velocity "laser angling" open
question `docs/fox-neutral-special.md` already flagged for Fox, now
confirmed character-independent. That same instance also surfaces a
discrepancy for the concurrent laser-hit-registration diagnosis in `src/
game/projectile.rs` (not fixed by this batch, per its own narrower scope):
`itfoxlaser.c:98-107`'s own terrain-collision branch
(`itFoxlaser_UnkMotion1_Coll`) sets the item's remaining lifetime to
exactly one frame and restores its pre-collision position rather than
despawning immediately, so a terrain-hit laser is observably still present
for one more frame in a real recording; `game::projectile::step` currently
despawns on the same frame it detects a terrain-line contact, with no such
grace frame. Recorded here for whoever picks up that diagnosis next.
Full citations, the difference table, and every test: `docs/falco.md`'s
own "Falco's neutral special (Laser): now wired" section.

The 2026-09-13 aerial-Blaster-entry velocity fix (the real-replay parity
loop, `fox-bf.slp`, `docs/parity.md`) stops `game::characters::fox::
neutral::update_actions` from zeroing velocity on an airborne Blaster
press. `ftFx_SpecialN_Enter` (the grounded entry, `ftfoxspecialn.c:255-
269`) zeros `gr_vel` and `self_vel.{x,y,z}` right after `Fighter_
ChangeMotionState`/`ftFox_SpecialN_InitializeState`; `ftFx_SpecialAirN_
Enter` (the aerial entry, `:274-284`), though, does not touch velocity at
all -- only the motion-state change, the shared `InitializeState` (extra
animation advance, `cmd_vars` reset) and the blaster-gun spawn. Before
this fix, the port applied the ground entry's own unconditional `self_
vel = 0` to both branches, so any airborne press (mid-jump, mid-fall)
hard-stopped the fighter's existing drift instead of carrying it through
untouched.

Confirmed directly against `fox-bf.slp`: P4 full-hops, then presses B in
the air at frame 14 (Slippi action state `344`, `SpecialAirNStart`); its
own frame-to-frame `position.x` delta is a smooth, continuously-
decelerating sequence both before and after this frame (`-0.8632`,
`-0.8432`, `-0.8232`, `-0.8032`(sic, `-0.8031`), `-0.7832`, `-0.7632`,
`-0.7432`...) -- the transition itself leaves no visible mark, meaning
`self_vel.x` was never reset. Before this fix, the port's own computed
`position.x` at frame 14 (`33.836673736572266`) was within noise of
frame 13's own value (`33.8367...`), i.e. velocity had been zeroed and
this frame's own displacement was near-nil, a `0.76`-unit gap from the
recording's own smoothly-continuing `33.07349395751953`.

**Tests**: `game_fox_neutral_special`'s new `aerial_entry_preserves_
velocity_but_grounded_entry_zeros_it` drops a fighter for several frames
to build up a known, nonzero falling velocity and its own per-frame
gravity delta, then presses B in the air and confirms the post-transition
velocity is exactly the pre-existing velocity plus one more frame's
ordinary gravity -- not zeroed, halved, or otherwise overridden -- while
a matching grounded press in the same test still zeros both `velocity`
and `ground_velocity`, confirming the fix is scoped to the air branch
only. `cargo fmt --check`, `cargo clippy --workspace --all-targets` and
`cargo test --workspace` (both default and `c-oracle` features) all pass.

Measured against the published gameplay-export pack v10 (`/mnt/archive/
datasets/melee/skirmish-gameplay/v10-snapshot-20260913/fox-bf`): 149
frames now match (`-123` through `25`), up from 137; `fox-bf-baseline.
json` moves to reflect this. The new first divergence is frame 26,
field `action_state` on P1 (expected `0x0018` = `KneeBend`, actual
`0x004e` = `DamageN1`) -- P1 is hit by something Skirmish's own
simulation does not expect at that frame, a separate, undiagnosed root
cause not pursued further in this entry.

The 2026-09-13 `fox-bf.slp` re-measurement against gameplay-export pack
v10 (the real-replay parity loop, `docs/parity.md`) confirms the
concurrent `fox-fd-4.slp` loop's EscapeAir entry-advance fix (this file's
own next entry, below) also resolves this recording's own frame -14
`action_age` divergence: `cargo run -p skirmish-cli -- make-
initialization`/`validate-replay` against pack v10 (`/mnt/archive/
datasets/melee/skirmish-gameplay/v10-snapshot-20260913/fox-bf`)
reproduces byte-identical numbers to pack v9 (`137` frames matching,
first divergence at frame `14`), confirming the pack switch is a pure
version bump and not a behavior change. `fox-bf-baseline.json` moves
from `v9`/`-14`/`109` to `v10`/`14`/`137`. The new first divergence
(frame 14, `position.x` on P4, on the frame P4 enters Fox's own aerial
neutral special) belongs to the concurrent script-driven Blaster-timing
batch and is not pursued here.

The 2026-09-12 EscapeAir entry-advance fix (the real-replay parity loop
on `fox-fd-4.slp`, `docs/parity.md`) covers another sibling instance of
the entry-advance bug: `ftCo_80099A9C` (the air dodge's entry, reached
from `ftCo_80099A58`'s fresh L/R check) calls `ftAnim_8006EBA4(gobj)`
immediately after `Fighter_ChangeMotionState`, the identical extra
advance `ftCo_Dash_Enter`/`ftCo_Squat_Enter` make. Fixed at the source
the same way: `escape_air::try_air_dodge` now sets `fighter.action_frame
= 1` (not `0`) right after `simulation::enter(fighter, Action::EscapeAir)`.

Unlike Dash/Turn/Squat, EscapeAir's own physics/pose sampling indexes a
supplied per-frame sample array directly by `action_frame`
(`escape_air::frame`, `update_animation`'s `Action::EscapeAir` arm), and
that array's own convention (`try_air_dodge`'s pre-existing comment,
"`ftAnim_8006EBA4` runs sample 0's script") already treated Melee's
`cur_anim_frame == 1` as sample index `0` -- i.e. `frames[cur_anim_frame
- 1]`. Setting `action_frame` to match `cur_anim_frame` directly (instead
of leaving it one behind, as the unfixed code effectively did) therefore
needs the sample lookups themselves to gain the matching `- 1`: both
`escape_air::frame` and the `Action::EscapeAir` bounds/index arm in
`update_animation` now index by `action_frame.saturating_sub(1)` rather
than `action_frame` directly. `tests/game_air_dodge.rs`'s existing sample-
content tests (intangibility windows, the FallSpecial hand-off frame
count, velocity-decay-by-sample) all pass unchanged with this
compensating shift, confirming the effective sample sequence is
preserved; only the one test that pinned the entry frame's own
`action_frame` (`1`) needed updating, to `2` (`start_squat`'s own entry
pin, `fighter::action_frame` plus the shared end-of-frame `+= 1`).

The 2026-09-12 Squat entry-advance fix (the real-replay parity loop on
`fox-fd-2.slp`/`fox-fd-4.slp`, `docs/parity.md`) covers a sibling instance
of the entry-advance bug the 2026-09-12 special-entry-advance batch fixed
below, this time in the common (not Fox-specific) `ftCo_Squat_Enter`
(`ftCo_Squat.c:60-69`): it calls `ftAnim_8006EBA4(gobj)` immediately after
`Fighter_ChangeMotionState`, the identical extra advance `ftCo_Dash_Enter`/
`ftCo_Turn_Enter` make. `ftCo_Squat_Anim`'s own exit check
(`!ftAnim_IsFramesRemaining(gobj)`, reached from `ftCo_Wait_IASA`/`ftCo_
Walk_IASA`/`ftCo_RunBrake_IASA`'s shared `ftCo_800D5FB0` call) reads `cur_
anim_frame` directly, so without this advance Squat's own `action_frame >=
crouch_animation_frames` gate (`game::locomotion`) read one frame behind,
holding Squat one frame too long before converting to SquatWait. Fixed at
the source the same way as `start_dash`/`start_turn`: a new `start_squat`
helper sets `action_frame = 1` (not `0`) at entry, used at both call sites
`ftCo_800D5FB0` reaches (the shared Wait/Walk/interruptible-tilt chain and
`RunBrake`'s own copy). `tests/game_locomotion.rs`'s `crouch_thresholds_
have_hysteresis_and_do_not_reverse_while_held` pins the entry frame's own
`action_frame` (`2`, matching `start_dash`'s own entry pin: `start_squat`'s
explicit `1` plus the shared end-of-frame `+= 1`) and the exact held-frame
count before SquatWait converts (five, one fewer than before this fix).
This fix alone does not move either `fox-fd-2.slp`'s or `fox-fd-4.slp`'s
own baseline (confirmed by re-measuring): the divergence this loop was
chasing on `fox-fd-4.slp` turned out to be `game::locomotion::update_
actions`'s Landing-specific squat-entry branch calling the wrong destination
entirely, not this timing bug -- see this file's own next entry.

The 2026-09-12 Landing-to-SquatWait routing fix (the same loop) fixes that
next-diagnosed cause: `ftCo_Landing_IASA`'s own squat check
(`ftCo_Landing.c:146-147`) calls `ftCo_SquatWait_CheckInput` directly, not
`ftCo_Squat_CheckInput` -- an interruptible Landing with the stick held down
steps straight into SquatWait (`fn_800D62C4`'s own `Fighter_
ChangeMotionState`, no extra `ftAnim_8006EBA4` advance), skipping the
ordinary crouch-down animation entirely, unlike Wait/Walk/RunBrake's own
down-stick check (`ftCo_800D5FB0`, above). `game::locomotion::update_
actions` previously routed every down-stick entry through the same
`start_squat`/`Action::Squat`, Landing included; it now branches on
`f.action == Action::Landing` and enters `Action::SquatWait` directly for
that case. Confirmed directly against `fox-fd-4.slp`: P4 lands an ordinary
short hop and holds down at frame -34 through -31 (`Action::Landing`,
`action_state` 42, ages 0..3), then the recording shows `action_state` 40
(SquatWait) at frame -30 with no intervening frame at `action_state` 39
(Squat) -- confirming the direct routing, not a fast crouch-down animation.
`tests/game_landing.rs`'s `first_interruptible_frame_opens_the_complete_
wait_chain_and_the_crouch` updates its own `"crouch"` case from
`Action::Squat` to `Action::SquatWait` to match. Moved `fox-fd-4.slp`'s
`checked_frames` from 93 to 103 (`first_divergent_frame` from -30 to -20);
`fox-fd-2.slp` is unaffected (its own divergence is on an unrelated
Fox-specials pack-export gap, `docs/parity.md`).

The 2026-09-12 edge-fall air-drift-clamp fix (the real-replay parity
loop, `fox-bf.slp`, `docs/parity.md`) adds the one call `game::collision::
resolve`'s ordinary ground-lost branch was missing: `ftCo_Fall_Enter`
(`ftCo_Fall.c:47-70`) unconditionally calls `ftCommon_ClampAirDrift`
(`ftcommon.c:457-459`, itself `ftCommon_ClampSelfVelX(fp, ca->
air_drift_max)`) right after `Fighter_ChangeMotionState`, regardless of
whether `ground_or_air` was `Ground` or already `Air` -- clamping
`self_vel.x` to `+/-ca->air_drift_max` on every entry into the ordinary
`Fall` state. `ft_80084104` (`ft_081B.c:1044-1050`, the generic per-frame
ground/collision dispatcher every Walk/Dash/Run/Turn `Phys` callback
routes through) is the exact call site: `if (!ft_800827A0(gobj))
ftCo_Fall_Enter(gobj);` -- "if the ground check just failed, enter Fall" --
matching `game::collision::resolve`'s own ground-lost branch (the
`f.grounded = false; ... simulation::enter(f, Action::Fall);` block,
reached when none of grab/specials/damage's own ground-air transfers
claim the frame first). That branch already ports `ftCo_Fall_Enter`'s
ECB-lock side effect (`ftCommon_8007D5D4`'s `ecb_lock = 10`) but not its
unconditional `ClampAirDrift` call: a fighter running or dashing off a
platform edge carries its full ground speed (well above `air_drift_max`
for every real character) straight into `f.velocity[0]` on the first
airborne frame instead of the clamped value.

`game::collision::resolve` now builds a scratch `Movement` (`self_
velocity` seeded from `f.velocity`, `attributes: data.movement.
physics()`) and calls `clamp_air_drift()` on it immediately after
`simulation::enter(f, Action::Fall)`, writing the clamped `self_velocity[0]`
back to `f.velocity[0]` -- the same pattern `game::collision::
begin_pass_as` already uses for the explicit platform-drop path
(`ftCo_8009A184`/`ftCo_8009A228`), which already carried this exact clamp
for its own, narrower transition.

Confirmed directly against `fox-bf.slp`: P1 runs off Battlefield's left
platform at frame -24 (Slippi action state 20, `Run`, ground speed
`2.172501` units/frame from consecutive frame deltas) and its very next
position sample (frame -23) reflects only `0.81` units/frame of drift --
consistent with `air_drift_max` scaled by that frame's own stick tilt
(`0.975`), not the carried-over run speed. Before this fix, `game::
collision::resolve` left `f.velocity[0]` at the full run speed on the
Fall-entry frame, landing `position.x` at `-17.28374481201172` instead of
the recording's own `-18.626245498657227` (a `1.34`-unit gap, not a
rounding artifact) one frame later.

**Tests**: `game_edges`'s existing `dash_and_run_past_the_end_fall_off_it`
(already exercising both a dashing and a running fall off a floor end)
gains an assertion that `velocity[0]` on the first airborne frame is
clamped to the fixture's own `air_drift_max` (`1.5`, `tests/fixtures/
game/integration-match.json`), well below both the dash (`dash_max_
velocity` `2.5`) and run ground speeds it was exercising unclamped
before this fix. `cargo fmt --check`, `cargo clippy --workspace
--all-targets` and `cargo test --workspace` all pass.

Measured against the published gameplay-export pack v9 (`/mnt/archive/
datasets/melee/skirmish-gameplay/v9-snapshot-20260912/fox-bf`): 109
frames now match (`-123` through `-14`), up from 100; `fox-bf-baseline.
json` moves to reflect this. The new first divergence is frame -14,
field `action_age` on P4 (expected `1.0`, actual `0.0`), a separate,
undiagnosed root cause -- fixed above by the EscapeAir entry-advance fix,
independently discovered on `fox-fd-4.slp` by a concurrent batch, which
already covers this exact bug (`ftCo_80099A9C`'s own extra advance).

The 2026-09-12 shield-regeneration-on-conversion-frame fix (the real-replay
parity loop, `fox-bf.slp`, `docs/parity.md`) stops `Fighter_ProcessHit_
8006D1EC`-equivalent regeneration (`game::shield::finish_frame`) from
landing on the exact frame a still-active `GuardOn`/`Guard` converts
straight into `Pass` via `begin_pass`. `Fighter_ProcessHit_8006D1EC`
(priority 0xE, `fighter.c:908,2821`) gates regeneration on `fp->x221A_b7`,
a flag only ever set by `GuardOn`/`Guard`/`GuardReflect`'s own entries
(`ftCo_Guard.c:267,523,708,803,984`) and unconditionally cleared by every
`Fighter_ChangeMotionState` call (`fighter.c:1048`) -- including the same
one `ftCo_8009A184`/`ftCo_8009A228` (`begin_pass`'s own entry) makes.
`fox-bf.slp`'s own P4 shows no regeneration lands on that conversion
frame regardless: its shield health is explained in full by the passive
drain alone.

`game::shield::finish_frame` gains an explicit `was_active: bool`
parameter in place of recomputing `active(f)` internally (the fighter's
action by the time this runs downstream, after any of this frame's own
transitions). Its caller, `game::simulation::advance`, still passes plain
`active(f)` in the ordinary case, folding in a new `shield_active_into_
pass: [bool; 2]` recorded at the exact `pass_request_after_actions`/
`begin_pass` call site: whether the fighter was still `active()`
immediately before that specific conversion. This is deliberately
narrower than "skip regeneration on any exit from an active shield
state": an existing, already-passing native test (`game_escape`'s
`escape_clears_the_shield_and_lets_health_regenerate`) already pins
*immediate* regeneration on the frame `Guard` converts into a roll via
`ftCo_8009917C`/`ftCo_8009980C` -- a different `Fighter_ChangeMotionState`
call this same unconditional `x221A_b7` clear also reaches. Broadening
the fix to cover every exit from an active shield state (snapshotting
`active()` once at the true start of the frame, before *any* of that
frame's own transitions) regressed that test (`49.999992` where `50.0`
was expected: one frame of `0.1` fixture-scale regeneration silently
missing); the narrower, call-site-scoped flag leaves every other exit
from `GuardOn`/`Guard`/`GuardReflect`/`GuardSetOff` (release into
`GuardOff`, rolls, spot dodges, jumps, shield breaks) exactly as before,
pending each transition's own independently diagnosed recording evidence
rather than a single inferred general rule.

Confirmed bit-exact against `fox-bf.slp` (together with the passive-drain
timing fix above): P4's shield lands on `59.76071548461914` at frame -27,
the recording's own value. Either fix alone still diverges, by a
different amount each: without the trigger-timing fix,
`59.83071517944336` (the conversion frame's own `1.0` trigger, loss
`0.28`, regeneration `0.07` wrongly added back); without this
regeneration fix, `59.79000091552734` (frame -28's own trigger, loss
`0.2393`, regeneration `0.07` still wrongly added back).

**Tests**: `game_platform_drop`'s new `no_regeneration_lands_on_the_
frame_guard_on_converts_into_pass` isolates this fix alone with a steady
digital press (unaffected by the separate trigger-timing fix, since the
digital override reads the same value every frame either way), against
the existing `shield.json` fixture's own rules: the entry frame itself
never runs the shield-owning `Anim` arm at all, so health stays `50.0`;
the next, still-held frame drains against the entry frame's own press
(`50.0 -> 49.8`); the conversion frame drains the same way again (`49.8
-> 49.6`) with regeneration correctly withheld -- an unfixed
regeneration gate would land on `49.7` instead (`49.6 + 0.1`). The full
existing suite, including `game_escape`'s roll-regeneration test above,
continues to pass unmodified. `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings` and `cargo test --workspace`
(both default and `c-oracle` features) all pass; `SKIRMISH_GAMEPLAY_DATA=
/mnt/archive/datasets/melee/skirmish-gameplay/v2 cargo test -p
skirmish-cli --test real_parity` (the multi-recording ratchet) passes
against the newly published gameplay-export pack v9, which moves
`fox-bf-baseline.json` from `96`/`-27` to `100`/`-23`.

The 2026-09-12 passive shield-drain input-timing fix (the real-replay
parity loop, `fox-bf.slp`, `docs/parity.md`) replaces `game::shield::
update_animation`'s `GuardOn`/`Guard`/`GuardReflect` arm's `input.
shield_pressure()` read with `f.previous_input.shield_pressure()`.
`Fighter_8006A360` (priority 1, `HSD_GObj_SetupProc(gobj, &Fighter_
8006A360, 1)`, `fighter.c:898`) calls the destination action's own
`anim_cb` (`ftCo_GuardOn_Anim`/`ftCo_Guard_Anim`, and so `ftCo_800925A4`'s
own read of `fp->input.triggers[0]`) before `Fighter_Spaghetti_8006AD10`
(priority 3, `fighter.c:900`) refreshes `fp->input.{lstick,cstick,
triggers,held_buttons}[0]` for the frame from the raw pad/CPU state
(`fighter.c:1790-1839`): the passive shield-health drain always reads the
previous frame's own processed trigger, one frame stale, unlike the IASA/
action-transition checks dispatched afterward at priority 3 that already
see the frame's own fresh value. `f.previous_input` (set from this
match's own raw input only later in `simulation::advance`, after
`update_animation` already ran) already holds exactly that value.

Confirmed bit-exact directly against `fox-bf.slp`: P4's shoulder trigger
rises `0.8928571343421936` (frame -28, `GuardOn`'s only frame) then
`1.0` (frame -27, the same frame `GuardOn` converts into `Pass`); reading
frame -28's own trigger for frame -27's drain (`strength` `0.8469387888908386`,
`drain_rate` `0.14000000059604645`, `drain_scales` `[0.10000000149011612,
2.0]`) computes a loss of `0.23928572237491608` from `60.0`, landing on
`59.76071548461914` -- the recording's own value bit-for-bit -- where
reading frame -27's own `1.0` trigger instead computes a loss of `0.28`,
landing on `59.72000122070312`.

**Tests**: `game_shield`'s new `passive_drain_reads_the_previous_frames_
trigger_not_the_current_frames` pins four consecutive synthetic frames
against the existing `shield.json` fixture's own rules (`analog_deadzone`
`0.2`, `drain_rate` `0.2`, `drain_scales` `[0.5, 1.0]`): the entry frame
itself never runs the shield-owning `Anim` arm at all (`update_actions`,
priority 3, only enters `GuardOn` after priority 1's `Anim` already ran
this frame against the old, non-shielding action), so health stays
`50.0`; the next, still-held frame drains against the entry frame's own
`1.0` trigger (`50.0 -> 49.8`); dropping to `0.6` still drains at the
*previous* frame's `1.0` rate on the very frame it changes (`49.8 ->
49.6`, not `49.65`), only catching up to `0.6`'s own rate the frame after
(`49.6 -> 49.449997`). `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings` and `cargo test --workspace` (both default
and `c-oracle` features) all pass.

Measured against the published gameplay-export pack v9 (`/mnt/archive/
datasets/melee/skirmish-gameplay/v2/fox-bf`, the first pack to embed
`fast_fall_window`): this fix alone does not move the checked-frame count,
since the pack's own P4 shoulder press is still masked by the separate
same-frame shield-regeneration bug fixed next; both together move it from
96 to 100 (`fox-bf-baseline.json`).

The 2026-09-12 script-driven Blaster timing batch (`docs/
fox-neutral-special.md`'s "Script-driven arming and fire timing") replaces
the neutral special's own approximated fire timing and Start-never-arms
repeat gate with the exporter's newly-decoded per-frame `SetCmdVar` trace
(`specials.neutral.script`/`specials.side.script`, `ftaction.c:454-475`
opcode 19), when a fighter's own resource supplies it. `NeutralSpecial`/
`SideSpecial` each gain `script: Option<Box<NeutralScript>>`/
`Option<Box<SideScript>>` (boxed: inlining it grew every `MatchData`
enough to overflow an unrelated, already-marginal deeply-recursive test's
default stack -- `game_damage_floor::malformed_floor_profiles_are_
rejected_transactionally`, which never touches Fox's specials at all;
boxing this optional, sparsely-populated field fixed it without touching
that test), validated to have exactly as many per-frame `cmd_vars`/
`allow_interrupt` entries as their own phase's pose count.
`neutral::apply_script_frame` recovers each slot's true one-shot `SetCmdVar`
frame from the exporter's forward-filled export by comparing consecutive
frames, since a naive per-frame overwrite would re-arm the fire flag every
frame after the real one (the exporter does not simulate the native side's
own same-frame consumption/clear of it). `neutral::enter_loop` gained a
`preserve_armed` parameter: a repeat Loop pass resets `isBlasterLoop`
(matching `FinishLoopTransition`'s own explicit reset), but the
Start->Loop transition does not (matching `ftFox_SpecialN_
StartAnimation`, which never touches it) -- so a press during Start's own
arm window (real Fox: frame 4) now correctly survives into the first Loop
pass, which the pre-script approximation (Start hard-coded as never-armed)
could not model at all. Both approximations remain reachable, used
whenever `script` is `None` (Falco, any not-yet-re-exported fixture).

`tests/oracle/fox_neutral_special.c` gained a second, stateful oracle
harness (`NeutralScriptTrace`) that replays a whole Start->Loop->Loop pass
against the real decomp functions, one `SetCmdVar` event applied per
simulated frame; `tests/fox_neutral_special_differential.rs`'s own
`script_trace_matches_the_real_fox_timings` (a concrete trace shaped like
Fox's real timings) and `script_trace_arms_only_at_or_after_the_scripted_
frame` (256 proptest cases fuzzing the set/press frame) both compare
against it. Two new native-engine tests (`tests/game_fox_neutral_
special.rs`) graft a synthetic `script` onto the existing fixture and
confirm end to end: a press during Start's own arm window arms the repeat
before Loop is ever entered, and the shot fires strictly after Loop's
entry tick, not on it.

Confirmed against a real recording, not merely the exported script:
`tests/fixtures/slippi/parity/fox-fd-3.slp` (`18_24_36 [H2O] Fox + Fox
(FD).slp`, the only one of the four `fox-fd`-pairing recordings new enough
to carry Slippi item events at all -- format `3.9.0`; `fox-fd-2.slp`/
`fox-fd-4.slp` are format `2.0.1`, predating item events entirely, and
record zero items throughout) shows every one of its five independent
`FOX_LASER` spawns landing at exactly `state_age == 5.0` on the shooting
fighter, matching the script's own Loop-air fire frame exactly. Measuring
`fox-fd-2.slp`/`fox-fd-4.slp` against a locally spliced pack (this batch's
own `specials.*.script`, from `/mnt/shared/tmp/skirmish-gameplay-cmdvars/
fighters/fox.json`, grafted onto a local, uncommitted copy of the
committed `fox-fd/match-data.json`) finds `fox-fd-2` first diverging at
frame -5 (`last_attack_landed`, 118 frames checked) and `fox-fd-4` at
frame -30 (`action_state`, 93 frames checked) -- both still inside the
pre-"GO" window, unrelated to Blaster and not chased further; the
committed baselines are unchanged (measured against the spliced copy only,
never against the official pinned export).

`cargo test --locked --workspace` (default and `c-oracle` features) and
`cargo clippy --locked --workspace --all-targets --all-features -- -D
warnings` both pass.

The 2026-09-12 fast-fall stick-timer window fix (the real-replay parity
loop, `fox-bf.slp`, `docs/parity.md`) replaces `game::simulation::
move_fighter`'s approximate `previous_input`-edge fast-fall heuristic with
an exact port of `ftCommon_CheckFallFast` (`ftcommon.c:492-503`) gated on a
new optional pack field. `ftCommonData+0x8C` (`types.h:88`, immediately
after the already-exported `fast_fall_threshold` at `+0x88`) was read
directly from the retail disc (`PlCo.dat`, via the same `ftLoadCommonData`
pointer resolution `skirmish-assets`'s own `gameplay::common::decode`
already uses for `fast_fall_threshold`, confirmed by that same read
recovering `0.6625000238418579` byte-identical to the published pack): a
plain `int`, value `4`, matching its already-named integer neighbors
`dash_smash_window` (`+0x40` = `2`) and `tap_jump_window` (`+0x74` = `4`)
in both type and magnitude.

`game::data::Rules` gains `fast_fall_window: Option<u32>` (`#[serde(
default, skip_serializing_if = "Option::is_none")]`, so every existing
fixture without it keeps loading and behaving exactly as before).
`fighter::damage::fast_fall_trigger` is a new pure function porting
`ftCommon_CheckFallFast`'s exact condition (`!fall_fast && velocity_y <
0.0 && stick_y <= -threshold && tilt_y_age < window`) against the already-
tracked `fighter.locomotion.tilt_y_age` (`fighter::damage::tilt_timer`,
already modeling `fp->x671_timer_lstick_tilt_y`). `game::simulation::
move_fighter` calls it when `rules.fast_fall_window` is `Some`, falling
back unchanged to the previous heuristic when it is `None`.

**Tests**: `fighter::damage`'s new `fast_fall_trigger_uses_the_tilt_timer_
not_previous_input` pins two real `fox-bf.slp` data points directly
(frame -27, P4: `tilt_y_age = 254` after a platform pass, does not
trigger; frame -21, P1: `tilt_y_age = 1`, an ordinary fresh press, does
trigger), the window's own exclusive boundary (age `3` triggers, age `4`
does not, matching decomp's `<`, not `<=`), and that every other
precondition (already fast-falling, non-negative velocity, stick short of
the threshold) still refuses the trigger regardless of `tilt_y_age`.
`cargo fmt --all -- --check`, `cargo clippy --locked --workspace
--all-targets --all-features -- -D warnings` and `cargo test --locked
--workspace` (both default and `c-oracle` features) all pass.

Confirmed end-to-end with a throwaway diagnostic copy of `fox-bf/
match-data.json` patched to add `"fast_fall_window": 4` (not committed,
deleted after use, the same kind of diagnostic patch this file's own
`friction_above_walk` entry used): `position.y` now matches at frame -27
against that patched copy. Measured against the real, unpatched live pack
(`/mnt/archive/datasets/melee/skirmish-gameplay/v2/fox-bf`, which does not
export `fast_fall_window` yet), behavior and the measured frame count are
byte-identical to before this fix (96 frames, first divergence -27) since
the fallback heuristic path is unchanged; `fox-bf-baseline.json` is
therefore left unmoved by this commit and will advance once the exporter
publishes the real field, without any further Skirmish code change. The
diagnostic copy's own new first divergence (still frame -27, field
`shield` on P4: expected `59.76071548461914`, actual `59.790000915527344`)
is a separate, undiagnosed shield-health decay-rate/timing discrepancy on
the same Guard->Pass transition frame -- reported in `docs/parity.md`
rather than chased in this batch.

The 2026-09-12 GuardOn/Guard/GuardReflect constant `state_age` fix (the
real-replay parity loop, `fox-bf.slp`, `docs/parity.md`) corrects
`crates/skirmish-replay/src/observation.rs`'s `action_age`: `ftCo_800923B4`/
`ftCo_80092C54` (GuardOn/Guard's own entries, `ftCo_Guard.c:386,509,790,
1012`) and `ftCo_8009388C` (GuardReflect, `:901`) all call `Fighter_
ChangeMotionState` with `Ft_MF_SkipAnim`. `Fighter_ChangeMotionState`
unconditionally lands `cur_anim_frame` on `anim_start - anim_speed`
(`fighter.c:1224`, `0 - 1 = -1` here) for every transition, but `Ft_MF_
SkipAnim` additionally skips the generic per-frame animation advance
(`Fighter_Spaghetti_8006AD10`'s unconditional `ftAnim_8006EBA4(gobj)`,
`fighter.c:1684`) that brings every *other* action's `cur_anim_frame` back
to `0` by the end of its own entry frame: the shield family owns no
scripted animation figatree for that advance to move, so `state_age` stays
a constant `-1` for the whole state, not just its entry frame -- unlike
`GuardSetOff` (`Ft_MF_None`, no `SkipAnim`), which keeps the ordinary rule.

Confirmed directly against `fox-bf.slp` (`/mnt/archive/datasets/melee/
slippi-public-dataset-v3.7/data/FOX/batch_00/18_21_03 Fox + Fox (BF).slp`,
the first Battlefield real-replay recording, ports P1/P4): P1 holds
GuardOn then Guard for 21 frames (1430-1450) with `state_age = -1.0`
throughout, then GuardSetOff at 1451 already reports the ordinary `0.0`;
P4's own one-frame GuardOn shield-drop at frame -28 (this measurement's own
divergence) reports the same `-1.0` on its only frame before converting
into `Pass` the next frame.

**Tests**: `crates/skirmish-replay/src/observation.rs`'s new
`guard_family_reports_a_constant_negative_one_state_age` calls the
extracted `action_age` directly for GuardOn/Guard/GuardReflect with an
artificially large `action_frame` (so a passing assertion cannot be a
coincidence of the old and new formulas agreeing) and asserts GuardSetOff
is unaffected. `crates/cli/tests/replay_match.rs`'s own harness-local
duplicate of this formula (its own doc comment already flags it as needing
to track `observation::observe`, the same one the landing-fall-special
batch updated) gained the identical branch; six existing shield tests
(`physical_l_drives_file_backed_shield_state_and_detects_its_removal`,
`file_backed_shield_drop_matches_and_detects_the_first_changed_stick_
frame`, `file_backed_cstick_shield_jump_matches_and_detects_the_first_
changed_frame`, `file_backed_shield_escapes_match_and_detect_their_first_
changed_input_frame`, `file_backed_shield_grabs_match_and_detect_their_
first_changed_button_frame`, `file_backed_powershield_covers_reflector_
immunity_and_guard_reflect_state`) already recorded synthetic fixtures
starting a fighter in GuardOn and would otherwise have started failing at
frame 0 once `observation::observe` itself changed (the harness's embedded
"expected" value would have stayed the stale `0.0` while the freshly
re-simulated "actual" value became the corrected `-1.0`). `cargo fmt --all
-- --check`, `cargo clippy --locked --workspace --all-targets
--all-features -- -D warnings` and `cargo test --locked --workspace` all
pass.

Measured against the live pack (`/mnt/archive/datasets/melee/
skirmish-gameplay/v2/fox-bf`; this pairing has no `SKIRMISH_GAMEPLAY_DATA`
snapshot yet): 96 frames now match (-123 through -28), up from 95 at the
initial measurement (itself already 95, not the replay's own -123, thanks
to origin/main's independent Walk entry-time fix landing first). The new
first divergence is frame -27, field `position.y` on P4, on the same frame
P4's GuardOn converts into `Pass`: `game::simulation::move_fighter`'s
fast-fall trigger approximates decomp's own `ftCommon_CheckFallFast`
(`fp->x671_timer_lstick_tilt_y < p_ftCommonData->x8C`, an unexported,
not-yet-named window constant adjacent to the already-exported
`fast_fall_threshold` at `+0x88`) with a `previous_input`-based heuristic
that does not consult `fighter.locomotion.tilt_y_age` (already modeled at
the source and already reset to the `254` sentinel by `game::locomotion::
pass_request_after_actions`, matching decomp's own `x671_timer_lstick_
tilt_y = 0xFE` reset in `begin_pass`) at all, so it wrongly re-triggers
fast-fall on the same continuously-held down-stick that just triggered the
platform pass. Reported per this loop's own stop condition (pack data
needed) rather than fixed with a guessed window value; see `docs/
parity.md`'s own "A first Battlefield recording" section for the full
diagnosis and citation.

`tests/fixtures/slippi/parity/fox-bf.slp` and its pairing, `recordings.json`
entry and `fox-bf-baseline.json` are new in this batch, adding Battlefield
(platforms, pass-through, ledge/teeter) to the real-replay ratchet for the
first time; `manifest.json` gains the recording's own provenance entry.

The 2026-09-12 special-entry-advance batch (the real-replay parity loop on
`fox-fd-2.slp`/`fox-fd-4.slp`, `docs/parity.md`) fixes the `action_age`
divergence (expected `1.0`, actual `0.0`) both recordings hit at Fox's
neutral special (Blaster) entry frame (`-31` on `fox-fd-2.slp`, `-38` on
`fox-fd-4.slp`; `falco-fox-fd.slp` hits the same shape at `-30`), diagnosed
by the previous batch as "almost certainly `ftFx_SpecialN_Enter`/
`ftFx_SpecialNStart` making the extra `ftAnim_8006EBA4` advance like Dash/
Turn/Walk". Confirmed exactly: `ftFox_SpecialN_InitializeState`
(`ftfoxspecialn.c:246-252`), called from both `ftFx_SpecialN_Enter` and
`ftFx_SpecialAirN_Enter`, makes an extra, explicit `ftAnim_8006EBA4(gobj)`
call immediately after `Fighter_ChangeMotionState` lands `cur_anim_frame`
on `0.0` -- the identical second advance `ftCo_Dash_Enter`/`ftCo_Turn_
Enter` make (`locomotion::start_dash`'s own comment above). Grepping every
`_Enter` in `src/melee/ft/` that calls `ftAnim_8006EBA4` (directly, or
through a same-file `_InitializeState`-style helper) and cross-referencing
against every Fox special move already ported turned up five more
undocumented instances of the identical bug, all fixed in this one batch
since they share one root cause and one fix shape (`fighter.action_frame =
1` immediately after `simulation::enter`, or `+= 1` on an already-nonzero
start frame):

| Decomp `_Enter` | Motion state | Rust site | Fix |
| --- | --- | --- | --- |
| `ftFx_SpecialN_Enter`/`ftFx_SpecialAirN_Enter` (via `ftFox_SpecialN_InitializeState`) | `SpecialNStart`/`SpecialAirNStart` | `characters::fox::neutral::Move::update_actions` | `action_frame = 1` at entry |
| `ftFx_SpecialSStart_Enter`/`ftFx_SpecialAirSStart_Enter` | `SpecialSStart`/`SpecialAirSStart` | `characters::fox::side::Move::update_actions` | `action_frame = 1` at entry |
| `ftFx_SpecialLw_Enter`/`ftFx_SpecialAirLw_Enter` | `SpecialLwStart`/`SpecialAirLwStart` | `characters::fox::down::enter_start` | `action_frame = 1` at entry |
| `ftFx_SpecialHi_Enter`/`ftFx_SpecialAirHiStart_Enter` | `SpecialHiHold`/`SpecialHiHoldAir` | `characters::fox::up::Move::update_actions` | `action_frame = 1` at entry |
| `ftFx_SpecialHiBound_Enter` | `SpecialHiBound` | `characters::fox::up::enter_bound` | `action_frame = 1` at entry |
| `ftFx_SpecialHiFall_Enter` (start frame `13.0`, not `0`) | `SpecialHiLanding` (from `SpecialHiFall`'s own ground contact) | `characters::fox::up::Move::land`'s `SpecialHiFall` arm | `action_frame = 13 + 1 = 14` (was pinned at `13`, missing the same extra advance) |

Every other `_Enter` in the same six files (Travel/Dash/End-phase entries:
`ftFx_SpecialAirHi_Enter`, `ftFx_SpecialHi_GroundToAir`/`_AirToGround`,
`ftFx_SpecialS_Enter`/`ftFx_SpecialAirS_Enter`/`ftFx_SpecialSEnd_Enter`/
`ftFx_SpecialAirSEnd_Enter`, `ftFx_SpecialLwLoop_Enter`/`ftFx_
SpecialAirLwLoop_Enter`/`ftFx_SpecialLwHit_Enter`/`ftFx_SpecialLwEnd_Enter`/
`ftFx_SpecialAirLwEnd_Enter`) does not call `ftAnim_8006EBA4` a second time
and is confirmed unchanged (checked directly against each file, not
inferred): only the very first phase a fresh `B` press or a Hold/Bound/
ground-contact re-entry reaches gets the extra advance; the Travel/Dash
phases and every subsequent internal hand-off do not. Captain Falcon,
Game & Watch, Mars (Marth-family), Popo/Nana and Seak (Sheik) each have
their own analogous `_Enter` instances in the same grep (not listed in the
table above): none of those characters are modeled in Skirmish yet, so
they are out of scope for this batch, left for whichever future batch
ports them.

Every fixed site's own existing unit test that stepped through the
transition needed its pinned `action_frame`/idle-frame-count updated to
match (each test's own comment now cites this batch): `tests/
game_fox_up_special.rs`'s `ground_entry_enters_hold_with_gravity_delay_
and_jumps_untouched` (`action_frame` `1` -> `2`) and `fall_lands_at_
frame_13_via_ordinary_ground_touch` (`14` -> `15`); `tests/
game_fox_side_special.rs`'s `ground_entry_from_wait_enters_start_with_
gravity_delay_and_jumps_untouched` (`1` -> `2`) and the Start->Dash
hand-off tests (`ground_dash_entry_state`, `dash_phase_air_trans_n_sets_
both_axes_unconditionally`, `b_press_shortens_the_air_dash_into_end`),
whose own 4-pose Start clip now hands off to Dash after 3 idle frames past
entry, not 4, since the clip's own `action_frame` count starts one frame
ahead; `tests/game_fox_neutral_special.rs`'s `a_fresh_b_press_repeats_
the_loop_while_no_press_ends_it` and `the_laser_travels_before_hitting_
and_despawns_on_contact`, whose 2-frame synthetic Start fixture now hands
off to Loop one idle frame sooner for the identical reason.
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings` and `cargo test --workspace --exclude
renderer --tests --lib` (161 test binaries, 0 failed) all pass.

Measured against the live gameplay export pack (`/mnt/archive/datasets/
melee/skirmish-gameplay/v2/fox-fd/match-data.json`, which now carries
`specials.neutral` for Fox): `fox-fd-2.slp` moves from 92 to 98 matched
frames (`-123` through `-26`); `fox-fd-4.slp` moves from 85 to 91 matched
frames (`-123` through `-33`). Both recordings' entry-frame `action_age`
now matches exactly (confirmed directly against `fox-fd-2.slp` via
`py-slippi`: P1's `state_age` is `1.0` on the very entry frame, `-31`,
climbing `2.0`..`6.0` through `-30`..`-26`). Both then hit a new, identical-
shape divergence one phase later, reported rather than chased further (see
`docs/parity.md`): `action_state` shows the recording already in
`SpecialNLoop` (`0x0156`) one frame before Skirmish, which is still in
`SpecialNStart` (`0x0155`). Traced to `characters::fox::neutral::Move::
update_animation`'s `action_frame >= parameters.start.ground.frames.len()`
gate: the live pack's `fighters[0].specials.neutral.start.ground.frames`
has length `8`, but `fox-fd-2.slp`'s own recorded `state_age` sequence
(`1.0` through `6.0`, six values, confirmed via `py-slippi`) shows the real
grounded Start phase lasting exactly 6 frames -- matching the pack's own
`start.air.frames` length (`6`), not `start.ground.frames` (`8`). Every
sampled ground pose is bit-identical (a flat, motionless clip, so no
per-frame content distinguishes a real 8-frame clip from a padded one),
and the aerial variant of the same phase is already the length the
recording demands, so this reads as a pack-export mismatch on the ground
variant specifically (report: `fighters[0].specials.neutral.start.ground.
frames` should have 6 entries, not 8, to match `start.air.frames` and both
recordings' own observed timing) rather than an unmodeled system or a
guessable Skirmish-side fix; not chased further per this loop's own stop
condition. `tests/fixtures/slippi/parity/fox-fd-2-baseline.json`/`fox-fd-4-
baseline.json` are updated accordingly.

The 2026-09-12 fused-multiply-add audit batch (`skirmish-fma`, `docs/math.md`
has the full trail) adds `tools/ppc_fma_audit.py` (stdlib + `capstone`, `uv
run --with capstone`, never `pip install`ed) to disassemble named functions
out of the retail `main.dol` and report every real Gekko `fmadds`/`fmsubs`/
`fnmadds`/`fnmsubs` (and double-precision forms) found, resolving each
address through the DOL's own section table and
`config/GALE01/symbols.txt`. It works around a real Capstone 5 PowerPC-
backend gap (`fcmpo` isn't decoded at all; `Cs.disasm` silently truncates
the rest of the function there) by resyncing one instruction at a time
instead of stopping. Audited every function named in the batch's own brief
(the Dash/ground-acceleration and air-dodge-launch chains: zero fused ops,
settling a question two sibling batches' own diagnoses were waiting on, see
below) plus every float-touching function the c-oracle harness pins (409 of
463 resolved to a real address; 47 contain a fused op, four mirrored here).

Mirrored with `f32::mul_add` (or a negated form) at the exact expression
the disassembly shows: `ftColl_80079AB0` (`fighter::combat::knockback`,
three sites), `ftCommon_CalcHitlag` (`fighter::combat::hitlag`, one),
`ftCo_800DA824` (`fighter::grab::escape_timer`, two) and
`ftCo_Damage_CalcAngle` (`fighter::damage::launch_angle`, one).
`lbVector_AngleXY` needed more than a `mul_add`: its length calls go
through `sqrtf_accurate`'s four fused Newton-Raphson iterations (not
`f32::sqrt`), ported as `game::characters::fox::up::sqrt_accurate`, seeded
from a correctly-rounded `1.0 / f64::sqrt` rather than a bit-exact
`__frsqrte` emulation (Newton's method for `1/sqrt(x)` has one stable,
quadratically-convergent fixed point, so both reach it well before the
fourth iteration); confirmed bit-for-bit against the real, pinned
`ftFox_SpecialHi_*` C (which calls the actual `lbVector_AngleXY`,
uncontracted) across 100,000 generated cases in
`tests/fox_up_special_differential.rs`.

Two of the batch's own working hypotheses (this brief's own "evidence 1 and
2") turned out to be false, settled by direct disassembly rather than
argument. `fox-fd-3.slp` frame -32 and `falco-fox-fd.slp` frame -25's
one-ULP `velocities.self_x_air`/`position.x` divergences were suspected to
come from a fused product+sum in the Dash `getAccelAndTarget`/
`ftCommon_8007C98C`/`ftCommon_ApplyGroundMovement(NoSlide)` chain:
`ftCo_Dash_Phys` (which inlines the former and calls the latter two)
disassembles to a plain `fmuls` followed by a plain `fadds` for `accel`,
not a single `fmadds`, and none of the three called functions contain any
fused op either. `fox-fd.slp` frame 5 (`ftCo_80099A9C`'s air-dodge launch)
is likewise confirmed fused-op-free — consistent with the concurrent
`skirmish-msl-trig` batch's own direct measurement that a fully
disassembly-verified trigonometry port also ties, not fixes, that same
frame. `docs/parity.md`'s falco-fox-fd and fox-fd-3 entries, and
`falco-fox-fd-baseline.json`'s own note (both previously read "pending the
fused-op audit"), are corrected accordingly; no recording's own ratchet
baseline changes, since ruling out one candidate explanation isn't fixing
the divergence itself. Measured directly against this batch's own two
named recordings' pinned export (`SKIRMISH_GAMEPLAY_DATA=.../
v6-snapshot-20260911`): `fox-fd.slp` is unchanged at 5/128, `fox-fd-3.slp`
unchanged at -32/91, confirmed again against the live `v2` export too.
`falco-fox-fd.slp` (measured directly against `v2`, the pack it's tracked
against) is likewise unchanged at -25/98. `fox-fd-2.slp`/`fox-fd-4.slp`
are a different, concurrently-owned parity loop's own recordings
(`docs/parity.md`); measuring them against the live `v2` pack during this
batch showed both matching further than their own just-updated baselines
(from a concurrent special-entry-advance fix landing in the same window),
which is that other loop's own result, not this batch's -- neither
baseline was touched here.

Oracle strategy: `build.rs` gained a second static library,
`skirmish_oracle_fma`, compiled with `-ffp-contract=fast -mfma` (plus a
forced `opt_level(2)` — GCC's contraction is a no-op at the `-O0` `cc`
mirrors from Cargo's dev/test profile by default, found by disassembling
the compiled object and seeing plain `vmulss`/`vaddss` instead of
`vfmadd`), for the two functions whose *entire* body is a single,
unambiguous `a * b + c`: `ftCommon_CalcHitlag`, and `ftCo_Damage_CalcAngle`
(split out of `damage_core.c`'s six-function bundle into its own
`damage_calc_angle.c` translation unit for exactly this reason — several
of its five former siblings there are not fusion-free, and contracting the
whole shared file would have risked changing their oracle output out from
under comparisons this batch never re-verified). Two more were tried there
and reverted after disassembling the compiled objects showed GCC choosing
a different fusion than the retail binary: `ftColl_80079AB0`'s `inner`
term is a sum of two products (only one of the two can be fused, and GCC
picks the other pairing than CodeWarrior did), and `ftCo_800DA824`'s
contraction reached across a statement boundary to fuse a product the
real PowerPC compiler never fuses at all. Both functions' differential
tests instead compare against the unmodified, uncontracted oracle with a
documented small relative tolerance (`1e-4`, two-plus orders of magnitude
above the worst of 50,000+ generated cases) and treat both sides landing
off the finite range as agreement (a fused computation's full-precision
intermediate product can't overflow the way an unfused one can, so the two
can diverge categorically right at `f32`'s range edge — confirmed both
directions while calibrating). Each of the four mirrored functions is
additionally pinned exactly by a native Rust unit test against a
hand-verified (`libm`'s `fmaf` outside Rust) value, independent of the
oracle's own limitations for the two reverted functions.

**Tests**: new/changed coverage in `tests/combat_differential.rs`,
`tests/escape_formula_differential.rs`, `tests/damage_differential.rs` and
`tests/fox_up_special_differential.rs`, plus four native unit tests pinning
hand-verified fused bit patterns (`fighter::combat`,
`fighter::grab::tests::escape_timer_matches_the_hardware_fused_rounding_not_naive_two_rounding`,
`fighter::damage::tests::launch_angle_matches_the_hardware_fused_rounding_not_naive_two_rounding`).
`cargo fmt --all --check` is clean; `cargo clippy --locked --workspace
--all-targets --all-features -- -D warnings`, `cargo test --locked
--workspace`, `cargo test --locked --workspace --features c-oracle` (debug
and release) and `git diff --check` all pass clean (1131 passed, 13
ignored, 0 failed, in this crate's own `--lib --tests --features c-oracle`
subset alone; the full workspace run adds every other crate's own suite on
top, unaffected by this batch's changes). Rebased onto `origin/main` twice
during this batch (once past `34f4fff`'s falco-fox-fd correction this batch
was itself waiting on, once more past `5394478`'s trigonometry batch,
which touches the same `game::characters::fox::up::angle_xy` function this
batch also changed — merged cleanly, both changes compose: the trig
batch's `crate::math::acosf` call and this batch's fused dot-product/
`sqrt_accurate` length calls are independent parts of the same function
body).

The 2026-09-12 double-precision audit batch (`skirmish-f64`, `docs/math.md`
has the full trail) picks up where `skirmish-fma` left off on the same
three recorded one-ULP divergences (`fox-fd-3.slp` frame -32,
`falco-fox-fd.slp` frame -25, `fox-fd.slp` frame 5): a fused product+sum
was already ruled out; the remaining candidate was a double-precision
*intermediate* — PowerPC FPRs are always the IEEE double format, and the
single-precision mnemonics (`fadds`/`fmuls`/etc.) are architecturally
"compute at double, round once to single," so a plain (non-fused) `fmul`
feeding a `fadds` would still round only once, differently from two
separately-rounded `f32` operations, and the FMA audit's own tool never
actually checked for that (it only reports fused mnemonics; a
zero-fused-op function prints nothing else). `tools/ppc_precision_audit.py`
extends the same disassembly approach to classify every floating-point
instruction into double-precision arithmetic, single-precision arithmetic,
`frsp` (the explicit single-rounding point), and `lfd`/`lfs` (double/single
loads), so a genuine double intermediate is now directly visible rather
than inferred from the decompiled C's declared types.

Every function in all three chains (Dash/ground-movement/friction,
air-dodge launch, landing friction, plus their immediate callees --
`ftCo_Dash_Phys`, `ftCommon_8007C98C`, `ftCommon_ApplyGroundMovement`/
`NoSlide`, `ftCommon_ApplyFrictionGround`, `ftCo_Dash_Enter`,
`ftCommon_800804A0`, `ft_GetGroundFrictionMultiplier`, `ftCo_80099A9C`,
`ftCommon_8007D9D4`, `ftCo_EscapeAir_Phys`/`IASA`, `ft_80084F3C`,
`ftCo_Landing_Enter`/`LandingFallSpecial_Enter`/`Landing_Phys`,
`ftCommon_Fall`/`8007CF58`/`ApplyFrictionAir`, `ftCo_800CB110`) disassembles
to zero double-precision arithmetic instructions; every `lfd` found is a
callee-saved FPR stack spill/restore around a call or at an epilogue, not a
constant load. For `fox-fd-3.slp` frame -32, this batch went further than
disassembly alone: it ran Fox's own exact recorded pack constants
(`dash_initial_velocity`/`dash_accel_mul`/`dash_accel_base`/
`dash_max_velocity`, `f32` bits `0x3ff33333`/`0x3dcccccd`/`0x3ca3d70a`/
`0x400ccccd`) through every possible rounding model for this expression --
single-precision step by step, fully double-precision with one final
round, and every partial mix -- and all of them produce the identical
`0x400147ae`, never the recording's own `0x400147ad`. No instruction
selection reaches the recorded value from these inputs at this expression;
the gap must be upstream (a perturbation sweep shows the incoming
`ground_velocity` would need to be about two ULP lower than modeled, which
points at the Dash-entry velocity computation instead, itself also
confirmed fused- and double-precision-free). `falco-fox-fd.slp` frame -25
runs through the identical, fighter-generic Dash chain (Falco's own lower
`dash_max_velocity` puts it through the clamp branch instead, not
independently re-swept). `fox-fd.slp` frame 5 is likewise confirmed
double-precision-free, consistent with (not a new explanation for) the
`skirmish-msl-trig` batch's own finding that a fully disassembly-verified
trigonometry port still ties, not fixes, that frame.

A broader sweep of the same ~500-function C-oracle-pinned list the FMA
batch drew from (434 resolved) finds 22 functions with genuine
double-precision arithmetic (208 instructions total) -- collision/ECB
geometry, ASDI/SDI redirection, and the Blaster laser's reflection code,
none reachable from any of the three named chains, so a real but unrelated
backlog rather than an explanation here (`docs/math.md`'s own new section
has the full list).

**No Rust source change and no baseline change**: the port already
computes all three chains exactly as the retail binary does, confirmed at
the instruction level this time, not only from the decompiled C's declared
types. `src/fighter/movement.rs`'s new
`dash_accel_reproduces_ieee754_not_the_recordings_one_ulp_lower_value` unit
test pins both the single- and double-precision-throughout evaluation of
the exact recorded inputs, so this can't silently regress into looking
"fixed" by accident. `docs/parity.md`'s three entries are updated with this
audit's result. Measured against the pack this batch's own recordings are
tracked against: `fox-fd-3.slp` unchanged at -32/91, `falco-fox-fd.slp`
unchanged at -25/98, `fox-fd.slp` unchanged at 5/128 -- ruling out a second
candidate explanation isn't fixing the divergence itself.

**Tests**: `src/fighter/movement.rs`'s new unit test (above); no new
C-oracle differential (this batch changes no arithmetic, only confirms the
existing port's own evaluation order already matches the retail binary's).
`cargo fmt --all -- --check`, `cargo clippy --locked --workspace
--all-targets --all-features -- -D warnings`, `cargo test --locked
--workspace` and `cargo test --locked --workspace --features c-oracle`
(debug) all pass clean; `git diff --check` clean.

The 2026-09-12 Blaster/laser C-oracle batch closes the primary gap the
2026-09-12 Fox neutral special batch flagged below ("No C-oracle
differential harness was added this batch for `ftfoxspecialn.c`/the laser
item's motion functions") and the shield-bounce/Reflector "not covered by
an automated test" flag in `docs/fox-neutral-special.md`'s own former
"Tests" section.

Pins `src/melee/ft/kinds/ftFox/ftfoxspecialn.c` (the Start/Loop/End state
machine's own Enter/Anim/IASA callbacks, `PrepareBlasterShot`/
`FireBlasterShot`), `src/melee/it/kinds/itfoxlaser.c` (the laser's own
spawn/motion/reflect callbacks), and `src/melee/it/item.c` (`Item_80269F14`,
the Reflector hand-off's owner-swap/damage-scaling excerpt), linking the
real `lbVector_Mirror`/`ftLib_80086990`/`it_8026BB68` directly rather than
stubbing them where the comparison depends on their exact arithmetic.
`ftColl_80077464`'s own `max_damage` eligibility gate is a verbatim
excerpt against the already-pinned `combat_knockback.c`, matching
`hit_direction.c`'s own established pattern. See `docs/fox-neutral-
special.md`'s own new "Oracle" section for the full citation list.

**The differential exposed a real, previously undetected bug**:
`Item_80269F14`'s own damage-scaling formula (`hit.damage * xC6C + 0.99f`,
truncated toward zero) was missing its `+ 0.99` term in `src/game/
projectile.rs`'s reflect handoff -- a plain product instead of the
source's own truncation idiom, silently rounding a reflected hit's damage
down more often than the real game does. Fixed with a citation; a native
regression (`damage_mul = 1.5` gives `5`, not the buggy `4`) pins the fix
independently of the C-oracle comparison. The global damage cap the
source also applies (`it_804D6D28->xD8`) has no reader anywhere in the
pinned decomp for its real runtime value, so it stays unmodeled, a
documented gap rather than a guess.

Also replaces the laser's own terrain despawn (previously the stage's
outer bounding box only) with a real swept ray-vs-stage-line cast, reusing
`collision::stage::Stage::sweep`'s existing pinned line-intersection
primitives across all four surface kinds; a native regression (a wall
between two fighters, well inside the blast zone) proves the old
bounding-box-only check would have let the laser fly straight through.
`mpCheckMultiple`'s own full stage-geometry-array scan is not itself
pinned as a new C-oracle differential, matching `ledge_snap.c`'s existing
precedent for the analogous function.

Checked the exporter's now-delivered `specials.neutral` pack
(`/mnt/archive/datasets/melee/skirmish-gameplay/v2/fox-fd/match-data.json`)
against the fire-timing/`cmd_vars[0]`-window approximations the original
Blaster batch flagged below: every previously-cited attribute and laser
hitbox value matches byte for byte (confirming, not merely repeating, the
earlier numbers), but the pack supplies only bone poses and hitboxes per
frame, no animation command-stream/script-opcode data at all, so it
cannot resolve that specific gap; still open, now with that checked and
ruled out as the source of resolution rather than left an open question.

**Tests**: `tests/fox_neutral_special_differential.rs` (9 tests: Enter,
the turnaround-latch predicate, Start->Loop, Loop's repeat/end decision
and fire capture, End's Wait/Fall/FallSpecial dispatch, a verbatim-source
check), `tests/fox_laser_differential.rs` (7 tests: spawn position/angle,
per-frame motion, the shield-bounce mirror and its derived angle, the
Reflector callback's facing/angle, the damage-scaling formula, a
verbatim-source check), `tests/reflect_gate_differential.rs` (3 tests:
the eligibility gate, a verbatim-source check), each with 256-512
proptest cases. `tests/game_fox_neutral_special_reflect.rs` (3 new native
tests: shield bounce, Reflector hand-off, eligibility gate) and a new
terrain-line despawn test added to `tests/game_fox_neutral_special.rs`.
`cargo fmt --all`, `cargo clippy --workspace --tests --features
c-oracle -- -D warnings` (and without the feature), and `cargo test
--workspace --features c-oracle` (debug) are all clean; the project's own
six-step local audit (`fmt`, `clippy`, `native`, `c-oracle` debug,
`c-oracle` release, `git diff --check`) ran clean, all six steps exit 0.

The 2026-09-12 Fox neutral special (Blaster) batch (`docs/fox-neutral-
special.md`) replaces the shared single-phase neutral-B shell
(`game::specials::neutral`, retired outright -- no character variant could
reach it once Fox has his own dedicated Start/Loop/End state machine) with
`game::characters::fox::neutral` and adds the minimal generic
fired-projectile system it needs (`game::projectile`, `State.projectiles`).
Six new `Action` variants (`SpecialNStart/Loop/End`, `SpecialAirNStart/
Loop/End`) replace the shell's single `SpecialN`/`SpecialAirN` pair
throughout the codebase (`fighter::action_instance::motion_identity`,
`crates/skirmish-replay::observation`, and the unrelated feature tests that
had borrowed the pair as a "some neutral-special action" stand-in).
`game::specials::grounded_chain_open` also gains `Landing`'s own
interrupt window (confirmed against a real recording where a B press
during it enters `SpecialN` directly from `Landing`) -- a shared-framework
fix, not specific to this move.

Numeric values are a mix of confirmed and invented data, cited individually
in the design note: `angle`/`speed`/`landing_lag` and `FoxLaserAttr`'s
`lifetime` come from the exporter's own disc read (`fighters/fox.json`);
laser speed (`7.0`), lifetime (`35` frames) and damage (`3`) are
independently cross-checked against a real recording (`/mnt/archive/
datasets/melee/slippi-public-dataset-v3.7/data/FOX/batch_00/18_24_36
[H2O] Fox + Fox (FD).slp`, dumped via `py-slippi`); the laser's four
hitbox offsets/sizes come from the exporter's own item-script decode
(`0.003906`-scaled raw integers, not the mathematically nicer `1/256`);
knockback growth/base/weight-independent are confirmed `0` (a laser
flinches without pushing); the fire-timing (which Loop frame fires) and
the `cmd_vars[0]` repeat-arming window are approximated, not decomp-visible
at all (animation-script data); the mid-flight ground-contact landing
velocity threshold (`ftCo_800D0EC8`) is not modeled (a conservative
always-ordinary-Landing simplification, since guessing its value was
refused). The Reflector hand-off (`owner` swap, `angle += pi`, no
`speed_mul`, `damage_mul` gated on `max_damage`) and the shield bounce
(a true `lbVector_Mirror`-style velocity mirror across the contact normal,
not a fixed reversal) were both corrected during the batch after an initial
misreading, verified against the exact decomp call chain and a real
recording's own observed reflected-shot velocity.

**Tests**: `tests/game_fox_neutral_special.rs` (10 new tests: strict
threshold entry, aerial entry, repeat/no-repeat Loop cycling, laser
travel/hit/despawn, staling, ground-leaves-to-Fall, Slippi ids, checkpoint
round trip). `crates/cli/tests/replay_match.rs`'s neutral-special
regression is rewritten against the real Blaster fixture (previously
borrowed the retired shell plus Fox's own jab hitboxes as a stand-in) to
assert a spawned laser travels before hitting, matching a real recording's
own observed multi-frame flight. `cargo fmt --all` and `cargo clippy
--workspace --lib --tests --bins --exclude renderer -- -D warnings` are
clean; `cargo test --workspace --exclude renderer --tests --lib` passes
(all prior tests plus the new ones, 0 failed). The full six-step local
audit (`fmt`, `clippy --all-targets --all-features -- -D warnings`,
`native`, `c-oracle` debug, `c-oracle` release, `git diff --check`) also
ran clean, all six steps exit 0 (924 native/1268 c-oracle passed, 19
ignored, 0 failed) -- recorded against this batch's own commit, one commit
before the concurrent Falco registration batch below merged in; re-running
it against the fully composed tree was not repeated, since the two
batches touch disjoint behavior (this one Fox's neutral special and the
generic projectile system, the other Falco's character registration) and
the rebase merge itself introduced no new conflicts beyond mechanical
line-adjacency (resolved by hand, `git diff --check` clean). No C-oracle
differential harness was added this batch for `ftfoxspecialn.c`/the laser
item's motion functions (unlike every other special covered so far) --
explicitly flagged as the primary remaining gap, not silently skipped: the
harness's own hand-written struct adapters need to be built correctly
against the `Fighter`/`Item` layouts and this batch's remaining time did
not allow doing that with confidence. The native Rust behavior is instead
verified by the tests above, the real-recording cross-checks in the design
note, and code review against the cited decomp functions.

The 2026-09-12 hit-record-refresh batch closes the gap the 2026-09-11
special-move script-hitbox batch left open: a script that clears a hitbox
and re-creates it later in the same action (Fire Fox Hold's own charge
pulse, frames 20/22/24/26/28/30/32) previously connected only on its first
active frame, since the per-attacker `hit_groups` bitmask
(`src/game/simulation.rs`) cleared only on a fresh `simulation::enter`, not
on a hitbox slot's own re-enable. `src/game/hitboxes.rs` gains
`refreshed_groups(tracks, frame)`: the generic counterpart of
`ftAction_8007121C`'s own re-enable gate (`ftaction.c:284-359`, the
CreateHitbox opcode) -- a hitbox is a fresh spawn when
`hitbox->state == HitCapsule_Disabled || hitbox->x4 != hit_group` (`x4`
being the capsule's own hit-group id, `Hitbox::group` here). On a fresh
spawn the source calls `ftColl_800768A0` (`ftcoll.c:301-314`), which copies
victim history from another still-enabled capsule sharing the same group
(`lbColl_CopyHitCapsule`, `lbcollision.c:1809-1822`) if one exists, or
clears it (`lbColl_80008440`, `lbcollision.c:1796-1807`, zeroing
`victims_1`/`victims_2` and their counts) otherwise; the explicit
ClearHitbox/ClearAllHitboxes opcodes (`ftAction_80071784`/
`ftAction_800717D8`, `ftaction.c:426-444`) only flip `state` to `Disabled`
and never touch the victim lists themselves, so the clear happens lazily on
the next creation. This codebase has one exported `AttackFrame` per
simulation frame rather than a per-opcode timeline, so `refreshed_groups`
reads "disabled" as "absent from `frame.hitboxes`" (matching
`update_tracks`'s own existing contract) and "another already-enabled
capsule sharing the group" as "any of the four `tracks` slots already
carried that group on the previous frame" -- checked across every slot, not
just the one being read, to match `ftColl_800768A0`'s own all-capsules
search. `simulation::advance` now clears the matching `hit_groups` bits
(`fighter.hit_groups &= !hitboxes::refreshed_groups(...)`) immediately
before calling `update_tracks` (which reads the same, not-yet-overwritten
`tracks`), for every attack generically -- no per-move opt-in. A hitbox
present on every frame from its own action's entry (jab, Travel's own
continuous hit, Reflector's Start hit) never sees a gap, so nothing past
its own first frame changes for it, matching the source's own same-group
reissue no-op. `jab.rs`'s own `FrameFlags::clear_hits` and the rapid-jab
loop's frame-zero `hit_groups = 0` are unchanged and still needed: a script
that clears and immediately re-creates a hitbox within the very same
exported frame is invisible to a presence/absence check at this
granularity, since the capsule never actually leaves `frame.hitboxes` at
that resolution. Native coverage: `tests/game_swept_hitboxes.rs` gains a
focused unit test pinning `refreshed_groups` itself (gap-then-return
refreshes, continuing/same-frame presence does not, a group shared by
another slot copies rather than clears); `tests/game_fox_up_special.rs::
hold_charge_hits_a_nearby_opponent_at_the_pack_documented_pulse_frames` now
asserts all seven pulses connect independently (14 damage total, not 2) --
this test previously asserted the *absence* of exactly this behavior, since
it did not exist yet. `tests/game_jab.rs`'s own
`rapid_jab_tapping_keeps_the_loop_going_and_rehits_each_cycle` (the rapid
loop) and `clear_hits_lets_third_jabs_group_hit_the_victim_twice`'s own
`without_clear` branch (an ordinary continuous hitbox hitting once) already
covered the other two required scenarios and needed no change. Separately,
`src/game/clank.rs`'s own `State`/`Slot`/`sample`/`blocked` already
implement the identical re-enable/copy/clear semantics independently,
scoped to matches with `rules.clank` configured (`clank::blocked` replaces
the `hit_groups` bitmask check entirely, match-wide, whenever
`data.rules.clank.is_some()`) -- this batch's fix is the non-clank engine's
own analogous correctness, not a duplicate of that path, and the two are
mutually exclusive per match by the existing dispatch in
`simulation::advance`.

This batch also closes the two gaps `specials::helpers::validate_hitboxes`
(the 2026-09-11 batch's own validation gap-closer for Fox's up/down
specials) deliberately left open, now that pack v6 supplies real
`Rules.clank`/`rebound` data and populates `move_id` on every up/down
special phase: the function takes the match-wide `Rules` and applies the
same two checks `validation.rs`'s own generic jab/aerial/tilt/smash/
neutral-special chain already runs on every other attack's hitboxes --
`rules.clank.is_some() || !(hit.clank || hit.rebound)` (Travel's own hit and
Reflector's Start hit both carry the pack's real values now, `clank`/
`rebound` both `true` for Travel, `clank` `true`/`rebound` `false` for
Start, instead of the forced-off placeholders the previous batch used) --
and a `move_id`-under-staling requirement, narrower than the generic chain
in one respect: required only for a phase that actually has a hitbox on
some frame, not unconditionally for every phase (a future pack need not
populate `move_id` on a hitbox-free phase like `bound.pose`, even though
pack v6 already does). `characters::fox::up::validate`/`down::validate`
each gained a `rules: &MatchRules` parameter (renaming their existing
`side::Rules` parameter to `specials_rules` to disambiguate) threaded from
`validation.rs`'s own per-fighter pass, which already holds the match-wide
`Rules` in scope under the same name. `tests/support/fox_up_special.rs`/
`fox_down_special.rs` each gain a `with_ordinary_clank` helper (the same
profile `tests/game_clank.rs` uses, plus the rebound animation
`rules.clank` requires for every fighter) that only the Travel/Reflector-
Start hit tests and the new validation cases opt into -- the Hold pulse
test deliberately keeps `rules.clank` unset, since enabling it match-wide
would divert its own hit-connect check onto `clank::blocked`'s independent,
already-correct bookkeeping instead of the `hit_groups`/`refreshed_groups`
path this batch exists to fix.

The new `move_id`-under-staling requirement broke an existing regression
(`crates/cli/tests/ecb_load_flags_v4.rs`): the archived gameplay export
pack v4 predates `move_id` on Fox's up/down special hitboxes entirely (an
already-known, already-documented gap -- this same file's 2026-09-11 entry
below notes "pack v4's missing specials `move_id` is fixed in v5"), so it
could no longer construct a `Match` at all. Fixed by having that test skip
(without failing) when construction fails for exactly that reason,
matching its own existing skip-on-missing-export convention, rather than
by touching the archived pack itself or weakening the new requirement.

The six-step local audit (the same six steps as the 2026-09-11 entry
below) ran clean against this batch's own rebased worktree: `fmt` (0
passed), `clippy --all-targets --all-features` (0 passed), `native` (938
passed/19 ignored), `c-oracle` (1288 passed/19 ignored), `release` (1288
passed/19 ignored), `diff` (0 passed). The archived run is stored outside
Git at `/mnt/archive/runs/skirmish-hit-refresh-20260912-verified`.

The 2026-09-12 Falco registration batch adds `game::characters::
Specials::Falco` on top of Fox's existing side/up/down special code
(`fox::side`/`fox::up`/`fox::down`, unchanged): every one of Falco's own
motion-state entries points at the identical Fox callbacks
(`ftFc_Init_MotionStateTable`, `ftfalco.c:23-370`), and `ftFc_Init_
LoadSpecialAttrs`/`ftFx_Init_OnLoadForFalco` (`ftfox.c:481-484,503-506`)
load Falco's own `PlFc.dat` attributes through the same `ftFox_DatAttrs`
shape Fox's own load uses, so `Specials::Falco` reuses `fox::side::
SideSpecial`/`fox::up::UpSpecial`/`fox::down::DownSpecial` verbatim rather
than duplicating them. `game::characters::fox::CHARACTER_IDS` gains
Falco's external Slippi id (20, alongside Fox's 2), correcting a stale
comment that had conflated it with Falco's unrelated *internal* fighter
kind (`FTKIND_FALCO`, 22); an identical mix-up in an `observation.rs` test
comment is also fixed. `Specials`' `#[serde(tag = "character")]` gains a
`#[serde(alias = "fox")]`/`#[serde(alias = "falco")]` on both variants, so
both the standalone `fighters/fox.json` sample's capitalized `"Fox"` and
every pack that also carries a Falco fighter's lowercase `"fox"`/`"falco"`
deserialize. Falco's neutral special (Laser) is not wired: no exported
pack supplies `specials.neutral` data for either character yet. See
`docs/falco.md` for the full citation trail and `docs/parity.md`'s "A
first Falco recording" section for the real-replay measurement.

**Tests**: `tests/game_falco_specials.rs` (a Falco fighter loads, plays
each of the three shared specials, and resolves the same Slippi ids as Fox
for external id 20); `crates/skirmish-replay/src/observation.rs`'s
existing specials test gains matching Falco (20) assertions.
`crates/cli/tests/real_parity_falco_fox_fd.rs` ratchets a real Falco-vs-Fox
Final Destination recording (`tests/fixtures/slippi/parity/
falco-fox-fd.slp`, ports P3/P4) against `falco-fox-fd-baseline.json`,
skipping without `SKIRMISH_GAMEPLAY_DATA`. `cargo fmt --all -- --check`,
`cargo clippy --locked --workspace --all-targets --all-features -- -D
warnings` and `cargo test --locked --workspace` (926 passed/0 failed/19
ignored) all pass. First parity measurement for any Falco recording (the
`falco-fox-fd` pairing, gameplay export v2): 93 frames match (-123 through
-31, the pre-game Entry warp-in); the first divergent frame is -30, P4
(Fox), field `action_age` (expected `1.0`, actual `0.0`) -- reported, not
diagnosed or fixed, in this batch; see `falco-fox-fd-baseline.json`'s own
note for what was ruled out.

The local audit is recorded at:

`/mnt/archive/runs/skirmish-falco-20260912-verified`

It validates the composed tree (base revision `08d51950fa48b9b85ce7d88
145d08a22f0979105`) with formatting, strict all-target/all-feature
Clippy, native workspace tests, original-C differential tests in debug
and release modes, and a clean `git diff --check`. All six steps exit 0:
`fmt` (0 passed), `clippy` (0 passed), `native` (926 passed/19 ignored),
`c-oracle` (1270 passed/19 ignored), `release` (1270 passed/19 ignored),
`diff` (0 passed). No Falco-specific gameplay behavior was added; this
batch is registration and data-shape wiring on top of already-tested Fox
move code, described above.

The 2026-09-11 special-move script-hitbox batch covers the hitboxes Fox's
up/down special scripts embed that earlier specials batches modeled with
empty hitbox lists: Fire Fox's Hold-phase charge pulse (frames 20/22/24/
26/28/30/32 of the 44-pose Hold set, bone 0, 2 damage, angle 70) and
Travel's own continuous hit (every one of the pack's 31 sampled frames,
bone 58, 14 damage, angle 80, clank/rebound both reported `true` in the
source data), and Reflector's Start-phase hit (frames 0/1 of the 5-pose
Start set, bone 3, 5 damage, angle 0). All three were confirmed by
`ftfoxspecialhi.c`/`ftfoxspeciallw.c` themselves creating no hitbox directly
(the schedule is script/DAT-embedded, an `ftaction.c` hitbox-opcode table
this codebase already processes generically through `game::hitboxes::
update_tracks`/`AttackFrame.hitboxes`), sourced from the gameplay export
pack (`/mnt/archive/datasets/melee/skirmish-gameplay/v6-snapshot-20260911/
fox-fd/match-data.json`). No pipeline code changed to make the hitboxes
connect -- `FighterData::attack` -> `specials::attack` -> each move's own
`attack()` already routed `Action::SpecialHi{,Hold}`/`SpecialLwStart` into
the ordinary hit-scan/damage pipeline every other move kind uses; only
supplying real `Hitbox` data (in dedicated native-test resources, not the
shared synthetic fixtures those two files' other tests depend on) was
needed. `characters::fox::{up,down}::validate` gained a shared
`specials::helpers::validate_hitboxes` call (bone/group bounds, finite/
nonnegative geometry, damage/growth/fixed/base ranges, an integral 0..=362
angle, and the transformed-shape sanity check every other move kind's
hitboxes already got from `validation.rs`'s own generic chain, which does
not itself walk Fox's specials) -- deliberately narrower than that generic
chain in two respects: no `move_id`-under-staling requirement (the real
export pack does not populate it for every phase of these two moves yet,
a separate, pre-existing gap this batch does not fix) and no
`rules.clank`-gate for the `clank`/`rebound` bits (this helper has no
match-wide `Rules` access). Attacker-side hitlag needed no change either:
neither pinned C file ever touches a hitlag field, so the ordinary,
already-generic per-attacker `hitlag` freeze in `damage::apply_hit`/
`simulation::advance` applies unmodified. Native regressions
(`tests/game_fox_up_special.rs`, `tests/game_fox_down_special.rs`) place a
stationary second fighter in reach and drive a real `Match` through
Controller input, confirming Hold's pulse connects exactly at frame 20 (and
does not independently reconnect at 22 through 32, since this engine's
shared per-attacker `hit_groups` bitmask is cleared only by a fresh
`simulation::enter`, not a hitbox slot's own disable/re-enable cycle within
one action -- the same rule every continuous/repeating hitbox in this
codebase already lives under, absent a `jab`-style `clear_hits` script flag
`Attack`/`AttackFrame` does not have), Travel's hit connects on its own
entry frame, and Reflector's Start hit connects on its own entry frame for
5 damage. A third self-recorded replay regression
(`crates/cli/tests/replay_match.rs::
firefox_hold_charge_hitbox_self_recorded_replay_matches`) exercises the
Hold pulse through the file-backed Peppi round-trip harness, alongside the
existing grounded/aerial Fire Fox pair -- self-consistency evidence, not
Melee parity. No C-oracle differential was added: hit resolution arithmetic
(`combat::knockback`/`hitlag`/`initial_hitstun`) is already oracle-pinned
and generic over `Hitbox` fields (`tests/combat_differential.rs`), and this
batch supplies new data through that existing pipeline rather than new
arithmetic. `docs/fox-up-special.md`/`docs/fox-down-special.md` each gained
a "Hitboxes" section (removing the up special's own now-outdated "Travel's
hitboxes stay empty" note) citing the exact pack values, the bone-index
adaptations the shared two-bone synthetic skeleton required (Travel's bone
58 -> 1, Reflector's bone 3 -> 1; Hold's own bone 0 needed none), and the
`hit_groups` scope boundary above.

The six-step local audit (`fmt`, `clippy --all-targets -- -D warnings`,
`cargo test --workspace`, `cargo test --workspace --features c-oracle`,
`cargo test --workspace --release --features c-oracle`, `git diff --check`)
ran clean against this batch's own worktree; the archived run is stored
outside Git at `/mnt/archive/runs/skirmish-specials-hitboxes-20260912-verified`,
matching the archived-audit convention `skirmish-rust-port-20260909-v2` and
its successors established -- see `validation.json` there for exact
pass/ignored counts and timings.

The 2026-09-11 landing-velocity fix (the second real-replay parity loop,
`tests/fixtures/slippi/parity/fox-fd-3.slp`, `docs/parity.md`) removes an
extra reset `game::collision::land` made on every landing that the pinned
decomp never makes. Every ordinary and tech landing call site
(`ft_081B.c`'s `ft_80082B1C`/`ft_80082C74`/`ft_80082D40`/`ft_80082F28`, and
`ftCo_PassiveStand.c`'s `ftCo_800989D4` for a floor tech) reaches
`ftCo_Landing_Enter_Basic`/`ftCo_Landing_Enter`, which calls `ftCommon_
8007D7FC` -> `ftCommon_8007D6A4`: that function sets `gr_vel = self_vel.x`
and flips `ground_or_air` to `GA_Ground`, but never assigns `self_vel.y` --
the vertical self-velocity this same frame's own fall physics already
computed survives the landing frame untouched. Confirmed directly against
`fox-fd-3.slp`: P2's recorded `velocities.self_y` at its own landing frame
(-44) is -2.53, one more gravity step past the previous frame's -2.3 (a
constant -0.23/frame trend through the fall), not the zero a hard stop
would report. `collision::land`'s `f.velocity[1] = 0.0;` (an unattributed
line from the crate's earliest history, `1a0077f4`, predating decomp-
citation discipline) had no such license. Removed it; the very next
grounded frame overwrites both self-velocity axes regardless
(`Movement::project_ground`, `ftCommon_ApplyGroundMovementNoSlide`, fully
replaces `self_velocity` from `ground_velocity`/`floor_normal` every
grounded frame that reaches it), so this only changes what a fighter's
landing frame itself reports, not any later physics.

**Tests**: a new `tests/game_landing.rs` regression,
`ordinary_landing_keeps_this_frames_fall_velocity_unzeroed`, pins a short
hop's landing frame velocity against the immediately preceding airborne
frame's own velocity minus gravity, confirming it is neither zero nor an
independently-reset value. Three existing tests asserted the old
(incorrect) zero and needed their pinned values corrected to the frame's
real fall velocity instead: `tests/game_air_dodge.rs`'s
`a_downward_dodge_lands_directly_and_slides_with_ground_friction`
(`LandingFallSpecial`, one decay step past `entry`'s own velocity, matching
its existing `ground_velocity` assertion's identical pattern on the other
axis); `tests/game_damage_floor.rs`'s
`buffered_neutral_tech_stops_launch_and_recovers_for_input` and
`directional_floor_tech_rolls_use_sampled_root_motion_and_bone_ecbs`
(`Passive`/`PassiveStandF`/`PassiveStandB`, one gravity step, `-0.2`, from
a purely vertical downward hit); `tests/game_damage_surface.rs`'s
`every_reflected_surface_action_lands_cleans_response_state_and_replays`
and `every_surface_tech_action_lands_cleans_shared_state_and_replays`
(`DownBound`/`PassiveWall`/`PassiveWallJump`/`PassiveCeiling`, `-0.6` or
`-2.0` depending on how many frames each reflected/tech path falls before
landing). No C-oracle differential was added: this removes a line with no
decomp counterpart rather than porting new pinned arithmetic, the same
rationale `docs/input-lock.md` used for its own non-decomp-cited gate.
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --
-D warnings` and `cargo test --workspace` (911 passed/0 failed/19 ignored,
up from 910/0/19 immediately before this fix, one net test added) all pass.

Measured against gameplay export pack v6 (`v6-snapshot-20260911`) on
`fox-fd-3.slp`: `checked_frames` went from 79 to 91 (frames -123 through
-33); the next divergence is -32, field `velocities.self_x_air` on P2
(expected `0x400147ad`, actual `0x400147ae`, a one-ULP rounding
difference) -- a separate, unrelated arithmetic-ordering matter, reported
rather than chased in this batch. `tests/fixtures/slippi/parity/
fox-fd-3-baseline.json` is updated accordingly; `docs/parity.md` records
the same measurement. This fix is against a separate recording than the
main loop's own `fox-fd.slp`; the main loop's `fox-fd.slp` first-divergent-
frame area has continued to move independently through its own concurrent
fixes (see its own entries below and `docs/parity.md`'s current
measurement), confirmed unaffected by re-running the full `real_parity`
ratchet, which passes for every recording.

The 2026-09-12 msl-trig batch (`docs/math.md`) replaces `libm`-based
`sinf`/`cosf`/`tanf`/`atan2f`/`atanf`/`acosf`/`asinf` at every fighter/
common-code call site with ports of the game's own pinned decompilation
(`src/MSL/trigf.c`/`math_data.c`, `src/melee/lb/lbtrigf.c`), in a new
`src/math.rs`. Every function's control flow and constants come from that
decompiled C, but which individual operations are *fused* (a single
correctly-rounded PowerPC `fmadds`/`fmsubs`/`fnmadds`/`fnmsubs`, not two
separately-rounded operations) does not come from the C at all -- a
decompiler reconstructs value-equivalent C, not instruction-equivalent C, so
the exact fusion is invisible there. This batch settles it by disassembling
the retail `main.dol` directly (Capstone, PowerPC big-endian, each pinned
function's address resolved through the DOL's own section table and
`config/GALE01/symbols.txt`), cross-checked against a sibling batch's own,
independently-built tool for exactly this (`skirmish-fma`'s `tools/
ppc_fma_audit.py`); the two agree on every fused instruction in every
function ported here. An earlier revision of this batch guessed the fusion
from real-replay measurement alone instead (fuse a chain, remeasure, keep
what ties or improves the baseline) and, despite reproducing the right
final bits for `fox-fd.slp`'s specific inputs, got the fusion shape itself
wrong in more than one place -- recorded in `docs/math.md` as a cautionary
finding about measurement's limits: it cannot distinguish "the right
fusion" from "a fusion that happens to agree with one recording."

The disassembly also settles `acosf`/`asinf`'s biggest open question. Both
depend on `__frsqrte`; this decompilation project's own `placeholder.h`
defines the *host-tooling* stand-in for that macro as plain `sqrt`, not a
reciprocal-square-root estimate, so seeding the following Newton-Raphson
refinement with it (as the pinned oracle does) does not converge except
very close to `x == 1` -- confirmed directly (`asinf(0.9999)` used to
return roughly `2.7°` instead of the true `~89.2°`). Disassembly shows the
retail binary instead calls `frsqrte`, the real PowerPC estimate
instruction; `placeholder.h`'s stand-in is exactly what its name says, a
decompiler convenience, never real hardware behavior here. `acosf`/`asinf`
now seed the same, disassembly-confirmed Newton refinement from an accurate
`1/sqrt(x)` instead (the exact hardware estimate table itself remains out
of scope, like `sqrtf` generally), converging to the same result real
hardware's estimate-then-three-Newton-steps would -- confirmed by sweeping
`-0.999..=0.999` against `std`'s `acos`/`asin` (worst disagreement around
`5e-7`) -- and are now wired to their real call sites
(`fighter::damage::vector_angle`, `game::characters::fox::up::angle_xy`,
`quaternion::interpolate`).

Measured against `fox-fd.slp`: porting every fused operation exactly as the
retail binary's own instructions compute it ties the existing real-replay
baseline exactly (same frame, same bits) -- a plain, fully-unfused port of
the same algorithm regresses it by one frame instead (a different mismatch,
rejected: this project's ratchet does not accept a lower
`first_divergent_frame`). Tying, not improving, means the pre-batch
suspicion that `libm`'s `cosf`/`sinf` caused the frame-5 divergence
(`tests/fixtures/slippi/parity/fox-fd-baseline.json`'s own note) is not
confirmed: the divergence is unchanged after replacing every fighter/common
trigonometry call site with a disassembly-verified port of the game's own
algorithm, so its true cause remains open.

A shared static-library naming hazard, found and fixed in the same batch:
this project's C oracle is one shared static library across every
`*_differential.rs` test, and Rust's own `std` links the platform C library
by the same symbol names its trig methods use (`f32::sin`/`cos`/`tan`/
`asin`/`acos`/`atan`/`atan2` call `sinf`/`cosf`/... via FFI). Defining
same-named strong `sinf`/`cosf`/... in the oracle silently replaced `std`'s
own calls process-wide once both were linked into the same test binary --
caught because a `math.rs` unit test comparing this port against `x.asin()`
"ground truth" started comparing the port against itself under
`--features c-oracle`. `tests/oracle/trigf_body.c`/`lbtrigf_body.c` rename
the pinned bodies to process-unique names before including the pinned
`.inc`, and `tests/oracle/trig.c`'s `oracle_*` wrappers call those renamed
symbols; a few existing adapters (`escape_air.c`, `aerial_input.c`,
`quaternion.c`) additionally rename the specific trig calls their own pinned
functions make, so they compare against the game's real algorithm too;
`ground_launch.c`/`lbvector.c`-based adapters do not, so `vector_angle`'s
new, real `acosf` is compared there against host `libm`'s instead -- two
different, both-reasonably-accurate implementations, handled with a small
tolerance and (at the rare exact quadrant-boundary input where the two
disagree on which branch `ground_launch` itself takes, amplified across
`knockback`/`floor_normal` magnitudes spanning dozens of orders of
magnitude) a narrow, explicitly-justified exclusion -- see "Tests" below.

**Tests**: `tests/math_differential.rs` (new) covers all seven ported
functions, NaN-safe; `atan2f`/`atanf` differ from the pinned, unfused oracle
by a small, fixed ULP bound over the full binary32 domain (no ambiguity
left once the fusion is read from disassembly rather than guessed); `sinf`/
`cosf` use an absolute bound and `tanf` a combined relative/absolute bound,
each over a bounded domain no real stick angle approaches, since a raw ULP
bound is the wrong tool near either function's own zero crossings/poles
(explained in place); a `full_domain_never_panics` liveness check covers the
true full-`u32`-domain the bounded checks trade away. `acosf`/`asinf` are
compared against `std`'s accurate `acos`/`asin` instead of the oracle (whose
placeholder-seeded iteration is not real hardware behavior). `src/math.rs`'s
own unit tests include a dedicated `acosf`/`asinf` near-domain-edge accuracy
check. Several existing tests tightened, corrected, or gained a narrow,
justified exclusion to reflect the game's real algorithm rather than host
`libm`: `tests/escape_air_differential.rs`'s launch-velocity comparison
(previously tolerant of `libm`-vs-oracle rounding, now a much tighter,
documented bound); `tests/aerial_differential.rs`'s stick-angle selection
(now passes without any tolerance) and its own hardcoded `atan2f(0.0, -0.0)
== PI` expectation, corrected to `FRAC_PI_2` (the game's own `atan2f` is not
the standard-library convention for `x == 0`, either sign -- its final
branch copies only `y`'s sign onto a bit-pattern `PI/2`, not `PI`, confirmed
against the pinned oracle); `tests/quaternion_differential.rs`'s
`matrix_to_euler` pole-case expectation, similarly corrected;
`tests/bones_differential.rs`'s and `tests/damage_differential.rs`'s
rotation/angle tolerances, widened with an absolute floor; and
`tests/ground_launch_differential.rs`'s angle comparison and exact-boundary
exclusion described above.

See `docs/math.md` for the full account: the disassembly methodology, what
was ported and from where, the `acosf`/`asinf` finding, the
fused-multiply-add findings (including what the earlier measurement-only
guess got wrong), and the real-replay measurement (unchanged at frame
5/128 -- `docs/parity.md`'s own entry has the provenance).

The 2026-09-11 ground-jump-direction fix (the real-replay parity loop,
`docs/parity.md`, `docs/state-parity.md`'s "Backward jumps") corrects
`game::locomotion::ground_jump`'s direction test and launch velocity, both of
which read `input.stick[0]` (the launch frame's own, already-updated
controller) where decomp's `ftCo_Jump_Enter`/`ftCo_800CB110` actually read
`fp->input.lstick[0].x` one frame stale. Both are dispatched from `ftCo_
KneeBend_Anim`, an Anim callback -- the same per-object ordering fact already
established for the generic per-frame animation advance
(`observation::observe`'s general `-1` rule): Anim runs before this same
frame's own controller read updates `fp->input`, so a fighter that changes
its main-stick direction on its very last JumpSquat frame launches using the
frame *before*, not its own. `ftCo_JumpAerial_CheckInput`'s identical-looking
direction test is dispatched from an IASA chain instead
(`ftCo_800CB870`/`ftCo_800CB8E0`, from `ftCo_Jump_IASA`/`ftCo_JumpAerial_
IASA`'s own `RETURN_IF` chains), so it already sees the current frame's
fresh input and needed no change -- confirmed by leaving `locomotion::
try_aerial_jump`'s two identical calls untouched. Both fixed reads now
consult `f.previous_input.stick[0]`; `ground_jump` no longer needs its own
`Controller` parameter, so the call site drops it.

Confirmed directly against `fox-fd.slp`: `fighter::locomotion::jump_backward`
fed the frame before each of the recording's 64 KneeBend->Jump transitions
(both ports, the full match) agrees with the recorded direction on every
one; fed the transition frame's own stick instead, three disagree (P4 at
frame 3 -- this loop's own next divergence -- and P1 at 775 and 2990, found
by scanning the whole file for every KneeBend->Jump transition, not just the
one this loop's own measurement reached).

**Tests**: `tests/game_jump_variants.rs`'s `full_hop`/`short_hop` helpers now
apply the direction-deciding stick to the JumpSquat frame *before* the launch
(previously the launch frame itself), leaving the launch frame's own stick
neutral; a new `ground_jump_direction_uses_the_previous_frames_stick_even_
when_the_launch_frames_own_stick_disagrees` pins the exact shape of the
recording's own disagreement (a stick reversed on the launch frame that the
fix must ignore) in both directions. `crates/cli/tests/replay_match.rs`'s
`file_backed_jump_variants_match_and_detect_their_first_changed_direction_
frame` moves its `stick_x` input to the JumpSquat frame before the launch,
and its "flip a sample, expect a mismatch" check now flips that same earlier
frame while still expecting the mismatch to surface one frame later, at the
launch row (whose own JumpSquat report is otherwise unaffected). `docs/
state-parity.md`'s "Backward jumps" section, its own `## Tests` note and
`ground_and_aerial_jump_direction_follows_the_launch_frames_stick_with_
backward_on_equality` (renamed to `..._the_direction_deciding_stick_...`) are
all updated to match. `cargo fmt --all -- --check`, `cargo clippy --locked
--workspace --all-targets --all-features -- -D warnings` and `cargo test
--locked --workspace` (912 passed/0 failed/19 ignored, one new test) all
pass. Measured against gameplay export v6: 128 frames matched (-123 through
4), up from 126; the new divergence is frame 5, a suspected cross-platform
floating-point limitation in a transcendental function rather than a
Skirmish bug (`docs/parity.md`'s current measurement). `tests/fixtures/
slippi/parity/fox-fd-baseline.json` is updated to match.

The 2026-09-11 `LandingFallSpecial`/aerial-landing `action_age` fix (the
real-replay parity loop, `docs/parity.md`) extracts `observation::observe`'s
inlined `state_age` computation into its own `action_age` function (unchanged
behavior, needed so a unit test can exercise it without a full `Match`) and
adds one more exception to it: `Action::LandingFallSpecial` and the five
`Action::LandingAirN`/`F`/`B`/`Hi`/`Lw` actions now report
`fighter.aerial.landing_elapsed`, the tracked float the game already advances
at `fighter.aerial.landing_rate` (`game::aerial.rs`, `game::escape_air.rs`),
instead of falling through to the generic `action_frame`-based rule the
Walk/Run/movement-pose/Entry exceptions already precede. `ftCo_
LandingFallSpecial_Enter`'s own anim-speed argument to `Fighter_
ChangeMotionState` is `(0.1F + fp->x2EC) / landing_lag` (`ftCo_Landing.c:
111`), not `1.0`: `fp->x2EC` is the character's own cached FallSpecial
animation-frame count (`fighter.c:836`) and `landing_lag` is `ftCommonData`'s
`x344` (`escape_air::Rules::landing_lag`), so decomp's `cur_anim_frame`
advances at that computed rate every frame, not one integer per game frame;
the ordinary aerial landings scale the same way through the L-cancel divisor
(`game::aerial::land`). Confirmed directly against `fox-fd.slp`: P1's
air-dodge landing enters `LandingFallSpecial` at frame -4 already reporting
`state_age = 0.0` on its own transition frame, then `3.01`, `6.02`, `9.03` on
-3, -2 and -1 -- a constant rate of `3.01`, not `1.0`.

**Tests**: `crates/skirmish-replay/src/observation.rs`'s new
`landing_fall_special_and_aerial_landings_report_the_tracked_animation_rate`
calls the extracted `action_age` directly for all six actions with an
artificially large `action_frame` (so a passing assertion cannot be a
coincidence of the old and new formulas agreeing) and the recording's own
`0.0`/`3.01`/`6.02`/`9.03` values for `fighter.aerial.landing_elapsed`.
`crates/cli/tests/replay_match.rs`'s own harness-local duplicate of this
formula (its own doc comment already flags it as needing to track
`observation::observe`) gained the same branch; three of its existing tests
(`file_backed_air_dodges_match_and_detect_their_first_changed_trigger_frame`,
`file_backed_cstick_aerial_and_l_cancel_match_and_changed_selection_
diverges`, `physical_b_drives_file_backed_fox_air_illusion_into_landing_
fall_special`) already exercised air-dodge and L-cancelled aerial landings
and would otherwise have started failing once `observation::observe` itself
changed. `cargo fmt --all -- --check`, `cargo clippy --locked --workspace
--all-targets --all-features -- -D warnings` and `cargo test --locked
--workspace` (911 passed/0 failed/19 ignored, one new test) all pass.

This batch lands alongside the independently-fixed gameplay-export pack:
`rules.friction_above_walk` moved from `1.0` (a no-op) to `2.0`
(`ftCommonData` +0x6C, `ft_80084F3C`) in the republished export
(`v6-snapshot-20260911`, `tests/fixtures/slippi/parity/gameplay-export.
lock.json` updated to `v6`). Measured together against the published v6
pack: 126 frames matched (-123 through 2), up from 116 against v5 -- the
friction fix alone reaches frame -3 (`action_age`, the divergence this
batch's own fix resolves); both together reach frame 3, `action_state`, on
P4's own Jump-direction selection (`docs/parity.md`'s current measurement).
`tests/fixtures/slippi/parity/fox-fd-baseline.json` is updated to match
(`first_divergent_frame: 3`, `checked_frames: 126`, `export_version: "v6"`).

The 2026-09-11 Dash/Turn entry-time consolidation (the real-replay parity
loop) moves the fix below -- and the observation-layer Dash/Turn exception
before it -- to their common source, rather than patching each downstream
comparison independently. Both are the same underlying fact: `ftCo_
Dash_Enter` (`ftCo_Dash.c:48-63`) and `ftCo_Turn_Enter`/`ftCo_Turn_
Enter_Smash` (`ftCo_Turn.c:49-62`, `:173-188`) call `ftAnim_8006EBA4(gobj)`
a second, explicit time immediately after `Fighter_ChangeMotionState`, so
decomp's `cur_anim_frame` is `1`, not `0`, from the entry frame on. Traced
frame by frame (E = the entry frame, "dispatch" = the value a same-frame
IASA/`update_actions` read sees, "end" = the value after that frame's own
processing): an ordinary action's `action_frame` already agrees with
decomp's `cur_anim_frame` at dispatch on every frame (`0` at E, `1` at
E+1, ...), and is exactly one *ahead* of it at end (matching
`observation::observe`'s general `-1` rule); Dash's `action_frame`, without
this batch, was `0` at E dispatch, `1` at E end, `1` at E+1 dispatch, `2`
at E+2 dispatch -- ends agreed with decomp exactly (why the observation
exception below reads `action_frame` unadjusted), but dispatch was one
*behind* decomp for the whole action (why `dash_run_frame` needed its own
`+1`). Setting `action_frame = 1` at Dash/Turn's entry (`game::locomotion::
start_dash`/`start_turn`), and leaving `simulation::advance`'s ordinary,
unconditional end-of-frame `action_frame += 1` untouched -- unlike the
already-rejected idea of suppressing or relocating that shared tail itself,
which would silently shift every other duration gate that reads
`action_frame` -- makes Dash/Turn's own `action_frame` exactly `1` at E
dispatch, `2` at E end, `2` at E+1 dispatch, `3` at E+2 dispatch: dispatch
now agrees with decomp on every frame like every other action (removing
the `dash_run_frame`/`+1` patch below and `game::movement::pose`'s own
existing `action_frame.saturating_add(1)` Dash special case, both reverted
to the unadjusted comparison), and end is now uniformly one ahead of
decomp like every other action (removing `observation::observe`'s Dash/
Turn exception, restoring its single general `-1` rule for every action).

`game::movement::pose`'s Turn arm had no such special case before this
batch (it already read `action_frame` unadjusted), so it was quietly one
frame behind decomp's own bone sampling for Turn specifically; this batch
fixes that latent gap as a side effect of the source-level change, with no
code change needed there. `src/game/grab.rs`'s `shield_entry_buffer`
(`ftCo_80091B9C`'s "Dash past the x4C frame" gate) and `src/game/
locomotion.rs`'s `Action::Dash if f.action_frame >= p.dash_animation_frames`
(`ftCo_Dash_Anim`'s animation-end check) and `Action::Turn`'s equivalent
`turn_animation_frames` check are the same kind of raw, unadjusted
comparison and needed no code change either, for the same reason.

**Tests**: every existing synthetic test that stepped a fixed number of
frames from a fresh Dash/Turn entry to land on a specific phase boundary
(`tests/game_dash.rs`, `tests/game_shield_grab.rs`, `crates/cli/tests/
replay_match.rs`) needed its frame count reduced by one to keep landing on
the same decomp-cited boundary now that entry itself starts one
`action_frame` higher, and every direct `action_frame` assertion on a
freshly-(re-)entered Dash changed from `1` to `2` (already-decomp-verified
`state_age`/`action_age` values, e.g. `dash_run_frame`'s own boundary and
the Dash/Turn 1.0 entry-frame age, are unaffected, since observation and
the fixed checks already produce the same numbers). `tests/game_dash.rs`'s
`holding_forward_through_dash_enters_run_one_frame_before_the_unadjusted_
threshold` is renamed `..._at_the_unadjusted_threshold` and now asserts
Run is entered exactly when `action_frame` reaches `dash_run_frame`
unadjusted, rather than one frame early. No C-oracle differential changed:
this still ports a control-flow/sequencing fact, not a pinned function's
arithmetic. `cargo fmt --all -- --check`, `cargo clippy --locked --
workspace --all-targets --all-features -- -D warnings` and `cargo test
--locked --workspace` (910 passed/0 failed/19 ignored, unchanged from
immediately before this batch -- no test added or removed, only corrected)
all pass. Measured against gameplay export pack v5: unchanged from the
measurement below (116 frames matched, first divergence at -7,
`position.x`, on P1's Run->KneeBend transition) -- this batch is a
behavior-preserving consolidation for every frame this recording already
reaches, confirmed rather than assumed.

The 2026-09-11 Dash->Run `action_frame` timing fix (the real-replay parity
loop, `docs/parity.md`) fixes a genuine one-frame-late boundary in
`game::dash::update_dash_or_run` and `game::locomotion::update_actions`'s
identical fallback copy: both compared `f.action_frame >= p.dash_run_frame`
to decide when a held Dash automatically becomes Run, modeling `fn_
800CA5F0`'s gate on the animation's own scripted run flag (`cmd_vars[0]`,
written by a set-cmd-var command at `ftaction.c:462`, which `dash_run_frame`
stands in for directly). The decomp side of that comparison,
`ftCo_Dash_IASA`, reads `fp->cur_anim_frame` after the *same* frame's own
generic per-frame animation advance (`Fighter_Spaghetti_8006AD10`'s
unconditional `ftAnim_8006EBA4(gobj)`, `fighter.c:1684`) has already run --
this is the identical ordering fact the Dash/Turn `state_age` fix above
relies on. Skirmish's shared end-of-frame `action_frame += 1`
(`simulation::advance`) runs *after* `update_actions`/`update_dash_or_run`
within the same frame, so `f.action_frame` at the point this check reads it
still holds the *previous* frame's count -- one less than the value decomp
compares at the equivalent instant. Confirmed directly against `fox-fd.slp`:
P1 holds forward through Dash frames -24..-14 (`action_age` 1..11, all
matching) and is already in Run at -13 -- the frame whose *unincremented*
`action_frame` is 11, one below `dash_run_frame` (12) -- not one frame
later, at -12, which the unadjusted comparison produced (the first
divergence this fix targets: `action_state` expected Run, actual Dash, at
-13). Both copies of the check now compare `action_frame + 1 >=
dash_run_frame`.

**Tests added**: `tests/game_dash.rs`'s new
`holding_forward_through_dash_enters_run_one_frame_before_the_unadjusted_
threshold` pins the synthetic fixture's own `dash_run_frame` (8) boundary,
confirming Run is entered once `action_frame` reaches 7 rather than 8. This
shifted an existing test's own boundary: `late_phase_redash_ground_
velocity_reflects_the_transition_friction_tail`'s `rules.dash = None`
scenario held neutral for 6 frames before its own fresh press, which used to
land one frame short of the (buggy) run-transition threshold and now lands
exactly on the corrected one, entering Run instead of continuing Dash (not
what that scenario tests, since it only tests the fallback path with no
friction tail); reduced to 5 frames of neutral to stay clear of the boundary
again, with a comment explaining why. `tests/support/dash.rs`'s own
fixture-rationale comment (`BUFFER.dash_buffer_frame_limit`) is updated to
describe the corrected timing (the threshold is now reachable exactly when
late phase begins, not one frame into it) instead of the stale, pre-fix
reasoning. No C-oracle differential was added, for the same reason as the
Dash/Turn `state_age` fix: this ports a control-flow/sequencing fact about
when in the frame a check runs, not a specific pinned function's arithmetic.
`cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-
targets --all-features -- -D warnings` and `cargo test --locked --workspace`
(910 passed/0 failed/19 ignored, up from 909/0/19 immediately before this
fix) all pass.

Measured against gameplay export pack v5 (`v5-snapshot-20260911`, published;
pack v4's missing specials `move_id` is fixed in v5, otherwise identical
fox-fd data through this range): `checked_frames` went from 110 to 116
(frames -123 through -8); the next divergence is -7, `position.x` (expected
`-17.7748`, actual `-17.6948`) on P1's Run->KneeBend (JumpSquat) transition,
with `action_state` itself already matching -- a separate, unrelated
physics subsystem, reported rather than chased in this batch.
`tests/fixtures/slippi/parity/fox-fd-baseline.json` and `tests/fixtures/
slippi/parity/gameplay-export.lock.json` are updated to pack v5
accordingly; `docs/parity.md` records the same measurement.

The 2026-09-11 Dash->Turn `action_age` fix (the real-replay parity loop,
`docs/parity.md`) extends the transition-frame fix below to `Action::Turn`:
`ftCo_Turn_Enter` and `ftCo_Turn_Enter_Smash` (`ftCo_Turn.c:49-62`, `:173-
188`, reached respectively from a plain standing-turn check and from
`ftCo_Dash_CheckInput`'s dash-back/re-dash logic) both call `ftAnim_
8006EBA4(gobj)` immediately after `Fighter_ChangeMotionState`, exactly the
extra animation advance `ftCo_Dash_Enter` already made and that
`observation::observe` already special-cased for `Action::Dash`. Confirmed
directly against `fox-fd.slp`'s dash-dance rally: P1 enters Turn at frames
-30 and -25 (each already reporting `state_age = 1.0`, not 0.0) and
re-enters Dash at -29 and -24 (also already 1.0, the existing exception).
`ftCo_TurnRun_Enter` (`ftCo_TurnRun.c:44-51`) changes motion state but does
not make this extra call, so `Action::RunTurn` keeps the general rule.
Fixed in `crates/skirmish-replay/src/observation.rs`'s `observe` (the
`Action::Dash | Action::Turn` match arm) and its harness-local duplicate in
`crates/cli/tests/replay_match.rs`. A new integration test,
`entering_a_smash_turn_from_dash_reports_the_replay_verified_age_of_one`
(`tests/game_dash.rs`), pins the recording's `state_age == 1.0` on a
dash-back smash-Turn entry the same way the existing Dash entry test
already did. No C-oracle differential was added, for the same reason as the
Dash fix below: this ports a control-flow/sequencing fact, not a specific
pinned function's arithmetic. `cargo fmt --all -- --check`, `cargo clippy
--locked --workspace --all-targets --all-features -- -D warnings` and
`cargo test --locked --workspace` (909 passed/0 failed/19 ignored, up from
908/0/19 immediately before this fix) all pass. Measured directly against
gameplay export pack v4: `checked_frames` went from 93 to 110 (frames -123
through -14); the next divergence is -13, `action_state` (expected Run,
actual Dash, on P1's Dash->Run transition) -- a separate, unrelated
subsystem, reported rather than chased in this batch. `docs/parity.md`
records the same measurement.

The 2026-09-11 ECB-load-flags batch (`docs/ecb-load-flags.md`) fixed two
real, decomp-cited bugs: `game::collision::sample` was loading every
`CollisionBox::Bones` ECB with one static resource flags value (0 in every
pack) instead of the per-collision-path value the source passes to
`mpColl_LoadECB_inline` (6 airborne, 5 grounded), and `game::collision::
resolve`'s floor-contact branch always rested `position + ecb.current.
bottom` on the floor instead of `position` alone when the raw (unanchored)
ECB bottom samples above position (`mpColl_80046904`'s `ecb_unlocked`,
forwarded to `mpColl_80044838_Floor`'s `ignore_bottom`). Both are cited and
explained in full in `docs/ecb-load-flags.md`, including why bit 0x2 needed
no sweep change (it belongs to an unrelated, already-correctly-ported
parameter) and a known resource gap (a missing `move_id` blocks stepping
pack v4's own recorded inputs past frame ~71, unrelated to this fix).
`load_joints` itself (`collision/ecb.rs`) was already differential-tested
against the oracle for every flags value 0..32, so no oracle adapter change
was needed; two new synthetic integration tests
(`tests/game_collision_bones.rs`) and a new real-recording regression
(`crates/cli/tests/ecb_load_flags_v4.rs`, gated on pack v4's fixed archive
path) were added. Measured against pack v4 directly: `checked_frames` went
from (uncorrected) landing one frame early at -50, to 74 with only the
`load_flags` fix (landing frame now matches, but at the wrong position), to
93 with both fixes (the entry fall and its landing position now match the
recording exactly); the next divergence (-30, `action_age` on a Dash->Turn
transition) is a separate, unrelated subsystem, reported rather than
chased. `docs/parity.md` records the same measurement against its own
`SKIRMISH_GAMEPLAY_DATA`-gated ratchet.

The 2026-09-11 ECB-timing investigation batch (`docs/ecb-timing.md`) fixed
one real, decomp-cited bug: `ftCo_Fall_Enter`/`ftCommon_8007D5D4`'s ten-frame
ECB bottom lock was applied on grounded-jump-launch, aerial-jump and
platform-pass-through transitions into Fall, but not on the plain "lost
ground support, falls without jumping" transition (`src/game/collision.rs`,
the branch that calls `simulation::enter(f, Action::Fall)` after
`f.grounded = false`). Fixed with a new test,
`dashing_off_an_edge_locks_the_ecb_bottom_for_ten_frames`
(`tests/game_locomotion.rs`), which also pins the asymmetry between a
jump's lock (visible to that same frame's `advance_ecb_lock`, reading back
as 9) and this collision-detected lock (set after that frame's
`advance_ecb_lock`, reading back as 10) that the decomp's own call order
requires. No pinned function's arithmetic changed (`collision/ecb.rs`'s
`State` methods, already faithful to `mpColl_LoadECB_JObj`/
`mpCollInterpolateECB`/`mpCollCheckBounding`, are untouched), so no new
C-oracle differential was needed.

This same investigation confirmed, independently of the movement-poses
batch below, that `fox-fd.slp`'s -51-vs.-49 landing divergence is not
fixable by this or any other purely Skirmish-side ECB timing/lock
correction: Fox's real, disc-decoded `collision_box.indices` include a
joint (index 0) whose real FigaTree track is verified channel-less (always
`[0,0,0]` local translation), and the decomp's own clamp
(`mpColl_LoadECB_JObj`'s `if (bottom_y < 0) bottom_y = 0`) forces the ECB
bottom to equal `position.y` on every frame as a mathematical consequence
of that data, independent of locking or frame order. Confirmed both by
derivation from the decomp and by runtime instrumentation of the real
simulation against the recording (temporary, removed before committing).
See `docs/ecb-timing.md` for the full chain of reasoning, the ruled-out
alternative mechanisms, and `docs/parity.md`'s updated measurement entry.
The full archived audit (`python3 /mnt/archive/runs/skirmish-ecb-response-
20260909/validation.py /mnt/shared/tmp/skirmish-ecb-timing
/mnt/archive/runs/skirmish-ecb-timing-20260911-verified
/mnt/shared/tmp/skirmish-target-ecb-timing-audit`) is recorded at that
archive path.

The 2026-09-11 `state_age`/`action_age` transition-frame fix (the real-replay
parity loop, `docs/parity.md`) validates formatting, strict all-target/
all-feature Clippy and the complete native workspace test suite (`cargo test
--locked --workspace`): 899 passed/0 failed/19 ignored (up from 898/0/19
immediately before this fix; 1 new integration test in `tests/game_dash.rs`,
plus a new pinned assertion added to an existing test in
`tests/game_entry.rs`, and the harness-local `action_age` duplicate in
`crates/cli/tests/replay_match.rs` updated to match). The full archived
audit (`python3 /mnt/archive/runs/skirmish-ecb-response-20260909/
validation.py`) is deferred to the end of this loop, per its own convention
of covering the loop's cumulative work in one pass rather than per fix.

Real-replay measurement (`docs/parity.md`): against both the published
snapshot (`/mnt/archive/datasets/melee/skirmish-gameplay/v2-snapshot-
20260911/`) and the live pack (`/mnt/archive/datasets/melee/skirmish-
gameplay/v2/`), `fox-fd.slp`'s first divergent frame moved from -59 (64
frames matched) to -51 (72 frames matched); `tests/fixtures/slippi/parity/
fox-fd-baseline.json` is updated accordingly.

**The bug**: `crates/skirmish-replay/src/observation.rs`'s `observe`
reported a freshly-entered (or restarted) action's `state_age` as 1 on its
own transition frame, for every action except the ones (`Action::
EntryStart`, and the constant-`-1` `Entry`/`EntryEnd`) already special-cased
by the match-start batch. `fox-fd.slp` shows this is wrong for the general
case: P1's EntryEnd->Fall handoff at frame -59 reports `state_age = 0` in
the recording, not 1 (`docs/match-start.md`'s frame table).

**The decomp mechanism** (`Fighter_ChangeMotionState`, `fighter.c:933-1230`):
a fresh `_Enter`'s call to `ChangeMotionState` synchronously lands `fp->
cur_anim_frame` on the destination motion's own `anim_start` (typically
`0.0f`) via its own internal `ftAnim_8006E9B4` call (`fighter.c:1224`,
`:1274`/`:1298`) before returning. The generic per-frame animation advance
(`Fighter_Spaghetti_8006AD10`'s unconditional `ftAnim_8006EBA4(gobj)` at
`fighter.c:1684`) runs once per frame *before* that frame's own `anim_cb`
(and any input-driven command dispatch), so it only ever advances whichever
action was already current before any transition this same frame triggers.
A fighter that changes (or restarts) action this frame therefore gets no
further advance until *next* frame's own call. Confirmed directly against
`fox-fd.slp` for four independent transitions this codebase already
implements: P1 Fall at -59, Landing at -49, Run at -13 and KneeBend at -7
all report `state_age = 0` on their own transition frame, then count up
normally (0, 1, 2, ...) on every frame after.

Skirmish's `simulation::advance` instead runs a single, unconditional,
shared end-of-frame `action_frame += 1` for every fighter every frame
(`src/game/simulation.rs`), one frame after `simulation::enter` already
reset `action_frame` to 0 -- so the reported `action_frame` is always one
frame ahead of Melee's own `cur_anim_frame`. Changing this shared tail
itself (tried first, reverted) would have been wrong: `action_frame` is
also this codebase's internal elapsed-frame counter, read directly by
dozens of unrelated duration gates throughout `src/game/*.rs` (for example
`src/game/simulation.rs`'s own `!(f.action == Action::Jump && f.action_frame
== 0)` gravity-skip-on-launch check); suppressing the tail's own increment
on the entry frame would silently shift every one of those gates by a
frame for the rest of the action's lifetime, a much larger and unverified
change than this fix's actual scope. The correct level is the observation
boundary, exactly where the match-start batch's own `EntryStart`-specific
`action_frame - 1` hack already lived: `observation::observe`'s default
branch now reports `action_frame.saturating_sub(1)` for every action.

**The one verified exception**: `Action::Dash`. `ftCo_Dash_Enter`
(`ftCo_Dash.c:48-63`) calls `ftAnim_8006EBA4(gobj)` a second, explicit time
immediately after `Fighter_ChangeMotionState` returns -- an extra advance
`ftCo_Fall_Enter`/`ftCo_Landing_Enter`/`ftCo_Run_Enter_Full`/`ftCo_
KneeBend_Enter` (read in full) do not make. `fox-fd.slp` confirms this
directly: P1 enters Dash from a fresh stick press at frame -37 and already
reports `state_age = 1.0` on that same frame. `observation::observe` keeps
`action_frame` unadjusted for `Action::Dash` specifically; every other
already-implemented action was checked against this replay only for the
four transitions above; a future divergence may surface another exception
this batch did not find.

**Tests added**: `tests/game_entry.rs`'s existing `the_replay_verified_
frame_table_is_reproduced_for_slots_zero_and_three` gained a pinned
`action_age == 0.0` assertion on the frame P1 exits EntryEnd into Fall
(the exact recording value, frame -59). `tests/game_dash.rs`'s new
`entering_dash_from_a_fresh_press_reports_the_replay_verified_age_of_one`
pins Dash's own `action_age == 1.0` on its entry frame. No C-oracle
differential was added: this fix ports a control-flow/sequencing fact (when
in the frame the increment happens), not a specific pinned function's
arithmetic, so there is no bit-exactness edge case to pin against a
compiled original function (the same reasoning `docs/validation.md`'s
input-lock entry already used for a different gap).

The 2026-09-11 pre-"GO" input lock and countdown-period simulation batch is
recorded at:

`/mnt/archive/runs/skirmish-input-lock-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, and the
complete native workspace (both without and with the `c-oracle` feature,
debug and release): 893 passed/0 failed/19 ignored without `c-oracle`, 1234
passed/0 failed/19 ignored with it (up from 886/0/19 and 1227/0/19
immediately before this batch; the 7 new tests -- 2 unit tests in
`src/game/entry.rs`, 5 integration tests in `tests/game_entry.rs` -- run
under both configurations, and the pre-existing suite is otherwise
unchanged). The audit's first attempt hit a release-mode-only failure in
`fox_side_special_differential::arbitrary_special_air_check_input`
(unrelated to anything in this diff -- that module was not touched);
retried clean (5/5 runs passed in isolation, then the full audit passed
end to end) and separately diagnosed and fixed as its own commit (below).

This batch implements `docs/input-lock.md`: with `rules.entry` present, the
match now simulates every frame fully from the very first frame (no
`Phase::Countdown` freeze at all, not just the match-start batch's own
narrower "run `entry::update_animation` during Countdown" scoping), and
every fighter's controller is replaced by a neutral `Controller` for the
first `rules.entry.input_lock_frames` frames (`u32`, serde default 84, new
field on `game::entry::EntryRules`, validated `<= Rules.countdown_frames`)
regardless of action state or `Phase`. `rules.entry.is_none()` keeps the
legacy frozen Countdown byte-for-byte, including every pre-existing fixture
that asserts frozen positions through it
(`tests/game_matches.rs::countdown_walk_jump_land_hitlag_respawn_and_second_stock_finish`).

Two changes in `game::simulation::advance` (`src/game/simulation.rs`), both
gated on `data.rules.entry.is_some()`:
1. The function now captures `was_countdown` (the phase *before* this
   frame's own Countdown -> Playing transition) and, when `rules.entry` is
   `Some`, no longer returns early out of the `Phase::Countdown` branch --
   it falls through to the complete per-frame pipeline (physics, collision,
   landing, combat) instead, so the match-start warp-in, the fall into
   ordinary `Action::Fall`, and any landing all actually run during the
   pre-"GO" period, matching what the replay shows. `state.remaining_frames`'s
   decrement and the time-limit finish check are now explicitly gated on
   `!was_countdown` (previously implicit, since the whole function returned
   before reaching them): the match clock still only starts once a frame
   begins already in `Phase::Playing`, `rules.countdown_frames` unchanged.
2. `inputs` is replaced with two neutral `Controller`s whenever
   `state.next_frame <= entry.input_lock_frames`, upstream of every other
   use of `inputs` that frame (dispatch, hitlag sampling, and the
   `previous_input` bookkeeping all see neutral during the lock).

**The decomp gate was searched for and not found; the design note's own
fallback (default 84, replay evidence cited, citation left pending) is
used.** Read in full and confirmed not to gate pad input by frame count:
`gm/gmvs.c`'s `gm_Scene_Vs_OnFrame` and its `fn_8016CD98`/`fn_8016CFE0`
neighbors (the VS-mode per-frame dispatch and the `frame_count`/HUD-timer
advance); `ft/fighter.c`'s `Fighter_Spaghetti_8006AD10` (the per-fighter pad
copy, unconditionally scheduled at fighter creation with no frame-count or
phase gate in its body) and its `x221D_b3` double-buffer bit (a pad
*history* selector, not a suppressor); `gm/gm_1A45.c`'s `gm_801A45E8`
(pause/camera/HUD bit queries, none frame-count-gated); `sysdolphin/
baselib/controller.c`'s `HSD_PadMasterStatus`; `pl/player.c`'s
`Player_80032828` (a pose-array setter, unrelated to input). This is
consistent with, but distinct from, the match-start batch's own finding
that Entry/EntryStart/EntryEnd's `_IASA` callbacks are unconditionally
empty: that fact only explains why input has no effect *during those three
states*, not this batch's own new replay evidence (`docs/input-lock.md`'s
frame table: P4 holds the stick through part of ordinary `Fall`, at
-44..-41, well past EntryEnd, with zero drift), which requires a broader
gate than an empty `_IASA`. No C-oracle differential was added as a
consequence: `docs/parity.md`'s three-level framework reserves differential
coverage for a Rust function that ports a specific pinned decomp function,
and no decomp function implementing this gate was found to pin one against.

Resource shape: `Rules.entry.input_lock_frames: u32` (serde default 84,
`game::entry::default_input_lock_frames`), validated
`entry.input_lock_frames <= rules.countdown_frames` in
`game::validation::validate`.

`src/game/entry.rs` gains 2 unit tests (`input_lock_frames_defaults_to_
eighty_four_when_absent_from_the_resource`, `input_lock_frames_round_trips_
when_present`) confirming the serde default and round trip.
`tests/game_entry.rs` gains 4 integration tests: `the_match_simulates_
fully_through_countdown_when_entry_rules_are_present` (the match-start
sequence itself progresses and reaches ordinary `Fall` while `Phase` is
still `Countdown`, and the clock/`Phase::Playing` transition still lands
exactly at `countdown_frames`, unaffected by the shorter input lock);
`a_held_stick_produces_no_aerial_drift_while_the_input_lock_is_active` (a
stick held hard toward the fighter's own facing direction produces zero
`velocity[0]` at every sampled locked frame in ordinary `Fall`, and a
neutral `previous_input`); `the_first_controlled_frame_acts_on_the_held_
stick` (the very next step past `input_lock_frames` shows nonzero aerial
drift and `previous_input` equal to the real controller); and
`rules_entry_none_keeps_the_legacy_frozen_countdown_unaffected_by_the_lock`
(a held stick during a `rules.entry.is_none()` Countdown produces no
position change at all, mirroring `tests/game_matches.rs`'s own frozen-
position assertion so this batch cannot have silently touched the legacy
path). `invalid_entry_rules_are_rejected` gains a case for
`input_lock_frames > countdown_frames`. Two existing struct literals
(`tests/game_entry.rs::data`, `crates/cli/tests/replay_match.rs::
entry_replay_data`) needed `input_lock_frames: 0` added since a bare Rust
struct literal does not apply serde defaults and both fixtures use
`countdown_frames: 0`; no other pre-existing test changed.

**The `state_flags.dead` bit (`Fighter::x221F_b1`), found via the real-file
measurement itself.** The published gameplay export gained `rules.shield`
data mid-batch (a separate, unrelated exporter update); once `shield` was no
longer masking every other field, the real-file measurement (below) landed
on `state_flags.dead` -- the match-start sequence's own dead-flag bit, which
neither this batch nor the match-start batch had modeled. Fixed here, since
it belongs to the same Entry/EntryStart sequence this batch already covers:
`ft_0C31.c:46` (`ftCo_800C61B0`) sets `fp->x221F_b1 = true` immediately
after `Fighter_ChangeMotionState(gobj, ftCo_MS_Entry, ...)`; `fighter.c:1066`
confirms that same function unconditionally clears `x221F_b1 = 0` on every
motion change. `game::simulation::enter` (the shared per-transition function
every `Action` change already funnels through) now unconditionally sets
`fighter.death.hidden = false` alongside its other unconditional resets --
`fighter.death.hidden` was already Skirmish's existing runtime bit for this
exact Slippi flag (`crates/skirmish-replay/src/observation.rs`'s
`state_flags`: `fighter.death.hidden || matches!(action, Respawn |
Eliminated)`), previously written only by `death::begin` for blast deaths.
`game::entry::enter` now sets `fighter.death.hidden = true` immediately
after its own `simulation::enter(fighter, Action::Entry)` call, mirroring
the source's "ChangeMotionState, then set the flag back" order exactly.
`game::death::begin` is reordered (`simulation::enter` first, then the
`fighter.death = State { ... }` assignment) so the new unconditional reset
does not clobber its own intentional `hidden` set; nothing between the two
statements previously read `fighter.death`, so this is behavior-preserving
for every existing death test. `tests/game_entry.rs` gains
`the_dead_flag_bit_is_set_through_entry_and_clears_at_entrystart`, pinning
the bit set at spawn and through every remaining Entry frame, cleared
starting at EntryStart's first frame, and still cleared through EntryEnd and
the Fall it exits into -- bringing this batch's own test count to 7 new (2
unit, 5 integration) instead of 6.

Real-file measurement (`docs/input-lock.md`, `docs/parity.md`): gameplay
export v2 already ships `rules.entry`/`countdown_frames: 123` and per-
fighter `trophy_scale`/`entry.start_frames` (no local patch needed, unlike
v1's match-start measurement). Copying
`/mnt/archive/datasets/melee/skirmish-gameplay/v2/` to
`/mnt/shared/tmp/skirmish-gameplay-v2-lock/` and running
`make-initialization` + `validate-replay --report` against the pinned
`fox-fd.slp` first reported the first divergent frame as -123, for the same
pre-existing reason already recorded for the match-start batch: `shield`
(expected `0x42700000` = 60.0, Skirmish reported `0x00000000`), since that
copy of export v2 still had no `rules.shield`. Re-copying the export after
it gained `rules.shield` moved the divergence past `shield`, still at -123,
to `state_flags.dead` (fixed above); re-running again after that fix moves
the divergence past it too, still at -123, landing on `position.x` for P4
(expected `0x42700000` = 60.0, Skirmish reports `0x41a00000` = 20.0) -- a
stage-spawn coordinate mismatch in the pack's own `stage.spawns` data,
unrelated to either this batch or match-start (neither reads spawn
coordinates from anywhere else). Both of this batch's own fields now agree
with the recording on frame -123, verified independently by the native
tests above. `fox-fd-baseline.json` is left unmoved (still -123).

The 2026-09-11 grab-escape timer standings/handicap batch is recorded at:

`/mnt/archive/runs/skirmish-grab-escape-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, and the
complete native workspace in debug (without and with the `c-oracle`
feature) and release (with `c-oracle`) modes, plus `git diff --check`: all
six steps exited 0 — `fmt` and `diff` (no output either way), `clippy`
(`-D warnings`, all features), `native` 891 passed/19 ignored, `c-oracle`
1234 passed/19 ignored, `release` 1234 passed/19 ignored (same counts as
debug `c-oracle`, as expected for a release rebuild of the same suite).

This batch models `ftCo_800DA824`, the real grab-escape capture timer,
replacing the flattened `timer_base + percent * timer_percent_scale`
constant with the source's own standings- and handicap-driven formula
(`docs/grab-escape-timer.md`). `Rules.grab.escape` gained
`formula: Option<EscapeFormula>` (the six `ftCommonData` constants,
`x354..x368`), used in place of the legacy fields when present;
`MatchData` gained `players: Option<[PlayerSettings; 2]>` (per-player
handicap, default 9); `crates/cli/src/initialization.rs::build` fills it
from the replay's `GameStart.players[_].handicap` (always present in
peppi 2.1.2). The standing helper reproduces `gm_80166378`/`fn_80165AC0`'s
ranking (count of opponents with a strictly greater score; ties share
standing 0) for the Stock-match branch of `fn_8016588C` specifically,
since Skirmish only models stock matches — a correction against this
batch's own design note, which read as if `percent` participated in the
tie rule; it does not for Stock matches (`docs/grab-escape-timer.md`'s
"Implemented" section has the full citation trail).

New coverage: a bit-exact C-oracle differential for `ftCo_800DA824`
across arbitrary constants, percent, handicap 1..9 and standing 0..3
(`tests/escape_formula_differential.rs`, reusing the existing
`ftCo_CapturePulled.c` snapshot via a new `adapters.json` alias); unit
tests for standing ordering/ties and the handicap default
(`src/game/grab.rs`); a unit test deriving the real Fox constants'
75.0 replay-settings value from the formula itself, not a hardcoded
literal (`src/fighter/grab.rs`); and a self-recorded replay regression
where a fighter is knocked down one stock and then grabbed, asserting its
`escape_timer` matches the standing-1 formula value and differs from the
tied-standing-0 value (`crates/cli/tests/replay_match.rs`).

The 2026-09-11 Fox up-special coverage batch (Fire Fox/Fire Bird) is
recorded at:

`/mnt/archive/runs/skirmish-fox-up-special-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace (both without and with the `c-oracle` feature, debug and
release): 849 passed/0 failed without `c-oracle`, 1166 passed/0 failed with
it, both green with no failures (counts as of the follow-up round below;
see that section for what changed). `game::characters::fox::up` (resource,
dispatch, `src/game/characters/fox/up.rs`) covers `Action::SpecialHiHold/
SpecialHiHoldAir/SpecialHi/SpecialAirHi/SpecialHiLanding/SpecialHiFall/
SpecialHiBound` (`docs/fox-up-special.md`), Fox's up special (Fire Fox;
Falco's Fire Bird shares the code with its own attributes). The core
implementation, the resource shape, the framework wiring (`SpecialMove::
update_animation`/`land` gaining `on_platform`, `Movement::drift_clamp`,
`locomotion::State::up_special_b_age`, the 7 new `Action` variants and
Slippi ids) predate this batch's own work (a prior session's foundation,
compiling clean with the pre-existing test suite unchanged); this batch
adds the oracle, the native and self-recorded regression coverage, and
fixes four real bugs the new coverage surfaced (below).

Two concrete facts confirmed for this batch: the frame-13 `SpecialHiLanding`
entry point is specifically Fall's own ordinary ground/ledge touch
(`ftFx_SpecialHiFall_Coll` -> `ftFx_SpecialHiFall_Enter`), not Travel's own
landing-bound decision (which enters at frame 0, or Bound); and the
Slippi ids (353..359) are confirmed directly against `ftFox/forward.h`'s
own enum order, continuing straight on from the side special's own
347..352, while the matching animation indices (307..313) remain an
unverified extrapolation of the same constant -46 offset the side special
batch already flagged, since the pinned C decomp has no figatree/
animation-index table for character-specific motion states.

Four real bugs were caught by this batch's own C-oracle differential
suite while building it (`tests/fox_up_special_differential.rs`), none of
them present in the prior session's design note, and all fixed in
`src/game/characters/fox/up.rs`:
- **The frame-13 gap itself.** `up.rs` had no `land()` arm for
  `Action::SpecialHiFall` at all, so Fall's own ordinary ground touch fell
  through to the generic `Action::Landing` instead of `SpecialHiLanding`
  at frame 13. This is the one item the batch's own brief asked to
  specifically confirm or refute; it was a real, previously unflagged
  implementation gap, now fixed with a dedicated regression
  (`tests/game_fox_up_special.rs::fall_lands_at_frame_13_via_ordinary_ground_touch`).
- **`angle_xy`'s zero-length guard used `product > 0.0`** where the pinned
  `lbVector_AngleXY`'s own `if (lena_lenb)` is a non-zero check; the two
  diverge for a NaN product (a near-zero vector paired with one large
  enough to overflow the other length to infinity), where `!= 0` reaches
  `acosf(NaN)` (propagating it) but `> 0.0` would have silently
  substituted 0.0.
- **`enter_from_ground_hold`'s own magnitude/angle gates used `>=`** where
  the pinned `ftFx_SpecialAirHi_AirToGround` writes both as negated
  less-than (unlike `ftFx_SpecialAirHi_Enter`'s own direct `>=` for the
  same magnitude check); the two diverge for a NaN operand, reachable
  through the previous fix.
- **The aerial launch velocity and the Travel air reverse-acceleration
  both regrouped `facing_dir * (x74_or_x78 * cosf(angle))`** as
  `(facing * speed) * cosf(angle)` (left-to-right evaluation); `f32`
  multiplication is not associative, so this could differ from the
  source's own grouping by an ULP.

See `docs/fox-up-special.md`'s own "Corrections against the pinned source"
section for the exact citations and fixes. This entry originally left the
wall/ceiling mid-Travel redirect, the shallow-floor-angle graze sub-case
of the landing bound decision, ground Travel's own per-frame
`rotateModel` re-derivation from the floor normal, and `FallSpecial`'s own
stored `landing_lag` argument unmodeled; a reviewer correctly flagged
those four as ordinary gameplay rather than exotic edge cases, and the
follow-up round below implements and tests all four. What remains
genuinely unmodeled after the follow-up is only Travel's empty hitboxes,
the visual model-rotation bone itself (rendering-only), and the
already-unreachable `x21F8` callback.

`tests/oracle/original/ftfoxspecialhi.c` pins the new source (sha256 in
`sources.json`); `tests/oracle/ftfoxspecialhi.functions.json` selects 41 of
its functions (every non-static callback plus its three static/
static-inline helpers, dropping only its two GFX-only `_CreateChargeGFX`/
`_CreateLaunchGFX` callbacks, matching the side special's own precedent).
Two dependencies from other already-pinned files are reused via new
`adapters.json` aliases rather than re-snapshotted: `up_special_angle` (->
the existing `lbvector` snapshot, for `lbVector_AngleXY`) and
`up_special_platform` (-> the existing `pass` snapshot, for
`ftCo_8009A134`). `tests/oracle/fox_specialhi.c` is the new host adapter;
its own header documents the capture/script/faithful treatment of every
dependency, including the two deliberately out-of-scope ones
(`ft_80084DB0`, Fall's own shared aerial-gravity Phys; `ftCommon_8007CF58`,
Bound's own air drift clamp in `ftcommon.c`) captured for dispatch only.
`tests/fox_up_special_differential.rs` (18 tests, 512 proptest cases each
plus exact boundaries) compares every pinned function; trig-derived
values use a small ULP/absolute tolerance (`close_bits`) instead of raw
bit equality, documented in place and in `docs/fox-up-special.md`'s own
Oracle section, since this crate's own `libm` dependency measurably
disagrees with this host's system C compiler's `atan2f`/`cosf`/`sinf`/
`acosf` by a handful of ULPs on some inputs (deliberately, for
cross-platform replay determinism, not a defect) -- confirmed directly
against both this host's C compiler and Rust's own `f32` methods.
`src/game/characters/fox/up.rs` gains 5 unit tests for `angle_xy`/
`face_stick` (zero vectors, the NaN-product case, general NaN-safety, the
sign/zero convention). `tests/game_fox_up_special.rs` originally added 19
integration tests covering grounded and aerial entry, the launch angle's
two thresholds and the straight-up default, the grounded-vs-declined
launch decision (along the floor, into the floor, and on a platform),
Travel's duration and reverse acceleration, the landing bound decision,
the frame-13 regression, the `FallSpecial` exits, Bound's own
exit-flag-forced early exit, Slippi ids, a checkpoint round trip and
invalid resources; the follow-up round below adds 4 more (23 total), see
that section.
`crates/cli/tests/replay_match.rs` gains 2 file-backed self-recorded
regressions (framed as self-consistency evidence, not Melee parity,
matching `docs/parity.md`): a grounded Fox Fire Fox through
SpecialHiHold/SpecialHi/SpecialHiLanding and an aerial one through
SpecialHiHoldAir/SpecialAirHi/SpecialHiFall/FallSpecial, each matching its
own generated bytes and reporting a `Mismatch` at frame 0 once the entry
press is removed.

## Follow-up: the four previously-unmodeled items are now implemented

The same 2026-09-11 Fox up-special batch above was revised in a follow-up
round after review: three (in fact, on closer reading of the pinned
source, four) of the original "Unmodeled" items are ordinary gameplay
reachable in normal play, not exotic edge cases, and are implemented and
tested here instead of being left out. The audit was re-run into the same
record path (`/mnt/archive/runs/skirmish-fox-up-special-20260911-verified`,
overwritten), and the existing commit was amended rather than adding a
second one, so the record and the commit both describe the batch's final
state.

1. **The mid-Travel wall/ceiling redirect** (`ftFx_SpecialAirHi_Coll`'s
   non-ground branch). A shallow (grazing) contact with a wall or ceiling
   -- `lbVector_AngleXY(contact_normal, self_vel) < 90 + x94` degrees --
   redirects `facing`/`rotateModel` from the current velocity instead of
   letting the ordinary auto-zero-into-wall collision response kill it;
   ceiling takes priority over wall when both are hit in the same step.
   Implemented as `up::bound_angle_gate`/`up::redirect_from_velocity`
   (shared with the landing bound's own graze case below) and
   `SpecialMove::wants_redirect`/`air_contact`, wired into
   `game::collision::resolve()`'s existing `response_eligible`/
   `surface_contacts` machinery right after the per-step sweep loop, the
   same place `can_surface_tech`/`can_reflect` already hook in.
2. **The shallow-floor-angle graze sub-case of the landing bound
   decision.** The gate above, evaluated against the *pre-landing*
   velocity (before `collision::land()` zeroes it), can also apply to an
   ordinary floor touch: a shallow-enough landing grazes and redirects
   instead of entering `SpecialHiBound`. `SpecialMove::land`/
   `specials::land` gained a new `pre_landing: &Fighter` parameter (a full
   snapshot taken in `collision::land()` right before the existing
   zero/bookkeeping); Fox's up special either enters the bound as before
   or restores `*fighter = pre_landing.clone()` plus the redirected
   `facing`/`rotate_model` for the graze case. `fox::side::land` gained
   the same parameter and ignores it, preserving its already-audited
   behaviour exactly.
3. **Ground Travel's own per-frame `rotateModel` re-derivation from the
   floor normal.** Previously only computed once at aerial-launch time;
   now re-derived every grounded Travel frame
   (`up::Move::update_ground_contact`, a new `SpecialMove` hook dispatched
   from `game::simulation` right after `collision::resolve()` -- not from
   `tick_ground_timers`, which runs too early in the frame to see the
   step's fresh `floor_normal`). This matters because an eventual edge
   drop back into the air phase carries the *last real grounded angle*
   into the post-`duration_end` reverse acceleration, not a stale
   aerial-launch angle from possibly several seconds earlier.
4. **`FallSpecial`'s own `landing_lag` (x90).** `FallSpecial` already
   entered `Action::LandingFallSpecial` on ground touch (the original
   round's "unmodeled" framing of this item was inaccurate on that point
   -- it did not fall through to plain `Landing`); the actual gap was that
   its landing rate always used the shared/common landing-lag rate rather
   than this move's own per-instance `x90` value. `helpers::
   enter_fall_special` gained an `Option<f32>` `landing_lag` parameter
   (mirroring the existing `mobility: f32` pattern), stored on the new
   `aerial::State::landing_lag` field; `escape_air::land` now consults it
   (`unwrap_or(rules.landing_lag)`) instead of unconditionally using the
   common rate. Only Fox's up special supplies `Some(x90)`; `fox::side`'s
   own `enter_fall_special` call site passes `None`, preserving its
   already-audited behaviour exactly.

Native coverage: 4 new tests in `tests/game_fox_up_special.rs` (23 total,
up from 19) --
`travel_air_redirect_does_not_zero_velocity_against_a_shallow_wall`/
`_ceiling` (the redirect's own observable effect: velocity survives a
wall/ceiling hit that would otherwise be auto-zeroed, since recomputing
`facing`/`rotateModel` from an unchanged velocity is mathematically a
no-op and can't itself be asserted on),
`travel_ground_rotation_reflects_the_last_grounded_floor_normal_after_leaving_the_edge`,
and `fall_special_landing_uses_this_move_s_own_landing_lag_not_the_common_one`
(numerically distinguishing `6.1 / x90 == 1.525` from `6.1 /
rules.landing_lag == 0.61`). The pre-existing
`hold_ground_anim_end_declines_on_a_platform` test's expectation was
updated to check the floor-derived `rotateModel` formula instead of a
stale aerial-launch angle, since item 3 makes that the correct observable
behaviour now.

Oracle coverage: no new pinned functions or host-adapter scripting were
added for items 1/2 specifically, because the bit-exact angle/gate
arithmetic they both share (`lbVector_AngleXY` against a contact normal,
the `90 + x94` threshold, `atan2f`-based `facing`/`rotateModel` recompute)
was already exercised for ceiling, both walls, and shallow/steep floor
contacts by the original round's `compare_travel_coll_air` in
`tests/fox_up_special_differential.rs`, which predates this follow-up and
already scripts `ftFx_SpecialAirHi_Coll` across all five contact shapes.
Items 3 and 4 are wiring/dispatch-order and parameter-threading changes
with no new arithmetic against the pinned source, so they are covered
natively rather than at the oracle level, consistent with how the rest of
this batch's framework wiring (`on_platform`, `drift_clamp`) was already
tested. `tests/fox_up_special_differential.rs` itself is therefore
unchanged at 18 tests; the workspace-wide count increase (845->849
without `c-oracle`, 1162->1166 with it) comes entirely from the 4 new
`tests/game_fox_up_special.rs` tests, run under both configurations.
The 2026-09-11 match-start (Entry/EntryStart/EntryEnd) coverage batch is
recorded at:

`/mnt/archive/runs/skirmish-match-start-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace (without and with the `c-oracle` feature, debug and
release) and `git diff --check`, all green: 835 passed/19 ignored without
`c-oracle`, 1140 passed/19 ignored with it (both debug and release; this
was a net-new-coverage batch, not a behavior-preserving refactor, so no
pre-batch baseline comparison applies the way it does for the
specials-framework entry below). `game::entry` (resource,
per-fighter timer/Y-curve state machine, `src/game/entry.rs`) and
`fighter::entry` (the pure `entry_delay`, `spawn_facing`, `amplitude`,
`start_progress`, `end_progress` helpers, `src/fighter/entry.rs`) cover
`Action::Entry`/`EntryStart`/`EntryEnd` (Slippi 322/323/324,
`docs/match-start.md`), the match's very first frames.

Entry-owned fighters are folded into the *existing* per-frame pipeline
rather than given a bypass branch (unlike `rebirth`/`death`/`ledge`): three
targeted hooks -- `entry::update_animation` inside `simulation::
update_animation`'s own dispatch chain (the Anim-phase timer/transition
logic, matching each pinned function's exact control flow, including
`ftCo_Entry_Anim`'s check-before-decrement whose unconditional trailing
decrement lands on EntryStart's freshly assigned timer on a transition
frame), an early return in `simulation::update_actions` (matching the
pinned source's genuinely empty `_IASA` callbacks for all three states),
and an early return in `simulation::move_fighter` (`entry::move_fighter`,
the Phys-phase position write) -- so `collision::sample`/`collision::
resolve`/`staling::flush` run unmodified and landing/wall/ceiling contact
works for free. `Match::new_with_slots` (new; `Match::new` now delegates to
it with `[0, 1]`) threads each player's real Slippi port (P1=0..P4=3) into
the per-port entry delay; `crates/skirmish-replay/src/match_validation.rs`'s
`initialize` supplies it from `Initialization.ports`.

Two decomp facts confirmed directly against `ft_0C31.c` that this batch's
own preceding design note (`docs/match-start.md`'s predecessor) did not
have: every one of `ftCo_800C6408`/`ftCo_800C6B6C`/`ftCo_EntryEnd_Coll`
branches on `Fighter::x221F_b4` (a secondary-entity "follow the leader"
path, e.g. Ice Climbers' partner) before touching position -- only
`!x221F_b4` is ported, since Skirmish models one fighter per port; and
`ftCo_EntryStart_Coll`/`ftCo_EntryEnd_Coll` both set `box.bottom = -x28`
the same frame Phys writes `position.y = x4 + x28`, so the box's
world-space bottom is *always exactly the spawn height* (`x4`) for the
whole sequence -- `game::entry` uses this to let Entry-owned fighters
share the ordinary collision pipeline instead of porting `ft_80083E64`/
`ft_800846B0`'s bespoke sweep (whose bodies live outside `ft_0C31.c`),
documented as a simplification rather than a fresh guess. The design
note's two open questions are both resolved, not left open: no separate
"input gate" exists or is needed, since all three states' `_IASA`
callbacks are unconditionally empty (confirmed directly), so nothing reads
pad state until `ftCommon_8007D92C` exits into ordinary Fall/Wait, where
Skirmish's existing input dispatch already resumes unmodified; and
`Phase::Countdown` is extended narrowly (Anim only, for entry-owned
fighters, before its early return) rather than fully reworked, since the
shipped gameplay pack's own `countdown_frames` (`2`) is an unrelated
pre-game buffer, not Melee's 123-frame pre-"GO" period -- documented as a
scoping decision in `docs/match-start.md`, not a silent limitation.

A genuine bit-exactness bug was found and fixed by the oracle, not assumed
correct: `ftCo_800C6408`'s `1.497345` is an unsuffixed C double literal, so
`1.497345 * sp48.y` computes in `double` before truncating to `f32` on
assignment; `fighter::entry::amplitude` initially computed directly in
`f32` (Rust's own literal-type inference), differing by one ulp from the
oracle for generic inputs -- caught immediately by `tests/
entry_differential.rs`'s `entry_start_enter_amplitude_matches_the_1_
497345_literal` proptest and fixed by promoting to `f64` explicitly to
match the source. Separately, `EntryStart`'s externally observed age
needed a `-1` adjustment relative to the internal `action_frame`
(`simulation::advance`'s own generic end-of-frame `action_frame += 1`
already runs once more before a transition frame's state is externally
observable, the same pre-existing behavior `crates/cli/tests/
replay_match.rs`'s idle-fighter regression already documents for its own
restart row) to recover the replay-verified 0-based state_age;
`crates/skirmish-replay/src/observation.rs`'s `observe` publishes
`action_frame - 1` for `Action::EntryStart` and the fixed `-1` for
`Action::Entry`/`Action::EntryEnd`. `animation_index` maps `ftCo_
SM_EntryStart` to `238`, counted directly from `kinds/ftCommon/
forward.h`'s submotion enum and cross-checked against this codebase's own
already-verified `Wait1_0 == 2`/`Fall == 20`/`Landing == 35` entries in the
same enum. The `player == 0` spawn-facing hardcode is replaced by the
`gmvs.c` two-slot rule (`fighter::entry::spawn_facing`) only when
`rules.entry` is `Some`: applying it unconditionally broke three
pre-existing `tests/game_edges.rs` cases that park a second fighter far to
one side purely as a non-interacting dummy, which the general rule --
correctly -- reads as a real opponent position; `rules.entry.is_none()`
keeps the exact old hardcode. No other contradiction between this batch's
implementation and the pinned source was found.

A follow-up correction (same batch, before merge): the real `fox-fd.slp`
recording's own `state_age` for EntryStart advances 0..10 and then holds
at 10 for the rest of the state, while `position.y` keeps changing
correctly the whole time. This is not a gap -- it is `state_age` reporting
the character's own (much shorter) EntryStart *animation* frame, not the
30-frame `x6BC` action duration, exactly the same "reported age caps at
the figatree length" semantics Skirmish already models for Walk/Run/idle.
`FighterData.entry: Option<entry::EntryAnimation { start_frames: u32 }>`
(the figatree's own frame count, 11 for Fox) makes this explicit;
`crates/skirmish-replay/src/observation.rs`'s `observe` reports
`min(action_frame - 1, start_frames - 1)` for EntryStart when supplied,
and keeps the previous uncapped approximation otherwise.
`tests/game_entry.rs` gained two tests pinning the exact replay values
(age 0 at EntryStart's first frame, held at 10 from there through the
frame before EntryEnd) and confirming the uncapped default is unchanged
when the resource is absent. The local patched pack now also carries
`fighter.entry {11}` for both Fox fighters; the real-file measurement
below was re-run after this correction and reports the same outcome.

Resource shape: `Rules.entry: Option<entry::EntryRules { start_frames:
u32 (x6BC), end_frames: u32 (x6C0), scale_y: f32 (x6C4, visual only),
invincibility_frames: u32 (x6C8) }>`; `FighterData.trophy_scale:
Option<f32>` (`co_attrs.trophy_scale`); `FighterData.entry:
Option<entry::EntryAnimation { start_frames: u32 }>` (the EntryStart
figatree's own frame count, distinct from `Rules.entry`'s action-duration
timers despite the shared field name); `Fighter.entry: entry::State
{ timer, y0, scale, amplitude, offset }` (`Fighter::mv.co.entry`, one
`timer` field deliberately reused verbatim across all three states,
matching the source's own union). `rules.entry.is_none()` keeps every
fighter spawning directly into `Action::Fall`/`Action::Wait`, unchanged
from before this resource existed.

`src/fighter/entry.rs` adds 6 unit tests for `entry_delay`, `spawn_facing`
(including a reversed spawn layout the old hardcode gets backwards),
`amplitude` and the progress fractions. `tests/game_entry.rs` (12 tests)
covers: the per-slot entry delay and `gmvs.c` facing on spawn; the
complete replay-verified frame table (`docs/match-start.md`) reproduced
for slots 0 and 3 (Y values checked to five decimal places); the
EntryStart age-counts-from-zero convention and its figatree-length cap
(both present and absent); landing on a floor within reach; the
invincibility branch behind a nonzero `invincibility_frames`; no input or
side effects during any of the three states; `rules.entry == None`
keeping the pre-batch Fall start and facing hardcode; checkpoints
mid-sequence; and invalid rules (zero frame counts, non-finite/negative
`trophy_scale`, a zero-length `entry` animation). `tests/
entry_differential.rs` (6 tests, 512 proptest
cases each plus exact boundaries) pins `ftCo_Entry_Anim`/`ftCo_800C6408`/
`ftCo_EntryStart_Anim`/`ftCo_EntryStart_Phys`/`ftCo_800C6B6C`/`ftCo_
EntryEnd_Anim`/`ftCo_EntryEnd_Phys` from a new `entry` snapshot of
`ft_0C31.c`, comparing the timer/transition logic and the Y-curve
arithmetic (including the `x6BC` divisor EntryEnd's own Phys uses instead
of `x6C0`) against a Rust mirror, bit-exactly. `crates/cli/tests/
replay_match.rs` gains a file-backed regression driving both fighters
through Entry/EntryStart/EntryEnd/Fall, matching its own generated Peppi
bytes and reporting a `Mismatch` starting at the exact row a corrupted
EntryStart state id is planted.

Real-file measurement against the pinned `fox-fd.slp` recording (see
`docs/parity.md`'s entry for this batch): the patched local copy of
gameplay export v1 still diverges at frame -123, unmoved, but now for an
unrelated, pre-existing reason (export v1 has no `rules.shield`, so
`shield` diverges on the very same frame Entry's own fields would first
matter) -- the Entry sequence's own correctness is verified instead by the
native and C-oracle tests above, not by this measurement yet.
`fox-fd-baseline.json` is left at -123, matching `docs/match-start.md`'s
own instruction.

The 2026-09-11 Fox/Falco down special (Reflector) batch is recorded at:

`/mnt/archive/runs/skirmish-fox-down-special-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace (both without and with the `c-oracle` feature) and a
`c-oracle` release build, all green: 837 passed/0 failed workspace-wide
without `c-oracle`, 1153 passed/0 failed with it (previously 809/1108,
i.e. this batch adds 28 native tests -- 15 in `tests/game_fox_down_special.
rs`, 2 self-recorded replay regressions in `crates/cli/tests/replay_match.
rs`, 1 pure-math unit test in `src/fighter/characters/fox.rs` -- plus 17
C-oracle differential tests in `tests/fox_down_special_differential.rs`
(six of them `proptest` cases run 256 times each, covering `ftCommon_
8007CF58`'s over-drift-maximum branch both through the air Phys wrapper
and through its own standalone extraction, `air_drift_recovery`).

Fox/Falco's down special (Reflector, `docs/fox-down-special.md`) is
implemented as `src/game/characters/fox/down.rs`: the five-phase Start/
Loop/Turn/Hit/End state machine (Slippi 360..369), the ground-fourth/air-
directional fresh entry, the mid-move turn (reusing `locomotion::
Parameters::turn_threshold`, distinct from the side special's own entry-
turn `x220`), the ground jump cancel and aerial jump (reusing the existing
`locomotion::jump_input`/`try_aerial_jump`), the platform drop (a new
`collision::begin_pass_as`, generalizing `begin_pass` to a caller-supplied
destination and frame-preserving entry), every phase's ground/air
conversion, and the `reflecting` bit (piggybacked on the existing
`fighter.shield.reflecting` field, since the source stores both the
powershield's and this move's reflect state in the same bit). A decomp
fact that contradicted the side-special-derived assumption: none of this
move's five *grounded* Phys callbacks touch `gravityDelay` (only the air
ones do), so `down.rs` has no `tick_ground_timers` override. Loop's
genuinely indefinite `Ft_MF_KeepGfx` duration surfaced a pre-existing
engine limitation (a fixed-length pose resource with no wrap/hold
mechanism for an unboundedly-held action); fixed by extending the existing
`locomotion::hold_action_frame` (already used by RunTurn/RunBrake) with a
`down_special.looping` flag. The Hit phase (reachable in the source only
through the unmodeled projectile-reflect callback) is fully implemented
and exercised by the C-oracle differential tests and a native wiring
check, but unreachable through ordinary play; the reflect bubble's own
geometry has no effect in this engine, only the `reflecting` bit is
observable; `xA0_FOX_REFLECTOR_UNK1` is never read anywhere this batch
cites. `ftCommon_8007CF58` (the common aerial drift/over-drift-maximum-
recovery function every air phase's Phys calls) is fully modeled, both
branches, as `game::specials::helpers::drift_or_friction_air`/
`gravity_delayed_fall_with_drift`: Fox's `air_drift_max` sits below his run
speed, so a jump out of a run, or `Fall` from the side special's own air
End, can carry an aerial Reflector's entry velocity above the maximum even
after the `xA8` division, making the over-drift branch genuinely
reachable. Its common step coefficient (`ftCommonData.x1FC`) is exposed as
a new `characters::fox::side::Rules::air_drift_recovery_step` field (the
same shared `ftCommonData` struct as `x218`/`x21C`/`x220`), read by the
down special even though the side special's own air phases never touch it.
`tests/oracle/original/ftfoxspeciallw.c` pins the whole decomp file
(`ftfoxspeciallw.functions.json` extracts all 72 of its own function
definitions -- every non-static callback plus every `static`/`static
inline` helper); its own necessarily-duplicated copy of `ftCommon_8007CF58`
(cross-file, so it cannot literally be extracted into this adapter, see
that file's own header) is independently pinned verbatim by a new small
adapter, `tests/oracle/air_drift_recovery.c` (aliased to "ftcommon",
extracting the real function from the same pinned `ftcommon.c` snapshot),
whose own differential tests compare it bit-exactly against both branches.
See `docs/fox-down-special.md`'s "C-oracle coverage" section for exactly
what the differential suite compares. `src/game/validation.rs`'s existing
side-special/`rules.specials` pairing check is relaxed: `rules.specials`
(`characters::fox::side::Rules`) is shared common data the down special's
own aerial entry also reads (`vertical_threshold`, `air_drift_recovery_
step`), so a fighter with only a down-special resource and no side-special
one is no longer an error.

The 2026-09-11 specials-framework refactor batch is recorded at:

`/mnt/archive/runs/skirmish-specials-framework-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace (both without and with the `c-oracle` feature) and the
`fox_side_special_differential` release build, all green with unchanged
test counts against the pre-refactor baseline recorded at the start of this
batch: 809 passed/0 failed workspace-wide without `c-oracle`, 1108 passed/0
failed with it (both counts identical before and after). This is a
behaviour-preserving modularity refactor, not a new-coverage batch: no
`Action` variant, resource field semantics, or per-frame arithmetic changed
meaning; only module boundaries, type paths and one resource field's shape
did (below). Where a test's own field access or import path no longer
compiled after the rename, the test was updated to the new path with no
change to what it asserts; no test's expected values, scenario or assertion
changed.

`src/game/special.rs` (the neutral-special shell) and `src/game/
fox_side_special.rs` (Fox/Falco's side special, `docs/fox-side-special.md`)
are replaced by a shared framework so that a queued future special (Fox has
three more; 25 more characters follow) touches only its own file, its
character's registry entry, and the observation id table -- never
`simulation`, `collision`, `edge`, `ledge` or another move's file.
`src/game/specials/mod.rs` holds the `SpecialMove` trait (the phase hooks:
`owns`, `attack`, `update_actions`, `update_animation`,
`ground_target_velocity`, `ground_friction_override`, `air_physics`,
`tick_ground_timers`, `transfer_ground_air`, `land`, `collision_mode`,
`ledge_catchable`) and the one shared dispatcher -- the grounded/aerial
entry-eligibility chains previously duplicated between the neutral shell
and the side special -- that calls every registered move in priority order.
`src/game/specials/helpers.rs` holds the phase behaviours reused across
moves (gravity-delayed fall, pose-driven ground/air velocity, frame-
preserving ground/air conversion, the `FallSpecial`/landing-fall-special
exits, restoring every jump, the ordinary Wait/Fall exit) so a future move
does not re-derive them. `src/game/specials/neutral.rs` re-expresses the
former `special.rs` shell as a `SpecialMove` impl with no behaviour change.
`src/game/characters/mod.rs` is the registry: `Specials` (the per-character,
per-move resource enum replacing the old flat `FighterData.special`/
`side_special` fields -- see below), `moves()` (a fighter's own enabled
moves in dispatch order) and `slippi_ids()` (the observation layer's single
entry point for a character's state/animation ids).
`src/game/characters/fox/{mod.rs,side.rs}` hold Fox's registry entry
(`MOVES = [side, neutral]`, matching the source's own before-neutral
ordering) and the former `fox_side_special.rs` content re-expressed as a
`SpecialMove` impl, with its dispatch/physics logic now calling the shared
helpers above instead of a local copy. `src/fighter/characters/fox.rs`
(moved from `src/fighter/fox_side_special.rs`, unit tests included) holds
the unchanged pure arithmetic (`has_input`, `should_turn`,
`entry_ground_velocity`). `simulation.rs`, `collision.rs`, `edge.rs` and
`ledge.rs` now call only the shared `specials::` entry points
(`update_animation`, `update_actions`, `ground_target_velocity`,
`ground_friction_override`, `air_physics`, `tick_ground_timers`,
`transfer_ground_air`, `land`, `collision_mode`, `ledge_catchable`) instead
of matching per-move `Action` variants or calling a specific move's module
directly.

Resource shape: `FighterData.special: Option<special::Parameters>` and
`FighterData.side_special: Option<fox_side_special::SideSpecial>` are
replaced by `FighterData.specials: Option<characters::Specials>`, an enum
tagged by character (`#[serde(tag = "character")]`); today's only variant,
`Specials::Fox { neutral: Option<specials::neutral::Parameters>, side:
Option<characters::fox::side::SideSpecial> }`, round-trips the exact same
JSON shape as before, just nested one level deeper under `"character":
"Fox"`. `Rules.specials` keeps its name and shared-rules role, retyped from
`fox_side_special::Rules` to `characters::fox::side::Rules`.
`Fighter.fox_side_special: characters::fox::side::State` keeps its field
name (deliberately not folded into a per-character enum like the resource
side, since it is the only piece of runtime specials state Fox needs today
and a rename would only have touched test call sites for no behavioural
reason); its type is retargeted to the moved module. `tests/support/
special.rs`, `tests/support/fox_side_special.rs`, `tests/game_special.rs`
and `tests/game_fox_side_special.rs` are updated to the new field/type
paths; `tests/fox_side_special_differential.rs` and `crates/skirmish-replay/
src/observation.rs` are updated to the new import paths. No fixture's JSON
content changed (`tests/fixtures/game/fox-side-special.json` deserializes
through the same `SideSpecial`/`Rules` shape it always did, via the test
support module's own wrapper struct, independent of `FighterData`'s field
layout).

`docs/architecture.md` records the new module layout and the "touches only
its own file, its registry entry and the id table" rule for new moves;
`docs/specials.md` and `docs/fox-side-special.md` are updated to the new
paths with no behavioural claim changed.

The 2026-09-11 real-replay comparison tooling batch is recorded at:

`/mnt/archive/runs/skirmish-real-parity-tooling-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, and the complete
native workspace (without and with the `c-oracle` feature, debug and release)
at 814/1113/1113 passed and 19 ignored, unchanged failure-wise from before
this batch: every addition here is new CLI/replay-crate/workflow/doc
plumbing, not a `src/game/**` change (owned by a parallel refactor task).

`skirmish-cli make-initialization --match-data <MatchData> --replay <.slp>
--output <Initialization>` derives ports, stage, characters, stocks and seed
from a replay's own `GameStart` instead of requiring them by hand
(`crates/cli/src/initialization.rs`): ports are the two occupied ports sorted
ascending; fighter/stage names are matched against public Slippi/CSS
external-ID tables and rejected with a clear `character mismatch`/`stage
mismatch` error on disagreement (checked in that order); each port's starting
stocks must equal `rules.stocks`; the seed is `GameStart.random_seed` unless
`--seed` overrides it (Peppi decodes that field unconditionally for every
version the importer accepts, so the override exists for a hypothetical
future format, not any file accepted today); and `warmup` stays `[]`,
refusing outright unless the replay's first selected frame is already `-123`,
since no warmup-stepping is implemented. Three unit tests cover it: the
committed real `tests/fixtures/slippi/parity/fox-fd.slp` against the
wholly-synthetic `integration-match.json` fixture (expects the character
mismatch), the same replay against that fixture patched to claim Fox/Final
Destination (expects a successful build with the replay's actual ports, seed
`3778252302` and frame `-123`), and the kebab-case slug helper. `validate-
replay` gained `--report <path>`, writing the exact JSON also printed to
stdout so a mismatch/error/matched outcome survives independent of captured
process output; covered by extending the existing late-difference CLI test
plus a new end-to-end test that chains both commands over a self-recorded
synthetic Fox-vs-Fox Battlefield replay and asserts a matched report.

`tests/fixtures/slippi/parity/fox-fd.slp` (1,084,706 bytes, sha256
`87971fc4608e577fe5a814fc3a04dee4c0d82f99bc9a54ee16797005c5a9085e`) is a real,
CC0-1.0 `erickfm/slippi-public-dataset-v3.7` recording (Slippi 2.0.1, stage
32, ports P1/P4, both Fox, 4 stocks each, seed `3778252302`, 4673 frames,
first/last frame -123/4549), copied unmodified with its own
`tests/fixtures/slippi/parity/manifest.json`. `crates/cli/tests/
real_parity.rs` builds an initialization from it plus `<SKIRMISH_GAMEPLAY_
DATA>/fox-fd/match-data.json`, runs the comparison, prints the report, and
ratchets `first_divergent_frame` against `fox-fd-baseline.json`
(currently the replay's own first frame, `-123`, the worst possible result,
so it never blocks until a reviewer tightens it after a real run); when the
env var or that file is absent it prints a skip message and passes, with no
`#[ignore]`, so the skip path itself runs in CI. The gameplay export does not
exist yet (see `docs/gameplay-export.md`); this batch verified both of
`make-initialization`'s paths against two throwaway stand-in directories
built from the synthetic fixture (one unpatched, producing the expected
character-mismatch failure; one patched to Fox/Final-Destination ids,
producing a `mismatch` report at frame -123 that still satisfies the trivial
baseline), neither committed.

`.github/workflows/system-tests.yml` (renamed from "System tests (Slippi file
parity)" to "System tests (Slippi replays)") adds a step, gated on `secrets.
HF_TOKEN` being set, that downloads `gameplay/<version>/<file>` from
`https://huggingface.co/datasets/cornerian/skirmish-datapacks`, verifies its
sha256/size against `tests/fixtures/slippi/parity/gameplay-export.lock.json`,
extracts it and exports `SKIRMISH_GAMEPLAY_DATA`; the step prints a notice and
skips while the lock's `sha256` is still the committed placeholder
`"pending"`. `real_parity` was added to the three system-test commands and to
`integration-tests.yml`'s `system_targets` set (the discovery assertion still
passes: 143 test targets, no name collisions). `tools/package_gameplay_
export.py` (stdlib-only, `uv run --no-project`) packages a local export
directory into that tarball and prints the lock JSON for the reviewer to
publish; `.tar.gz` was chosen over `.tar.zst` specifically so the script needs
no non-stdlib dependency.

Self-recorded replay regression wording (`crates/cli/tests/replay_match.rs`'s
doc comment and test names, `docs/replays.md`, `docs/testing.md`, the system
workflow's name/steps/comments) now says "self-recorded" rather than
"parity", to stop describing harness self-consistency as Melee evidence. The
new `docs/parity.md` lays out the three verification levels this repository
actually has -- function-level C-oracle equivalence, self-recorded replay
regression, and real-replay comparison with the ratchet -- and states plainly
what each does and does not prove; earlier validation entries that already
used "parity" for unrelated named profiles (e.g. `docs/state-parity.md`) were
left untouched, since this rename is about mislabeled self-recorded evidence,
not the word itself.

The 2026-09-11 Fox side-special coverage batch is recorded at:

`/mnt/archive/runs/skirmish-fox-side-special-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace (both without and with the `c-oracle` feature), and the
new original-C functions in debug and release modes. `game::fox_side_
special` (resource, dispatch, `src/game/fox_side_special.rs`) and `fighter::
fox_side_special` (the pure input gate, turn check and entry ground-speed
blend, `src/fighter/fox_side_special.rs`) cover `Action::SpecialSStart/
SpecialS/SpecialSEnd/SpecialAirSStart/SpecialAirS/SpecialAirSEnd`
(`docs/fox-side-special.md`), Fox's side special (Illusion; Falco's
Phantasm shares the code with its own attributes).

Two concrete facts requested for this batch: the ghost item (`itfoxillusion.
c`, `it_3F2F.c:360-390`) is confirmed hitbox-free -- every one of its three
`Coll` callbacks unconditionally returns `false`, and its `DmgDealt` slot
only clears a bookkeeping field it can never reach -- so it stays entirely
unmodeled as GFX; and the Slippi ids are confirmed directly against `ftFox/
forward.h`'s own enum order (347..352), while the matching animation
indices (301..306) remain an unverified extrapolation, since the pinned C
decomp has no figatree/animation-index table for character-specific motion
states (only DAT-resource data, owned by the separate `skirmish-assets`
project, would confirm it) -- flagged in code, in `docs/fox-side-special.md`
and here rather than silently assumed.

This batch's own preceding design note (`docs/fox-side-special.md`'s
predecessor) was found to be wrong in two places while implementing
against the pinned source, both corrected with exact citations before any
code was written against them: `doEnter`'s ground-velocity blend is scaled
by `ft_GetGroundFrictionMultiplier(fp)` (`ft_081B.c:1235-1240`, a per-
floor-material lookup), not a fixed `1.0` -- this port still uses `1.0`
(no per-surface friction-material table exists anywhere in this codebase),
a documented simplification, exact on ordinary terrain and exercised
directly by `fox_side_special_differential.rs`'s `entry_multiplier_gap_
is_the_documented_one`; and the End phase's ground/air conversions are
*not* symmetric with Start/Dash (ground leaving the floor enters ordinary
Fall; air landing enters `LandingFallSpecial` directly via `ftCo_
LandingFallSpecial_Enter`, bypassing SpecialSEnd/SpecialAirSEnd entirely),
not a uniform GroundToAir/AirToGround pair. A third, implementation-only
gap was caught by this batch's own test suite before being recorded here:
the Start/End grounded phases tick `mv.fx.SpecialS.gravityDelay` down
every frame even though gravity is never read on the ground (`ftFx_
SpecialSStart_Phys`/`ftFx_SpecialSEnd_Phys`); an early draft only ticked it
in the air, which would have desynced the counter across a mid-Start
ground<->air conversion. No contradiction between this batch's final
implementation and the pinned source was found; the pre-existing test
suite remains green unchanged.

Resource shape: `Rules.specials: Option<fox_side_special::Rules {
side_stick_threshold (x218), turn_threshold (x220), vertical_threshold
(x21C) }>`; `FighterData.side_special: Option<fox_side_special::
SideSpecial { ground_speed_retention, start, dash (with per-pose ground/
air TransN), end, attributes: 12 fields covering the gravity delays,
speeds and frictions of all three phases }>`; `Fighter.fox_side_special:
State { gravity_delay: f32 }`; `aerial::State` gained `mobility: f32`
(default `1.0`, recovering every previously modeled `FallSpecial` entry's
exact prior behavior); `locomotion::State` gained `side_special_b_age: u8`
(`x688`, distinct from the existing `attack_b_age`/x67D, since `x688`
additionally requires the stick past the side threshold);
`edge::mode_for_action` gained the End-ground clamp; `ledge::catchable`
gained the three aerial phases.

`src/fighter/fox_side_special.rs` adds 3 unit tests. `tests/
game_fox_side_special.rs` adds 16 integration tests covering grounded and
aerial entry (retention blend, entry-speed division, the `x688` age gate,
jumps left untouched or restored), the strict turn-around boundary, a
vertical stick suppressing the unmodeled aerial Hi/Lw branches, the ground
and air TransN-driven dash (including the no-sample friction fallback),
B-press shortening on the ground and in the air, a mid-Start ground<->air
conversion preserving the frame and the gravity delay, the End phase's
speeds and dedicated frictions, the FallSpecial exit with the scaled
mobility and every jump restored, the Slippi ids, a full-phase checkpoint
round trip, invalid resources and `None` keeping B+side inert.
`tests/oracle/original/fox_specials.c` snapshots `ftfoxspecials.c`
(`fox_specials.functions.json`: 42 of its 43 functions -- every non-static
callback plus all four `static inline` helpers except `ftFx_
SpecialS_CreateGFX`, hand-written as a no-op since its real body needs the
full `ftParts` system and it is never invoked by anything this oracle
calls); `tests/oracle/original/special_s.c`/`special_air.c` snapshot
`ftCo_SpecialS.c`/`ftCo_SpecialAir.c` in full, with the character tables
stubbed to record which entry fired. `tests/oracle/fox_specials.c` treats
`Fighter_ChangeMotionState`/`ftCo_80096900`/`ftCo_LandingFallSpecial_
Enter`/`ftCo_Fall_Enter`/`ft_8008A2BC` as captured; `ftAnim_
IsFramesRemaining`/`ft_80082708`/`ft_800827A0`/`ft_CheckGroundAndLedge`/
`ftCliffCommon_80081298`/`ft_GetGroundFrictionMultiplier` as scripted;
`ftCommon_Fall`/`ApplyFrictionAir`/`ApplyFrictionGround`/`8007D60C`/
`8007D6A4`/`8007D7FC`/`AirToGroundStateChange`/`UseAllJumps`/`ft_
80084F3C`/`ft_800850E0`/`ft_80085088`/`ft_80085134` as faithful verbatim
reimplementations against this file's own `Fighter` struct (not linked
against `physics.c`/`locomotion.c`'s own extractions of the same
functions, to avoid an unchecked cross-adapter struct-offset assumption);
`ftCommon_ApplyGroundMovement` and every ghost/GFX call as documented
no-ops. `tests/fox_side_special_differential.rs` (13 tests, 512 proptest
cases each plus exact boundaries) compares entry selection, per-phase
speed/gravity-delay arithmetic, TransN velocities, the B-press IASA
shortening and the Coll conversion/exit decisions against the C oracle,
plus the documented friction-multiplier gap exercised directly; all pass
in both debug and release. `crates/cli/tests/replay_match.rs` gains 2
file-backed regressions: a grounded Fox Illusion through SpecialSStart/
SpecialS/SpecialSEnd (Slippi 347/348/349) and an aerial one free-falling
through SpecialAirSStart/SpecialAirS/SpecialAirSEnd/FallSpecial into
LandingFallSpecial (Slippi 43), each matching its own generated bytes and
reporting a `Mismatch` at frame 0 once the entry press is removed.

The 2026-09-11 taunt coverage batch is recorded at:

`/mnt/archive/runs/skirmish-taunt-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace (both without and with the `c-oracle` feature), and the
new/extended original-C functions in debug and release modes. `game::taunt`
(wiring: the resource, the entry dispatch across every chain that lists
it, the animation/physics/collision callbacks, `src/game/taunt.rs`) and
`fighter::taunt` (the pure D-pad-up press check and facing/availability
side selection, `src/fighter/taunt.rs`) cover `Action::AppealSR`/
`AppealSL` (`docs/taunt.md`). D-pad input: `game::BUTTON_DPAD_LEFT/RIGHT/
DOWN/UP` (`0x1`/`0x2`/`0x4`/`0x8`, the native `HSD_Pad` low nibble); the
replay importer's `BUTTONS` mask and `game::validation::inputs`'s own mask
both accept all four, so a replay containing a D-pad left/right/down press
now imports, while only D-pad up has any observable effect. Taunt's D-pad-
up press is checked once, in `simulation::update_actions` right after the
shield check and before the jump dispatch, for every chain the pinned
source's own `ftCo_800DE9D8` reaches (Wait, Walk, Squat, SquatWait,
SquatRv, Turn, Landing's and AttackS4's interruptible frames, the down
tilt's interruptible frames, and Ottotto/OttottoWait), plus a matching
check inside `dash::update_dash_or_run`'s shared `block_42` tail for both
Dash (falling through into its own `x54` friction step, matching the
pinned source's fall-through-instead-of-return contract) and Run (no such
tail). Entry picks AppealSL when facing left and a left motion is
supplied, else AppealSR; `allow_interrupt` starts false and is governed by
the supplied pose's own flags from then on, exposing (`tilt::Chain::
Taunt`, treated as `Chain::Wait` by shield/catch/special dispatch but
excluded from locomotion's jump/dash/squat/turn/walk arm) specials, catch,
smashes, tilts, jab, the Wait-chain spot dodge and shield -- never jump,
dash, squat, turn or walk. Physics apply root motion when a pose supplies
it, else ordinary ground friction; collision uses the existing mode-2
clamp (`edge::mode_for_action`). The same batch corrects the Wait (and
interruptible AppealS) input chain: `game::escape::try_wait_chain_spot_
dodge` (`ftCo_80099794`, held logical shoulder AND a fresh downward main
stick only -- no C-stick alternative, unlike the existing guard-IASA spot
dodge) now runs before the ordinary shield check, so a shoulder pressed
while the stick is already down enters EscapeN on that exact frame
instead of raising GuardOn first and dodging a frame later; Walk's own
chain has no such call and is unaffected.

No contradiction between this batch's implementation and the pinned source
was found; the pre-existing test suite remains green unchanged (one
existing unit test and one existing integration test that used D-pad bit
`0x1` as an example of an unsupported button were updated to use a still-
unsupported bit instead, with the reason cited in each).

`src/fighter/taunt.rs` adds 2 unit tests for `pressed`'s exact-bit check
and `select_side`'s exact `facing == -1.0` comparison. `tests/game_taunt.rs`
(12 tests, `tests/support/taunt.rs` supplies an invented right/left motion
pair) covers every chain position, facing-based side selection, D-pad-bit
inertness besides up, the taunt's own interruptible chain (catch, smash,
tilt, jab, the Wait-chain spot dodge, shield; excluding jump, a dash stick
alone and a downward stick without the shoulder), root motion ending into
Wait, floor-end clamping, the Wait-chain spot dodge's own cases (including
Walk's exemption), checkpoints and invalid resources. `tests/oracle/
original/appeal.c` snapshots `ftCo_AppealS.c` for the first time
(`appeal.functions.json`: `ftCo_800DE9B8`/`ftCo_800DE9D8`/`ftCo_800DEAE8`/
`ftCo_800DEBD0`/`ftCo_AppealS_IASA`; `ftCo_800DEA28` is hand-written in the
host adapter instead, since it is outside this batch's selection, with
every per-character side effect stubbed and `kind` fixed at 0). `tests/
oracle/escape.functions.json`/`escape.c` (the existing pinned `ftCo_
Escape.c` snapshot) gain `ftCo_80099794` and `oracle_wait_spot_dodge`.
`tests/taunt_differential.rs` (4 tests, 512 proptest cases each plus exact
boundaries) compares entry selection, the complete `ftCo_AppealS_IASA`
consultation order (a pure Rust mirror of its 14-check chain) and the
Wait-chain spot dodge predicate against the C oracle; all pass in both
debug and release. `crates/cli/tests/replay_match.rs` gains 5 file-backed
regressions: a D-pad-up taunt from Wait reporting Slippi state/animation
264/239, the same facing left with a left motion supplied reporting
265/240, the Wait-chain spot dodge reporting state 235 on the L-press
frame (each matching its own generated bytes and reporting a `Mismatch`
at the row the removed input's effect first appears), and a replay
containing an inert D-pad-down press importing successfully.

The 2026-09-11 idle-animation coverage batch is recorded at:

`/mnt/archive/runs/skirmish-idle-animations-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and
release modes. `game::idle` (wiring: the resource, the state, the per-frame
animation-phase draw, `src/game/idle.rs`) and `fighter::idle` (the pure
weighted pick and its re-draw gate, `src/fighter/idle.rs`) cover
`Action::Wait`'s idle-animation cycling (`docs/idle.md`). Two new resources:
`FighterData.idle: Option<idle::IdleAnimations { wait1_length: f32, entries:
Vec<IdleEntry { animation: u32, weight: i32, length: f32 }> }>` and
`Fighter.idle: idle::State { animation: u32, frame: f32 }` (reset to `{ 2,
0.0 }` on every action transition, unconditionally like `dash::State`/
`smash::State`, since both fields are read only while `Action::Wait` is
current). Every Wait frame, once the current sub-motion's own tracked frame
reaches its length: with an empty (or absent) table, restart with no RNG
draw; otherwise draw from the match's shared `HsdRng` and walk the weighted
table (`fighter::idle::pick`), re-drawing while a repeated pick is neither
Wait1_0 (2) nor a fresh pick from Wait1_0/31. The draw happens in
`simulation::advance`'s per-player animation-phase loop, in player order,
which reassigns `state.rng_seed` before the same frame's blast-zone death
draw reconstructs its own `HsdRng` from it -- so an idle draw always shifts
a later death draw's position in the shared sequence, exactly as Melee's
single per-frame RNG stream would. `crates/skirmish-replay/src/
observation.rs`'s `animation_index` gains a dedicated `14 => fighter.idle.
animation` arm (previously `12..=14 => 2` covered Rebirth/RebirthWait/Wait
alike with the same constant; Rebirth/RebirthWait are out of this batch's
scope and keep the constant). `action_age` needed no change, since Wait was
already covered by `observe`'s catch-all `action_frame` branch and this
batch's own `action_frame` reset on every restart/pick keeps that branch
correct. With the resource absent, `Action::Wait` keeps its pre-batch
behavior unchanged: `action_frame` grows without bound and the reported
animation index stays 2.

No contradiction between this batch's implementation and the pinned source
was found; the pre-existing test suite remains green unchanged. This
batch's own explicit modeling choices, documented in `docs/idle.md` rather
than treated as verified source behavior: `ftAnim_IsFramesRemaining`'s real
joint-based check (`ftanim.c:515-530`) is replaced by a tracked-frame/
length comparison, not independently reverse-verified against `ftAnim`/
`lbAnim`'s own bookkeeping (the same caveat the walk/run batches recorded
for their own frame-length wrap rules); every character's Wait entry is
assumed to set `fp->anim_id` to 2 (Wait1_0) via `Fighter_ChangeMotionState`'s
own per-character table lookup (`fighter.c:1220`), since that table itself
is compiled character data, not part of the decomp source available here.
Item-holding, the Mewtwo/Fox exception (`ftwaitanim.c:66-69`) and Wait
script bytecode execution are pre-existing gaps (no `Fighter` item-holding
state or script-bytecode execution exists anywhere in this codebase) left
unmodeled, not introduced or silently changed by this batch.

`src/fighter/idle.rs` adds 5 unit tests for `pick`'s boundary selection,
the Wait1_0/31 exemption, the non-exempt re-draw and the "table falls short
of `max`" panic. `tests/game_idle.rs` (7 tests, `tests/support/idle.rs`
supplies an invented two-entry table) covers: a table-less restart at
`wait1_length` leaving the animation at 2, resetting the tracked frame and
`action_frame`, and drawing no RNG; a two-entry table's pick matching
`pick` called directly against a fresh `HsdRng` from the same match seed;
a repeated pick from a non-Wait1 current animation re-drawing, with the
seed advancing by two draws for that event; the idle draw shifting a
same-frame blast-death roll's `Kind` between resource-present and
resource-absent runs of an otherwise-identical real hit-driven scenario (a
grounded fighter can never itself exceed a valid `blast.top`, since stage
validation requires `floor.y < blast.top` strictly, so this reuses `tests/
support/death.rs`'s own proven hit/knockback shape rather than a synthetic
position); checkpoints restoring the idle state and seed exactly across a
pick; invalid resources (non-finite/non-positive `wait1_length` or entry
fields, weights summing below 100) rejected before a match exists; and
`None` keeping the pre-batch unbounded integer `action_frame` with the
animation index fixed at 2. `tests/idle_differential.rs` (8 tests, 512
proptest cases for the pick comparison) pins `ftCo_8008A698`/
`ftCo_8008A6D8`/`inlineA0`/`getAnimID`/`ftCo_8008A7A8` from a new
`waitanim` snapshot of `ftwaitanim.c`; `fighter::idle::pick` is compared
against the C oracle over equal-share-weight tables of 1..=5 entries,
current animations in `{2, 3, 6, 31, 40}` and 64-value scripted `HSD_Randi`
sequences, plus exact boundaries (`max == count` inclusive selection, the
repeat re-draw in both implementations, Wait1_0/31 never re-drawing,
weights summing below 100 asserting in the oracle and panicking in `pick`,
frames still remaining, and an empty table restarting without drawing).
The oracle adapter's `HSD_ASSERTREPORT` override `longjmp`s back to the
call site rather than falling through a failed table walk, reproducing the
pinned source's own "never returns" contract (the real `__assert` it calls
is `ATTRIBUTE_NORETURN`, and `getAnimID` has no `return` after its own
`HSD_ASSERTREPORT` call) -- an earlier version of the adapter let execution
fall through instead, reading an undefined return value that the caller
then used as an out-of-bounds `fp->x24`/`x28` array index and crashed the
test binary; this was found and fixed during this batch's own
implementation, not left as a discrepancy. `crates/cli/tests/
replay_match.rs`'s `file_backed_idle_fighter_reports_the_picked_animation_
index_with_a_restarting_age` (1 test) drives an idle fighter with the
two-entry table, locates the restart/pick row from a trace of the tracked
idle frame, confirms the reported animation index and restarted
`action_frame` there, matches its own generated Peppi bytes end to end,
then corrupts that row's animation-index field and confirms a `Mismatch`
starting there.

The 2026-09-11 run-corrections batch is recorded at:

`/mnt/archive/runs/skirmish-run-corrections-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. This batch fixes three behaviors the run-animation-rate batch below
audited and reported as discrepancies/gaps rather than silently changing,
out of that batch's stated scope; no further contradiction against the
pinned source was found while fixing them (`docs/run.md`'s "Corrections"
section has the full detail with source line citations).

RunTurn's flip check (`game::locomotion::update_animation`'s `Action::
RunTurn` arm) now reads `f.locomotion.run_turn_facing` -- the per-entry
facing `start_run_turn` already captured for the physics branch -- instead
of the removed `Parameters::run_turn_velocity_scale` fixed resource
constant, matching `ftCo_TurnRun.c:67`'s `facing_at_entry * gr_vel <=
0.01F` exactly. `run_turn_velocity_scale` is removed from `Parameters`
(`deny_unknown_fields` required dropping it from every fixture).

RunBrake's velocity-gated marker freeze (`ftCo_RunBrake.c:49-77`) is now
modeled: `Parameters` gains `run_brake_marker_frame: Option<u32>` and
`run_brake_freeze_speed: Option<f32>` (paired; `None` keeps the pre-batch
unfrozen behavior), `locomotion::State` gains `run_brake_frozen: bool`, and
`hold_action_frame` now also freezes `action_frame` while RunBrake is
frozen, alongside its existing RunTurn case. The `frames` countdown keeps
ticking regardless and can still end the brake into `Wait` while frozen.

`run.x0`, the turn-run lockout gating `ftCo_Run_IASA`'s RunTurn/RunBrake
entry (`ftCo_Run.c:125-126`), is now modeled: `Parameters` gains
`run_turn_lockout_frames: Option<f32>` (`x430`), `locomotion::State` gains
`run_lockout: f32`, set by `enter_run`'s new `lockout` parameter (`0.0`
from both Dash-to-Run call sites, `run_turn_lockout_frames.unwrap_or(0.0)`
from a RunTurn-to-Run re-entry) and counted down every Run animation frame
independent of `MovementData.run_animation`. Both `game::locomotion::
update_actions`'s `Action::Run` arm and `game::dash::update_dash_or_run`'s
Run arm now gate their RunTurn/RunBrake entry checks behind `run_lockout <=
0.0`, matching `ftCo_Run_IASA`'s own gate order.

`tests/game_run_corrections.rs` (9 tests) covers: a right-facing and a
left-facing run-turn reversal each flipping only once ground velocity has
crossed past the *entry* facing's own sign, with the frozen frames holding
`action_frame`; the RunBrake freeze holding frames while fast and resuming
once slow with the countdown still ticking; the freeze speed never reached
still ending the brake on the countdown while frozen; the marker absent
keeping the pre-batch unfrozen behavior; the lockout blocking both RunTurn
and RunBrake entry from Run for exactly its frame count (a neutral and a
reversed stick, branched from the same pre-lockout checkpoint via a cloned
`Match`); an ordinary Dash-to-Run entry never setting the lockout;
checkpoints mid-lockout; and every new field's invalid/unpaired/
out-of-range values rejected before a `Match` exists.
`tests/turn_run_anim_differential.rs` (7 tests, 512+512 proptest cases)
extends the pinned `turn_run.c` snapshot (already used for
`ftCo_TurnRun_Phys`) with `ftCo_TurnRun_Enter`/`_Anim`/`fn_800C9CEC`/
`fn_800C9D40`, comparing a Rust mirror of the freeze/resume/flip/
animation-end decision against the C oracle bit-exactly, including a
generated 8-frame velocity sequence per fixed facing threaded frame to
frame. `tests/runbrake_differential.rs` (5 tests, 512+512 proptest cases)
snapshots `ftCo_RunBrake.c` for the first time
(`tests/oracle/original/runbrake.c`, `ftCo_RunBrake_Anim`/`_IASA`),
comparing a Rust mirror of the freeze/resume/end decision against the C
oracle, including a generated 12-frame, physically realistic
(monotonically non-increasing magnitude) velocity sequence, plus every
boolean combination of the IASA gate order. `tests/run_differential.rs`
gains `run_iasa_lockout_gate_order_matches_c` (extending `ftCo_Run_IASA`
into the pinned `run.c` snapshot), proving the jump check always runs
first and the RunTurn/RunBrake checks only run while `run.x0 <= 0.0`, over
every scripted boolean combination and `run_x0` in `{-1, 0, 1, 5, NaN}`.

The 2026-09-11 run-animation-rate coverage batch is recorded at:

`/mnt/archive/runs/skirmish-run-animation-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. `game::locomotion`/`fighter::locomotion` extend the walk batch's float
animation-frame model (`docs/walk.md`) to `Action::Run` (`docs/run.md`) with
one new resource, `MovementData.run_animation: Option<RunAnimation { length,
scaling }>` (the Run figatree's frame count and `run_animation_scaling`,
`types.h:698`) -- unlike Walk, not paired with any `Rules` entry, since Run
has no kind-selection thresholds. `game::locomotion::State.run: RunState
{ frame, last_rate }` tracks a float animation frame that advances one frame
behind `ftAnim_SetAnimRate`'s own delay and wraps at the Run figatree's
length; both of this codebase's reachable Run entries (Dash-to-Run and
RunTurn-to-Run, via the new `game::locomotion::enter_run`) always start it at
frame 0.0 with `last_rate` 1.0, matching the pinned source's fixed
`ChangeMotionState` rate argument. Slippi's `action_state`/`animation_index`
for Run were already correct (21/13, unchanged); `action_age` now publishes
the tracked float frame instead of the integer `action_frame` while the
resource is present. With the resource absent, Run keeps its pre-batch
behavior unchanged: state 21 with an integer `action_frame` published as
`state_age`, rate 1 always.

Two discrepancies against the pinned source were found and reported rather
than silently fixed (both out of this batch's scope, the Run animation-
frame/rate layer only; documented in full in `docs/run.md`): RunBrake's own
velocity-gated marker freeze (`cmd_vars[1]`/`x42C`, `ftCo_RunBrake.c:49-77`)
is not modeled anywhere in this codebase, contrary to an earlier design
draft's assumption that it was -- only the unrelated `mv.co.runbrake.frames`
countdown exists; and `game::locomotion`'s existing RunTurn flip check
(`update_animation`'s `Action::RunTurn` arm) multiplies by a fixed
`Parameters::run_turn_velocity_scale` resource constant instead of the
per-entry facing the pinned source actually uses (`ftCo_TurnRun.c:67`,
`facing_at_entry * gr_vel <= 0.01F`, sourced from `ftCo_TurnRun_Enter`'s own
`turnrun.accel_mul = facing_dir`) -- `game::locomotion::start_run_turn`
already captures that exact per-entry value as `run_turn_facing` for the
physics branch, but the flip check does not use it. [Corrected by the
2026-09-11 run-corrections batch above: the flip check now reads
`run_turn_facing` and `run_turn_velocity_scale` is removed.] Both are
pre-existing gaps in already-audited code, left unchanged per this batch's
"verify, don't silently change" instruction. [RunBrake's own freeze is also
corrected by the run-corrections batch above.] Separately, `run.x0` (the
turn-run lockout countdown gating `ftCo_Run_IASA`'s RunTurn/RunBrake entry,
`ftCo_Run.c:125-126`) remains unmodeled, as it was before this batch --
outside this batch's resource/state shape, which covers only the
animation-frame/rate layer. [Also corrected by the run-corrections batch
above.]

This batch's own explicit modeling choices, documented in `docs/run.md`
rather than treated as verified source behavior: `last_rate` is initialized
to 1.0 at Run entry (mirroring `Fighter_ChangeMotionState`'s own `rate = 1`
argument, the same choice the walk batch made for Walk), and the `frame >=
length` wrap rule (not independently reverse-verified against `ftAnim`/
`lbAnim`'s own loop bookkeeping, the same caveat the walk batch recorded).
`run_animation_rate` is not a call into the existing `walk_animation_rate`
(identical branch shape/order, but Run has a single figatree/scaling
constant rather than three kind-indexed ones, so a shared call would need a
fabricated `rates` array and an unused kind axis); this is noted rather than
treated as an unexplained duplication.

`src/fighter/locomotion.rs` adds 1 unit test for the pure `run_animation_rate`
zero-rate/facing/`x4`-branch selection, mirroring `walk_animation_rate`'s own
test. `tests/game_run.rs` (7 tests) covers: a Dash-to-Run transition entering
at frame 0.0 with `last_rate` 1.0; the first Run frame afterward advancing by
exactly 1.0 and every later frame matching the pure helper bit-exactly
against the previous frame's velocity/facing; wrapping at the figatree
length; checkpoints; invalid resources; `None` keeping the pre-batch integer
frame through a real Dash-to-Run transition; and a Run-to-RunTurn-to-Run
reversal confirming `ground_velocity` already agrees with the flipped facing
on every observed Run frame, documenting (not asserting) that the zero-rate-
against-facing branch is not independently reachable through an actual Run
frame in this codebase. `tests/run_differential.rs` (4 tests, 512 proptest
cases for the rate comparison) pins `ftCo_Run_Enter`/`_Full`/`_Anim` from a
new `run` snapshot of `ftCo_Run.c`; `run_animation_rate` is compared over the
full binary32 domain including NaN, plus exact boundaries (velocity exactly
0, negative velocity, velocity negative with facing also negative, scaling
as small as `f32::MIN_POSITIVE`/`1e-30`, the friction-multiplier branch, and
the `run.x0` countdown at several values); the oracle's own `run.x0`
countdown and `ftCo_Run_Enter`'s literal field assignments are checked
directly against the source lines, since no Rust helper models them.
`crates/cli/tests/replay_match.rs`'s
`file_backed_dash_into_a_run_reports_state_21_with_float_ages_and_a_reduced_
stick_mismatch` (1 test) drives a dash into a run, confirms every Run row
reports state 21/animation 13 with the tracked float frame, matches its own
generated Peppi bytes end to end, then reduces a stick sample well into the
ramp and confirms a `Mismatch` starting at that row -- and separately
confirms, by re-simulating with the same edited input directly, that the
reported age itself only starts to differ two rows later (Anim precedes the
ground-movement physics that reacts to the edited stick).

The 2026-09-11 walk-speed variants coverage batch is recorded at:

`/mnt/archive/runs/skirmish-walk-speeds-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. `game::locomotion`/`fighter::locomotion` subdivide `Action::Walk` into
WalkSlow/WalkMiddle/WalkFast (`docs/walk.md`) by `|ground_velocity|` against
two new resources: `MovementData.walk_animation` (the three figatree lengths
and animation-rate divisors) and `Rules.walk` (the middle/fast selection
thresholds), paired the same way as the edge/teeter and escape/escape-air
resources. `game::locomotion::State.walk: WalkState { kind, frame, last_rate
}` tracks the current kind and a float animation frame that advances one
frame behind `ftAnim_SetAnimRate`'s own delay and wraps at the current kind's
figatree length; a velocity crossing re-enters Walk (a genuine
`ChangeMotionState`, so `action_instance.id` is unaffected since Walk and
Dash already share motion identity 102) with the frame remapped
proportionally into the new kind's length, reproducing the source's
truncating quotient/remainder/remap chain exactly
(`fighter::locomotion::walk_retype_frame`). Slippi's `action_state` now
reports 15/16/17 by kind with animation indices 7/8/9, and `action_age`
publishes the tracked float frame instead of the integer `action_frame`
while the resource is present; `action_frame` itself is kept for Walk's own
internal bookkeeping. With either resource absent, Walk keeps its pre-batch
behavior unchanged: a single state (15/7) with an integer `action_frame`
published as `state_age`.

No discrepancy against the pinned source was found; the existing walk
physics (`getWalkAccel`/`ftWalkCommon_800E0060`, already audited by an
earlier batch) is unchanged. This batch's own explicit modeling choices,
documented in `docs/walk.md` rather than treated as verified source
behavior: `last_rate` is initialized to 1.0 at Walk entry (mirroring
`Fighter_ChangeMotionState`'s own `rate = 1` argument, not independently
confirmed against the animation system's own sub-frame scheduling), and the
`frame >= length` wrap rule (not independently reverse-verified against
`ftAnim`/`lbAnim`'s own loop bookkeeping).

`src/fighter/locomotion.rs` adds 3 unit tests for the pure `walk_kind`
threshold/sign arithmetic, `walk_animation_rate`'s zero-rate/`x0`-branch
selection and `walk_retype_frame`'s truncating remap chain.
`tests/game_walk.rs` (9 tests) covers: Slow/frame-0 entry from Wait;
a full-stick ramp (kept below the dash-magnitude threshold throughout)
reaching Middle then Fast with a stable `action_instance.id` and every
remapped frame bit-exact against the pure helper; the animation rate's
one-frame delay and its zero value while moving against facing, checked
bit-exactly across a whole ramp; wrapping at the Slow kind's length; Wait-
chain exit on a reversed or below-threshold stick, with Walk's own Anim-
phase advance for that exit frame still accounted for; a tilt press
preempting the retype check on its own frame; checkpoints; invalid
resources; and `None` keeping the pre-batch behavior.
`tests/walkcommon_differential.rs` (5 tests, 512 proptest cases each for the
type/rate/retype helpers) pins `ftWalkCommon_GetWalkType`,
`ftWalkCommon_800DFC70`, `ftWalkCommon_800DFCA4`, `ftWalkCommon_800DFDDC` and
`ftWalkCommon_800DFEC8` from a new `walkcommon` snapshot of `ftwalkcommon.c`
(distinct from the existing `ftwalk` adapter, which pins the same upstream
file's unrelated walk-physics functions), mapping the `static inline`
`..._800DFBF8_fake` duplicate to the pinned, non-static
`ftWalkCommon_GetWalkType` in the adapter. `walk_kind`/`walk_animation_rate`
are compared over the full binary32 domain including NaN; `walk_retype_frame`
is scoped to the finite, positive frame/length domain this codebase ever
calls it with (C's float-to-`s32` truncation is undefined outside it, unlike
Rust's saturating cast), with a fixed threshold/velocity table that
deterministically reaches every `(cur_kind, new_kind)` pair including the
same-kind no-op. `crates/skirmish-replay/src/observation.rs`'s existing
common-action/animation-id table test adds direct Middle/Fast kind coverage
(16/8, 17/9). `crates/cli/tests/replay_match.rs`'s
`file_backed_walk_ramp_reports_15_16_and_17_with_float_ages_and_a_reduced_
stick_mismatch` walks a full ramp, confirms every recorded row's reported
state/animation matches its current kind and that the ramp reaches Middle
then Fast, matches its own generated Peppi bytes (including the float
`state_age`), then reduces the first retype row's stick sample and confirms
a `Mismatch` starting at that exact row.

The 2026-09-11 floor-end collision modes and edge-teeter coverage batch is
recorded at:

`/mnt/archive/runs/skirmish-floor-ends-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. `game::collision` now derives one of three floor-end rules
(`CollisionMode::{Plain,Clamp,Teeter}`, `docs/edges.md`'s per-action table,
including `Catch`/`CatchDash` and `RunTurn` alongside every other mode-2
action) from the fighter's action wherever the ordinary grounded floor
projection fails past a line end: Plain keeps the previous fall-off
behavior; Clamp holds the position at the end (`Fighter.edge_contact`, no
facing/stick condition, `gr_vel` untouched); Teeter does the same clamp only
when facing and stick admit it (mpcoll's own 0.75 literal, `rules.edge.
teeter_stick_limit`), then enters the new `Action::Ottotto`/`OttottoWait`
(Slippi 245/246, animation 210/211) -- a zero-velocity, no-physics teeter
that exposes the full Wait input chain (catch, smash, tilt, jab, shield,
jump, dash, squat, turn all work unmodified through the existing
`tilt::interrupt_chain` mechanism) plus its own walk-away threshold
(`rules.edge.teeter_walk_threshold`) and exit-to-Wait distance check
(`rules.edge.teeter_exit_distance`/`teeter_exit_tolerance`). This corrects
the 2026-09-10 shield-escape batch's "falls off a floor edge" expectation
for rolls and the spot dodge (mode 2, always clamp); that entry below is
annotated with a bracketed correction rather than rewritten.

Two bugs were found and fixed as part of this batch. `locomotion::
update_actions` computed `interruptible_tilt` once, before dispatching the
jump-squat entry; entering `Action::JumpSquat` did not return, so the shared
Wait/Walk arm below (whose guard still matched on the stale
`interruptible_tilt`) ran again the same frame and could overwrite the fresh
JumpSquat with Walk whenever the jump press carried an admissible walk
stick -- reachable from Ottotto/OttottoWait but not unique to them. Every
Wait-chain IASA is a `RETURN_IF` chain that returns immediately once a jump
fires (`ftCo_Wait_IASA`: `RETURN_IF(ftCo_Jump_CheckInput(gobj))`), so
`update_actions` now does too; see `docs/edges.md` for why no other action
entry in that function shares the exposure. `tests/game_edges.rs`'s
`jump_squat_overwrites_walk_only_after_it_wins_not_before` covers an
interruptible smash pose, an interruptible jab pose and Ottotto itself.
Separately, `passive_wall_launch_differential.rs`'s bit-exact proptest
compared two independently computed NaN results by raw bits; NaN payload
propagation through arithmetic is unspecified, so a generated case landing
on two distinct quiet-NaN payloads (0x7FC00001 vs 0x7FC00002) was a
test-design flake, not a divergence. Hardened to assert only "both NaN" when
either side is NaN, otherwise the exact bits as before, matching the
existing `same_float`-style idiom already used by several other differential
tests (e.g. `ground_launch_differential.rs`). The same audit found three
more differential tests feeding arbitrary `any::<u32>()` bits into real
floating-point arithmetic (not a pure copy/selection, which cannot diverge)
without that guard, and applied it there too: `dash_differential.rs`
(`apply_friction`'s addition), `escape_air_differential.rs` (`decay`'s
multiplication) and `grab_mash_differential.rs` (the mash timer's `-=
penalty`); `shield_grab_differential.rs` (the dash-grab buffer's `-= 1.0`)
already had an equivalent flake risk and is hardened the same way.
`clank_differential.rs`, `hit_direction_differential.rs`,
`jab_differential.rs`, `landing_differential.rs`, `smash_differential.rs`
and `wall_jump_differential.rs` were checked and found not to need it: each
either rejects non-finite inputs before any arithmetic, constrains its
proptest generator to a finite range, or only copies/negates/selects an
input's exact bits without combining it arithmetically with another value.

`tests/game_edges.rs` (14 tests) covers attacks (a forward smash's root
motion, a supplied root-motion dash attack) and a roll reaching a floor end
clamping and staying grounded; walking into an admissible teeter entering
Ottotto with zero velocity then OttottoWait; an outward stick at exactly the
0.75 limit and a fighter facing away (pushed by an overlap nudge) both
falling instead; Dash and Run past an end falling as before; every
Wait-chain dispatcher reachable from Ottotto, including a same-direction
dash correctly carrying the fighter off the cliff it was teetering at rather
than staying frozen, and jump correctly reaching JumpSquat instead of being
overwritten by Walk; the teeter walk-away threshold; checkpoints; invalid
`rules.edge`/`fighter.teeter` combinations; and `rules.edge = None` keeping
Wait/Walk falling while a smash still clamps. `src/fighter/edge.rs` adds 5
unit tests for the pure floor-end decision, teeter gate, clamped position
and exit-distance arithmetic. `tests/edge_differential.rs` (3 tests, 512
proptest cases) compares that same pure decision against pinned
`mpColl_8004A45C_Floor`/`mpColl_8004A678_Floor` (a new `edge_floor` alias
adapter reusing the existing `mpcoll` snapshot) over generated positions,
facings, sticks and wall answers plus exact boundaries; the search for the
real invariant behind an early spurious mismatch (`ecb.bottom.x` is always
exactly 0.0, enforced by every ECB loader) is recorded in `docs/edges.md`.
`tests/ottotto_differential.rs` (6 tests, 512 proptest cases) pins
`ftCo_Ottotto.c`'s complete `ftCo_Ottotto_IASA` call order and the shared
Coll fall/exit decision against a Rust mirror. `tests/walk_differential.rs`
(3 tests, 512 proptest cases) pins `ftCo_Walk_CheckInput_Ottotto` from a new
snapshot of `ftCo_Walk.c` (distinct from the existing `ftwalk` adapter,
which pins the unrelated `ftwalkcommon.c`). Peppi-written replays match a
walk into a floor end (states 245/246, animations 210/211) that teeters and
then jumps out, and report a Mismatch at the exact row the walk crosses the
edge when that row's stick sample is pushed out to precisely 0.75 instead of
an admissible value.

The 2026-09-11 state-parity coverage batch is recorded at:

`/mnt/archive/runs/skirmish-state-parity-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. `observation::action_state` and `animation_index` now map FallSpecial
to 35/26 instead of 31/22, which were FallB's numbers; FallF/FallB and
FallSpecialF/FallSpecialB stay unmodeled, since `ftCo_Fall_Anim_Inner` swaps
their blend skeleton through `ftAnim_8006EDD0` without writing `fp->anim_id`.
An optional `locomotion::Parameters.jump_backward_threshold` (`ftCommonData.
x78`) reports a backward ground or aerial jump through its own Slippi id
(26/17, 28/19 instead of the ordinary 25/16, 27/18), selected by the exact
stick/facing test `ftCo_Jump_Enter` and `ftCo_JumpAerial_Enter_Basic` run at
their own launch frame, with equality on the threshold selecting backward;
`None` keeps every jump forward. An aerial jump's own animation end now
reports the aerial variant of Fall (32/23) instead of the ordinary 29/20;
every other Fall entry (an aerial attack's own end, walking off a platform, a
ledge drop) is unaffected. `tests/game_jump_variants.rs` covers forward and
backward short/full hops including the exact threshold boundary, a backward
double jump independent of its preceding ground jump, both new flags clearing
on every other transition, checkpoint replay through all three flagged
phases, `None` and invalid-threshold rejection. `tests/jump_differential.rs`
compares the pure `fighter::locomotion::jump_backward` predicate against
`ftCo_Jump_Enter` pinned in `tests/oracle/original/jump.c` over proptest (512
cases, including NaN and infinities) plus the exact equality and adjacent
boundaries. Peppi-written replays match a backward short hop into a backward
double jump into the aerial fall (26/17, 28/19, 32/23) and a parallel forward
run reaching the ordinary apex fall before its own forward double jump
(25/16, 29/20, 27/18), and report a Mismatch at the ground-jump launch row
where a flipped stick sample first flips the reported direction; the
air-dodge replay's FallSpecial row now asserts 35/26.

The 2026-09-11 landing-window coverage batch is recorded at:

`/mnt/archive/runs/skirmish-landing-window-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. `Fighter.landing_allow_interrupt` is set only by the ordinary Landing
entry and reset to `false` by every other transition, including
`LandingFallSpecial`, which keeps its own separate `aerial.allow_interrupt`
full lockout unaffected. Once `MovementData.normal_landing_lag` is supplied
and `cur_anim_frame` reaches it, `tilt::interrupt_chain` opens the same
complete Wait chain an interruptible tilt or dash attack pose already
exposes (grab, shield, special, smash, tilt, jab, jump, dash, turn, walk),
narrowed only by the crouch entry, which `locomotion::update_actions` gates
to the single first interruptible frame. Match integration coverage steps a
short hop into Landing with the fixture's `landing_frames` widened to 6 and
`normal_landing_lag` set to 3.0: every one of A, L, X, Z, stick down, a fresh
dash stick and a fresh C-stick is ignored on both frames before the lag; the
first interruptible frame accepts a jab, a moderate-stick tilt, a catch, a
fresh shield press, a jump, a fresh dash stick, a fresh C-stick smash and the
crouch; the second accepts everything again except the crouch; Turn and Walk
open from an interruptible frame; the animation still ends in Wait without
input; checkpoints restore every interrupt phase; invalid resources
(non-finite, negative, above `landing_frames`) are rejected; and omitting the
resource keeps a chainless Landing where a fresh A press well past where the
lag would otherwise open the chain still does nothing. `landing_differential`
traces `ftCo_Landing_IASA`'s complete dispatch order against a Rust mirror of
the exact pinned control flow (the lag and allow_interrupt gates, every
attack/movement callee in source order, the frame-gated squat check, turn and
walk) over generated frame/rate/lag/allow/answer combinations, plus boundary
cases, and separately compares the entry family's motion/allow/rate
arithmetic, including `ftCo_LandingFallSpecial_Enter`'s `(0.1 + x2EC) / lag`
bit-exact. `tests/game_landing.rs` covers the full per-phase dispatch order
end to end against the Rust implementation. A Peppi-written replay matches a
short-hop landing followed by a C-stick smash on the first interruptible
frame and reports a Mismatch at the row where the removed C-stick sample's
effect first appears.

The 2026-09-11 dash-attack coverage batch is recorded at:

`/mnt/archive/runs/skirmish-dash-attack-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage dashes into the early phase and, with the
entry stick already stale (`ftCo_Dash_Enter` resets the age to 254), confirms
the dash-specific forward smash needs no age window and keeps facing, while
the C-stick crossing can still flip it; the held-shoulder forward roll fires
through its own limit and does nothing once past it while still inside the
early phase. A fresh A enters the dash attack from the middle phase and from
Run, arming the shared shield-grab buffer, but never from the early phase
(a neutral stick) or the late phase. An opposite fresh stick in the middle
phase enters a smash Turn without restarting the dash; the same direction in
the late phase restarts Dash with `dash_from_input` set and `action_frame`
back at 1. Middle- and late-phase shield entry differ only by the buffer
(unarmed before the limit, armed after), and a fresh press opens GuardReflect
with the same buffer in both phases. The `x54` transition-friction tail
applies on the same frame as every fall-through transition (the early
forward smash/roll, the middle dash-back Turn/shield entry, the late
re-dash/Turn/shield entry, and a neutral special entered from Dash) but
never after the catch, the AttackDash entry, the jump-squat entry, the run
transition, "nothing fired", or ever for Run: a late-phase re-dash's first
observed `ground_velocity` matches `dash_initial_velocity - gr_vel_before *
x54` by hand computation (`ftCo_Dash_Enter` reads `gr_vel` before the tail
reduces it), a late-phase GuardOn entry's `ground_velocity` matches the tail
composed with GuardOn's own ordinary ground friction, and an idle Dash frame
(nothing fired) leaves `ground_velocity` exactly equal to the same frame
with `rules.dash = None` — since the tail is otherwise gated behind the
unmodeled taunt (`ftCo_800DE9D8`/`ftCo_800DE9B8`, D-pad up entering AppealS,
`ftCo_AppealS.c:35,43`) in the pinned source's own `block_42`. AttackDash's
catch buffer fires on a held shoulder without A, counts down and expires;
its Wait chain opens only on the flagged pose; its physics apply friction
absent supplied root motion; a close victim is hit exactly once; checkpoints
and invalid resources (non-finite/negative rules, a repeat flag, mismatched
root length, a missing grab shield-grab pairing, missing locomotion, and
dash rules without a per-fighter attack or vice versa) are covered.
`dash_differential` traces `ftCo_Dash_IASA`'s complete dispatch order against
a Rust mirror of the exact pinned control flow (catch, forward smash, roll,
AttackDash entry, the real `ftCo_Dash_CheckInput` predicate, both shield
entries, the guard buffer arm, the taunt, jump and run checks, and the
friction tail itself) over generated phases, sticks, ages, run flags and
answer combinations, plus boundary cases, and separately compares
`ftCo_Dash_CheckInput`, `ftCo_800D8AE0`'s catch buffer and
`ftCommon_ApplyFrictionGround` (already pinned by the `locomotion_common`/
`physics` adapters, which `apply_friction` mirrors, exercised here for
AttackDash's `x50` deceleration) with pinned C. `tests/game_dash.rs` covers
the full per-branch dispatch order, including the transition-friction
arithmetic, end to end against the Rust implementation. Peppi-written
replays match a dash attack entered from Run and report Slippi state 50 with
animation 52, match a late-phase re-dash, and detect a removed press at its
first affected frame in both.

The 2026-09-11 jab-combo coverage batch is recorded at:

`/mnt/archive/runs/skirmish-jab-combos-20260911-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage starts the first jab from Wait (a held
press is not fresh), latches a buffered press on an uninterruptible pose and
fires the second jab on the first pose whose follow-up flag is raised, and
chains the same pattern into the third jab, which has no follow-up of its
own. The follow-up window survives into Wait and decays there without a
press; any other motion resets it immediately rather than gradually. The
first and second jabs' interruptible poses try smashes, then tilts, then the
ordinary jump/dash/squat/turn/walk dispatch, and never reach shield or catch;
the third jab's interruptible pose reaches the complete Wait chain, including
catch, shield and a fresh first jab. The rapid jab counts fresh presses and
releases (a held button does not add to the count) into Attack100Start,
which plays into Attack100Loop keeping the action instance
(`Ft_MF_SkipAttackCount`) before the loop's own frame zero restarts the stale
identity and allocates a fresh instance id; tapping through the loop's
continuation check keeps it going and re-hits an in-reach victim each cycle,
and no input at the check ends it into Attack100End and Wait. A later pose's
explicit `rapid: Some(false)` turns the flag back off, a script's clear_hits
command lets one hitbox group hit the same victim twice within a jab, the
rapid press count persists into the second jab and resets on a fresh first
jab, the second jab's TransN root motion is applied in both facings,
checkpoints restore every phase (first jab both before and after
`allow_interrupt`, second, third, rapid start/loop/end and Wait with the
window still open), and invalid resources (mismatched flag/root-translation
lengths, a loop check outside the rapid cycle, a follow-up flag without its
next jab, a rapid flag without the rapid jab, a cycle without any
continuation check, a negative rapid window, a non-finite window, staled
rapid animations with differing move identities and missing locomotion) are
rejected before a match is constructed; `jab_combo = None` keeps the
original chainless jab. `ftCo_Attack1_CheckInput`, `checkAttack11`,
`doAttack12`, `checkAttack12`, `doAttack13`, `checkAttack13`,
`ftCo_Attack11_IASA`, `ftCo_Attack12_IASA`, `ftCo_Attack13_IASA`,
`ftCo_Attack_800D6A50`, `ftCo_800D6B00`, `ftCo_Attack100Loop_Anim` and
`ftCo_Attack100Loop_IASA` are compared with pinned C over generated
press/release and script-flag sequences. Peppi-written replays match a
three-jab combo and a rapid jab, report Slippi states 44..46 with animation
indices 46..48 and states 47..49 with animation indices 49..51, and detect a
removed press at its first affected frame.

The 2026-09-10 smash coverage batch is recorded at:

`/mnt/archive/runs/skirmish-smashes-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage selects every forward-smash angle variant
with the stick-sign facing and availability fallbacks, enforces the strict
dash-stick window, orders up and down smashes before tilts with inclusive
thresholds and float windows, enters from fresh C-stick crossings without a
button (also over a same-frame shield press), keeps Turn's post-turn facing,
reaches the smashes from SquatWait and Walk, jump-cancels the windowless up
smash and the grab from JumpSquat, leaves a released press unarmed, freezes the
charge pose while A is held until release or the hold limit, scales released
damage (including a fractional value with its truncated stale count), scales a
charging victim's knockback before armor, applies TransN root motion in both
facings with a frozen charge pose, opens the Wait chain only on flagged
samples, re-enters from a released and re-crossed C-stick, reads Z as the
logical A press from the down tilt, restores checkpoints in every charge phase
and rejects invalid resources. The complete input predicates, the forward
entry with `decideFighter`/`doEnter`, the windowless KneeBend variant, the
fresh C-stick predicates and the charge lifecycle are compared with pinned C.
Peppi-written replays match a charged forward smash, a C-stick down smash and
an up smash, report Slippi states 58..64 with animation indices 60..66 and the
frozen action frame, and detect a removed press, an early release or a removed
C-stick sample at its first affected frame. The tilt batch's SquatWait/SquatRv
catch was corrected: `ftCo_Catch_CheckInput` is not in those chains.

The 2026-09-10 tilt coverage batch is recorded at:

`/mnt/archive/runs/skirmish-tilts-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage selects every forward-tilt angle variant and
its availability fallbacks, orders forward, up, down and jab from each grounded
state with Turn's post-turn facing, opens the Wait chain or the down tilt's
narrower chain only on flagged samples, ends in Wait or SquatWait, buffers and
repeats the down tilt while keeping the entry action instance and restarting
the stale instance on exit, accepts A with a held shoulder as a catch, restores
checkpoints and rejects invalid resources. The complete input predicates,
`decideAngle` and `checkPadA` are compared with pinned C. Peppi-written replays
match straight, high, up and repeated down tilts, report Slippi states 51..57
with animation indices 53..59, and detect a removed attack press at its first
affected frame.

The 2026-09-10 air-dodge coverage batch is recorded at:

`/mnt/archive/runs/skirmish-air-dodge-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage enters EscapeAir from Jump, Fall and
JumpAerial on a fresh physical L or R press only, launches along the stick
angle outside the strict deadzone, decays both axes without gravity until the
scripted resume, re-arms fast fall afterwards, blocks hits on scripted
intangible samples, continues into FallSpecial with every jump used and no
aerial attack or jump, lands into the special landing at the source rate with
input locked out, wavedashes straight from the dodge with ground friction,
falls through one-way platforms only while holding down, orders the dodge
before aerial attacks and double jumps, restores checkpoints in every phase and
rejects invalid resources. The complete trigger, launch, decay and
platform-landing bodies are compared with pinned C. Peppi-written replays
match a landing dodge and a FallSpecial continuation, report Slippi states
236/31/43 with animation indices 44/22/36 [FallSpecial's 31/22 were FallB's
numbers; corrected to 35/26 by the 2026-09-11 state-parity batch] and the
scripted hurtbox byte, and detect a removed trigger press at its first
affected frame.

The 2026-09-10 shield-grab coverage batch is recorded at:

`/mnt/archive/runs/skirmish-shield-grab-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage grabs from GuardOn, Guard and GuardReflect
with A plus a held shoulder or with physical Z, rejects A without a shoulder,
orders the grab after the spot dodge and roll and before the jump dispatchers,
arms the dash-grab buffer from Run and late Dash shields (including a
powershield), counts it down only on GuardOn/GuardReflect callbacks, expires
it, excludes Guard, GuardOff and shield stun, clears it on a fresh shield,
restores checkpoint branches, requires explicit rules and rejects invalid ones.
The complete `ftCo_Catch_CheckInput` and `ftCo_800D8B9C` bodies are compared
with pinned C over arbitrary button words and binary32 buffers. Peppi-written
replays match the A, Z and buffered late-dash suffixes and detect a removed
grab button at its first affected frame. Every ordinary exit from a guard
state (escape, grab or jump) now clears the Slippi-visible reflect bit and the
powershield entry latch as `Fighter_ChangeMotionState` does.

The 2026-09-10 grounded shield-evasion coverage batch is recorded at:

`/mnt/archive/runs/skirmish-shield-escape-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage enters EscapeF, EscapeB and EscapeN from
GuardOn, Guard, GuardReflect and (spot dodge only) GuardOff through fresh
main-stick and held C-stick input, checks main-stick priority and
facing-relative direction, applies exact sampled root motion and bone poses,
clears the shield, blocks hits and grabs only on scripted intangible samples,
exempts the evading fighter from overlap nudges, falls off a floor edge
[corrected 2026-09-11: rolls and the spot dodge use mode 2, always clamp;
they stop at a floor end instead of falling off it -- see the floor-ends
batch below],
returns to Wait, restores checkpoints inside every escape action and rejects
invalid motions before a match exists. The complete `ftCo_8009917C` and
`ftCo_8009980C` dispatchers plus `ftCo_800DF8B0` and `ftCo_800DF8E8` are
compared with pinned C over arbitrary bit patterns and boundaries. Peppi-written
replays match each escape suffix, report Slippi states 233..235, animation
indices 42/43/41 and the scripted hurtbox byte, and detect a changed roll
stick, C-stick X or C-stick Y sample at its first affected frame.

The 2026-09-10 C-stick shield-jump coverage batch is recorded at:

`/mnt/archive/runs/skirmish-cstick-shield-jump-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration coverage holds an upward C-stick before shield entry,
uses it on the next Guard callback, checks main-stick/X/Y priority, the inclusive
threshold, release-based short hops, ordinary-state exclusion and checkpoint
restoration. The complete `ftCo_800DF910` predicate is compared with pinned C
over arbitrary binary32 values. A Peppi-written replay matches the
GuardOn-to-JumpSquat suffix and detects a changed C-stick at its first affected
frame.

The 2026-09-10 shield-drop coverage batch is recorded at:

`/mnt/archive/runs/skirmish-shield-drop-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Integration coverage drives digital and processed analog shields through
an immediate Pass transition on a one-way platform, rejects the same request on
a solid floor, preserves the single-transition scheduler boundary and reuses
the existing collision-line skip and ECB lock. The exact `ftCo_8009A080` and
`ftCo_80099F1C` predicates are compared with pinned C over arbitrary input bit
patterns and boundary values. A Peppi-written replay matches the resulting
GuardOn-to-Pass suffix and detects a changed down-stick on its first affected
frame.

The 2026-09-10 inert shield-touch batch is recorded at:

`/mnt/archive/runs/skirmish-shield-touch-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and all selected original-C functions in debug and release
modes. Match integration tests prove that an inert hitbox can overlap a shield
without shield/body damage, hitlag, stun or hit-history consumption, that the
one-frame signal survives checkpoints, and that inert body overlap is ignored.
The `fighter-post-v11` policy compares the recorder's `x221C_b5` bit. A
Peppi-written GuardReflect replay contains a positive shield-touch frame and
requires an independent corruption to report `state_flags.shield_touch`.

The 2026-09-10 powershield batch is recorded at:

`/mnt/archive/runs/skirmish-powershield-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, the complete
native workspace, and the selected original-C functions in debug and release
modes. GuardReflect integration tests cover fresh digital entry, conversion
from analog GuardOn, expiration boundaries, shield-damage immunity, hitlag,
shield stun, defender push, attacker recoil and checkpoint replay. The exact
`ftCo_80093BC0` timer routine and both ordinary/powershield push branches are
also differential-tested against the pinned C source. The `fighter-post-v10`
policy maps state 182 to the GuardOn animation and compares `reflecting` and
`x221C_b2`; a Peppi-written file requires independent corruptions to report
`state_flags.reflect` and `state_flags.powershield`.

The 2026-09-10 Slippi sleep-state batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-sleep-state-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, 567 native
workspace tests, and 761 original-C differential tests in both debug and release
modes. The `fighter-post-v9` policy maps Skirmish's inactive respawn interval to
Melee's common `Sleep` state 11, its absent animation index, and
`Fighter::x221F_b3`. Peppi-written ordinary, star and screen death replays now
continue through sleep and return to active play. They require an independently
corrupted positive sleep bit to report `state_flags.sleep`. The 19 ignored tests
require external references, extracted assets, or a GPU/window and are
unchanged.

The 2026-09-10 Slippi death-flag batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-death-flag-20260910-verified`

It validates formatting, strict all-target/all-feature Clippy, 567 native
workspace tests, and 761 original-C differential tests in both debug and release
modes. The `fighter-post-v8` policy adds the recorder's `Fighter::x221F_b1`
bit. Native lifecycle tests cover immediate ordinary deaths, delayed star/screen
disappearance, inactive respawn retention and rebirth clearing. Peppi-written
replay tests exercise all three death paths with a positive flag value and
require an independently corrupted fifth-byte bit to report
`state_flags.dead`. The 19 ignored tests require external references, extracted
assets, or a GPU/window and are unchanged.

The 2026-09-10 Slippi animation-index batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-animation-index-20260910`

It validates formatting, strict all-target/all-feature Clippy, 566 native
workspace tests, and 760 original-C differential tests in both debug and release
modes. The `fighter-post-v7` replay policy now compares Slippi 3.11's exact
32-bit `Fighter::anim_id`, separately from the motion-state ID. The pinned
common motion table supplies shared animation indices and preserves `-1` as
`0xffffffff`; mapped Fox neutral-special startup states use their character
table indices. Focused unit coverage checks ordinary, shared, damage, prone,
ledge, no-figatree and Fox-specific entries. Peppi-written file tests enforce
the 3.11 version boundary and independently corrupt the new field in the full
reported-field loop. The 19 ignored tests require external references, extracted
assets, or a GPU/window and are unchanged.

The 2026-09-10 Slippi action-instance batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-action-instance-20260910`

It validates formatting, strict all-target/all-feature Clippy, 566 native
workspace tests, and 760 original-C differential tests in both debug and release
modes. The `fighter-post-v6` replay policy now compares Slippi 3.16's current
action-instance ID and retained last-hit source instance. Independent nonzero
16-bit match counters reproduce identity changes, shared motion-family
retention, explicit restarts, wrap and zero skipping. The selected complete
`ft_800895E0`, `ft_80089824` and `plAttack_80037B08` bodies are compared over
1,024 arbitrary states. Match coverage checks hit attribution,
aerial-to-landing retention, zero-identity actions, stock cleanup, checkpointed
state and independent resets; Peppi-written replay files version-gate and corrupt
each new field independently. The 19 ignored tests require external references,
extracted assets, or a GPU/window and are unchanged.

The 2026-09-10 retained-combo push batch is recorded at:

`/mnt/archive/runs/skirmish-combo-push-20260910`

It validates formatting, strict all-target/all-feature Clippy, 563 native
workspace tests, and 755 original-C differential tests in both debug and release
modes. Repeated same-move hits now start the fighter-owned separation timer and
apply the original grounded floor-tangent displacement during ordinary frames
and hitlag. The exact `ftcoll.c` bodies are compared over arbitrary retained
pointer identities, signed thresholds, 32-bit timer values, native counter
wraps, eligibility states and binary32 bit patterns. Match coverage checks both
timer frames, position changes, checkpoints and stock-loss reset. The 19 ignored
tests require external references, extracted assets, or a GPU/window and are
unchanged.

The 2026-09-10 Slippi combat-provenance batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-combat-provenance-20260910`

It validates formatting, strict all-target/all-feature Clippy, 562 native
workspace tests, and 752 original-C differential tests in both debug and release
modes. The `fighter-post-v5` replay policy now compares internal character ID,
last landed attack, retained combo count and last-hitting physical port. The
native scheduler retains the source combo pointer through hitstun and the
configured post-hitstun grace period. Exact unit tests cover every translated
counter branch and the complete character-ID mapping; match and Peppi-written
file tests cover source attribution, repeated hits, checkpoint replay and an
independently corrupted value for every new field. The 19 ignored tests require
external references, extracted assets, or a GPU/window and are unchanged.

The 2026-09-10 Slippi state-flags batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-state-flags-20260910`

It validates formatting, strict all-target/all-feature Clippy, 558 native
workspace tests, and 748 original-C differential tests in both debug and release
modes. The `fighter-post-v4` replay policy now compares protection, fast-fall,
hitlag, active-shield and hitstun bits, plus the action-state union as a hitstun
counter only while Slippi marks that interpretation valid. File-backed tests
drive digital-L shield entry, fast fall, hitlag and hitstun and corrupt every new
reported field independently. The 19 ignored tests require external references,
extracted assets, or a GPU/window and are unchanged.

The 2026-09-10 Slippi post-frame v3 batch is recorded at:

`/mnt/archive/runs/skirmish-slippi-post-v3-20260910`

It validates formatting, strict all-target/all-feature Clippy, 557 native
workspace tests, and 747 original-C differential tests in both debug and release
modes. File-backed replay coverage now compares retained ground-line identity,
successful and unsuccessful l-cancel results, and version-gated hurtbox collision
state. Native physics keeps ordinary invincibility and intangibility separate;
ledge intangibility blocks hits and grabs and serializes with the recorder's
priority. Debug and release traces are identical. The 19 ignored tests require
external references, extracted assets, or a GPU/window and are unchanged.

The 2026-09-10 airborne Damage callback batch is recorded at:

`/mnt/archive/runs/skirmish-damage-air-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. A long sampled airborne Damage motion proves the last hitstun
frame retains locked physics and input, then checks ordinary drift, fast fall,
neutral special, aerial attack and double jump on a fresh legal frame with
checkpoint replay.

The 2026-09-10 DamageFall air-callback batch is recorded at:

`/mnt/archive/runs/skirmish-damage-fall-air-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Integration coverage holds DamageFall fast fall, neutral special,
aerial attack and double jump through hitstun, accepts fresh input after the
exact boundary and replays every branch from checkpoints.

The 2026-09-10 reflected damage-air callback batch is recorded at:

`/mnt/archive/runs/skirmish-surface-reflect-air-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Both reflected actions now run ordinary aerial gravity and stick
drift. Match coverage holds fast fall, neutral special, aerial attack and double
jump through hitstun, accepts fresh input after the exact boundary and replays
every branch from checkpoints.

The 2026-09-10 moving-wall reflection batch is recorded at:

`/mnt/archive/runs/skirmish-surface-reflect-moving-wall-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. A zero-speed `FlyReflectWall` integration fixture isolates the
moving collision plane: inward translation pushes the ECB by the exact line
delta, outward translation separates without dragging, and both paths replay
from checkpoints.

The 2026-09-10 reflected-surface landing batch is recorded at:

`/mnt/archive/runs/skirmish-surface-reflect-landings-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Match-level coverage drives both `FlyReflectWall` and
`FlyReflectCeiling` through ordinary floor collision, missed-tech `DownBound`
entry, velocity and knockback cancellation, response-history cleanup and
deterministic checkpoint replay.

The 2026-09-10 reflected-surface chaining batch is recorded at:

`/mnt/archive/runs/skirmish-surface-reflect-chains-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Integration coverage drives wall-to-ceiling and
ceiling-to-opposite-wall reflections while the configured repeat lockout remains
active, checks response ordering and remembered surface-class replacement, and
replays each cross-surface suffix from a checkpoint.

The 2026-09-10 damage-surface pose and delayed-jump batches are recorded at:

`/mnt/archive/runs/skirmish-surface-tech-poses-20260910`

`/mnt/archive/runs/skirmish-surface-tech-jump-queue-20260910`

Each isolated batch runs formatting, strict all-target/all-feature Clippy,
native workspace tests and original-C differential tests in debug and release
modes. The pose batch supplies complete wall, wall-jump and ceiling physics
skeletons, validates every sample and distinguishes ordinary wall-jump ownership
through the headless bone ECB. The delayed-jump batch covers a neutral wall
tech's queued jump transition, same-frame pose handoff, release-frame boundary
and deterministic checkpoint suffix. Exact jump selection and launch arithmetic
remain compared with their complete pinned C bodies over arbitrary binary32
inputs.

The 2026-09-10 wall-tech air-interrupt batch is recorded at:

`/mnt/archive/runs/skirmish-wall-tech-air-interrupts-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage holds actions through frozen wall
tech startup, rearms fresh input after release, dispatches neutral special,
aerial attack and double jump in source priority, applies the same branches to
ordinary wall jumps and preserves the ceiling tech's empty interrupt callback.

The 2026-09-10 surface-tech landing coverage batch is recorded at:

`/mnt/archive/runs/skirmish-surface-tech-landings-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. One match-level integration test drives neutral wall tech,
wall-jump tech and ceiling tech through gravity, floor contact, shared Landing
timing, transient surface/wall-jump cleanup and checkpoint replay.

The 2026-09-10 moving-wall tech coverage batch is recorded at:

`/mnt/archive/runs/skirmish-surface-tech-moving-wall-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. The integration scenario schedules a collision-line translation
immediately after wall-tech entry and checks inward displacement, outward
separation, contact state, frozen self velocity, one timer callback and
deterministic checkpoint replay.

The 2026-09-10 reflected-surface pose batch is recorded at:

`/mnt/archive/runs/skirmish-surface-reflect-poses-20260910`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Per-fighter complete `FlyReflectWall` and `FlyReflectCeiling`
pose tracks now drive the headless bone ECB. Integration coverage checks both
selectors and checkpoint replay; resource tests cover pairing, exact duration,
skeleton topology, finite transforms and serialization.

The 2026-09-09 grounded-launch coverage batch is recorded at:

`/mnt/archive/runs/skirmish-ground-launch-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises flat and sloped tangent
projection, exact floor departure, fly bounce, hitlag freezing, scalar friction,
prone DownDamage's explicit fly override, malformed resources and checkpoint
replay. The complete retained launch branch and extracted vector-angle helper
are compared over arbitrary binary32 inputs.

The 2026-09-09 prone DownDamage coverage batch is recorded at:

`/mnt/archive/runs/skirmish-down-damage-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises both prone orientations,
strict threshold equality, the source face-down selector quirk, launch and
landing, shared countdown recovery, malformed resources and checkpoint replay.
The complete retained eligibility/selector callback is compared with pinned C.

The 2026-09-09 prone-orientation coverage batch is recorded at:

`/mnt/archive/runs/skirmish-prone-orientation-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage selects face-up and face-down from
the evaluated hip matrix, preserves the choice through checkpoints and drives
distinct bound/wait poses, both roll directions, stand poses and get-up attacks.
The strict matrix-axis/inversion selector has direct unit boundary coverage.

The 2026-09-09 floor-recovery coverage batch is recorded at:

`/mnt/archive/runs/skirmish-floor-recovery-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises the full supplied
Passive/DownBound/DownWait pose suffix, reset and checkpoint behavior for the
source A/B attack buffer, DownBound attack/roll priority, every recovery
invincibility category and combat on the first vulnerable frame. Exact selector
boundaries remain covered by Rust unit tests and the existing complete C-stick
predicate differentials.

The 2026-09-09 missed-tech recovery batch is recorded at:

`/mnt/archive/runs/skirmish-knockdown-options-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage exercises attack/roll/stand input
priority, both roll directions, fresh and held C-stick history, exact sampled
root motion and bones, get-up attack contact, resource rejection and checkpoint
replay. The two exact C-stick predicates are compared against pinned original C
over arbitrary binary32 values.

The 2026-09-09 floor-tech roll batch is recorded at:

`/mnt/archive/runs/skirmish-floor-tech-roll-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs neutral, forward and backward
floor techs, inclusive input selection, separate sampled action durations,
TransN root motion, bone-derived ECB changes, serialization, malformed resources
and exact checkpoint replay. Direction selection compares against complete
pinned original C across arbitrary binary32 inputs. Independent scheduler traces
remain pending.

The 2026-09-09 damage-surface tech batch is recorded at:

`/mnt/archive/runs/skirmish-damage-surface-tech-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs neutral and jump wall techs,
both wall orientations, ceiling input motion, exact action timing, floor/wall
priority, repeat lockout, invalid resources and exact checkpoint replay. Wall
tech jump selection compares against complete pinned original C across timer
boundaries and arbitrary binary32 inputs. Independent scheduler traces remain
pending.

The 2026-09-09 damage-surface batch is recorded at:

`/mnt/archive/runs/skirmish-damage-surface-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs wall and ceiling reflection,
configured action timing, floor-first collision priority, omission and unmet
threshold behavior, malformed resources and exact checkpoint replay. The
retained reflection arithmetic compares against complete pinned original C and
vector-helper bodies. Independent scheduler traces remain pending.

The 2026-09-09 ECB-response batch is recorded at:

`/mnt/archive/runs/skirmish-ecb-response-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs four-sided moving compression,
horizontal and vertical squeeze, next-frame ECB restoration, moving-floor
landing, one-way direction filtering, tangential-motion rejection and exact
checkpoint replay. The reusable squeeze functions continue to compare against
their complete pinned original C bodies. Independent corner/squeeze scheduler
traces remain blocked pending a separate producer.

The 2026-09-09 stage-motion batch is recorded at:

`/mnt/archive/runs/skirmish-stage-motion-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs affine/cyclic collision-line
motion, current geometry, grounded carry after self movement and during hitlag,
airborne detachment/relanding, countdown timing, checkpoint replay and invalid
resources. Generated cases compare the complete original moving-line remap,
including exceptional binary32 inputs. The independent moving-platform trace
remains blocked pending a separate producer.

The 2026-09-09 blast-death batch is recorded at:

`/mnt/archive/runs/skirmish-deaths-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Focused integration coverage runs every normal blast direction,
forced top death, star and screen phases, delayed/final stock loss, input
suppression, checkpoint replay and invalid resources. The complete original
blast selector is compared across generated states, including exact HSD RNG
consumption. Thirteen fidelity cases remain blocked on independent reference
fixtures, including the separate star/screen trace comparison.

The 2026-09-09 neutral-special batch is recorded at:

`/mnt/archive/runs/skirmish-specials-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. Both formerly ignored neutral-special scenarios now pass, with
focused integration coverage for input priority and rearming, both player slots,
ground/air physics and terrain conversion, bone-attached combat, checkpoints and
invalid resources. The exact neutral input predicate is compared with complete
pinned C. The conformance audit now has no executable unimplemented scenarios;
13 cases remain blocked on independent reference fixtures.

The 2026-09-09 rebirth-platform batch is recorded at:

`/mnt/archive/runs/skirmish-rebirth-20260909`

It validates the isolated commit with formatting, strict all-target/all-feature
Clippy, native workspace tests and original-C differential tests in debug and
release modes. The stock-loss platform scenario now passes, with focused
integration coverage for both player slots, exact target travel, wait/release,
invulnerability, checkpoint restoration and invalid resources. Two complete
original leader physics callbacks are compared bit for bit. The conformance
audit now has 2 executable unimplemented scenarios and 13 cases blocked on
independent reference fixtures.

The 2026-09-09 static ledge-action batch is recorded at:

`/mnt/archive/runs/skirmish-ledge-20260909`

It validates the composed tree after the native asset-import commit with
formatting, strict all-target/all-feature Clippy, native workspace tests and
original-C differential tests in debug and release modes. Six formerly ignored
ledge scenarios now pass, with focused integration coverage for endpoint
eligibility and ownership, bone attachment, every ledge option, combat contact,
damage/KO release, cooldown/regrab and checkpoint restoration. The current
conformance audit has 3 executable unimplemented scenarios and 13 cases blocked
on independent reference fixtures.

The 2026-09-09 movement/combat coverage milestone is recorded at:

`/mnt/archive/runs/skirmish-gameplay-20260909-v6b`

It checks the isolated gameplay snapshot with formatting, strict Clippy, native
workspace tests and original-C differential tests in debug and release modes.
Native trace comparisons exercise both the original demo and explicit movement,
combat/displacement/armor and stacked-platform profiles. The run preserves its
commands, input resources, scripts, output traces and source hashes. Synthetic
debug/release agreement checks determinism, not original-game equivalence.

Eleven previously ignored scenarios are now active, accompanied by focused
integration regressions for input history, movement/action timing, displacement,
armor, platform collision and blast boundaries. The remaining audit contains
21 executable unimplemented scenarios and 13 independent-reference-blocked
cases; all 34 are invoked explicitly and remain failures rather than passing
coverage. Authentic movement poses, special-character callbacks and the full
game scheduling order are still outside this milestone.
The earlier `v6` run caught inconsistent synthetic movement/displacement
thresholds after input-history consolidation; `v6b` uses corrected explicit data.

The 2026-09-09 Peppi Slippi importer validation run is stored outside Git at:

`/mnt/archive/runs/skirmish-slippi-20260909-v5c`

It contains command/exit-code metadata and debug, optimized, lint and Rust-only
test logs, plus debug/optimized synthetic match traces and their comparison.
Peppi 2.1.2 writes wholly synthetic `.slp` fixtures covering supported versions,
ports, follower presence/absence, exact input bits, rollback, finalization,
malformed events and executable error handling. A synthetic position protocol
checks the generic transition interface. Separate file-backed scenarios drive
the real native match across walking, jumping, landing and combat, detect late
corruption and changed inputs, and ensure reference observations never reset
simulated state. Their expectations originate from the experimental native
match and do not establish independent Melee equivalence. Explicit conformance
audits report missing implementation and independent-reference fixtures as
failures, separately from the normal passing regressions; see [testing.md](testing.md).
That archived audit had 32 executable missing-behavior scenarios and 13 cases blocked on
independent reference fixtures. It invoked all 45 explicitly and recorded
their failures; default test discovery reports them as ignored. Three real
Slippi 2.0.1 corpus files are also parsed with hashes and frame counts recorded.
This smoke check covers parsing, without compatible native-match initialization
or a gameplay equivalence assertion. The earlier `v5` attempt stopped at
formatting while another task was editing menu tests. `v5b` passed in the shared
workspace; `v5c` validates the fixed commit snapshot in a temporary worktree,
separately from concurrent menu/presentation additions.
The earlier `skirmish-physics-20260909-v4` contains stage, ECB, swept-contact and
damage physics validation. `skirmish-headless-match-20260909-v3` contains the first native match
and skeletal physics milestone. `skirmish-rust-port-20260909-v2` contains the runtime/physics/input/
replay milestone. `skirmish-rust-port-20260909-v1` also contains a
hash inventory of all 2,441 upstream C/C++/header/assembly files. The input source
revision and toolchain version are recorded in `validation.json`. The upstream
revision is `0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`. Original test snapshots are
tracked separately in `tests/oracle/sources.json` and checked for exact coverage
and SHA-256 equality by the test suite.

Build products remain in `/mnt/shared/tmp/skirmish-target`. Local absolute paths
are provenance, not build requirements; CI builds in its own temporary directory.
No Melee game image or full-game behavior comparison participated in this run.
The synthetic native match reaches stock-based termination. Original-C checks
cover selected functions, while debug/optimized match agreement validates the
experimental scheduler's determinism, not Melee compatibility.
