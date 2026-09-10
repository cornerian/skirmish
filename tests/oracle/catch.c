/* Host adapter for the complete shield-grab input dispatchers. The item
 * pickup path and the Link/Samus tether gates are fixed to their ordinary
 * results; the transition callback only captures the selected motion. */
#include <stdbool.h>
#include <stdint.h>

typedef int FtMotionId;
typedef struct Fighter {
    struct { uint32_t held_buttons[1]; uint32_t pressed_buttons; } input;
    struct { struct { struct { float x24; } guard; } co; } mv;
} Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;

#define GET_FIGHTER(g) ((g)->user_data)
#define HSD_PAD_A 0x100
#define HSD_PAD_LR 0x60
enum { ftCo_MS_Catch = 212, ftCo_MS_CatchDash = 214 };

static _Thread_local int entered_motion;
static bool ftCo_800951D0(Fighter_GObj* gobj) { (void) gobj; return false; }
static bool fn_800D8E94(Fighter_GObj* gobj) { (void) gobj; return true; }
static bool fn_800D952C(Fighter_GObj* gobj) { (void) gobj; return true; }
static void ftCo_800D8C54(Fighter_GObj* gobj, FtMotionId msid)
{
    (void) gobj;
    entered_motion = msid;
}

#include "catch_original.inc"

/* Complete ftCo_Catch_CheckInput over arbitrary logical button words. */
int oracle_shield_grab(uint32_t held, uint32_t pressed, int* motion)
{
    Fighter fighter = { .input = { .held_buttons = { held }, .pressed_buttons = pressed } };
    Fighter_GObj gobj = { &fighter };
    entered_motion = 0;
    int result = ftCo_Catch_CheckInput(&gobj);
    *motion = entered_motion;
    return result;
}

/* Complete ftCo_800D8B9C over an arbitrary binary32 buffer. */
int oracle_dash_shield_grab(uint32_t pressed, float buffer, float* buffer_after,
                            int* motion)
{
    Fighter fighter = { .input = { .pressed_buttons = pressed },
                        .mv = { .co = { .guard = { buffer } } } };
    Fighter_GObj gobj = { &fighter };
    entered_motion = 0;
    int result = ftCo_800D8B9C(&gobj);
    *buffer_after = fighter.mv.co.guard.x24;
    *motion = entered_motion;
    return result;
}
