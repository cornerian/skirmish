/* Host adapter for the complete ftCommon_GrabMash state transition. */
#include <stdbool.h>
#include <stdint.h>

typedef int8_t s8;
typedef uint8_t u8;
typedef uint32_t u32;
typedef float f32;
typedef struct { f32 x, y; } Vec2;
typedef struct { u32 pressed_buttons; Vec2 lstick[1]; } FighterInput;
typedef struct Fighter {
    FighterInput input;
    f32 grab_timer;
    s8 x1A50, x1A51;
    u8 x1A52, x1A53;
    u8 x2224_b5, x2224_b6;
    int player_id, x221F_b4;
} Fighter;
typedef struct { f32 x308; } ftCommonData;

#define HSD_PAD_A (1 << 8)
#define HSD_PAD_B (1 << 9)
#define HSD_PAD_X (1 << 10)
#define HSD_PAD_Y (1 << 11)
#define HSD_PAD_LR (1U << 31)
#define HSD_PAD_AB (HSD_PAD_A | HSD_PAD_B)
#define HSD_PAD_XY (HSD_PAD_X | HSD_PAD_Y)

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static void pl_800402D0(int player, int subchar, bool result) {
    (void) player; (void) subchar; (void) result;
}

#include "grab_mash_original.inc"

int oracle_grab_mash(float* timer, int8_t* axes, uint8_t* shake,
                     uint32_t pressed, float stick_x, float stick_y,
                     float penalty, float threshold) {
    Fighter fp = { 0 };
    fp.input.pressed_buttons = pressed;
    fp.input.lstick[0] = (Vec2) { stick_x, stick_y };
    fp.grab_timer = *timer;
    fp.x1A50 = axes[0]; fp.x1A51 = axes[1];
    fp.x2224_b6 = shake[0]; fp.x2224_b5 = shake[1];
    fp.x1A52 = shake[2]; fp.x1A53 = shake[3];
    common.x308 = threshold;
    p_ftCommonData = &common;
    int result = ftCommon_GrabMash(&fp, penalty);
    *timer = fp.grab_timer;
    axes[0] = fp.x1A50; axes[1] = fp.x1A51;
    shake[0] = fp.x2224_b6; shake[1] = fp.x2224_b5;
    shake[2] = fp.x1A52; shake[3] = fp.x1A53;
    return result;
}
