/* Verbatim selected jump/walk and supporting movement functions. The adapter
 * supplies material friction as an explicit environment input. Jump sound flag
 * is false; sound callbacks abort if unexpectedly reached. Thread-local common
 * data isolates independent concurrent test calls, not gameplay global state.
 */
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef uint8_t u8;
typedef float f32;
typedef struct { float x, y; } Vec2;
typedef struct { float x, y, z; } Vec3;
typedef struct {
    float ground_to_air_jump_momentum_multiplier, jump_h_initial_velocity;
    float jump_h_max_velocity, jump_v_initial_velocity, hop_v_initial_velocity;
    float walk_accel_mul, walk_accel_base, walk_max_vel, ground_friction;
    float ground_max_horizontal_velocity;
} ftCo_DatAttrs;
typedef struct { float x438, x440, walk_accel_taper_gain; } CommonData;
static _Thread_local CommonData* p_ftCommonData;
typedef struct {
    Vec3 self_vel, x74_anim_vel;
    float gr_vel, xE4_ground_accel_1, ground_friction_multiplier;
    struct { Vec2 lstick[1]; } input;
    struct { struct { Vec3 normal; } floor; } coll_data;
    struct { struct {
        struct { bool x0, x4; float jump_mul; } jump;
        struct { float accel_mul, x0; } walk;
    } co; } mv;
    ftCo_DatAttrs co_attrs;
    u8 x671_timer_lstick_tilt_y;
    void* x197C;
    struct { struct { int x10; }* x4C_sfx; }* ft_data;
} Fighter;
typedef struct { Fighter* user_data; } HSD_GObj;
typedef HSD_GObj Fighter_GObj;

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define ABS(x) ((x) < 0 ? -(x) : (x))
#define SFX_VOLUME_MAX 127
#define SFX_PAN_MID 64
static void ft_800881D8(Fighter* fp, int id, int volume, int pan) { abort(); }
static void ft_PlaySFX(Fighter* fp, int id, int volume, int pan) { abort(); }
static float ft_GetGroundFrictionMultiplier(Fighter* fp) { return fp->ground_friction_multiplier; }

/* Distinct linkage permits this compact Fighter adapter and the separate
 * movement adapter to execute the same original function bodies safely. */
#define ftCommon_ApplyFrictionGround locomotion_ApplyFrictionGround
#define ftCommon_8007C98C locomotion_AccelerateGround
#define ftCommon_ApplyGroundMovement locomotion_ApplyGroundMovement
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-variable"
#include "locomotion_common_original.inc"
#include "ftjump_original.inc"
#include "ftwalk_original.inc"
#pragma GCC diagnostic pop

void oracle_locomotion_jump(const float old[3], float stick, int short_hop,
                            float jump_mul, const float attributes[5], float out[3])
{
    Fighter fp = {0};
    Fighter_GObj object = {&fp};
    CommonData common = {1.0f, 0.0f, 0.0f};
    p_ftCommonData = &common;
    memcpy(&fp.self_vel, old, sizeof(Vec3));
    memcpy(&fp.co_attrs, attributes, 5 * sizeof(float));
    fp.input.lstick[0].x = stick;
    fp.mv.co.jump.x0 = short_hop != 0;
    fp.mv.co.jump.x4 = true;
    ftCo_800CB110(&object, false, jump_mul);
    if (fp.mv.co.jump.x4 || fp.x671_timer_lstick_tilt_y != 0xFE) abort();
    memcpy(out, &fp.self_vel, sizeof(Vec3));
}

/* state: velocity XYZ, animation XYZ, ground velocity, acceleration, stick X,
 * floor normal XYZ, ground maximum. parameters follow WalkParameters order. */
float oracle_locomotion_walk(float state[13], const float parameters[8])
{
    Fighter fp = {0};
    HSD_GObj object = {&fp};
    CommonData common = {0.0f, parameters[7], parameters[5]};
    p_ftCommonData = &common;
    memcpy(&fp.self_vel, state, sizeof(Vec3));
    memcpy(&fp.x74_anim_vel, state + 3, sizeof(Vec3));
    fp.gr_vel = state[6];
    fp.xE4_ground_accel_1 = state[7];
    fp.input.lstick[0].x = state[8];
    memcpy(&fp.coll_data.floor.normal, state + 9, sizeof(Vec3));
    fp.co_attrs.ground_max_horizontal_velocity = state[12];
    fp.mv.co.walk.accel_mul = parameters[0];
    fp.co_attrs.walk_accel_mul = parameters[1];
    fp.co_attrs.walk_accel_base = parameters[2];
    fp.co_attrs.walk_max_vel = parameters[3];
    fp.co_attrs.ground_friction = parameters[4];
    fp.ground_friction_multiplier = parameters[6];
    ftWalkCommon_800E0060(&object);
    memcpy(state, &fp.self_vel, sizeof(Vec3));
    memcpy(state + 3, &fp.x74_anim_vel, sizeof(Vec3));
    state[6] = fp.gr_vel;
    state[7] = fp.xE4_ground_accel_1;
    return fp.mv.co.walk.x0;
}
