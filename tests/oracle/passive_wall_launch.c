/* Host adapter for the PassiveWallJump launch branch of its full Anim callback. */
#include <stdbool.h>
#include <math.h>
#include <stdint.h>

typedef uint8_t u8;
typedef struct { float x, y, z; } Vec3;
typedef struct Fighter {
    struct {
        struct {
            struct {
                int timer;
                bool x8;
                int vel_y_exponent;
            } passivewall;
        } co;
    } mv;
    int motion_id;
    Vec3 self_vel;
    float facing_dir;
    struct {
        float passivewall_vel_x;
        float wall_jump_horizontal_velocity;
        float wall_jump_vertical_velocity;
    } co_attrs;
    u8 x1969_walljumpUsed;
} Fighter;
typedef struct Fighter_GObj { Fighter* user_data; } Fighter_GObj;
typedef struct { float passive_wall_vel_y_base; } ftCommonData;

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define PAD_STACK(bytes)
#define ftCo_MS_PassiveWall 76
#define ftCo_MS_PassiveWallJump 77

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static void ft_PlaySFX(Fighter* fp, int id, int volume, int pan) {
    (void) fp; (void) id; (void) volume; (void) pan;
}
static void inlineA0(Fighter_GObj* gobj) { (void) gobj; }
static void ftAnim_SetAnimRate(Fighter_GObj* gobj, float rate) {
    (void) gobj; (void) rate;
}
static bool ftAnim_IsFramesRemaining(Fighter_GObj* gobj) {
    (void) gobj; return true;
}
static void ftCo_Fall_Enter(Fighter_GObj* gobj) { (void) gobj; }

#include "passive_wall_launch_original.inc"

typedef struct { uint32_t x_bits; uint32_t y_bits; } OracleVelocity;

OracleVelocity oracle_passive_wall_jump_launch(
    float facing, float horizontal, float vertical, float base,
    u8 used, u8 exponent) {
    Fighter fp = { 0 };
    Fighter_GObj gobj = { &fp };
    fp.mv.co.passivewall.timer = 1;
    fp.motion_id = ftCo_MS_PassiveWallJump;
    fp.facing_dir = facing;
    fp.co_attrs.wall_jump_horizontal_velocity = horizontal;
    fp.co_attrs.wall_jump_vertical_velocity = vertical;
    fp.x1969_walljumpUsed = used;
    fp.mv.co.passivewall.vel_y_exponent = exponent;
    common.passive_wall_vel_y_base = base;
    p_ftCommonData = &common;
    ftCo_PassiveWall_Anim(&gobj);
    union { float f; uint32_t u; } x = { fp.self_vel.x };
    union { float f; uint32_t u; } y = { fp.self_vel.y };
    return (OracleVelocity) { x.u, y.u };
}
