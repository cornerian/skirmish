/* Host ABI shim for the unmodified pinned spline.c snapshot.
 * build.rs removes only #include lines into spline_original.inc.
 * GameCube scalar typedefs and Vec3/HSD_Spline are recreated here. Host pointers
 * use the host ABI; the production Rust API does not expose this struct.
 * sqrtf__Ff maps to host sqrtf: these tests establish host-C arithmetic agreement,
 * not agreement with the GameCube sqrt implementation or whole-game execution.
 */
#include <math.h>
#include <stdint.h>

typedef float f32;
typedef int16_t s16;
typedef int32_t s32;
typedef uint8_t u8;
typedef struct { f32 x, y, z; } Vec3;
typedef struct {
    u8 type;
    s16 numcv;
    f32 tension;
    Vec3* cv;
    f32 totalLength;
    f32* segLength;
    f32 (*segPoly)[5];
} HSD_Spline;

#define ABS(x) ((x) < 0 ? -(x) : (x))
#define sqrtf__Ff sqrtf
#include "spline_original.inc"

/* Expose the original static helper without duplicating its arithmetic. */
f32 oracle_spline_polynomial(const f32 coeffs[5], f32 t)
{
    return splArcLengthPolynomial(coeffs, t);
}
