/* Verbatim lbcollision/HSD/SDK functions from existing pinned snapshots. Types,
 * assertion macros and approximatelyZero's header inline adapt the host ABI.
 * PSMTXMultVec uses the SDK's scalar C equivalent already compiled in bones.c;
 * this is a native scalar reference, not a paired-single instruction emulator.
 */
#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
typedef int bool;
typedef float f32;
typedef float Mtx[3][4];
typedef float Mtx44[4][4];
typedef float (*MtxPtr)[4];
typedef struct { float x, y, z; } Vec3;
typedef Vec3 Vec;
#define true 1
#define false 0
#define PAD_STACK(n)
#define EPSILON 0.0000000001f
#define ASSERTMSGLINE(line, condition, message) do { if (!(condition)) abort(); } while (0)
#define MTXIdentity C_MTXIdentity
#define MTXCopy C_MTXCopy
static inline float fabsf_bitwise(float x)
{
    uint32_t bits;
    memcpy(&bits, &x, sizeof bits);
    bits &= UINT32_C(0x7fffffff);
    memcpy(&x, &bits, sizeof x);
    return x;
}
static inline bool approximatelyZero(float x) { return x < .00001f && x > -.00001f; }
extern float lbColl_80005EBC(Vec3*, Vec3*, Vec3*, float*);
extern void C_MTXMultVec(Mtx44, const Vec*, Vec*);
extern void C_MTXCopy(Mtx, Mtx);
static void PSMTXMultVec(Mtx matrix, const Vec* source, Vec* target)
{
    Mtx44 wide = {{0}};
    memcpy(wide, matrix, sizeof(Mtx));
    C_MTXMultVec(wide, source, target);
}
#include "shield_sdk_original.inc"
#include "shield_mtx_original.inc"
#include "shield_collision_original.inc"

void oracle_shield_inverse(const float input[12], float output[12])
{
    Mtx matrix, inverse;
    memcpy(matrix, input, sizeof matrix);
    HSD_MtxInverse(matrix, inverse);
    memcpy(output, inverse, sizeof inverse);
}

int oracle_shield_collision(const float input[27], float output[10])
{
    Vec3 hit_start, hit_end, hurt_start, hurt_end, a, b, contact;
    Mtx matrix;
    memcpy(&hit_start, input, sizeof hit_start);
    memcpy(&hit_end, input + 3, sizeof hit_end);
    memcpy(&hurt_start, input + 6, sizeof hurt_start);
    memcpy(&hurt_end, input + 9, sizeof hurt_end);
    memcpy(matrix, input + 12, sizeof matrix);
    memcpy(&a, output, sizeof a);
    memcpy(&b, output + 3, sizeof b);
    memcpy(&contact, output + 6, sizeof contact);
    float overlap = output[9];
    int result = lbColl_80006E58(&hit_start, &hit_end, &hurt_start, &hurt_end,
        &a, &b, matrix, &contact, &overlap, input[24], input[25], input[26]);
    memcpy(output, &a, sizeof a);
    memcpy(output + 3, &b, sizeof b);
    memcpy(output + 6, &contact, sizeof contact);
    output[9] = overlap;
    return result;
}
