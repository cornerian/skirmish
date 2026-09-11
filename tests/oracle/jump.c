/* Host adapter for the ground jump direction test: ftCo_Jump_Enter
 * (ftCo_Jump.c:153-165). ftCommon_8007D5D4 (pre-jump cleanup) and
 * ftCo_800CB110 (jump velocity/sound, x2227_b0's late set) are unconditional
 * no-ops; only the motion the direction test selects is observable here.
 * Fighter_ChangeMotionState captures the requested motion id, matching the
 * pattern in tests/oracle/landing.c.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct Fighter_GObj Fighter_GObj;
typedef int FtMotionId;
typedef int MotionFlags;

typedef struct {
    float x;
    float y;
} Vec2;

typedef struct {
    float x78;
} ftCommonData;

typedef struct Fighter {
    struct {
        Vec2 lstick[1];
    } input;
    float facing_dir;
    int x2227_b0;
} Fighter;
struct Fighter_GObj {
    Fighter *user_data;
};

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define Ft_MF_None 0

/* Sentinel motion ids distinguishing JumpF/JumpB in this harness, matching
 * the real ftCo_MS_JumpF/ftCo_MS_JumpB values (`forward.h:314-315`); not
 * claimed to be authoritative beyond that coincidence. */
enum {
    ftCo_MS_JumpF = 25,
    ftCo_MS_JumpB = 26,
};

static _Thread_local ftCommonData *p_ftCommonData;
static _Thread_local int captured_motion;

static void ftCommon_8007D5D4(Fighter *fp)
{
    (void) fp;
}
static void ftCo_800CB110(Fighter_GObj *gobj, bool arg1, float jump_mul)
{
    (void) gobj;
    (void) arg1;
    (void) jump_mul;
}
static void Fighter_ChangeMotionState(Fighter_GObj *gobj, FtMotionId msid,
                                      MotionFlags flags, float start,
                                      float rate, float blend, void *callback)
{
    (void) gobj;
    (void) flags;
    (void) start;
    (void) rate;
    (void) blend;
    (void) callback;
    captured_motion = msid;
}

#include "jump_original.inc"

/* Runs ftCo_Jump_Enter with the given stick X, facing and x78 threshold and
 * returns the captured motion id (ftCo_MS_JumpF or ftCo_MS_JumpB above). */
int oracle_jump_enter(float stick_x, float facing, float x78)
{
    ftCommonData common = { .x78 = x78 };
    p_ftCommonData = &common;
    captured_motion = -1;
    Fighter fighter = {
        .input = { .lstick = { { stick_x, 0.0F } } },
        .facing_dir = facing,
    };
    Fighter_GObj gobj = { &fighter };
    ftCo_Jump_Enter(&gobj);
    return captured_motion;
}
