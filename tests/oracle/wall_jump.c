/* Host adapter for complete ftWallJump_8008169C state/comparison logic. */
#include <stdbool.h>
#include <stdint.h>

typedef int32_t s32;
typedef uint8_t u8;
typedef struct { float x, y, z; } Vec3;
typedef struct { float x, y; } Vec2;
typedef struct { int index; } WallContact;
typedef struct {
    int env_flags;
    struct { Vec2 left, right; } ecb;
    WallContact right_facing_wall;
    WallContact left_facing_wall;
} CollData;
typedef struct Fighter {
    bool can_walljump;
    CollData coll_data;
    u8 wall_jump_input_timer;
    float x2110_walljumpWallSide;
    Vec3 cur_pos;
    Vec3 pos_delta;
    struct { float wall_jump_min_approach_speed; } co_attrs;
    struct { struct { float x, y; } lstick[1]; } input;
    u8 x670_timer_lstick_tilt_x;
    u8 x1969_walljumpUsed;
} Fighter;
typedef struct HSD_GObj { Fighter* user_data; } HSD_GObj;
typedef HSD_GObj Fighter_GObj;
typedef int FtMotionId;
typedef struct {
    float x768;
    float x76C;
    float x770;
    int x774;
} ftCommonData;

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define Collide_RightWallHug 1
#define Collide_LeftWallHug 2
#define ftCo_MS_PassiveWallJump 77

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local float supplied_wall_speed;
static _Thread_local bool supplied_speed_valid;
static _Thread_local int entered_timer;
static _Thread_local int entered_exponent;
static _Thread_local float entered_side;
static int const max_input_frames = 254;

static bool mpGetSpeed(int line, Vec3* point, Vec3* speed) {
    (void) line;
    (void) point;
    speed->x = supplied_wall_speed;
    speed->y = 0.0f;
    speed->z = 0.0f;
    return supplied_speed_valid;
}

static void ftCo_800C1E64(HSD_GObj* gobj, FtMotionId motion, int timer,
                          int exponent, float side) {
    (void) gobj;
    (void) motion;
    entered_timer = timer;
    entered_exponent = exponent;
    entered_side = side;
}

#include "wall_jump_original.inc"

typedef struct {
    uint32_t wall_side_bits;
    uint32_t entered_side_bits;
    int32_t entered_timer;
    int32_t entered_exponent;
    u8 input_timer;
    u8 used;
    u8 triggered;
    u8 padding;
} OracleWallJumpResult;

OracleWallJumpResult oracle_wall_jump(
    int can_walljump, int contact, int speed_valid, u8 input_timer,
    float wall_side, u8 used, float position_delta_x, float wall_speed_x,
    float minimum_approach_speed, float stick_x, u8 tilt_x_age,
    float input_window, float stick_threshold, float tilt_window,
    int startup_frames) {
    Fighter fp = { 0 };
    HSD_GObj gobj = { &fp };
    fp.can_walljump = can_walljump != 0;
    fp.coll_data.env_flags = contact == 1 ? Collide_RightWallHug
                              : contact == 2 ? Collide_LeftWallHug : 0;
    fp.wall_jump_input_timer = input_timer;
    fp.x2110_walljumpWallSide = wall_side;
    fp.x1969_walljumpUsed = used;
    fp.pos_delta.x = position_delta_x;
    fp.co_attrs.wall_jump_min_approach_speed = minimum_approach_speed;
    fp.input.lstick[0].x = stick_x;
    fp.x670_timer_lstick_tilt_x = tilt_x_age;
    supplied_wall_speed = wall_speed_x;
    supplied_speed_valid = speed_valid != 0;
    common = (ftCommonData) {
        input_window, stick_threshold, tilt_window, startup_frames
    };
    p_ftCommonData = &common;
    entered_timer = -1;
    entered_exponent = -1;
    entered_side = 0.0f;
    bool triggered = ftWallJump_8008169C(&gobj);
    union { float f; uint32_t u; } side = { fp.x2110_walljumpWallSide };
    union { float f; uint32_t u; } entered = { entered_side };
    return (OracleWallJumpResult) {
        side.u, entered.u, entered_timer, entered_exponent,
        fp.wall_jump_input_timer, fp.x1969_walljumpUsed, triggered, 0
    };
}
