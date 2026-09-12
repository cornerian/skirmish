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
