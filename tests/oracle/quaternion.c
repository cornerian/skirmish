/* Scalar/layout declarations and array-to-struct copies only. The six original
 * quaternion functions below are unchanged apart from removed includes.
 * Tests use distinct input/output objects, like both upstream slerp callers.
 * Host libm results do not certify PowerPC runtime rounding or NaN payloads.
 */
#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef int32_t s32;
typedef float f32;
typedef double f64;
typedef float Mtx[3][4];
typedef struct { float x, y, z; } Vec3;
typedef struct { float x, y, z, w; } Quaternion;
#define PAD_STACK(bytes)

/* `HSD_QuatLib_8037EB28` (Euler extraction) calls `atan2f`; renamed via
 * macro to the game's own pinned `lbtrigf.c` port (`tests/oracle/
 * lbtrigf_body.c`) rather than left on the host's system libm, so this
 * adapter matches `quaternion::matrix_to_euler`'s own `crate::math::atan2f`
 * bit-exactly instead of by tolerance. `EulerToQuat`'s and
 * `HSD_QuatLib_8037EF28`'s (interpolate's) own `sinf`/`cosf` calls are left
 * on host libm: `quaternion::from_euler` still calls `glam`'s `sin_cos`, not
 * `crate::math`, and `interpolate`'s tolerant/bit-exact comparisons already
 * pass without renaming them. See `docs/math.md`. */
extern float skirmish_lb_atan2f(float y, float x);
#define atan2f skirmish_lb_atan2f
#include "quaternion_original.inc"

int32_t oracle_quaternion(unsigned op, const float *a, const float *b,
                          float t, float *out) {
    Mtx matrix;
    Vec3 vector;
    Quaternion p, q, result;
    int32_t status = 0;
    memcpy(&result, out, sizeof(result));
    switch (op) {
    case 0:
        memcpy(matrix, a, sizeof(matrix));
        status = MatToQuat(matrix, &result);
        break;
    case 1:
        memcpy(matrix, a, sizeof(matrix));
        status = HSD_QuatLib_8037EB28(matrix, &vector);
        memcpy(out, &vector, sizeof(vector));
        return status;
    case 2:
        memcpy(&p, a, sizeof(p));
        memcpy(&q, b, sizeof(q));
        status = HSD_QuatLib_8037EC4C(&p, &q, &result);
        break;
    case 3:
        memcpy(&vector, a, sizeof(vector));
        status = HSD_QuatLib_8037ECE0(&vector, &result, t);
        break;
    case 4:
        memcpy(&vector, a, sizeof(vector));
        status = EulerToQuat(&vector, &result);
        break;
    case 5:
        memcpy(&p, a, sizeof(p));
        memcpy(&q, b, sizeof(q));
        status = HSD_QuatLib_8037EF28(&p, &q, &result, t);
        break;
    default:
        abort();
    }
    memcpy(out, &result, sizeof(result));
    return status;
}
