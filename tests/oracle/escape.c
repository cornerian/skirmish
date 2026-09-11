/* Host adapter for the complete grounded shield-evasion input dispatchers.
 * The transition callbacks only capture the selected motion and flag; the
 * Samus/Yoshi entry branches, throw-flag reset and statistics are not part of
 * the compared predicates. */
#include <stdbool.h>
#include <stdint.h>

typedef uint8_t u8;
typedef uint32_t u32;
typedef int FtMotionId;
typedef struct { float x, y; } Vec2;
typedef struct { Vec2 lstick[1]; Vec2 cstick[1]; u32 held_buttons[1]; } FighterInput;
typedef struct Fighter {
    FighterInput input;
    float facing_dir;
    u8 x670_timer_lstick_tilt_x;
    u8 x671_timer_lstick_tilt_y;
} Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;
typedef struct { float x314; int x318; float x31C; int x320; int x324; } ftCommonData;

/* Runtime/platform.h definition, including signed-zero behavior. */
#define ABS(x) ((x) < 0 ? -(x) : (x))
enum { ftCo_MS_EscapeF = 233, ftCo_MS_EscapeB = 234, ftCo_MS_EscapeN = 235 };
/* controller.h: HSD_PAD_L = 1 << 6, HSD_PAD_R = 1 << 5. */
#define HSD_PAD_LR ((1 << 6) | (1 << 5))

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local int entered_motion;
static _Thread_local int entered_flag;

static void ftCo_800992A8(Fighter_GObj* gobj, FtMotionId msid, bool arg2)
{
    (void) gobj;
    entered_motion = msid;
    entered_flag = arg2;
}
static void ftCo_80099894(Fighter_GObj* gobj)
{
    (void) gobj;
    entered_motion = ftCo_MS_EscapeN;
}

#include "escape_cstick_original.inc"
#include "escape_original.inc"

int oracle_cstick_roll(float cstick_x, float threshold)
{
    common = (ftCommonData) { .x31C = threshold };
    p_ftCommonData = &common;
    Fighter fighter = { .input = { .cstick = {{ cstick_x, 0 }} } };
    return ftCo_800DF8B0(&fighter);
}

int oracle_cstick_spot_dodge(float cstick_y, float threshold)
{
    common = (ftCommonData) { .x314 = threshold };
    p_ftCommonData = &common;
    Fighter fighter = { .input = { .cstick = {{ 0, cstick_y }} } };
    return ftCo_800DF8E8(&fighter);
}

/* Complete ftCo_8009917C: returns the predicate result and the captured
 * motion (233/234, or 0 when nothing was entered) plus the x324 copy. */
int oracle_escape_roll(float stick_x, uint8_t tilt_x_age, float cstick_x,
                       float facing, float threshold, int window, int flag,
                       int* motion, int* captured_flag)
{
    common = (ftCommonData) { .x31C = threshold, .x320 = window, .x324 = flag };
    p_ftCommonData = &common;
    Fighter fighter = {
        .input = { .lstick = {{ stick_x, 0 }}, .cstick = {{ cstick_x, 0 }} },
        .facing_dir = facing,
        .x670_timer_lstick_tilt_x = tilt_x_age,
    };
    Fighter_GObj gobj = { &fighter };
    entered_motion = 0;
    entered_flag = -1;
    int result = ftCo_8009917C(&gobj);
    *motion = entered_motion;
    *captured_flag = entered_flag;
    return result;
}

/* Complete ftCo_8009980C: returns the predicate result and whether EscapeN
 * was entered. */
int oracle_escape_spot_dodge(float stick_y, uint8_t tilt_y_age, float cstick_y,
                             float threshold, int window, int* motion)
{
    common = (ftCommonData) { .x314 = threshold, .x318 = window };
    p_ftCommonData = &common;
    Fighter fighter = {
        .input = { .lstick = {{ 0, stick_y }}, .cstick = {{ 0, cstick_y }} },
        .x671_timer_lstick_tilt_y = tilt_y_age,
    };
    Fighter_GObj gobj = { &fighter };
    entered_motion = 0;
    int result = ftCo_8009980C(&gobj);
    *motion = entered_motion;
    return result;
}

/* Complete ftCo_80099794 (the Wait/AppealS-chain-only spot dodge: a held
 * logical shoulder AND inlineB0's fresh downward main stick, unlike
 * ftCo_8009980C's inlineB0-OR-C-stick gate above): returns the predicate
 * result and whether EscapeN was entered. `held` scripts
 * `input.held_buttons[0] & HSD_PAD_LR`. */
int oracle_wait_spot_dodge(int held, float stick_y, uint8_t tilt_y_age, float threshold,
                           int window, int* motion)
{
    common = (ftCommonData) { .x314 = threshold, .x318 = window };
    p_ftCommonData = &common;
    Fighter fighter = {
        .input = {
            .lstick = {{ 0, stick_y }},
            .held_buttons = { held ? (uint32_t) HSD_PAD_LR : 0 },
        },
        .x671_timer_lstick_tilt_y = tilt_y_age,
    };
    Fighter_GObj gobj = { &fighter };
    entered_motion = 0;
    int result = ftCo_80099794(&gobj);
    *motion = entered_motion;
    return result;
}
