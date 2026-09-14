/* Drives the pinned HSD_JObjMakeMatrix (sysdolphin/baselib/jobj.c) over a
 * caller-supplied flat joint chain, exactly as `simulation::pose` builds
 * `bones::Bone`. This is the C-oracle counterpart to `collision::bones`'s
 * `Pose::evaluate`: HSD's own JOBJ_CLASSICAL_SCALE bookkeeping, scale
 * compensation and matrix concatenation, run through the real decompiled
 * function bodies instead of a Rust transliteration.
 *
 * ABI adaptations from the real `HSD_JObj`/`jobj.h` (documented here, not
 * silently assumed):
 *   - `rotate` is `Quaternion` (x, y, z, w), the real field's own type: a
 *     joint with `JOBJ_USE_QUATERNION` (0x20000) set uses it directly
 *     (`HSD_MtxSRTQuat(..., &jobj->rotate, ...)`, no cast in the pinned
 *     extraction); an ordinary Euler joint's `HSD_MtxSRT` call casts the
 *     same field to `Vec3*` (`(Vec3*) &jobj->rotate`, also uncast in the
 *     source), reading its first three components -- `Vec3`'s layout is a
 *     prefix of `Quaternion`'s, so both reinterpretations are valid.
 *   - `next`/`child` (sibling/child links) and `robj` are omitted: this
 *     driver walks the supplied joints in caller order and calls
 *     `HSD_JObjMakeMatrix` on each directly (see "marks matrices dirty,
 *     calls HSD_JObjMakeMatrix top-down" in the differential test's own
 *     module doc), so nothing here ever needs to traverse children or
 *     dispatch through `HSD_JObjSetupMatrixSub`'s IK/RObj branches. Every
 *     supplied joint's own `flags` never sets JOBJ_JOINT/JOBJ_EFFECTOR, so
 *     the real `HSD_JObjSetupMatrixSub` would take the same "do nothing
 *     extra" path this driver's own minimal replacement takes.
 *   - `aobj` is always NULL here (no bone in the supplied pose uses an
 *     AObj-driven translation override), so `HSD_JObjMakeMatrix`'s own
 *     `aobj->hsd_obj` branch is never taken; `HSD_AObj` is a one-field stub
 *     purely so that dead branch still type-checks.
 *   - `PSMTXConcat`/`MTXConcat`/`MTXScale`/`MTXQuat` are real GameCube
 *     paired-single assembly (or, for `MTXQuat`, the macro the retail SDK
 *     build resolves to `PSMTXQuat`); `C_MTXConcat`/`C_MTXScale`/`C_MTXQuat`
 *     (already pinned in `sdk_mtx.c`, `docs/math.md`'s FMA audit;
 *     `C_MTXQuat`/`C_MTXScale` added for this batch's own quaternion-path
 *     coverage) are their scalar C equivalents and are substituted with
 *     macros, the same substitution `bones.c`'s own callers already rely
 *     on for `PSMTXConcat`. `PSMTXTrans` (`dolphin/mtx/mtx.c:706-723`) has
 *     no scalar `C_` sibling in the pinned snapshot; its own body is
 *     definitionally identity-plus-translation-column (assignment only, no
 *     arithmetic to disagree on), so this driver defines it directly
 *     instead of extracting PS asm. `MTXMultVec` is the AObj-
 *     translation-override path's own paired-single primitive (distinct
 *     from the already-pinned scalar `C_MTXMultVec` in `sdk_mtxvec.c`,
 *     which a *different* `HSD_JObjMakeMatrix` caller uses for ordinary
 *     point transforms); every joint here has `aobj == NULL`, so it is
 *     never reached and stays an abort-on-call stub.
 *   - `HSD_MtxSRT`/`HSD_MtxSRTQuat` (`sysdolphin/baselib/mtx.c:362-434`)
 *     are re-extracted under their own adapter name (`bones_pose_mtx`,
 *     `adapters.json`, `bones_pose_mtx.functions.json`) instead of reusing
 *     `bones.c`'s own copy: that copy deliberately keeps host libm
 *     `sinf`/`cosf`
 *     (`bones.c`'s own header comment, `docs/math.md`) because its own
 *     proptests only need a documented numerical tolerance. A 73-joint
 *     chain concatenates many rotated bones, so per-joint libm-vs-MSL trig
 *     noise compounds; matching `Pose::evaluate` bit-for-bit here instead
 *     macro-substitutes the pinned MSL `sinf`/`cosf` (`trigf_body.c`,
 *     already compiled into this same static library as `skirmish_msl_
 *     sinf`/`skirmish_msl_cosf`, the same functions `crate::compat::math::
 *     trig::sinf`/`cosf` themselves port) in place of libm's, and renames
 *     the resulting function (`HSD_MtxSRT` -> `bones_pose_HSD_MtxSRT`) so
 *     it does not collide with `bones.c`'s own libm-backed `HSD_MtxSRT` in
 *     the same static library. The rename macro stays active through the
 *     `jobj_original.inc` include below too, so `HSD_JObjMakeMatrix`'s own
 *     (otherwise-verbatim) call site resolves to this MSL-trig copy.
 */
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef float f32;
typedef uint32_t u32;
typedef float Mtx[3][4];
typedef struct {
    float x, y, z;
} Vec3;
typedef struct {
    float x, y, z, w;
} Quaternion;

typedef struct HSD_AObj {
    void* hsd_obj;
} HSD_AObj;

typedef struct HSD_JObj {
    struct HSD_JObj* parent;
    u32 flags;
    Quaternion rotate;
    Vec3 scale;
    Vec3 translate;
    Mtx mtx;
    Vec3* scl;
    HSD_AObj* aobj;
} HSD_JObj;

#define JOBJ_MTX_DIRTY (1 << 6)
#define JOBJ_USE_QUATERNION (1 << 17)
#define JOBJ_USER_DEF_MTX (1 << 23)

/* HSD_VecAlloc/HSD_VecFree (mtx.c:454-472) are thin wrappers around HSD's
 * generic fixed-size object allocator (HSD_ObjAlloc/HSD_ObjFree over a
 * HSD_Mtx_804C2310 pool); pulling that allocator in is unnecessary for a
 * test driver that starts every joint's `scl` at NULL and discards the
 * whole chain after one evaluation, so this stubs the same alloc/free
 * *contract* (a live Vec3 the algorithm can write through and later
 * free) with the host allocator instead. */
static void* HSD_VecAlloc(void)
{
    void* vec = malloc(sizeof(Vec3));
    if (vec == NULL) abort();
    return vec;
}

static void HSD_VecFree(void* arg0)
{
    free(arg0);
}

extern void C_MTXConcat(Mtx a, Mtx b, Mtx ab);
extern void C_MTXQuat(Mtx m, Quaternion* q);
extern void C_MTXScale(Mtx m, f32 xS, f32 yS, f32 zS);
extern float skirmish_msl_sinf(float x);
extern float skirmish_msl_cosf(float x);

/* Unreachable with the flags this driver ever sets (see the module
 * comment): the AObj-translation-override path's own paired-single
 * primitive, distinct from the already-pinned scalar `C_MTXMultVec` in
 * `sdk_mtxvec.c`, which a *different* `HSD_JObjMakeMatrix` caller uses for
 * ordinary point transforms. Every joint here has `aobj == NULL`, so this
 * body never runs; it must still exist for `HSD_JObjMakeMatrix`'s own
 * compiled body to link, and aborts loudly instead of silently doing the
 * wrong thing if that invariant is ever violated. */
static void MTXMultVec(Mtx m, Vec3* src, Vec3* dst)
{
    (void) m;
    (void) src;
    (void) dst;
    abort();
}

/* `dolphin/mtx/mtx.c:706-723`: identity with `(xT, yT, zT)` in column 3.
 * No scalar `C_MTXTrans` sibling exists in the pinned snapshot (see the
 * module comment) -- this is exactly that assignment-only body, not an
 * approximation of it. */
static void PSMTXTrans(Mtx m, f32 xT, f32 yT, f32 zT)
{
    m[0][0] = 1.0f; m[0][1] = 0.0f; m[0][2] = 0.0f; m[0][3] = xT;
    m[1][0] = 0.0f; m[1][1] = 1.0f; m[1][2] = 0.0f; m[1][3] = yT;
    m[2][0] = 0.0f; m[2][1] = 0.0f; m[2][2] = 1.0f; m[2][3] = zT;
}

#define PSMTXConcat C_MTXConcat
/* `sysdolphin/baselib/mtx.h`'s own scalar-SDK-build mapping
 * (`#define MTXConcat C_MTXConcat`, `#define MTXScale C_MTXScale`,
 * `#define MTXQuat C_MTXQuat`), matching `HSD_MtxSRTQuat`'s own call sites
 * (`MTXScale`, `MTXConcat`, `MTXQuat`, `PSMTXTrans`) once it is extracted
 * below. */
#define MTXConcat C_MTXConcat
#define MTXScale C_MTXScale
#define MTXQuat C_MTXQuat

void HSD_JObjMakeMatrix(HSD_JObj* jobj);

static inline bool jobj_mtx_is_dirty(HSD_JObj* jobj)
{
    return !(jobj->flags & JOBJ_USER_DEF_MTX) && (jobj->flags & JOBJ_MTX_DIRTY);
}

/* Minimal stand-in for the real `HSD_JObjSetupMatrixSub` (jobj.c:1385-1441):
 * every joint this driver builds is an ordinary bone (no JOBJ_JOINT1/2,
 * JOBJ_EFFECTOR, or `robj`), so the real function's own IK/RObj branches
 * are always inert here and its net effect on this subset -- call
 * `make_mtx`, clear JOBJ_MTX_DIRTY -- is exactly this. In practice this is
 * dead code for a correctly topologically-ordered caller (see the module
 * comment): each joint's own dirty flag is already cleared by the driver
 * loop before any descendant's `HSD_JObjMakeMatrix` runs. */
static void HSD_JObjSetupMatrixSub(HSD_JObj* jobj)
{
    HSD_JObjMakeMatrix(jobj);
    jobj->flags &= ~JOBJ_MTX_DIRTY;
}

static inline void HSD_JObjSetupMatrix(HSD_JObj* jobj)
{
    if (jobj == NULL || !jobj_mtx_is_dirty(jobj)) return;
    HSD_JObjSetupMatrixSub(jobj);
}

/* Renames both the definition (in `bones_pose_mtx_original.inc`) and every
 * call site (in `jobj_original.inc`, included below while this macro is
 * still active) from `HSD_MtxSRT` to `bones_pose_HSD_MtxSRT`, and swaps in
 * the pinned MSL trig in place of libm for this one, private copy -- see
 * the module comment. */
#define HSD_MtxSRT bones_pose_HSD_MtxSRT
#define sinf skirmish_msl_sinf
#define cosf skirmish_msl_cosf
#include "bones_pose_mtx_original.inc"
#undef sinf
#undef cosf

#include "jobj_original.inc"
#undef HSD_MtxSRT

/* Builds `count` joints from flat per-axis arrays (matching
 * `bones::Bone`/`LocalTransform`'s own field order), calls the real,
 * pinned `HSD_JObjMakeMatrix` on each in the caller's supplied order
 * (parent-before-child, exactly as `Pose::evaluate`'s own topological
 * walk visits a bone only after its parent), and writes every joint's
 * resulting 3x4 world matrix back out row-major. `parent[i] < 0` means no
 * parent; otherwise `parent[i]` must already have been resolved (index
 * strictly less than `i` is sufficient and is what every supplied pose
 * satisfies). `use_quaternion[i]` selects `JOBJ_USE_QUATERNION`
 * (`bones::LocalTransform.rotation_quaternion.is_some()`): when set, the
 * corresponding four floats of `quaternion` (x, y, z, w) drive that
 * joint's rotation through `HSD_MtxSRTQuat` instead of `rotation`/
 * `HSD_MtxSRT`.
 */
void oracle_bones_pose(const int32_t* parent, const int32_t* classical_scale,
                        const float* scale, const float* rotation,
                        const float* translation, const int32_t* use_quaternion,
                        const float* quaternion, int32_t count,
                        float* out_matrices)
{
    HSD_JObj* joints = calloc((size_t) count, sizeof(HSD_JObj));
    if (joints == NULL) abort();
    for (int32_t i = 0; i < count; i++) {
        joints[i].parent = parent[i] < 0 ? NULL : &joints[parent[i]];
        joints[i].flags = JOBJ_MTX_DIRTY | (classical_scale[i] ? 8u : 0u)
                           | (use_quaternion[i] ? JOBJ_USE_QUATERNION : 0u);
        joints[i].scale = (Vec3){ scale[3 * i], scale[3 * i + 1], scale[3 * i + 2] };
        if (use_quaternion[i]) {
            joints[i].rotate = (Quaternion){
                quaternion[4 * i], quaternion[4 * i + 1],
                quaternion[4 * i + 2], quaternion[4 * i + 3]
            };
        } else {
            joints[i].rotate = (Quaternion){
                rotation[3 * i], rotation[3 * i + 1], rotation[3 * i + 2], 0.0f
            };
        }
        joints[i].translate = (Vec3){
            translation[3 * i], translation[3 * i + 1], translation[3 * i + 2]
        };
        joints[i].scl = NULL;
        joints[i].aobj = NULL;
    }
    for (int32_t i = 0; i < count; i++) {
        HSD_JObjMakeMatrix(&joints[i]);
        joints[i].flags &= ~JOBJ_MTX_DIRTY;
        memcpy(out_matrices + 12 * i, joints[i].mtx, sizeof(Mtx));
    }
    for (int32_t i = 0; i < count; i++) {
        if (joints[i].scl != NULL) HSD_VecFree(joints[i].scl);
    }
    free(joints);
}
