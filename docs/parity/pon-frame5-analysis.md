# Pon Fox frame-5 parity analysis

Run date: 2026-09-15. This records the frame-5 diagnosis from the unchanged
v10 compact gameplay pack and `tests/fixtures/slippi/parity/fox-fd.slp`.

The fresh full-pack artifact run reaches the first observation mismatch at
frame 5 after 128 matching frames:

| port | field | expected bits | actual bits |
| --- | --- | --- | --- |
| P1 | `position.x` | `0xc1edec00` | `0xc1edec01` |

The mismatch is isolated to P1. Frames -123 through 4 match bit-for-bit for
all compared fields. P1 remains action state `0x2b` (Wait) through frame 5;
there is no action transition, input conversion error, landing transition, or
stage motion at the boundary. The diagnostic trace shows P1's position and
action age matching at frames 3 and 4, then the one-ULP position difference at
frame 5. P2, which is in Jump at the same boundary, remains bit-identical.

The common movement path is `simulation::move_fighter`'s grounded ordinary
friction branch. It calls `Movement::friction_ground`, then
`Movement::project_ground`, updates `ground_velocity`, forms `f.velocity` as
`self_velocity + animation_velocity`, and integrates position in the source
order: nudge, self velocity, knockback, and attacker-shield push. P1's trace
has no relevant knockback or nudge contribution; the position delta is the
grounded velocity produced after an earlier air-dodge launch and decay.

The corresponding pinned C path is `ftCo_Wait_Phys` → `ft_80084F3C` and
`Fighter_procUpdate`. `ftCommon_ApplyFrictionGround` uses the same strict
comparison as `Movement::friction_ground`; `ftCommon_ApplyGroundMovementNoSlide`
projects the same ground normal; and `Fighter_procUpdate` performs the same
ordered position additions. The existing host C differential evidence for the
generic arithmetic path agrees bit-for-bit, so changing addition order,
introducing an epsilon, or changing the replay tolerance would be unjustified.

The candidate upstream source is the earlier `ftCo_80099A9C` air-dodge launch
(`fighter::escape_air::launch_velocity`), whose `atan2f`/`cosf`/`sinf` result is
then multiplied by the launch force and decayed over subsequent air frames.
The current implementation uses the disassembly-backed compatibility trig
functions in `src/compat/math/trig.rs`; the retail launch function has no fused
or double-precision arithmetic. The full chain has already been tested against
the host-compiled decomp C and the retail instruction audit rules out the two
common explanations (FMA contraction and a hidden double intermediate).

Therefore the evidence establishes only a one-ULP difference whose first
observable effect is frame 5. A different hidden launch velocity or angle is a
plausible upstream explanation, but it is not proved: Slippi does not expose
P1's hidden `self_vel` on the frame-4 landing transition, and the observed
position/action fields do not identify which intermediate value differed.
There is no correct source-level arithmetic fix established by this run.
Ownership remains an investigation boundary around the generic escape-air
math (`src/fighter/escape_air.rs` plus the compatibility trig module), pending
an independent reference for the exact SDK result. No code, baseline, resource
pack, or observation tolerance was changed for this analysis.

## Authoritative source check

The pinned local upstream is `/mnt/shared/Projects/Code/External/melee` at the
revision recorded in `upstream.lock.json`. The relevant source is
`src/melee/ft/kinds/ftCommon/ftCo_EscapeAir.c:36-65`: `inlineA0` stores the
pre-launch velocity, computes `angle = ftCommon_8007D9D4(fp)`, and assigns
`escapeair_force * cosf(angle)` and `escapeair_force * sinf(angle)` before the
motion change. `ftCommon_8007D9D4` is
`src/melee/ft/ftcommon.c:616-619` and returns `atan2f(stick.y, stick.x)` with
no facing transform. `ftCo_EscapeAir_Phys` at lines 98-106 applies the decay
as two independent scalar products.

The current Rust boundary mirrors that source in
`src/fighter/escape_air.rs:7-15` and `:19-23`; its trig calls route to the
source-backed `src/compat/math/trig.rs`, which ports `src/MSL/trigf.c` and
`src/melee/lb/lbtrigf.c`. The frame-4 controller sample delivered by the
adapter is P1 stick `[-0.7375, -0.6625]`, with buttons `0x440` and trigger
`1.0`; these are processed values exposed by the replay adapter, not a claim
that the original console retained more precision. The trace records the
simulator's hidden post-step P1 velocity as x `0xc004d5a1` (`-2.0755389`) and y
`0xbfeea6de` (`-1.8644674`). Slippi's expected observation leaves these
velocity slots unmapped on this grounded landing frame, so there is no
independent expected launch-velocity bit pattern to compare. Subsequent
grounded x velocities are
`0xbff53061`, `0xbfe0b580`, `0xbfcc3a9f`, `0xbfc1fd2e`, `0xbfb7bfbd`,
`0xbfad824c`, `0xbfa344db`, `0xbf99076a`, and `0xbf8ec9f9` through frame 5;
the first position bit exposed by accumulation is frame 5's
`0xc1edec00`/`0xc1edec01` split.

The authoritative files and the trace were inspected with:

```text
sed -n '36,106p' /mnt/shared/Projects/Code/External/melee/src/melee/ft/kinds/ftCommon/ftCo_EscapeAir.c
sed -n '616,619p' /mnt/shared/Projects/Code/External/melee/src/melee/ft/ftcommon.c
sed -n '1,30p' src/fighter/escape_air.rs
rg -n 'frame -4|frame -3|frame -2|frame -1|frame 0|frame 1|frame 2|frame 3|frame 4|frame 5' /tmp/pon-frame5-dump.log
```

The v10 Fox pack supplies escape-air constants as f32 values: deadzone
`[0.25, 0.25]`, force `3.0999999046325684` (`0x40466666`), and decay
`0.8999999761581421` (`0x3f666666`). The adapter quantizes the frame-4 stick
sample to the values above, and the implementation uses those pack constants
directly. This constrains the launch calculation but cannot prove the
console's hidden intermediate without an independent velocity trace or a
verified SDK result.

The archive contains nine additional pairing packs (`fox-bf`, `fox-dl`,
`fox-fod`, `fox-ps`, `fox-ys`, plus the Falco packs), and the manifest lists
`fox-fd-2.slp`, `fox-fd-3.slp`, and `fox-fd-4.slp` against the same `fox-fd`
pack. A fresh artifact-only check was started for `fox-fd-2.slp` with the
unchanged compact pack. `make-initialization` succeeded for P1/P2, seed
`3847017820`, and `next_frame = -123`, proving that this sibling uses the same
resource schema and reaches native initialization. The subsequent full
`validate-replay` invocation did not produce a report before the execution
harness stopped the long-running Pon process, so this run supplies no new
first-divergence field. The historical baseline records `-9`, but it is not
treated as a fresh result here.
