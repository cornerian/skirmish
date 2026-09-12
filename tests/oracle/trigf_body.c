/* Host adapter for `sinf`/`cosf`/`tanf` from `src/MSL/trigf.c` (Metrowerks
 * MSL runtime). Compiled as its own translation unit, matching the pinned
 * source's own separate-file layout: `math_data_body.c` provides
 * `__sincos_on_quadrant`/`__sincos_poly` as a distinct object (their type
 * there, `const float[]`, conflicts with this file's own `extern f32 (aka
 * float)[]` declaration if both appear in one translation unit, even though
 * neither the original build nor the linker cares about the mismatch across
 * separate object files).
 *
 * `sinf`/`cosf`/`tanf` are renamed via macro to `skirmish_msl_*` (below,
 * before the pinned include) rather than kept under their libm-shaped names:
 * this static library is linked into the same test binary as Rust's std,
 * whose own `f32::sin`/`cos`/`tan`/`asin`/`acos`/`atan`/`atan2` call the
 * platform's C library by these exact symbol names. A same-named strong
 * definition here would silently replace std's calls with this pinned body
 * everywhere in the process, not just in this crate's own `oracle_*`
 * comparisons -- confirmed the hard way (see `docs/math.md`): every
 * `x.acos()`/`x.asin()` "ground truth" comparison elsewhere in this crate's
 * own tests would have started comparing the port against itself.
 */
#include <stdint.h>

typedef int32_t s32;
typedef float f32;

#define M_PI 3.14159265358979323846

/* `fabsf__Ff` (`src/MSL/math_1.c`): declared via the pinned source's own
 * stripped `#include "math.h"`; a bit-exact `fabsf` alias. */
static float fabsf__Ff(float x)
{
    return __builtin_fabsf(x);
}

/* `__sinit_trigf_c_reference` (`trigf.c`) is normally a CodeWarrior
 * section-constructor pointer; the host constructor below calls the same
 * pinned `__sinit_trigf_c` directly instead. */
#define SECTION_CTORS
#define sinf skirmish_msl_sinf
#define cosf skirmish_msl_cosf
#define tanf skirmish_msl_tanf
#include "trigf_original.inc"

__attribute__((constructor)) static void run_trigf_ctor(void)
{
    __sinit_trigf_c();
}
