/* Complete upstream reflection entry with no-result engine services stubbed.
 * The compared velocity, facing and timer arithmetic remains in the body. */
#include <math.h>
#include <stddef.h>
#include <stdint.h>

typedef uint8_t u8;
typedef int ftCommon_MotionState;
typedef struct { float x, y, z; } Vec3;
typedef struct {
    float x1B0;
    int x1B8;
    float x1BC, x1C0;
} ftCommonData;
typedef struct Fighter Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;
struct Fighter {
    Vec3 cur_pos, self_vel, x8c_kb_vel, x68C_transNPos;
    float facing_dir;
    struct { struct { struct { u8 x18, x19; } damage; } co; } mv;
    void* x60C;
    struct { float weight; } co_attrs;
};

static _Thread_local ftCommonData* p_ftCommonData;
#define GET_FIGHTER(gobj) (((Fighter_GObj*) (gobj))->user_data)
#define Ft_MF_Unk06 0
#define Ft_MF_SkipNametagVis 0
#define Ft_MF_KeepColAnimPartHitStatus 0
#define Ft_MF_SkipHitStun 0
#define SQ(value) ((value) * (value))
enum { ftCo_MS_FlyReflectWall = 1, ftCo_MS_FlyReflectCeil = 2,
       QuakeKind_Small = 0 };
static int ftCo_DownBound_SfxIds[1];

static void efAsync_Spawn(Fighter_GObj* object, void* slot, ...) {
    (void) object; (void) slot;
}
static void Camera_RequestQuake(int kind, Vec3* position) {
    (void) kind; (void) position;
}
static void Fighter_ChangeMotionState(Fighter_GObj* object, int state, ...) {
    (void) object; (void) state;
}
static void ft_80081F2C(Fighter_GObj* object) { (void) object; }
static void ft_80082084(Fighter_GObj* object) { (void) object; }
static void ftCommon_8007EBAC(Fighter* fighter, int kind, int value) {
    (void) fighter; (void) kind; (void) value;
}
static void ftColl_8007B760(Fighter_GObj* object, int value) {
    (void) object; (void) value;
}
static void ftCo_80097630(Fighter* fighter, int* ids, float magnitude) {
    (void) fighter; (void) ids; (void) magnitude;
}

#include "lbvector_original.inc"
#include "fly_reflect_original.inc"

void oracle_damage_reflect(const float* values, float multiplier, u8 lockout,
                           int wall, float* output, u8* timer) {
    static _Thread_local ftCommonData common;
    common = (ftCommonData) { .x1BC=multiplier, .x1C0=lockout };
    p_ftCommonData = &common;
    Fighter fighter = {
        .cur_pos={values[0], values[1], 0},
        .self_vel={values[2], values[3], 0},
        .x8c_kb_vel={values[4], values[5], 0},
        .x68C_transNPos={values[6], values[7], 0},
        .facing_dir=values[8],
        .co_attrs={.weight=values[9]},
    };
    Fighter_GObj object = { &fighter };
    Vec3 normal = {values[10], values[11], 0};
    Vec3 offset = {values[12], values[13], 0};
    ftCo_800C18A8(&object,
        wall ? ftCo_MS_FlyReflectWall : ftCo_MS_FlyReflectCeil,
        &normal, &offset);
    output[0] = fighter.x8c_kb_vel.x;
    output[1] = fighter.x8c_kb_vel.y;
    output[2] = fighter.self_vel.x;
    output[3] = fighter.self_vel.y;
    output[4] = fighter.facing_dir;
    output[5] = fighter.cur_pos.x;
    output[6] = fighter.cur_pos.y;
    *timer = fighter.mv.co.damage.x18;
}
