/* Host adapter for the complete leader branch of ftCo_Rebirth_Phys. */
#include <stdbool.h>

typedef float f32;
typedef unsigned char u8;

typedef struct Vec3 {
    float x;
    float y;
    float z;
} Vec3;

typedef struct Fighter {
    bool x221F_b4;
    struct {
        int x2135;
    } smash_attrs;
    struct {
        struct {
            struct {
                int x0;
                Vec3 x4;
            } common;
        } co;
    } mv;
    int player_id;
    float facing_dir;
    Vec3 self_vel;
} Fighter;

typedef struct HSD_GObj {
    void* user_data;
} HSD_GObj;
typedef HSD_GObj Fighter_GObj;

#define PAD_STACK(bytes) ((void) (bytes))

static _Thread_local Vec3 sampled_position;

static void Stage_80224E38(Vec3* result, int stage_id);
static void Player_GetSomePos(int player_id, Vec3* result);
static float ftCommon_800804EC(Fighter* fp);
static void ftCommon_8007F8B4(Fighter* fp, Vec3* result);
static HSD_GObj* Player_GetEntityAtIndex(int player_id, int entity_index);

#include "rebirth_original.inc"

static void Stage_80224E38(Vec3* result, int stage_id) {
    (void) stage_id;
    *result = (Vec3) { 0 };
}

static void Player_GetSomePos(int player_id, Vec3* result) {
    (void) player_id;
    *result = (Vec3) { 0 };
}

static float ftCommon_800804EC(Fighter* fp) {
    (void) fp;
    return 0.0f;
}

static void ftCommon_8007F8B4(Fighter* fp, Vec3* result) {
    (void) fp;
    *result = sampled_position;
}

static HSD_GObj* Player_GetEntityAtIndex(int player_id, int entity_index) {
    (void) player_id;
    (void) entity_index;
    return 0;
}

typedef struct OracleRebirthVelocity {
    float x;
    float y;
} OracleRebirthVelocity;

OracleRebirthVelocity oracle_rebirth_velocity(float current_x, float current_y,
                                               float target_x, float target_y,
                                               int remaining) {
    Fighter fp = { 0 };
    Fighter_GObj gobj = { &fp };
    sampled_position = (Vec3) { current_x, current_y, 0.0f };
    fp.smash_attrs.x2135 = -1;
    fp.mv.co.common.x0 = remaining;
    fp.mv.co.common.x4 = (Vec3) { target_x, target_y, 0.0f };
    ftCo_Rebirth_Phys(&gobj);
    return (OracleRebirthVelocity) { fp.self_vel.x, fp.self_vel.y };
}

OracleRebirthVelocity oracle_rebirth_wait_velocity(
    float current_x, float current_y, float target_x, float target_y,
    int remaining) {
    Fighter fp = { 0 };
    Fighter_GObj gobj = { &fp };
    sampled_position = (Vec3) { current_x, current_y, 0.0f };
    fp.smash_attrs.x2135 = -1;
    fp.mv.co.common.x0 = remaining;
    fp.mv.co.common.x4 = (Vec3) { target_x, target_y, 0.0f };
    ftCo_RebirthWait_Phys(&gobj);
    return (OracleRebirthVelocity) { fp.self_vel.x, fp.self_vel.y };
}
