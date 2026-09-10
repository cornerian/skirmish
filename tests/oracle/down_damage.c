/* Host adapter for the complete original prone low-damage transition. */
#include <stdbool.h>

typedef int FtMotionId;
enum {
    ftCo_MS_DownBoundU = 183,
    ftCo_MS_DownWaitU = 184,
    ftCo_MS_DownDamageU = 185,
    ftCo_MS_DownBoundD = 191,
    ftCo_MS_DownWaitD = 192,
    ftCo_MS_DownDamageD = 193,
};

typedef struct Fighter Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;
typedef struct { float x1838_percentTemp; } DamageState;
struct Fighter {
    FtMotionId motion_id;
    DamageState dmg;
    bool x2224_b2;
    float facing_dir;
};
typedef struct { int x428; } ftCommonData;

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local FtMotionId selected_motion;

static void ftCo_8008DCE0(Fighter_GObj* gobj, int motion, float facing)
{
    (void) gobj;
    (void) facing;
    selected_motion = motion;
}

static void ftCommon_8007E2F4(Fighter* fighter, int enabled)
{
    (void) fighter;
    (void) enabled;
}

#include "down_damage_original.inc"

int oracle_down_damage(bool prone_action, bool face_up_wait, bool forced,
                       float pending_damage, int threshold)
{
    Fighter fighter = {
        .motion_id = !prone_action ? 0
                                  : face_up_wait ? ftCo_MS_DownWaitU
                                                 : ftCo_MS_DownBoundU,
        .dmg = { .x1838_percentTemp = pending_damage },
        .x2224_b2 = forced,
        .facing_dir = 1.0F,
    };
    Fighter_GObj gobj = { &fighter };
    common = (ftCommonData) { .x428 = threshold };
    p_ftCommonData = &common;
    selected_motion = -1;
    if (!ftCo_8009F0F0(&gobj)) {
        return 0;
    }
    return selected_motion == ftCo_MS_DownDamageU ? 1 : 2;
}
