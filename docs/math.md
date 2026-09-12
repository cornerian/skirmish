# The game's own trigonometry (`src/math.rs`)

## Why

`docs/parity.md`'s real-replay measurement (gameplay export v6,
`fox-fd.slp`) reached frame 5 before diverging on `position.x` by a few
ULP, traced to `fighter::escape_air::launch_velocity`'s `libm::cosf`/`sinf`
of an air dodge's stick angle around frame -70: `libm` (a portable, generic
`f32`/`f64` reimplementation used here for cross-platform replay
determinism) is a different transcendental-function implementation than the
one the original GameCube binary actually shipped, and the two do not agree
bit for bit. This batch replaces the game's own transcendental call sites
with direct ports of the pinned decompilation's own math routines instead.

## Method: disassembly, not decompiled C, decides what is fused

The pinned decompilation (`src/MSL/trigf.c`, `src/melee/lb/lbtrigf.c`) is
*value*-equivalent to the retail binary, not *instruction*-equivalent: a
decompiler reconstructs readable C that computes the same result, but the
exact grouping of `a * b + c` into a single fused, once-rounded PowerPC
instruction (`fmadds`/`fmsubs`/`fnmadds`/`fnmsubs`) instead of two
separately-rounded operations is invisible in that C text. An earlier
revision of this batch guessed which operations were fused from real-replay
measurement alone (fuse a chain, remeasure, keep whichever ties or improves
the baseline) and got the *shape* of the fusion wrong in more than one place
even where it accidentally reproduced the right final bits (documented below
under "What measurement-only guessing got wrong"). This revision instead
disassembles the actual retail `main.dol` (Capstone, PowerPC, big-endian) and
reports, address by address, which instructions are genuinely fused, then
ports each one with `f32::mul_add` (or a negated/subtracted form) at the
exact expression the disassembly shows -- not the expression shape the
decompiled C happens to display.

Concretely: `main.dol` (`/mnt/archive/runs/melee-assets-20260909/disc/sys/
main.dol`) is parsed as a standard Nintendo DOL (the 7 text-section-header
triples of file offset/load address/size, then 11 data-section triples,
starting at file offsets `0x00`/`0x1C`/`0x48`/`0x64`/`0x90`/`0xAC`), giving a
load-address-to-file-offset mapping for each pinned function's address
(`config/GALE01/symbols.txt`, e.g. `sinf = .text:0x803263D4; ... size:0x1A4`).
Capstone's PowerPC big-endian mode disassembles each function's raw bytes at
that address, and the output is read directly for `fmadds`/`fmsubs`/
`fnmadds`/`fnmsubs` (the "s" suffix is the single-precision, one-rounding
form) versus plain `fmuls`/`fadds`/`fsubs`/`fdivs`. A sibling batch
(`skirmish-fma`) built a small, reusable version of exactly this tool
(`tools/ppc_fma_audit.py`, `uv run --with capstone`); this batch's own
findings (below) were derived independently first, then cross-checked
against that tool's output for every function ported here, and the two
agree exactly, function for function and instruction for instruction.

## What was ported, and from where

- `sinf`, `cosf`, `tanf` -- `src/MSL/trigf.c` (the Metrowerks MSL runtime
  library the retail build links). `tanf` is literally `sin__Ff(x) /
  cos__Ff(x)`, the pinned source's own non-inlined aliases (confirmed:
  `tanf`'s own compiled body, `0x803261BC`, has no fused arithmetic of its
  own -- it just calls `cos__Ff`/`sin__Ff`, i.e. `cosf`/`sinf`, and divides).
  Their `__sincos_on_quadrant`/`__sincos_poly` tables and
  `__four_over_pi_m1` range-reduction constants come from the same
  library's `src/MSL/math_data.c`; `trigf.c`'s own static constructor
  (`__sinit_trigf_c`) that would otherwise copy them in at process start is
  inlined directly, since the values never change at runtime.
- `atan2f`, `atanf`, `acosf`, `asinf` -- `src/melee/lb/lbtrigf.c` (HSD's own
  "lb" math library; this, not the MSL library, is what the fighter/common
  code actually calls for these -- confirmed by tracing `ftCommon_8007D9D4`,
  the stick-angle helper `fighter::escape_air::launch_velocity` and
  `fighter::aerial`/`game::shield`/`game::characters::fox::up` all port,
  which calls `atan2f`). `atanf` uses the pinned source's `__MWERKS__`-only
  body (a silver-ratio range reduction into a five-way piecewise table, a
  degree-13 odd minimax polynomial, then table-selected offsets) rather
  than the non-`__MWERKS__` fallback in the same file: the retail binary
  was built with Metrowerks CodeWarrior, so the `__MWERKS__` body, not the
  unreachable fallback, is what actually shipped (also directly confirmed by
  disassembly: the compiled `atanf` matches this body's structure, not the
  fallback's).
- **Deferred, out of scope for this batch:** `sqrtf`. `src/MSL/math_ppc.h`'s
  `sqrtf` is `__frsqrte`-based (the same real hardware reciprocal-square-root
  *estimate* instruction discussed below for `acosf`/`asinf`, refined by
  three Newton-Raphson steps); modeling the exact hardware estimate table
  (a specific bit-manipulation lookup, not visible in any C source) is a
  separate, larger effort than this trigonometry batch.
  `lbvector.c`'s `lbvector_sin`/`lbvector_cos` (a "best quintic
  approximation" fast path used only by `lbVector_RotateAboutUnitAxis`) was
  also surveyed but has no current Skirmish caller to wire up.

Every ported function's control flow, branch structure and constant tables
come from the pinned decompiled C; every fused operation comes from the
disassembly instead, documented function by function below and at each
`f32::mul_add` call site in `src/math.rs` with its instruction address.

## Call sites replaced

`libm::sinf`/`cosf`/`tanf`/`atan2f`/`atanf`/`acosf`/`asinf` calls that port
game logic (not rendering) now go through `crate::math`:
`src/collision/{ecb,bones}.rs`, `src/compat/{bytecode,math}.rs` (the
`sin`/`cos`/`tan`/`atan`/`asin`/`acos` bytecode opcodes and
`atan2_degrees`; `log`/`exp`/`sqrt` are unchanged), `src/fighter/{damage,
aerial,escape_air}.rs`, `src/game/{damage,ledge,shield}.rs`,
`src/game/characters/fox/up.rs`, and `src/quaternion.rs` (`matrix_to_euler`,
`interpolate`'s `atan2f`/`sinf`/`acosf`; `from_euler`'s `glam`-based
`sin_cos` is unaffected -- see that module's own doc comment).
`crates/renderer` is untouched throughout, as directed.

## The `acosf`/`asinf` finding: real hardware, not the decompiler's placeholder

The first revision of this batch left `acosf`/`asinf` ported but unwired,
based on reading `lbtrigf.c`'s decompiled C: both call `__frsqrte`, and this
decompilation project's own `placeholder.h` defines the *host-tooling*
stand-in for that macro as `#define __frsqrte(x) sqrt(x)` (guarded
`#ifndef MWERKS_GEKKO`) -- a square root, not a reciprocal-square-root
estimate, which does not converge under the following Newton-Raphson
refinement except very close to `x == 1` (confirmed empirically:
`asinf(0.9999)` returned roughly `2.7°` instead of the true `~89.2°`, when
seeded that way). Disassembling `acosf`/`asinf`/`lb_sqrtf` at
`0x80022D1C`/`0x80022DBC`/`0x80022DF8` settles the question the decompiled C
cannot: the retail binary's compiled instructions call `frsqrte`, the
genuine PowerPC reciprocal-square-root *estimate* instruction (accurate to
roughly 1/4096 relative error, per the architecture manual), not `sqrt`.
`placeholder.h`'s stand-in is exactly what its name says -- a decompiler
tooling convenience, never real hardware behavior, on this function.

Modeling the exact hardware estimate table (the specific bit-manipulation
lookup PowerPC's `frsqrte` uses) is still out of scope here, like `sqrtf`
generally. Instead, `frsqrte_newton3` (`src/math.rs`) seeds the same
disassembly-derived Newton-Raphson refinement from an accurate `1/sqrt(x)`
(`1.0_f64 / f64::from(x).sqrt()`) rather than the hardware estimate or the
placeholder's wrong-direction `sqrt(x)`. Three Newton iterations converge
quadratically regardless of which sufficiently-close seed they start from,
so this reaches the same correctly-rounded fixed point as real hardware for
essentially every input -- confirmed directly, not assumed: swept across
`-0.999..=0.999` in 2000 steps, the largest disagreement against `std`'s
`acos`/`asin` was on the order of `5e-7`, near the limit of `f32` precision
itself. `acosf`/`asinf` are wired to their real call sites
(`fighter::damage::vector_angle`, `game::characters::fox::up::angle_xy`,
`quaternion::interpolate`) on the strength of that measurement.

Each Newton-Raphson iteration itself (confirmed by disassembly, at
`0x80022D54`/`0x80022D64`/`0x80022D74` for `acosf` and the analogous
addresses in `lb_sqrtf`) computes `g*g` and `0.5*g` as separate plain
multiplies, but fuses `3.0 - x*(g*g)` into one `fnmsubs`; `acosf`'s and
`asinf`'s own leading `1.0 - x*x` (`0x80022D30`/`0x80022DD4`) is likewise one
fused `fnmsubs`, not a separate multiply and subtract -- both ported that way
in `src/math.rs`.

(A second, unrelated discovery from the same investigation: this crate's C
oracle is one shared static library across every `*_differential.rs` test,
and Rust's own `std` links against the platform C library by the same
symbol names -- `f32::sin`/`cos`/`tan`/`asin`/`acos`/`atan`/`atan2` call
`sinf`/`cosf`/... via FFI. Defining same-named strong `sinf`/`cosf`/...
symbols in the oracle's static library silently replaced `std`'s own trig
methods process-wide the moment both were linked into the same test binary
-- caught because a `math.rs` unit test comparing this port against
`x.asin()` "ground truth" started comparing the port against itself, once
`--features c-oracle` was enabled. `tests/oracle/trigf_body.c`/
`lbtrigf_body.c` rename the pinned bodies to process-unique names
(`skirmish_msl_sinf`, `skirmish_lb_atan2f`, ...) via macro before including
the pinned `.inc`, and `tests/oracle/trig.c`'s `oracle_*` wrappers call
those renamed symbols. A few existing adapters (`escape_air.c`,
`aerial_input.c`, `quaternion.c`) additionally rename the specific trig
calls their own pinned functions make, so they compare against the game's
real algorithm too; `ground_launch.c`/`lbvector.c`-based adapters do not, so
they still compare `vector_angle`'s new, real `acosf` against host `libm`'s
`acosf` -- two different, both-reasonably-accurate implementations, not a
bit-exact pair; see `tests/ground_launch_differential.rs`'s own comments for
how that test accounts for it, including the boundary-flip case below.)

## The fused-multiply-add finding

`sinf`/`cosf`'s compiled bodies (`0x803263D4`/`0x80326240`) fuse in three
places, all now ported as such, each confirmed independently by both this
batch's own disassembly and the sibling `ppc_fma_audit.py` tool:

1. **Range reduction** (`reduce` in `src/math.rs`): after computing `x - n*2`
   (an unfused `fsubs`, using an exact integer-to-double conversion so the
   subtraction itself is not rounded early), the four `__four_over_pi_m1[i] *
   x` correction terms are folded into the running total via four chained
   `fmadds` (`0x80326470`/`74`/`78`/`7c` for `sinf`; the equivalent addresses
   in `cosf`), not computed as four separate products then summed.
2. **The small-angle branch**: `cosf`'s `on_quadrant[n+1] - y*on_quadrant[n]`
   is one fused `fnmsubs` (`0x80326318`); `sinf`'s `on_quadrant[n] +
   (on_quadrant[n+1]*y)*poly[9]` computes the inner product as a plain
   `fmuls` but fuses the final `*poly[9] + on_quadrant[n]` into one `fmadds`
   (`0x803264BC`).
3. **The main Horner-chain polynomial** (four coefficients per branch,
   `sinf`'s two branches and `cosf`'s two branches): every combining step is
   a fused `fmadds`, except `cosf`'s odd branch, whose *last* step negates
   the whole running sum as it adds the final coefficient, hence `fnmadds`
   (`0x80326368`) rather than `fmadds` -- three fused `fmadds` then one fused
   `fnmadds`, not three fused steps plus a separate final negation (though
   the two are bit-identical here, since negation never rounds: confirmed
   algebraically and empirically).

Against the pinned, `-ffp-contract=off` oracle (deliberately *not* fused,
since the point of pinning it is a stable, literal reference), this fused
port cannot be bit-exact; `tests/math_differential.rs`'s `close_abs` (an
absolute tolerance, since `sinf`/`cosf` are bounded to `-1.0..=1.0` and a raw
ULP distance explodes near either function's own zero crossings) and
`close_rel` (for `tanf`, whose *poles* are the mirror problem) cover the
difference, measured directly rather than guessed: across the full
`-1000.0..1000.0` domain those tests exercise, the worst absolute
`sinf`/`cosf` difference found was under `1e-5`.

Measured against `fox-fd.slp`, this fused port (matching the retail binary's
own compiled instructions) **ties** the existing real-replay baseline
exactly -- same frame (5), same `checked_frames` (128), same expected/actual
bit patterns as the pre-batch `libm`-based baseline. A plain (fully unfused)
port of the same algorithm, tried first, **regresses** the baseline by one
frame (a different mismatch at frame 4, rejected: this project's ratchet
does not accept a lower `first_divergent_frame`).

**`atanf`**: fuses in two places (confirmed by disassembly at `0x80022E68`):
the source's own explicit PowerPC `fnmsub` intrinsic pair (`__fnmsubs`, from
`MetroTRK/intrinsics.h`) -- `result = __fnmsubs(result, lookup_ptr[7],
offset_33) + __fnmsubs(result, lookup_ptr[13], offset_39)`
(`0x80022F90`/`0x80022F94`), unambiguous since `fnmsub` is a real, explicit
single-instruction op named in the source itself, not a question of
compiler contraction -- and, less obviously, the tail Horner polynomial,
which the decompiled C shows as `result * result_squared * (nested) +
result` but the compiled binary computes differently: `result_squared` and
`result_cubed` (`result * result_squared`) are each a separate plain
multiply computed once and reused, the five-term nested polynomial is five
fused `fmadds` (`0x80022FC8` through `0x80022FE8`), and the *final*
combination -- `result_cubed * poly + result` -- is itself one more fused
`fmadds` (`0x80022FEC`), not a separate multiply and add. `crate::math`'s
`atanf` ports both exactly this way, matching the pinned, unfused oracle
within `close_ulps`'s small, fixed bound (the residual gap being unfused
range-selection/table-index arithmetic elsewhere in the function, identical
on both sides).

**`atan2f`**: no fused arithmetic of its own (confirmed by disassembly,
`0x80022C30`) -- composed entirely from `atanf` plus bit-pattern branches --
so it is ported plain and differs from the oracle only by the same small
bound `atanf` does.

## Measurement

Before this batch (recorded in `tests/fixtures/slippi/parity/
fox-fd-baseline.json`, gameplay export v6): `first_divergent_frame: 5`,
`checked_frames: 128`, diagnosed in that file's own note as "most likely
from `fighter::escape_air::launch_velocity`'s `libm::cosf`/`sinf` of the
dodge's stick angle not being bit-identical to the original GameCube SDK's
own trigonometric routines".

After this batch: unchanged -- `first_divergent_frame: 5`, `checked_frames:
128`, identical expected/actual bit patterns, even with every fused
operation now ported exactly as the retail binary's own disassembled
instructions compute it (not merely a fused-vs-unfused guess that happened
to tie). This is a genuinely mixed result, not the clean "yes, this was the
bug" the pre-batch diagnosis invited: **replacing every fighter/common
trigonometry call site with a disassembly-verified port of the game's own
routines did not move the frame-5 divergence at all.** That diagnosis is
therefore not confirmed; the true cause of the frame-5 divergence remains
open, likely in a different function or a different subsystem entirely, not
chased further here (consistent with this project's existing precedent of
reporting rather than chasing an unrelated divergence once one is reached).
See `docs/parity.md`'s own entry for this measurement's full provenance.

## What measurement-only guessing got wrong

Worth recording precisely, since it is the reason this batch disassembles
rather than guesses: an earlier revision, before consulting the DOL,
inferred `sinf`/`cosf`'s fusion by trying a plain port, then a
fully-Horner-fused port, and measuring each against `fox-fd.slp`. The fully-
fused guess happened to reproduce the correct final bits for this
recording's specific inputs -- but only because it fused the *right*
operations by coincidence (the Horner chain is the least ambiguous part; the
decompiled C's flat `x - n*2 + f0*x + f1*x + f2*x + f3*x` and `on_quadrant[n]
+ (on_quadrant[n+1]*y*poly[9])` do not obviously suggest that the retail
compiler fused *those* additions too, and the first attempt at this batch
left them unfused). Measurement alone cannot distinguish "the right fusion
shape" from "a fusion shape that happens to agree with this one recording's
inputs"; only the disassembly can, which is why every fused operation
`src/math.rs` ports now cites an instruction address rather than a
measurement outcome.

## Tests

- `tests/math_differential.rs`: `atan2f`/`atanf` differ from the pinned,
  unfused oracle by a small, fixed ULP bound (`close_ulps`) over the full
  binary32 domain, NaN-safe. `sinf`/`cosf` use an absolute bound
  (`close_abs`) over a bounded, still far-beyond-any-real-angle domain
  (`-1000.0..1000.0`); `tanf` a combined relative/absolute bound
  (`close_rel`) over the same domain, excluding inputs within 1% of an
  actual pole (see those functions' own doc comments for why ULP distance
  and, for `tanf`, any finite tolerance at all, are the wrong tool very
  close to a zero crossing or pole respectively). `full_domain_never_panics`
  is the true full-`u32`-domain check these trade precision for: every
  input returns a value without panicking, including past the point (`|x| >
  2^24` or so) where `reduce`'s own cancellation leaves no meaningful
  residual for *any* implementation to agree on. `acosf`/`asinf` are
  compared against `std`'s accurate `acos`/`asin` (not the oracle, whose
  placeholder-seeded Newton iteration is not real hardware behavior; see
  above), full domain, NaN-safe.
- `src/math.rs`'s own `#[cfg(test)]` module: ordinary-angle sanity checks
  against `std`, plus a dedicated `acosf`/`asinf` near-domain-edge accuracy
  check (`acos_asin_agree_with_std_near_the_domain_edges`) exercising
  exactly the inputs the placeholder-seeded predecessor failed on.
- `tests/escape_air_differential.rs`, `tests/aerial_differential.rs`,
  `tests/quaternion_differential.rs`, `tests/bones_differential.rs`,
  `tests/damage_differential.rs` and `tests/ground_launch_differential.rs`
  each tightened or corrected an existing tolerance or fixed expectation
  to reflect the game's real algorithm rather than host `libm`, or (
  `ground_launch_differential.rs`) added a narrow, explicitly-justified
  exclusion for the exact-boundary branch-flip case the `acosf` finding
  above describes; see each file's own comments for specifics.
- One correctness note for the `(int)` cast in `sinf`/`cosf`'s quadrant
  selection: it is undefined behavior in C for a NaN, infinite, or
  out-of-range operand, and this project's C oracle is compiled for this
  host with a plain `cc` (no `-fsanitize=undefined`). Confirmed directly
  against a standalone probe compiled with the same flags: on this
  host/target, that cast lowers to x86-64 `cvttss2si`, whose result for
  exactly those inputs is the "integer indefinite" value, `i32::MIN` --
  different from Rust's `as i32`, which instead saturates (`i32::MAX` for
  positive overflow, `0` for NaN). `crate::math`'s private `c_int_cast`
  reproduces the `i32::MIN` behavior so the full-domain proptests (which do
  exercise this range) match the oracle rather than silently narrowing the
  tested domain, per this project's existing "stays within C's defined
  float-to-int range" precedent (`tests/oracle/README.md`) -- except here
  the range itself is reproduced instead of avoided.

## Fused multiply-add outside trigonometry: auditing the retail binary and mirroring it (`skirmish-fma` batch)

MetroWerks CodeWarrior contracts `a * b + c`-shaped float expressions into
single Gekko instructions (`fmadds`/`fmsubs`/`fnmadds`/`fnmsubs`, and the
double-precision forms without the trailing `s`) wherever the source
expression permits it. Those instructions compute the product and sum with
one final rounding, not two — an f32 port that writes the same expression
as a separate multiply then add reproduces the *value* almost always, but
not always the *bit pattern*, because Rust does not contract float
arithmetic on its own. This document is the audit this batch ran to find
out, function by function, where that actually matters, and records what
changed in the Rust port as a result.

### The tool: `tools/ppc_fma_audit.py`

```
uv run --with capstone python3 tools/ppc_fma_audit.py \
    --dol /mnt/archive/runs/melee-assets-20260909/disc/sys/main.dol \
    --symbols /mnt/shared/Projects/Code/External/melee/config/GALE01/symbols.txt \
    ftCommon_8007C98C ftColl_80079AB0 ...

# Or from a file, one name per line (# comments allowed):
uv run --with capstone python3 tools/ppc_fma_audit.py --dol ... --symbols ... \
    --functions-file names.txt --summary-only
```

Given function names, it parses the DOL's own section table to find each
symbol's file offset (no hardcoded load address), disassembles the bytes at
`symbols.txt`'s recorded address and size with Capstone 5's PowerPC
backend, and prints every fused instruction found together with a few
neighboring instructions and a per-function summary. Only the standard
library and `capstone` are used, invoked exactly as shown above (capstone
is never `pip install`ed).

**A real gap in capstone's PPC support, and the fix.** Capstone 5's
PowerPC backend does not decode every legacy PowerPC FPU opcode — `fcmpo`
appears throughout this binary and capstone's `Cs.disasm` simply stops
there, silently truncating the rest of the function (confirmed: without a
fix, `ftCommon_8007C98C` disassembled to 4 instructions instead of the 61 its
`size:0xF4` implies). The tool works around this by disassembling one
instruction at a time and, whenever capstone fails to decode a word,
recording it as `.long 0x........` and advancing by the fixed PowerPC
instruction width (4 bytes) instead of stopping. This is the only reason a
custom decoder is needed at all; every *decodable* instruction, including
every fused-multiply-add form used below, capstone handles natively and
correctly (verified against hand-encoded `fmadds`/`fmsub`/`fnmadds`/
`fnmsubs` test vectors before trusting it against the retail binary).

### Per-function findings

#### The batch's named targets

Audited directly (`ftCo_80099A9C` is the actual address; `ftCo_EscapeAir`'s
own callbacks just dispatch to it):

| Function | Fused ops | Notes |
| --- | --- | --- |
| `ftCommon_8007C98C` (`getAccelAndTarget`'s caller / `accelerate`) | **none** | plain `fadds`/`fmuls` throughout |
| `ftCommon_ApplyGroundMovement` | **none** | |
| `ftCommon_ApplyGroundMovementNoSlide` | **none** | |
| `ftCommon_ApplyFrictionGround` | **none** | |
| `ftCommon_ApplyFrictionAir` | **none** | |
| `ftCommon_Fall` | **none** | |
| `ftCo_800CB110` (jump launch) | **none** | |
| `ftCo_80099A9C` (`ftCo_EscapeAir` launch, inlines `inlineA0`) | **none** | `force * cosf(angle)` is a plain `fmuls`, not `a*b+c` shaped at all |
| `ftCo_Dash_Phys` (inlines `getAccelAndTarget`, calls `ftCommon_8007C98C`/`ftCommon_ApplyGroundMovement`) | **none** | `stick * dash_accel_mul` then a separate `fadds` for `+= dash_accel_base` |
| `ftColl_80079AB0` (knockback) | **6** (`fmaddsx6`) | see below — mirrored |
| `ftCo_Damage_CalcVel` | **none** | |
| `ftCommon_CalcHitlag` | **1** (`fmaddsx1`) | mirrored |
| `lbVector_AngleXY` | **9** (`fmaddsx1`, `fnmsubx8`) | mirrored |
| `ft_80084F3C` | **none** | |
| `ftCo_800DA824` (grab-escape timer) | **2** (`fmaddsx2`) | mirrored |

**This falsifies the batch's own working hypotheses (evidence 1 and 2 in
the brief).** Both recorded 1-ULP divergences named in this batch's brief
were suspected to come from a fused product+sum rounding once instead of
twice somewhere in these chains:

- `fox-fd-3.slp` frame -32, `velocities.self_x_air` (`0x400147ad` expected,
  `0x400147ae` actual): the suspect chain was `getAccelAndTarget` /
  `ftCommon_8007C98C` / `accelerate_ground`, all audited above with **zero**
  fused ops. `ftCo_Dash_Phys` disassembles to a plain `fmuls` (`stick *
  dash_accel_mul`) followed by a plain `fadds` (`+= dash_accel_base`), not a
  single `fmadds`. The retail binary computes this exact chain with the
  same separately-rounded arithmetic Rust does; the divergence is real but
  is not this.
- `fox-fd.slp` frame 5, `position.x` (decayed from an air dodge's launch
  velocity): the suspect was `ftCo_EscapeAir`'s launch (`force * cosf(angle)`
  / `force * sinf(angle)`). `ftCo_80099A9C` (the real launch function,
  inlining `inlineA0`) has zero fused ops — each product is a standalone
  `fmuls`, never combined with an addition. This is consistent with
  `docs/parity.md`'s own diagnosis for this frame: the divergence traces to
  `libm`'s portable `cosf`/`sinf` not being bit-identical to the original
  GameCube SDK's trig routines, a transcendental-function difference, not a
  rounding-contraction one.

Both are recorded in `docs/parity.md` with this audit's result; neither
recording's baseline changed (see `docs/validation.md`).

#### Functions with real fused ops, mirrored in this batch

| Decomp function | Fused ops | Rust site | `f32::mul_add` at |
| --- | --- | --- | --- |
| `ftColl_80079AB0` | 3 distinct sites (2 mutually-exclusive branch variants + 2 shared = 6 static occurrences) | `fighter::combat::knockback` | the branch `inner` term (`x118*x110 + x114*(x118*x28)` fixed, or the count-based equivalent), `growth_term` (`x11C*(decay*inner)+x120`), and `scaled` (`0.01*growth*growth_term+x2C`) |
| `ftCommon_CalcHitlag` | 1 | `fighter::combat::hitlag` | `dmg * x198 + x19C` |
| `ftCo_800DA824` | 2 | `fighter::grab::escape_timer` | `handicap_scale*temp+base`, and the final `percent*percent_scale+temp` |
| `ftCo_Damage_CalcAngle` | 1 | `fighter::damage::launch_angle` | `x148 * ratio + 1` (in the grounded branch, before the separate degrees-to-radians multiply) |
| `lbVector_AngleXY` | 1 `fmadds` (the dot product) + 8 `fnmsub` (double precision, inside the inlined `sqrtf_accurate` Newton-Raphson refinement, 4 iterations × 2 calls) | `game::characters::fox::up::angle_xy` (private) | the dot product `a.y*b.y + a.x*b.x`, and a new `sqrt_accurate` helper's `3.0 - guess*guess*x` per iteration |

`lbVector_AngleXY` needed more than a `mul_add` at one call site: its own
length calls go through `lbVector_Len_xy_accurate` → `sqrtf_accurate`
(`MSL/math_ppc.h`), which is *not* Rust's `f32::sqrt` but four fused
Newton-Raphson iterations refining a `__frsqrte` hardware estimate, in
double precision. `up.rs::angle_xy` previously called `.sqrt()` directly —
correctly rounded, but not what the retail binary actually computes. This
batch ported the real algorithm as `sqrt_accurate`, seeded from `1.0 /
(x as f64).sqrt()` instead of a bit-exact `__frsqrte` emulation: Newton's
method for `1/sqrt(x)` has one stable, quadratically-convergent fixed
point, and `__frsqrte`'s own documented accuracy (~12 bits, doubling each
iteration to 24, 48, then 96 bits over the four iterations) is *less*
accurate than seeding from a correctly-rounded double reciprocal sqrt, so
both converge to the identical fixed point well before the fourth
iteration; iterating past convergence is idempotent up to rounding. This
was directly confirmed, not just argued: `tests/fox_up_special_differential.rs`
compares the real, pinned `ftFox_SpecialHi_*` C (which calls the actual
`lbVector_AngleXY`, uncontracted) against a test-local mirror of the new
Rust algorithm, and it matches bit-for-bit across 100,000 generated cases.

#### Every C-oracle-pinned function with float math: what has fused ops

`tools/ppc_fma_audit.py` was run in bulk against the 463 unique function
names across every `tests/oracle/*.functions.json` (409 resolved to a
`symbols.txt` address and disassembled cleanly; the other 54 are
`static inline` helpers or local statics that get folded into a caller and
have no standalone symbol — `inlineA0`/`inlineA1`/`mn_8022C7CC_inline` and
similar). 409 functions audited, 244 fused instructions found across 47 of
them:

```
HSD_MtxInverse: 20 (fmaddsx2, fmsubsx8, fnmsubsx10)
HSD_MtxSRT: 4 (fmaddsx2, fmsubsx2)
doEnter: 1 (fmaddsx1)
ftCo_8008E5A4: 7 (fmaddsx4, fnmsubx3)
ftCo_800925A4: 1 (fmaddsx1)
ftCo_80092ED8: 2 (fmaddsx2)
ftCo_80092F2C: 4 (fmaddsx4)
ftCo_80093240: 2 (fmaddsx2)
ftCo_800932DC: 2 (fmaddsx2)
ftCo_80099D9C: 1 (fmaddsx1)
ftCo_800C18A8: 5 (fmaddsx1, fnmsubx3, fnmsubsx1)
ftCo_800C6408: 1 (fnmsubsx1)
ftCo_800DA824: 2 (fmaddsx2)                    -- mirrored, see above
ftCo_800DEEB8: 1 (fmaddsx1)
ftCo_Damage_CalcAngle: 1 (fmaddsx1)             -- mirrored, see above
ftCo_Damage_OnExitHitlag: 4 (fmaddsx1, fnmsubx3)
ftCo_Dash_IASA: 1 (fmaddsx1)
ftCo_EntryStart_Phys: 1 (fmaddsx1)
ftCo_RebirthWait_Phys: 1 (fmaddsx1)
ftCo_Rebirth_Phys: 1 (fmaddsx1)
ftCo_TurnRun_Phys: 2 (fmaddsx1, fnmsubsx1)
ftColl_80076528: 2 (fnmsubsx2)
ftColl_8007699C: 2 (fmaddsx2)
ftColl_80079AB0: 6 (fmaddsx6)                   -- mirrored, see above
ftCommon_8007DD7C: 2 (fmaddsx2)
ftCommon_8007DFD0: 2 (fmaddsx2)
ftCommon_CalcHitlag: 1 (fmaddsx1)                -- mirrored, see above
ftFx_SpecialAirHi_Phys: 2 (fnmsubsx2)
ftFx_SpecialAirLwTurn_Anim: 1 (fnmsubsx1)
ftFx_SpecialLwTurn_Anim: 1 (fnmsubsx1)
ftFx_SpecialLw_Turn: 1 (fnmsubsx1)
ftWalkCommon_800DFEC8: 1 (fnmsubsx1)
ft_800CB6EC: 1 (fnmsubsx1)
lbColl_80005C44: 9 (fmaddsx9)
lbColl_80005EBC: 9 (fmaddsx9)
lbColl_80005FC0: 5 (fmaddsx5)
lbColl_80006094: 36 (fmaddx3, fmaddsx30, fmsubsx3)
lbColl_80006E58: 46 (fmaddx3, fmaddsx34, fmsubsx3, fnmsubx6)
lbVector_Angle: 8 (fmaddsx2, fnmsubx6)
lbVector_AngleXY: 9 (fmaddsx1, fnmsubx8)         -- mirrored, see above
lbVector_Mirror: 3 (fmaddsx3)
mpCollInterpolateECB: 8 (fmaddsx8)
mpColl_LoadECB_Fixed: 4 (fmaddsx2, fmsubsx2)
mpLib_8004ED5C: 6 (fnmsubx6)
mpLineIntersection: 7 (fmaddx2, fmsubx5)
mpLineIntersectionH: 1 (fmaddx1)
mpLineIntersectionV: 1 (fmaddx1)
mpRemap2d: 6 (fmaddx6)
```

Four of these are the ones this batch mirrored (above). The remaining 43
are a real, documented backlog, deliberately **not** touched in this batch:

- `ftCo_8008E5A4` and `ftCo_Damage_OnExitHitlag` are ASDI/SDI redirection —
  they call `atan2f`/`cosf`/`sinf`/`sqrtf`, squarely the territory of the
  concurrent `skirmish-msl-trig` batch porting `MSL/trigf.c`; touching them
  here risked stepping on that work mid-flight, per this batch's own
  coordination instructions.
- `lbVector_Angle` (the 3D, non-XY vector angle) very likely goes through
  the same or a similar Newton-Raphson `sqrtf`-family routine as
  `lbVector_AngleXY` did (the double-precision `fnmsub` pattern is the same
  shape, just 3 iterations' worth instead of 4 — 6 `fnmsub` for two calls
  instead of 8), but this wasn't independently verified against `MSL`'s
  `sqrtf` source and no Rust call site currently depends on it matching
  bit-for-bit.
- `HSD_MtxInverse`/`HSD_MtxSRT`/`mpLineIntersection*`/`mpRemap2d`/
  `mpCollInterpolateECB`/`mpColl_LoadECB_Fixed`/the `lbColl_*` capsule/sweep
  routines are collision/ECB geometry, largely outside this batch's named
  physics/damage/knockback scope.
- The rest (`doEnter`, `ftCo_800925A4`, `ftCo_80092ED8`, `ftCo_80092F2C`,
  `ftCo_80093240`, `ftCo_800932DC`, `ftCo_80099D9C`, `ftCo_800C18A8`,
  `ftCo_800C6408`, `ftCo_800DEEB8`, `ftCo_Dash_IASA`, `ftCo_EntryStart_Phys`,
  `ftCo_RebirthWait_Phys`, `ftCo_Rebirth_Phys`, `ftCo_TurnRun_Phys`,
  `ftColl_80076528`, `ftColl_8007699C`, `ftCommon_8007DD7C`,
  `ftCommon_8007DFD0`, the four `ftFx_Special*`/`ftWalkCommon_800DFEC8`
  functions, `ft_800CB6EC`, `lbVector_Mirror`, `mpLib_8004ED5C`) are simply
  not yet checked against their own Rust ports one by one; each is a single
  `f32::mul_add` review away, following exactly this batch's method.

### The oracle strategy: what worked, what didn't, and why

The task's own question — does `-ffp-contract=fast` with `-mfma` on x86-64
reproduce PowerPC single-rounding semantics closely enough to build the
c-oracle adapters that way — has a real, nuanced answer, established here
empirically rather than assumed. `build.rs` compiles two static libraries:
`skirmish_oracle` (the default, `-ffp-contract=off`, matching every
existing adapter) and `skirmish_oracle_fma` (`-ffp-contract=fast -mfma`,
plus a forced `opt_level(2)` — GCC's contraction pass is part of its
optimizer and is silently a no-op at `-O0`, which is what `cc` mirrors from
Cargo's dev/test profile by default; this was found by disassembling the
resulting object and seeing plain `vmulss`/`vaddss` instead of `vfmadd`
before adding the explicit `opt_level`).

**Where it works cleanly: `combat_hitlag.c` and `damage_calc_angle.c`.**
Both wrap a function whose *entire* body contains exactly one `a * b + c`
shape, with no other multiply nearby for the compiler to consider
contracting instead. Disassembling the compiled objects confirms a single
`vfmadd`-family instruction each, matching the real hardware bit-for-bit;
`tests/combat_differential.rs`'s `hitlag_and_initial_hitstun_match_c` and
`tests/damage_differential.rs`'s `generated_angles_match` compare against
these with an exact bit match, run to 100,000+ generated cases without a
single mismatch. (`ftCo_Damage_CalcAngle` needed splitting out of
`damage_core.c`'s existing six-function bundle into its own translation
unit, `damage_calc_angle.c`, specifically *because* four of its five
former siblings there — `ftCo_Damage_CalcVel`, `ftCo_8008E5A4`,
`ftCo_Damage_CalcKnockback`, `ftCo_Damage_OnEveryHitlag`,
`ftCo_Damage_OnExitHitlag` — are not all fusion-free, and contracting the
whole shared file would have risked changing their oracle output out from
under comparisons this batch never re-verified.)

**Where it does not, and had to be reverted: `combat_knockback.c` and
`escape_formula.c`.** Two distinct failure modes were found, both by
disassembling the actual compiled object after a test failure, not by
guessing:

1. **Multiple candidate multiplies competing for one addition.**
   `ftColl_80079AB0`'s `inner` term is `A*B + C*(D*E)`, a sum of two
   products. A fused multiply-add can only remove *one* of the two
   products' rounding, by folding it into the addition; which one is a
   compiler choice IEEE 754 does not constrain. Disassembling
   `combat_knockback.c` compiled with contraction showed GCC choosing the
   *other* pairing than the retail PowerPC binary does for this exact
   expression (confirmed against `tools/ppc_fma_audit.py`'s own
   disassembly of `ftColl_80079AB0`). Splitting the *oracle wrapper's* own
   copy of the `KNOCKBACK` macro into isolated per-statement form (mirroring
   the technique that worked for the two functions above) fixed the two
   *other* fused sites in the same function (`growth_term`, `scaled`) but
   could not fix `inner`, because `inner`'s own expression is computed
   inline inside `ftColl_80079AB0`'s pinned, verbatim-extracted body — not
   something an oracle wrapper is free to restructure.
2. **Contraction reaching across a statement boundary the source never
   suggested.** `ftCo_800DA824`'s six statements each look like a clean,
   isolated `a * b + c` candidate (unlike `ftColl_80079AB0`'s single nested
   expression), so this was tried first, expecting it to work like
   `ftCommon_CalcHitlag` did. It didn't: disassembling the compiled object
   showed GCC's `-ffp-contract=fast` fusing a single-use `rank_scale *
   ratio` product (computed two statements earlier, and used nowhere else)
   into the later, plain `temp += value` addition — a pairing the real
   PowerPC compiler never makes at all (confirmed: a plain `fadds` there in
   the retail disassembly). GCC's contraction is not limited to the
   syntactic shape of one statement; it can reach any single-use product
   into a later addition if the intervening code doesn't observe it.

Both files were reverted to their original, uncontracted form (matching
every other adapter); `fighter::combat::knockback` and
`fighter::grab::escape_timer` still use `f32::mul_add` at the exact sites
`tools/ppc_fma_audit.py` identified (unaffected by any of this — the
oracle's limitations don't change what the real PowerPC hardware does).
Their differential tests (`combat_differential.rs`'s `knockback_matches_c`,
`escape_formula_differential.rs`'s `escape_timer_matches_ftco_800da824`)
compare against the now-uncontracted oracle with a documented small
*relative* tolerance (`1e-4`, calibrated with two-plus orders of magnitude
of margin above the worst of 50,000+ generated cases, which stayed under
4e-7 and 2e-6 respectively) instead of requiring an exact bit match, and
treat "both sides land off the finite range" as agreement rather than
asserting a specific relationship between the two non-finite values (an
FMA's full-precision intermediate product genuinely can't overflow the way
an unfused multiply alone can, so a fused and an unfused computation of the
same expression can diverge categorically — not just by a few ULPs — right
at the edge of `f32`'s range; both directions of this were hit empirically
while calibrating this test). Each function is additionally pinned exactly
by a native Rust unit test against a hand-verified (`libm`'s `fmaf` outside
Rust) fused value, independent of the C oracle's own limitations for these
two functions specifically:
`fighter::combat::tests::knockback_matches_a_hardware_fused_value_pinned_from_the_retail_dol`
and
`fighter::grab::tests::escape_timer_matches_the_hardware_fused_rounding_not_naive_two_rounding`.

**Bottom line for future batches doing this:** `-ffp-contract=fast -mfma`
reproduces PowerPC single-rounding semantics reliably only when a
function's *entire* translation unit contains exactly the one fused
expression and nothing else that shares an operand with it; multi-term
sums of products, or any statement whose product could plausibly flow
into a *different* nearby addition, need per-site verification (compile,
disassemble, compare register operands against `tools/ppc_fma_audit.py`'s
own output) rather than an assumption that the flag alone suffices.

### See also

- `docs/parity.md` — the `fox-fd-3.slp` and `falco-fox-fd.slp` entries cite
  this audit's negative result for the Dash acceleration chain.
- `docs/validation.md` — the recordings' measurements after this batch.

## Double-precision intermediates outside trigonometry: auditing the same three divergences again (`skirmish-f64` batch)

The `skirmish-fma` batch above ruled out a fused product+sum for the three
recorded one-ULP divergences it inherited (`fox-fd-3.slp` frame -32,
`falco-fox-fd.slp` frame -25, `fox-fd.slp` frame 5) but left the actual
mechanism open. A second, distinct candidate remained on the table: the
Gekko FPU's registers are always the IEEE double format, and PowerPC's
*single-precision* arithmetic mnemonics (`fadds`/`fsubs`/`fmuls`/`fdivs`)
are architecturally defined as "compute at double precision, then round
once to single" — so a chain of two such ops is not automatically the same
as two independent, separately-rounded `f32` operations *unless* both
actually use the suffixed forms. If MetroWerks had instead emitted the
*double*-precision forms (`fadd`/`fsub`/`fmul`/`fdiv`, no trailing `s`,
which round only when something later forces it — an explicit `frsp`, a
`stfs`, or a suffixed op) anywhere in this chain and only rounded to
single once at the end, that would be a real, measurable double-rounding
difference from Rust's own step-by-step `f32` arithmetic, distinct from
(and invisible to) the fused-op audit: `ppc_fma_audit.py` only reports
`fmadds`/`fmsubs`/`fnmadds`/`fnmsubs`-family mnemonics; for a function with
none of those it prints nothing else at all, so whether the *other*
instructions it skipped over were single or double precision was never
actually checked, only asserted in that batch's own prose from reading the
decomp's C types (all plain `float`, no unsuffixed double literals in this
exact chain) — correct reasoning, but not yet the same thing as reading it
off the instructions themselves.

### The tool: `tools/ppc_precision_audit.py`

```
uv run --with capstone python3 tools/ppc_precision_audit.py \
    --dol /mnt/archive/runs/melee-assets-20260909/disc/sys/main.dol \
    --symbols /mnt/shared/Projects/Code/External/melee/config/GALE01/symbols.txt \
    ftCo_Dash_Phys ftCommon_8007C98C ...

uv run --with capstone python3 tools/ppc_precision_audit.py --dol ... --symbols ... \
    --functions-file names.txt --summary-only
```

Shares its DOL/section-table parsing, symbol loading, and the
`fcmpo`-triggered resync workaround with `ppc_fma_audit.py` (duplicated
rather than imported, so either tool stands alone). Where the FMA tool
classifies only fused mnemonics, this one classifies every floating-point
instruction in a disassembled function into: **double-precision
arithmetic** (`fmul`/`fadd`/`fsub`/`fdiv`/`fmadd`/`fmsub`/`fnmadd`/`fnmsub`,
no trailing `s` — computed and rounded to double), **single-precision
arithmetic** (the same mnemonics with the `s` suffix — double-precision
compute, one rounding to single), `frsp` (the explicit, otherwise-implicit
single-rounding point), and `lfd`/`lfs` (loads a double or single from
memory). A function that is genuinely single-precision throughout prints
"pure single-precision f32 throughout" with zero of the first count; any
`lfd` or double-arith instruction is printed with full context so a real
double constant load can be told apart from an ordinary callee-saved-FPR
stack spill/restore (which also uses `lfd`/`stfd` — every `lfd` found in
this audit turned out to be exactly that, immediately preceded by a
register-save sequence and followed by an epilogue `blr`, not a load of
any constant or attribute).

### Per-function findings: the three named divergence chains

Every function named in the batch's own brief (the Dash/ground-movement/
friction chain, the air-dodge launch, and the landing-friction chain),
plus their immediate float-touching callees, disassemble to **zero
double-precision arithmetic instructions**:

| Function | double-arith | lfd | Notes |
| --- | --- | --- | --- |
| `ftCo_Dash_Phys` | 0 | 0 | 4 single-arith, 9 `lfs` |
| `ftCommon_8007C98C` | 0 | 0 | 11 single-arith |
| `ftCommon_ApplyGroundMovement` | 0 | 0 | 5 single-arith |
| `ftCommon_ApplyGroundMovementNoSlide` | 0 | 0 | 4 single-arith |
| `ftCommon_ApplyFrictionGround` | 0 | 0 | pure compare/select, no arith |
| `ftCo_Dash_Enter` | 0 | 0 | |
| `ftCommon_800804A0` | 0 | 1 (epilogue FPR restore, not a constant) | |
| `ft_GetGroundFrictionMultiplier` | 0 | 0 | |
| `ftCo_80099A9C` (air-dodge launch, inlines `inlineA0`) | 0 | 1 (epilogue restore) | calls `cosf`/`sinf` directly |
| `ftCommon_8007D9D4` (stick-angle helper) | 0 | 0 | calls `atan2f` |
| `ftCo_EscapeAir_Phys` / `ftCo_EscapeAir_IASA` | 0 | 0 | |
| `ft_80084F3C` (landing/ground friction dispatch) | 0 | 0 | |
| `ftCo_Landing_Enter` / `ftCo_LandingFallSpecial_Enter` | 0 | 2 (epilogue restores) | |
| `ftCo_Landing_Phys` | 0 | 0 | |
| `ftCommon_Fall` / `ftCommon_8007CF58` / `ftCommon_ApplyFrictionAir` / `ftCo_800CB110` | 0 | 0 | |

Every `lfd` found is a callee-saved FPR (`f30`/`f31`) stack spill/restore
around a `bl` call site or at a function epilogue — standard PowerPC EABI
prologue/epilogue code, using the double format because that's what the
ABI's register-save area always uses, not a double constant or attribute
load. None of these functions call any of the genuinely double-precision
functions found below either (checked directly: each function's `bl`
targets were resolved through `symbols.txt` and cross-referenced against
the double-precision list).

**This falsifies the double-precision-intermediate hypothesis too, and not
merely by absence of evidence.** For `fox-fd-3.slp` frame -32
specifically, the exact recorded inputs (Fox's own pack constants:
`dash_initial_velocity` f32 bits `0x3ff33333`, `dash_accel_mul`
`0x3dcccccd`, `dash_accel_base` `0x3ca3d70a`, `dash_max_velocity`
`0x400ccccd`, stick `1.0`) were run through every rounding model there is
for this expression: separately-rounded single precision at each step
(what both the retail disassembly and this Rust port do), fully
double-precision arithmetic with one final round to `f32`, and every
partial mix in between (multiply in double/add in single, multiply in
single/add in double). **Every model produces the identical bit pattern,
`0x400147ae`** — never the recording's own `0x400147ad`. This is not a
coincidence of IEEE 754 correctly-rounded single-precision arithmetic:
because both float operands to each step are exactly representable in
double with no rounding error, "compute in double, round once" and
"round after each single-precision step" happen to agree for this
particular set of inputs whenever no intermediate rounding boundary is
crossed twice differently — which sweeping the model space above confirms
directly rather than assumes. So there is no possible instruction
selection, fused or not, single or double, that reaches the recording's
value from these inputs at this expression. The one-ULP gap must
originate somewhere upstream of this frame's own arithmetic. A quick
sensitivity check (perturbing the incoming `ground_velocity` by ±1-2 ULP
and re-running the identical expression) shows a `ground_velocity` two ULP
lower than modeled (`0x3ff33331` instead of `0x3ff33333`) is sufficient to
reproduce `0x400147ad` exactly — pointing at the Dash-entry velocity
computation (`ftCo_Dash_Enter`/`ftCommon_800804A0`, both independently
confirmed fused- and double-precision-free above) or an even earlier
frame, not this expression's own evaluation order. Not chased further in
this batch, per this project's established stop condition for a diagnosis
that has been narrowed but not resolved — left as a concrete lead
(`docs/parity.md`'s entry below records it) for whichever future batch
picks this up.

`falco-fox-fd.slp` frame -25 shares the identical disassembled functions
(the Dash chain is fighter-generic `ftCommon`/`ftCo_Dash_Phys` code, not
per-character), so the same "zero double-precision arithmetic anywhere in
this chain" finding applies directly; Falco's own dash constants put frame
-25 through the chain's clamp branch (`gr_vel` already above Falco's lower
`dash_max_velocity`), which changes which comparisons run but not which
instructions compute them, so the exhaustive per-input sweep above was not
independently re-run for Falco's own clamp-branch case. `fox-fd.slp` frame
5 (the air-dodge launch, `ftCo_80099A9C`/`inlineA0`) is confirmed
double-precision-free the same way; this frame was already known (from the
`skirmish-msl-trig` batch's own measurement, `docs/math.md` above) to
persist even after every trigonometric call site in the chain was replaced
with a disassembly-verified, bit-exact port of the retail routines, so
this batch's finding is consistent with — not a new explanation for — that
already-open gap.

### Broader sweep: which C-oracle-pinned functions genuinely use double precision

Running `tools/ppc_precision_audit.py --summary-only` against the same
~500-function list the `skirmish-fma` batch drew from every
`tests/oracle/*.functions.json` (434 resolved to a real address) finds 22
functions with real double-precision arithmetic (208 double-precision
instructions total) — a strict superset of, but distinct from, the fused-op
list above (a double-precision fused instruction, e.g. plain `fmadd`
without the `s`, is both a fused op *and* a double-precision op; several
functions below are exactly that):

```
HSD_MtxSRT: 3 double-arith                    ftFx_SpecialAirNLoop_Anim: 1
ftCo_8008E5A4: 13                             ftFx_SpecialNLoop_Anim: 1
ftCo_800C18A8: 13                             ftFx_SpecialN_CreateBlasterShot: 1
ftCo_800C6408: 2                              itFoxLaser_Logic94_Reflected: 4
ftCo_Damage_OnExitHitlag: 13                  itFoxLaser_Logic94_ShieldBounced: 2
lbColl_80006094: 3                            itFoxlaser_UnkMotion1_Anim: 2
lbColl_80006E58: 29                           it_8029C504: 2
lbVector_Angle: 26                            mpLib_8004DD90_Floor: 1
lbVector_AngleXY: 34 (mirrored, see above)    mpLib_8004E090_Ceiling: 1
mpLib_8004ED5C: 26                            mpLineIntersection: 13
mpLineIntersectionH: 4                        mpLineIntersectionV: 4
mpRemap2d: 10
```

None of these 22 functions are called, directly or through the callees
audited above, from any of the three named divergence chains — they are
collision/ECB geometry (`lbColl_*`, `mpLineIntersection*`, `mpRemap2d`,
`HSD_MtxSRT`), ASDI/SDI redirection (`ftCo_8008E5A4`,
`ftCo_Damage_OnExitHitlag`), the Blaster laser's reflection code
(`itFoxLaser_*`, `it_8029C504`, `ftFx_Special*Loop_Anim`,
`ftFx_SpecialN_CreateBlasterShot`), or `lbVector_Angle`/`mpLib_8004ED5C`
(already flagged as a backlog item by the `skirmish-fma` batch above for
unrelated reasons — this confirms they do use genuine double-precision
arithmetic, not just a fused op, consistent with `lbVector_AngleXY`'s own
already-mirrored `sqrt_accurate` Newton-Raphson refinement, which these
likely share the shape of). None of them is touched by this batch: no
recording currently pins their exact bits, and mirroring 22 functions'
worth of double-precision arithmetic without a measurement that needs it
would be exactly the kind of unverifiable, un-measurable change this
project's own equivalence discipline (`docs/equivalence.md`) warns against.
Recorded here as a real, disassembly-confirmed backlog for whichever
future batch's own divergence traces into one of them.

### Bottom line

Both leading hypotheses for these three recorded one-ULP divergences — a
fused product+sum (`skirmish-fma`) and a double-precision intermediate
(this batch) — are now ruled out by direct disassembly of the retail
binary, and for `fox-fd-3.slp` frame -32 specifically, by an exhaustive
sweep of every possible rounding model for the exact recorded inputs. No
Rust source change follows from this batch: the port already computes
this chain exactly as the retail PowerPC binary does, confirmed at the
instruction level, not merely at the level of the decompiled C's types.
`tests/fighter/movement.rs`'s new
`dash_accel_reproduces_ieee754_not_the_recordings_one_ulp_lower_value` unit
test pins this finding directly (both the single-precision and the
double-precision-throughout evaluation, so a future change can't
"accidentally" reproduce the recording's bits without first explaining
why that would actually be correct). No baseline changes: ruling out a
second candidate explanation isn't fixing the divergence, and forcing a
bit-pattern match without a mechanism to justify it would be exactly the
kind of measurement-only guessing `docs/math.md`'s own trigonometry
section already documented going wrong once.

### See also

- `docs/parity.md` — the `fox-fd-3.slp`, `falco-fox-fd.slp` and
  `fox-fd.slp` entries record this audit's negative result alongside the
  `skirmish-fma` batch's.
- `docs/validation.md` — this batch's own measurement entry.
