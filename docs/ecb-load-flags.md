# ECB load flags per collision path (design note)

Evidence: with the corrected ECB joints (pack v4: Fox `[41, 55, 25, 13, 7,
4]`, raw `ftData.x44` joint indices), Skirmish lands the entry fall at -50
while the recording lands at -49 (TopN: -52: 1.7649, -51: -0.3051, -50:
-2.6051, -49: 0.0001). With bottom offset b (lowest ECB joint above TopN
after the loader's adjustments), the game's numbers need 2.6051 < b <
5.135 while Skirmish's need 0.3051 < b < 2.6051: a two-unit difference,
which is exactly `mpColl_LoadECB_JObj`'s padding (`mpcoll.c:392-397`:
`if (!(flags & 4)) { left -= 2; right += 2; bottom -= 2; top += 2 }`).

Skirmish applies one static `CollisionBox::Bones.flags` (0 in every pack)
on every load (`src/game/collision.rs::sample` -> `ecb::load_joints`).
The game passes the flags per collision entry point (`mp/mpcoll.c`):
- airborne ordinary: `mpColl_80047E14` (2831-2837) and the other
  `mpColl_LoadECB_inline(coll, 6)` sites (2741, 2755, 2777, 2784, 2806):
  flags 6 = 0x4 (no padding) | 0x2;
- grounded ordinary: `mpColl_8004B2DC` (4010-4015) and the other
  `mpColl_LoadECB_inline(coll, 5)` sites (2762, 2791, 2908, 3999, 4013,
  4027, 4394): flags 5 = 0x4 | 0x1 (bottom anchored to 0);
- a grounded variant with flags 9 (4034: 0x8 narrow width | 0x1);
- the special paths with 0x12 (2823, 2852, 2878, 2897, 2916: 0x10
  two-unit height | 0x2) reached through `mpColl_80047D20` and its
  siblings (which fighter callers use them: `ft_081B.c`).
`mpColl_LoadECB_inline` (624-641) also preserves the desired bottom while
`CollData_X130_Locked` is set. Bits: 0x1 anchor bottom (`if flags & 1`),
0x2 (consumed by the sweep `inline1(coll, 2|6, ...)`: find its meaning),
0x4 skip the two-unit padding (`CollisionFlagAir_CanGrabLedge` is the
enum name; check that name's other uses), 0x8 narrow width to ±1, 0x10
two-unit height.

## Behaviour implemented

`game::collision::sample` (`src/game/collision.rs`) now computes the
`load_joints` flags per call from `f.grounded` (`load_flags`) instead of
reading `CollisionBox::Bones.flags`: 6 (airborne) or 5 (grounded). Every
`sample` call site in this codebase (the hitlag, rebirth, ledge-attach and
grab-capture variants, and the ordinary per-frame call at the bottom of the
main loop) is this same fighter-collision-check, distinguished at runtime
only by whether the fighter is currently grounded, matching the source: the
airborne callers (`ft_CheckGroundAndLedge`'s `mpColl_800473CC`/
`mpColl_800471F8`, `ft_80083090_inline`/`ft_800831CC`'s `mpColl_80047AC8`/
`mpColl_80047E14`) all pass 6; the grounded caller (`ft_800827A0`'s
`mpColl_8004B2DC`) passes 5. `CollisionBox::Bones.flags` is kept in the
schema only so existing/exported packs still deserialize (every pack sets
it to 0, and it is read nowhere); this is documented on the field itself.

The narrow-width (9, `ft_800843FC`'s `mpColl_8004B5C4`) and two-unit-height
(0x12, the special-state entry points) variants are not wired to any
Skirmish collision path: this codebase does not yet model the distinct
fighter states that use them (e.g. a dash-pushed-into-wall grounded
narrowing) as separate collision paths from ordinary grounded/airborne
movement. `load_joints` already accepts any flags value (it is exhaustively
differential-tested against the oracle for every value 0..32, not just
5/6/9/0x12 -- see "Tests" below), so wiring such a path in later only needs
its own call to `load_flags`'s pattern, not a change to the loader.

### Bit 0x2, resolved

0x2 is `CollisionFlagAir_PlatformPassCallback`, but it is **not** a bit
`mpColl_LoadECB_JObj`/`load_joints` ever reads (grep the function: only
bits 1, 4, 8 and 0x10 appear). It belongs to a different, independently
chosen `u32` -- the sweep engine `mpColl_80046904`'s own `flags` argument
(the "mode" `i` passed to `inline0`/`inline1`, e.g. `ft_80083090_inline`
passes loader-flags 6 but sweep-mode 2 or 6 depending on ledge cooldown,
while `ft_CheckGroundAndLedge` passes the same loader-flags 6 but
sweep-mode 0 or 4, never setting bit 0x2). Both parameters happen to reuse
the same literal names (`CollisionFlagAir_StayAirborne = 1`,
`_PlatformPassCallback = 2`, `_CanGrabLedge = 4`) for two unrelated
bitfields -- a decomp naming coincidence, not a shared value.

When set in the sweep's flags, bit 0x2 installs the fighter's own floor
callback (`mpColl_804D64A0`, set by `inline1` before the callback graph
runs) so `mpColl_80044628_Floor`/`mpCheckFloor` (`mpcoll.c:1396-1462`,
1611-1671) can let a one-way platform's line pass instead of counting it as
solid ground. This codebase already ports exactly that substitution as
`escape_air::platforms_land` (FallSpecial's stick-down pass-through,
`ftCo_80096CC8`) feeding `stage::sweep_filtered` in `game::collision::
resolve`'s floor query, and `f.skip_floor` already ports the unconditional
platform-index skip (`floor_skip`) that applies regardless of this bit. No
sweep code change was needed for this batch; it was already correct.

The grounded mode argument (`ft_800827A0`'s literal `2` passed to
`inline2`) is a wholly separate "clamp/teeter" selector into
`mpColl_8004ACE4` (3848-3967: `flags & 1` / `flags & 2`), already ported as
`floor_end_clamp`'s `Mode` enum (`docs/edges.md`'s "Ground collision modes"
table).

### The `ecb_unlocked` floor-snap fix

Fixing the flags alone corrected the entry fall's *trajectory* (frames
-52..-50 now match the recording bit-for-bit) but exposed a second, real
bug at the landing frame itself (-49): the recording lands at position.y
`0.0001`, while Skirmish (with only the flags fix) computed `-4.0348`.

The airborne (unanchored) ECB's raw sampled bottom for this pose is
consistently *above* position (about +4.02 to +4.03 across these frames,
not clamped since `load_joints` only clamps a negative bottom to 0, never a
positive one down). `game::collision::resolve`'s floor-contact branch
unconditionally rested `position + ecb.current.bottom` on the floor line,
which is wrong whenever that raw bottom is positive: the source's
`mpColl_80046904` computes `bool ecb_unlocked = coll->ecb.bottom.y > 0.0F`
and forwards it as `mpColl_80044838_Floor`'s `ignore_bottom` (mpcoll.c:2469-
2486, 1464-1500): when true, that function rests `cur_pos` **itself** on the
floor (`bottom = coll->cur_pos`) instead of `cur_pos + ecb.bottom`. Ported
in `game::collision::resolve`'s floor-contact branch (the `unlocked`
local): the projection point becomes `position` alone whenever
`f.ecb.current.bottom[1] > 0.0`. With that fix, frame -49 lands at
`0.0001001358`, matching the recording. The same `ignore_bottom` pattern
also appears in the wall-hug floor functions (`mpColl_80043C6C`/
`mpColl_80043F40`); this codebase's wall-hug handling does not implement
their floor-follow variant at all (`game/collision.rs`'s own header
comment already disclaims this), so it is unchanged.

### Known gap: a missing `move_id` blocks stepping the v4 pack past frame ~71

Independent of anything in this batch: replaying pack v4's actual recorded
inputs frame-by-frame (not just checking `validate-replay`'s first
divergence, which stops earlier) hits `Error::Data("staling requires an
explicit attack move_id")` at frame 71, when P1 transitions out of GuardOn
into an action `game::staling::flush` classifies as an attack with no
`move_id` set in the pack. This blocks reaching the jump landing at frames
418-421 through the pack's own real inputs; only the recording's own
(ground-truth, independent of Skirmish) values at 421 -- landed,
position.y `0.0001001358` -- have been confirmed, via the same mechanism
as the entry fall. This is a resource-export gap, not a collision-flags
bug; a future batch that finishes exporting `move_id` for every staling-
relevant action would unblock it.

## Tests

- `tests/ecb_differential.rs` (existing, unchanged): `load_joints` is
  already differential-tested against the oracle for every flags value
  0..32 (`load_modes_flags_rotation_lock_and_clear_match_source`'s
  `for source_flags in 0..32`, plus the property test's `options in
  0_u32..32`), so it already covered 5, 6, 9 and 0x12 before this batch;
  no adapter change was needed.
- `tests/game_collision_bones.rs`: two new synthetic tests --
  `airborne_bones_ecb_has_no_two_unit_padding` (mode 6, no padding, via the
  default idle/fall pose) and
  `falling_bones_ecb_lands_on_position_then_anchors_the_bottom_to_zero`
  (a fall from a raw-bottom-above-position pose, landing on `position`
  itself, then the grounded mode 5 anchoring the bottom to exactly 0).
- `crates/cli/tests/ecb_load_flags_v4.rs`: pins the real recording's entry
  fall (P1 frames -52..-49, exact values above) against pack v4, gated on
  that export existing at its fixed archive path (skips otherwise, like
  `real_parity.rs`'s `SKIRMISH_GAMEPLAY_DATA` gate).

No existing test's expectation was wrong under the old (buggy) behavior in
a way this batch needed to update: `tests/game_collision_bones.rs`'s prior
tests already hardcoded `flags: 5` in their own fixture (sidestepping the
resource-flags bug rather than exercising it), so they pass unchanged
under the new per-path computation.

## Measurement (2026-09-11, gameplay export v4)

`make-initialization` + `validate-replay` against pack v4's
`fox-fd/match-data.json` and `tests/fixtures/slippi/parity/fox-fd.slp`:

- Before this batch's fixes (flags always read from the resource, always
  0): entry fall lands one frame early, at -50 instead of -49.
- After the `load_flags` fix alone: the trajectory for -52..-50 matches
  the recording exactly, but landing position at -49 is wrong (-4.0348
  instead of 0.0001) -- `checked_frames` 74, first divergence at -49,
  field `position.y`.
- After the `ecb_unlocked` floor-snap fix too: `checked_frames` 93, first
  divergent frame -30, field `action_age` (P1's Dash -> Turn transition
  reports age 0 where the recording reports 1). This is a different,
  unrelated subsystem (action/turn-state age tracking, not collision or
  ECB flags) -- outside this batch's scope, reported here rather than
  chased further. `tests/fixtures/slippi/parity/fox-fd-baseline.json` is
  updated to this new frame/field (see `docs/parity.md`'s own entry).

This measurement used the v4 pack directly
(`/mnt/archive/datasets/melee/skirmish-gameplay/v4-snapshot-20260911/
fox-fd/match-data.json`), not the `SKIRMISH_GAMEPLAY_DATA`-gated v3 pack
`docs/parity.md`'s main ratchet measures against; that ratchet will move
once the v4 pack (or later) is published at the `SKIRMISH_GAMEPLAY_DATA`
location.
