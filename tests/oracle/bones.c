/* Verbatim selected matrix routines from HSD and Dolphin snapshots. Scalar C
 * matrix paths are the native reference; PowerPC paired-single assembly is not
 * compiled. HSD sine/cosine use host libm, compared numerically to Rust libm.
 */
#include <math.h>
#include <stdlib.h>
#include <string.h>

typedef float f32;
typedef float Mtx[3][4];
typedef float Mtx44[4][4];
typedef struct { float x, y, z; } Vec3;
typedef Vec3 Vec;
#define ASSERTMSGLINE(line, condition, message) do { if (!(condition)) abort(); } while (0)
#include "hsd_mtx_original.inc"
#include "sdk_mtx_original.inc"
#include "sdk_mtxvec_original.inc"

void oracle_bones_srt(const float scale[3], const float rotation[3],
                       const float translation[3], const float* parent_scale,
                       Mtx out)
{
    Vec3 s, r, t, p;
    memcpy(&s, scale, sizeof(s));
    memcpy(&r, rotation, sizeof(r));
    memcpy(&t, translation, sizeof(t));
    if (parent_scale) memcpy(&p, parent_scale, sizeof(p));
    HSD_MtxSRT(out, &s, &r, &t, parent_scale ? &p : NULL);
}

void oracle_bones_transform(Mtx matrix, const float input[3], float output[3],
                            int vector_only)
{
    Vec src, dst;
    Mtx44 wide = {{0}};
    memcpy(wide, matrix, sizeof(Mtx));
    memcpy(&src, input, sizeof(src));
    if (vector_only) C_MTXMultVecSR(wide, &src, &dst);
    else C_MTXMultVec(wide, &src, &dst);
    memcpy(output, &dst, sizeof(dst));
}
