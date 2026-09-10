/* Host adapter for the immediate shield-platform-drop predicate. */
#include <stdbool.h>
#include <stdint.h>

typedef uint8_t u8;
typedef struct { float x, y, z; } Vec3;
typedef struct { int platform; } CollData;
typedef struct {
    uint32_t held_buttons[1];
    Vec3 lstick[1];
} FighterInput;
typedef struct Fighter {
    FighterInput input;
    u8 x671_timer_lstick_tilt_y;
    CollData coll_data;
} Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;
typedef struct { float x464, x468; } ftCommonData;

#define HSD_PAD_LR 0x60
static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local int entered;
static bool mpColl_IsOnPlatform(CollData* coll) { return coll->platform != 0; }
static void ftCo_8009A228(Fighter_GObj* gobj) { (void) gobj; entered = 1; }

#include "pass_original.inc"

int oracle_shield_drop_request(int shield_held, float stick_y, uint8_t tilt_age,
                               float threshold, uint8_t window,
                               int on_platform, int* did_enter)
{
    common = (ftCommonData) { threshold, window };
    p_ftCommonData = &common;
    Fighter fighter = {
        .input = {
            .held_buttons = { shield_held ? HSD_PAD_LR : 0 },
            .lstick = {{ 0, stick_y, 0 }},
        },
        .x671_timer_lstick_tilt_y = tilt_age,
        .coll_data = { on_platform },
    };
    Fighter_GObj gobj = { &fighter };
    entered = 0;
    int result = ftCo_8009A080(&gobj);
    *did_enter = entered;
    return result;
}
