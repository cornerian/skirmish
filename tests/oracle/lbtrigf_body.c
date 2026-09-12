/* Host adapter for `atan2f`/`atanf`/`acosf`/`asinf` from
 * `src/melee/lb/lbtrigf.c` (the HSD "lb" math library the fighter/common
 * code actually calls).
 *
 * `__MWERKS__` is defined so the pinned source's Metrowerks-only `atanf` body
 * (a silver-ratio lookup-table algorithm) compiles; that body, not the
 * unreachable non-`__MWERKS__` fallback, is what the retail binary contains.
 * `__frsqrte(x)` expands to `sqrt(x)` and `__fnmsubs` to a fused
 * negate-multiply-add, matching the pinned source's own `placeholder.h`/
 * `MetroTRK/intrinsics.h` host stand-ins (`placeholder.h`'s `__frsqrte` is
 * `#ifndef MWERKS_GEKKO`, i.e. it is *not* real PowerPC `frsqrte` even on the
 * upstream decompilation project's own host builds); see `docs/math.md`.
 *
 * `atan2f`/`atanf`/`acosf`/`asinf` are renamed via macro to `skirmish_lb_*`
 * (below, after `<math.h>`'s own declarations but before the pinned include)
 * rather than kept under their libm-shaped names, for the same reason
 * `trigf_body.c` renames `sinf`/`cosf`/`tanf`: this static library shares a
 * process with Rust's std, whose own trig methods call the platform C
 * library by these exact symbol names, and a same-named strong definition
 * here would silently replace them everywhere, not just in this crate's own
 * `oracle_*` comparisons. See `trigf_body.c` and `docs/math.md`.
 */
#include <math.h>
#include <stdbool.h>
#include <stdint.h>

typedef int32_t s32;
typedef uint32_t u32;

#define M_PI 3.14159265358979323846
#define M_PI_2 1.57079632679489661923

#define atan2f skirmish_lb_atan2f
#define atanf skirmish_lb_atanf
#define acosf skirmish_lb_acosf
#define asinf skirmish_lb_asinf

/* Declared ahead of use: `atan2f`/`acosf`/`asinf` below call `atanf`, whose
 * `__MWERKS__` definition appears later in the pinned source. */
float atanf(float x);

#define __MWERKS__ 1
#define __frsqrte(x) sqrt(x)
#define __fnmsubs(a, c, b) fmaf(-(a), (c), (b))

/* `MSL_TrigF_8040077{0,4}` (`lbtrigf.c`): the MSL data section's NaN/Infinity
 * float constants that its `NAN`/`INF` macros read `[0]` from. */
float MSL_TrigF_80400770[] = { NAN };
float MSL_TrigF_80400774[] = { INFINITY };

#include "lbtrigf_original.inc"
