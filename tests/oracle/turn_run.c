/* Host adapter for the complete ordinary TurnRun physics callback. */
#include <stddef.h>

typedef float f32;
typedef struct {
    float x, y, z;
} Vec3;
typedef struct {
    float ground_friction;
} ftCo_DatAttrs;
typedef struct Fighter {
    float gr_vel;
    float xE4_ground_accel_1;
    Vec3 self_vel;
    Vec3 x74_anim_vel;
    struct {
        struct {
            Vec3 normal;
        } floor;
    } coll_data;
    struct {
        struct {
            struct {
                float accel_mul;
            } turnrun;
        } co;
    } mv;
    ftCo_DatAttrs co_attrs;
} Fighter;
typedef struct {
    Fighter* user_data;
} Fighter_GObj;
typedef struct {
    float run_dash_turn_friction_multiplier;
} ftCommonData;

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local float supplied_acceleration;
static _Thread_local float supplied_target;

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define PAD_STACK(size) ((void) (size))
#define ABS(value) ((value) < 0 ? -(value) : (value))

static void getAccelAndTarget(Fighter* fp, float* acceleration, float* target) {
    (void) fp;
    *acceleration = supplied_acceleration;
    *target = supplied_target;
}

static void ftCommon_ApplyFrictionGround(Fighter* fp, float friction) {
    if (ABS(friction) > ABS(fp->gr_vel)) {
        friction = -fp->gr_vel;
    } else if (fp->gr_vel > 0) {
        friction = -friction;
    }
    fp->xE4_ground_accel_1 = friction;
}

static void ftCommon_ApplyGroundMovement(Fighter_GObj* gobj) {
    Fighter* fp = GET_FIGHTER(gobj);
    fp->x74_anim_vel.x = fp->coll_data.floor.normal.y * fp->xE4_ground_accel_1;
    fp->x74_anim_vel.y = -fp->coll_data.floor.normal.x * fp->xE4_ground_accel_1;
    fp->x74_anim_vel.z = 0.0F;
    fp->self_vel.x = fp->coll_data.floor.normal.y * fp->gr_vel;
    fp->self_vel.y = -fp->coll_data.floor.normal.x * fp->gr_vel;
    fp->self_vel.z = 0.0F;
}

#include "turn_run_original.inc"

/* state: ground velocity/acceleration, self XY, animation XY, floor normal XY.
 * parameters: supplied acceleration/target, entry facing, ground friction,
 * common friction multiplier.
 */
void oracle_turn_run(float state[8], const float parameters[5]) {
    Fighter fighter = { 0 };
    Fighter_GObj object = { &fighter };
    fighter.gr_vel = state[0];
    fighter.xE4_ground_accel_1 = state[1];
    fighter.self_vel.x = state[2];
    fighter.self_vel.y = state[3];
    fighter.x74_anim_vel.x = state[4];
    fighter.x74_anim_vel.y = state[5];
    fighter.coll_data.floor.normal.x = state[6];
    fighter.coll_data.floor.normal.y = state[7];
    fighter.co_attrs.ground_friction = parameters[3];
    fighter.mv.co.turnrun.accel_mul = parameters[2];
    common.run_dash_turn_friction_multiplier = parameters[4];
    p_ftCommonData = &common;
    supplied_acceleration = parameters[0];
    supplied_target = parameters[1];
    ftCo_TurnRun_Phys(&object);
    state[0] = fighter.gr_vel;
    state[1] = fighter.xE4_ground_accel_1;
    state[2] = fighter.self_vel.x;
    state[3] = fighter.self_vel.y;
    state[4] = fighter.x74_anim_vel.x;
    state[5] = fighter.x74_anim_vel.y;
    state[6] = fighter.coll_data.floor.normal.x;
    state[7] = fighter.coll_data.floor.normal.y;
}
