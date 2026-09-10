/* Verbatim MakeTextureMtx from the pinned tobj.c snapshot over the scalar
 * Dolphin matrix routines. HSD_MkRotationMtx, C_MTXConcat and C_MTXScale are
 * compiled once from the matrix snapshots in bones.c; MTXTrans's only original
 * form is paired-single assembly, so a scalar assignment shim stands in for it.
 * tobj.c defines its own FLT_EPSILON (1.00000001335e-10F) ahead of this
 * function; the adapter repeats that exact value.
 */
#include <math.h>
#include <stdlib.h>
#include <string.h>

typedef float f32;
typedef unsigned char u8;
typedef unsigned int u32;
typedef float Mtx[3][4];
typedef struct { float x, y, z; } Vec3;
typedef struct { float x, y, z, w; } Quaternion;
typedef enum { GX_CLAMP, GX_REPEAT, GX_MIRROR } GXTexWrapMode;

#define FLT_EPSILON 1.00000001335e-10F
#define PAD_STACK(bytes)
#define HSD_ASSERT(line, cond) do { if (!(cond)) abort(); } while (0)

typedef struct {
    Quaternion rotate;
    Vec3 scale;
    Vec3 translate;
    GXTexWrapMode wrap_s;
    GXTexWrapMode wrap_t;
    u8 repeat_s;
    u8 repeat_t;
    Mtx mtx;
} HSD_TObj;

extern void C_MTXConcat(Mtx a, Mtx b, Mtx ab);
extern void C_MTXScale(Mtx m, f32 xS, f32 yS, f32 zS);
extern void HSD_MkRotationMtx(Mtx m, Vec3* rotation);

static void MTXTrans(Mtx m, f32 xT, f32 yT, f32 zT)
{
    m[0][0] = 1.0F; m[0][1] = 0.0F; m[0][2] = 0.0F; m[0][3] = xT;
    m[1][0] = 0.0F; m[1][1] = 1.0F; m[1][2] = 0.0F; m[1][3] = yT;
    m[2][0] = 0.0F; m[2][1] = 0.0F; m[2][2] = 1.0F; m[2][3] = zT;
}
#define MTXConcat C_MTXConcat
#define MTXScale C_MTXScale

#include "tobj_original.inc"

void oracle_tobj_make_mtx(const float rotate[3], const float scale[3],
                          const float translate[3], int wrap_t,
                          unsigned repeat_s, unsigned repeat_t, Mtx out)
{
    HSD_TObj tobj;
    memset(&tobj, 0, sizeof(tobj));
    tobj.rotate.x = rotate[0];
    tobj.rotate.y = rotate[1];
    tobj.rotate.z = rotate[2];
    memcpy(&tobj.scale, scale, sizeof(tobj.scale));
    memcpy(&tobj.translate, translate, sizeof(tobj.translate));
    tobj.wrap_t = (GXTexWrapMode) wrap_t;
    tobj.repeat_s = (u8) repeat_s;
    tobj.repeat_t = (u8) repeat_t;
    MakeTextureMtx(&tobj);
    memcpy(out, tobj.mtx, sizeof(Mtx));
}
