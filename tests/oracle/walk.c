/* Host adapter for the pinned `ftCo_Walk_CheckInput_Ottotto` teeter walk
 * predicate. `ftWalkCommon_800DFC70` (the ordinary walk predicate this
 * function ANDs its own extra stick gate with) is stubbed to a scripted
 * answer rather than extracted, matching the task's own scoping: this
 * oracle exists only to pin the extra gate
 * (`stick_x * facing_dir >= teeter_walk_threshold`), not to re-derive the
 * ordinary walk predicate a second time. `ftCo_Walk_Enter` is stubbed and
 * logged. */
#include <stdbool.h>
#include <stdint.h>

typedef float f32;
typedef struct {
    f32 x;
} Vec1;
typedef struct Fighter_GObj Fighter_GObj;

typedef struct Fighter {
    struct {
        Vec1 lstick[1];
    } input;
    float facing_dir;
} Fighter;
struct Fighter_GObj {
    Fighter *user_data;
};
#define GET_FIGHTER(g) ((g)->user_data)

typedef struct {
    float teeter_walk_threshold;
} ftCommonData;

static _Thread_local ftCommonData common;
_Thread_local ftCommonData *p_ftCommonData;
static _Thread_local bool scripted_ordinary_walk;
static _Thread_local bool entered;

static bool ftWalkCommon_800DFC70(Fighter_GObj *gobj)
{
    (void) gobj;
    return scripted_ordinary_walk;
}
static void ftCo_Walk_Enter(Fighter_GObj *gobj, f32 arg8)
{
    (void) gobj;
    (void) arg8;
    entered = true;
}

#include "walk_original.inc"

/* Returns whether `ftCo_Walk_CheckInput_Ottotto` entered Walk. */
int oracle_walk_ottotto_predicate(float stick_x, float facing_dir, float threshold,
                                  int ordinary_walk_ok)
{
    common = (ftCommonData) { .teeter_walk_threshold = threshold };
    p_ftCommonData = &common;
    scripted_ordinary_walk = ordinary_walk_ok != 0;
    entered = false;
    Fighter fighter = { .input = { .lstick = { { stick_x } } }, .facing_dir = facing_dir };
    Fighter_GObj gobj = { &fighter };
    int result = ftCo_Walk_CheckInput_Ottotto(&gobj);
    return result && entered;
}
