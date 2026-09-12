/* Host adapter for the MSL math data tables (`src/MSL/math_data.c`): defines
 * `__sincos_on_quadrant` and `__sincos_poly`, which `trigf_body.c`'s `sinf`/
 * `cosf` reference as `extern`. Kept in its own translation unit, matching
 * the pinned source's own separate-compilation-unit layout (see
 * `trigf_body.c`'s header comment for why that separation matters here). */
#include "math_data_original.inc"
