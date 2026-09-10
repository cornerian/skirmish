/* Complete upstream floor-tech roll selector with minimal transition stubs. */
#include <stdbool.h>

#define ABS(value) ((value) < 0 ? -(value) : (value))

typedef int FtMotionId;
enum {
    ftCo_MS_PassiveStandF = 200,
    ftCo_MS_PassiveStandB = 201,
};

typedef struct {
    struct { struct { float x, y; } lstick[1]; } input;
    float facing_dir;
} Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;
typedef struct { float x254; } ftCommonData;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local bool tech_eligible;
static _Thread_local FtMotionId selected_motion;

static bool ftCo_800986B0(Fighter_GObj* gobj) {
    (void) gobj;
    return tech_eligible;
}

static void ftCo_800989D4(Fighter_GObj* gobj, FtMotionId msid) {
    (void) gobj;
    selected_motion = msid;
}

#include "passive_stand_original.inc"

int oracle_floor_tech_roll(bool eligible, float stick_x, float facing,
                           float threshold) {
    static _Thread_local Fighter fighter;
    static _Thread_local Fighter_GObj gobj;
    static _Thread_local ftCommonData common;
    fighter = (Fighter) { .input.lstick={{stick_x, 0}}, .facing_dir=facing };
    gobj = (Fighter_GObj) { &fighter };
    common = (ftCommonData) { threshold };
    p_ftCommonData = &common;
    tech_eligible = eligible;
    selected_motion = -1;
    if (!ftCo_80098928(&gobj)) {
        return 0;
    }
    return selected_motion == ftCo_MS_PassiveStandF ? 1 : 2;
}
