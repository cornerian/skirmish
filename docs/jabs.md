# Jab combo profile

`skirmish::game::jab` ports the jab combo from `ftCo_Attack1.c` and the rapid
jab from `ftCo_Attack100.c`. The first jab remains the fighter's ordinary `jab`
attack; the optional `jab_combo` resource adds that attack's decoded script
(`first`), the second and third jabs (`second`, `third`), the rapid jab's
start, looping cycle and end animations (`rapid`) and the `ftCo_DatAttrs`
windows (`attributes`: `second_window` = `jab_2_input_window`, `third_window`
= `jab_3_input_window`, `rapid_window` = `rapid_jab_window`).
`tests/support/jab.rs` builds an invented profile. Without the resource the
jab keeps its previous behaviour: it plays out and returns to Wait with no
input chain. Locomotion parameters are required for the chains.

Every jab animation is a supplied `Attack` (poses, hitboxes, hurtbox samples,
native move identity) plus one decoded command sample per pose:

- `allow_interrupt`: `Fighter::allow_interrupt` after the pose's commands.
- `follow_up_ready`: `x2218_b1` after the pose's commands (the jab-combo
  command `ftAction_80071AE8` was issued on or before it; each jab entry
  clears the flag first).
- `rapid`: the jab-rapid command's state (`ftAction_80071B28`,
  `x2218_b2`) when it is issued on this pose; absent otherwise. The flag
  keeps its value across jabs, and only the first jab's entry clears it.
- `loop_check` (loop only): the throw-flag command (`set_throw_flags` hit 0,
  `throw_flags_b3`) that `ftCo_Attack100Loop_Anim` consumes on this pose.
- `clear_hits`: the clear-hitboxes command (`ftColl_8007AFF8`). Needed
  because the jab's hitbox stays in `Attack.frames` on the very pose this
  command runs (the script clears and immediately re-creates it within the
  same exported frame), which the generic per-attacker `hit_groups`
  re-enable (`hitboxes::refreshed_groups`, `docs/validation.md`'s
  2026-09-12 entry) cannot see, since it only reacts to a hitbox actually
  being absent for a whole frame -- exactly what Fire Fox Hold's own charge
  pulse does instead, needing no `clear_hits`-equivalent field at all.

Optional per-pose `root_translations` supply the TransN delta that
`ft_80084FA8` turns into ground speed (every jab callback uses it).

The implementation preserves these examined source branches:

- `ftCo_Attack1_CheckInput` is the last attack in the Wait, Walk, Turn,
  Squat, SquatWait, SquatRv, Landing, Guard, AppealS, Ottotto and AttackS4
  chains and in the down tilt's interruptible block. With a fresh logical A
  press: inside the follow-up window (`hitlag_mul`, reused as the jab timer)
  with `x2218_b1` raised, `unk_msid` 44 starts the second jab and 45 the
  third, any other remembered jab does nothing; otherwise `checkAttack11`
  starts the first jab. Without a press the timer counts down once. A press
  that selects nothing also counts down.
- `checkAttack11`: enters Attack11 (`Ft_MF_None`), sets the timer to
  `jab_2_input_window`, remembers 44, clears `x2218_b1`, `x2218_b2`,
  `mv.co.attack1.x0` and `x1A54`. `doAttack12Normal`: enters Attack12, timer
  `jab_3_input_window`, remembers 45, clears `x2218_b1` and `attack1.x0`
  (`x2218_b2` and `x1A54` persist). `doAttack13`: enters Attack13 and clears
  `x2218_b1` only.
- `Fighter_ChangeMotionState` resets the timer unless the new motion is
  Wait or a walk (ids 14..=17, `fighter.c:1143`), so a jab that ended into
  Wait still accepts its follow-up from Wait or Walk inside the window.
  `unk_msid`, `x2218_b1` and `x2218_b2` survive motion changes.
- `ftCo_Attack11_IASA` / `ftCo_Attack12_IASA`: while `allow_interrupt`,
  smashes then tilts; on every frame `ftCo_Attack_800D6A50` then
  `checkAttack12` / `checkAttack13`; while `allow_interrupt`, jump, dash,
  squat, turn and walk. No jab, catch, shield or special.
  `ftCo_Attack13_IASA`: `ftCo_Attack_800D6A50` first, then the complete Wait
  chain while `allow_interrupt`.
- `checkAttack12` / `checkAttack13`: while the timer is positive it counts
  down and a fresh A press latches `attack1.x0`; the follow-up starts when
  the latch and `x2218_b1` are both set.
- `ftCo_Attack_800D6A50`: every frame, a fresh A press or release increments
  `x1A54`; reaching `rapid_jab_window` with `x2218_b2` raised enters
  Attack100Start (`ftCo_800D6B00`: clears `mv.co.attack100.x0` and `x4`).
- Attack100Start has no chain and enters Attack100Loop with
  `Ft_MF_SkipAttackCount` (the action instance is kept) when it ends. The
  loop figatree wraps: on its frame zero `ftCo_Attack100Loop_Anim` sets `x0`
  and runs `ft_800892A0` and `ft_80089824` (stale-instance restart and two
  identity-0 action instances, as the down tilt's exit does); the script
  re-creates the loop's hitboxes each cycle, which clears their victims.
  `ftCo_Attack100Loop_IASA` latches `x4` on a fresh A press or release. At
  the loop check, `x0 && !x4` enters Attack100End, otherwise `x4` clears.
  Attack100End has no chain and returns to Wait (Fall off a floor) through
  `ft_8008A2BC`, as the first three jabs do.
- Physical Z is the logical A press and release (`Fighter_procInput`).

Not modeled: item throws and drops, the Game & Watch, Pikachu/Pichu
(`Ft_MF_SkipAttackCount` first jab with the deferred instance restart) and
Marth (third jab repeating the first) overrides, the rapid loop's sound and
graphics.

The jab family maps to Slippi states 44..49 with animation indices 46..51
(`Attack100Start`, `Loop` and `End` share move identity 4; the first three
jabs use 1, 2 and 3). The timer, remembered jab, latches, flags and press
count survive checkpoints. `jab_differential` drives the pinned
`ftCo_Attack1_CheckInput`, `checkAttack11`, `doAttack12`, `checkAttack12`,
`doAttack13`, `checkAttack13`, `ftCo_Attack11_IASA`, `ftCo_Attack12_IASA`,
`ftCo_Attack13_IASA`, `ftCo_Attack_800D6A50`, `ftCo_800D6B00`,
`ftCo_Attack100Loop_Anim` and `ftCo_Attack100Loop_IASA` over generated
press/release and script-flag sequences and compares every state field with
the native module.

Entry (`checkAttack11`/`ftCo_800D6B00` and the next animation update for
`doAttack12Normal`/`doAttack13`, before any callback reads them) samples the
entry pose's own commands immediately, so the first jab's `rapid: Some(true)`
on pose 0 is live from the frame it starts.

`tests/game_jab.rs` covers the first jab from Wait (a held press is not
fresh), the buffered follow-up latching on an uninterruptible pose and firing
on the first follow-up-ready pose, the second/third chain and the third
jab's lack of a follow-up, the follow-up window surviving Wait and decaying
or resetting there, interruptible-pose priority (smash, then tilt, then
jump/dash/squat/turn/walk, with the first two jabs reaching neither shield
nor catch and the third reaching the complete Wait chain), the rapid jab's
press/release entry count (holding does not add to it), the Start-to-Loop
transition's kept action instance and the loop's own frame-zero identity
restart, tapping through the loop's continuation check to keep it going and
re-hit an in-reach victim each cycle, ending the loop without input, a later
pose's `rapid: Some(false)` turning the flag back off, `clear_hits` letting
one group hit twice within a jab, the press count persisting into the second
jab and resetting on a fresh first jab, the second jab's root motion in both
facings, checkpoint replay in every phase, and invalid resources.
`crates/cli/tests/replay_match.rs` matches a three-jab combo and a rapid jab
against their own Peppi-written bytes and detects a removed press.
