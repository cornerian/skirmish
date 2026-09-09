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
