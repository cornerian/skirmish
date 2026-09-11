# Landing interrupt window

`skirmish::game::landing` ports the interrupt window of the ordinary Landing
from `ftCo_Landing.c` (pinned decomp rev 0bac93a5). `LandingFallSpecial` (air
dodge and other special landings) is the separate, always-locked-out state
covered in [air-dodge.md](air-dodge.md); this profile does not change it.
`ftCo_Landing_Phys`/`ftCo_Landing_Coll` (the ground friction and collision
callbacks) are untouched by this batch; whether `ft_80084280` differs from
the ordinary `ft_80084104` ground-collision variant remains unexamined.

`movement.normal_landing_lag` (`ftCo_DatAttrs.normal_landing_lag`, fp+1F4) is
an optional per-fighter float on `MovementData`. Without it, Landing keeps its
previous chainless behaviour: it plays for `landing_frames` and returns to
Wait with no input at all. With it, `Fighter.landing_allow_interrupt`
(`mv.co.landing.allow_interrupt`, set `true` by the ordinary Landing entry
and reset to `false` by every other transition, including
`LandingFallSpecial`) and `tilt::interrupt_chain` open the complete Wait
chain once `cur_anim_frame >= normal_landing_lag`, exactly like an
interruptible tilt or dash attack pose already does:

- `smash::interruptible`, `dash::interruptible` and the new
  `landing::interruptible` are the three conditions `tilt::interrupt_chain`
  checks before returning `Chain::Wait`, so every existing Wait-chain
  dispatcher (grab, shield, special, smash, tilt, jab, jump, dash, turn,
  walk) already reaches Landing for free once it is interruptible.
- The one Landing-specific narrowing is the squat entry
  (`ftCo_Landing.c:146-147`): `RETURN_IF((cur_anim_frame < frame_speed_mul +
  normal_landing_lag) && ftCo_SquatWait_CheckInput(gobj))`. Ordinary
  Landing's `anim_speed` (and so `frame_speed_mul`) is always `1.0`
  (`ftCo_Landing_Enter_Basic`), so this is the single first interruptible
  frame only; `locomotion::update_actions` gates the crouch branch of its
  Wait/Walk/interruptible-tilt match arm on `landing::squat_window` to
  reproduce it. Turn and Walk are not narrowed and stay reachable on every
  interruptible frame.
- `landing::interruptible` mirrors `dash::interruptible`'s shape: grounded,
  owns the action, `landing_allow_interrupt` set, and
  `cur_anim_frame >= normal_landing_lag`.

`ftCo_Landing_Enter(gobj, msid, allow_interrupt, flags, start, rate)` runs
`ftCommon_8007D7FC` (landing cleanup, unmodeled), `Fighter_ChangeMotionState`,
then sets `mv.co.landing.allow_interrupt`; `ftCo_Landing_Enter_Basic`
(ordinary landings, `collision.rs`'s Landing entry) passes `Landing, true`.
`ftCo_LandingFallSpecial_Enter_Basic` passes `LandingFallSpecial, false`, and
`ftCo_LandingFallSpecial_Enter(gobj, allow_interrupt, landing_lag)` (air dodge
and special landings) passes the caller's flag with rate `(0.1 + x2EC) /
landing_lag`; both remain `escape_air.rs`'s unmodeled-input
`LandingFallSpecial`, whose own `aerial.allow_interrupt` stays `false` and is
not the same field as `landing_allow_interrupt`. The per-kind resets in
`ftCo_Landing_Enter`'s switch (Mario/Dr. Mario's tornado/cape flags, Peach's
`specialairn_used` and conditional `ftPe_8011D598`, Marth/Roy's `x222C`,
Game & Watch's `x2234`, the Ice Climbers' `x224C`, Kirby's three flags,
Mewtwo's confusion-boost flag) are character-specific state this port does
not model.

`ftCo_Landing_Anim` (end at `landing_frames` into `ft_8008A2BC`, i.e. Wait)
is unchanged by this batch.

Validation: `normal_landing_lag`, if present, must be finite, `>= 0` and
`<= landing_frames`.

`tests/game_landing.rs` builds a short hop (X press then release) into
Landing with `landing_frames` widened to 6 and `normal_landing_lag` set to
3.0 (the fixture's own `landing_frames` is 2, too short for a three-frame
window) over combined grab, tilt, smash, jab and dash-attack profiles. It
covers: every one of A, L, X, Z, stick down, a fresh dash-magnitude stick and
a fresh C-stick being ignored on the two frames before the lag; the first
interruptible frame accepting a jab, a moderate-stick tilt, a catch, a fresh
shield press, a jump, a fresh dash stick, a fresh C-stick smash and the
crouch; the second interruptible frame accepting everything again except the
crouch (stick down leaves Landing unchanged, since `cur_anim_frame` is no
longer below `frame_speed_mul + normal_landing_lag`); Turn and Walk from an
interruptible frame; the animation still ending in Wait with no input;
`landing_allow_interrupt` being `true` for the ordinary entry and `false`
(reached the same way as `tests/game_air_dodge.rs`) and still fully locked
for `LandingFallSpecial`; checkpoint replay across every interrupt phase;
invalid resources (non-finite, negative, above `landing_frames`); and `None`
keeping a chainless Landing where a fresh A press well past where the lag
would otherwise open the chain still does nothing.

## Oracle

`ftCo_Landing.c` is pinned at `tests/oracle/original/landing.c`
(`ftCo_Landing_Enter`, `ftCo_Landing_Enter_Basic`,
`ftCo_LandingFallSpecial_Enter_Basic`, `ftCo_LandingFallSpecial_Enter`,
`ftCo_Landing_IASA`). The host adapter `tests/oracle/landing.c` exposes
`oracle_landing_iasa` (frame, rate, lag, allow_interrupt and a per-call
`answers` bitmask in, the ordered call trace and which callee fired out) and
`oracle_landing_enter` (an entry variant, the caller allow flag and a fixed
`x2EC` in, the resulting motion/allow/rate out); every non-Landing callee is
a simple stub, and the fighter-kind switch's character union fields are
reproduced so the pinned body compiles unchanged with `kind` fixed at the
default (no case matches). `landing_differential` is a Rust mirror of the
exact `ftCo_Landing_IASA` dispatch order and the entry helper's `msid`/
`allow`/`rate` arithmetic (`(0.1 + x2EC) / lag`, bit-exact), checked with
proptest over generated frame/rate/lag/allow/answers combinations plus exact
boundary cases (`frame == lag`, `frame == rate + lag`, `allow == false`).

`crates/cli/tests/replay_match.rs` matches a short-hop landing followed by a
C-stick smash on the first interruptible frame against its own Peppi bytes
(Slippi state 42 for the Landing rows, then the smash state), and reports a
Mismatch at the row where a removed C-stick sample's effect first appears.
