/* Host adapter for the complete ftCo_8009AAFC ledge stick-region callback. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct Fighter {
    float facing_dir;
    int x2064_ledgeCooldown;
    struct {
        float x1830_percent;
    } dmg;
    bool x221D_b7;
    bool x221D_b5;
    struct {
        struct {
            struct {
                bool x8;
            } cliff;
        } co;
    } mv;
} Fighter;
typedef struct Fighter_GObj {
    Fighter* user_data;
} Fighter_GObj;
typedef struct ftCommonData {
    float x20_radians;
    float x488;
    int ledge_cooldown;
} ftCommonData;
typedef int FtMotionId;

enum {
    ftCo_MS_CliffClimbQuick = 10,
    ftCo_MS_CliffClimbSlow = 11,
};
#define Ft_MF_None 0

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local int outcome;
static _Thread_local int selected_motion;

static void ftCo_8009AB9C(Fighter_GObj* gobj);
void ftCo_Fall_Enter(Fighter_GObj* gobj);
static void Fighter_ChangeMotionState(Fighter_GObj* gobj, int motion, int flags,
                                      float frame, float rate, float blend,
                                      void* callback) {
    (void) gobj;
    (void) flags;
    (void) frame;
    (void) rate;
    (void) blend;
    (void) callback;
    outcome = 1;
    selected_motion = motion;
}
static void ftAnim_8006EBA4(Fighter_GObj* gobj) { (void) gobj; }
static void ftCommon_8007E2F4(Fighter* fighter, int mask) {
    (void) fighter;
    (void) mask;
}
static void ftCo_CliffCatch_Phys(Fighter_GObj* gobj) { (void) gobj; }

#include "ledge_option_original.inc"

void ftCo_Fall_Enter(Fighter_GObj* gobj) {
    (void) gobj;
    outcome = 2;
}

uint64_t oracle_ledge_option(int main_stick, int input_ready, float stick_x,
                             float angle, float facing, float angle_threshold,
                             int initial_cooldown, int drop_cooldown) {
    Fighter fp = { 0 };
    Fighter_GObj gobj = { &fp };
    fp.facing_dir = facing;
    fp.x2064_ledgeCooldown = initial_cooldown;
    fp.mv.co.cliff.x8 = input_ready != 0;
    common.x20_radians = angle_threshold;
    common.ledge_cooldown = drop_cooldown;
    p_ftCommonData = &common;
    outcome = 0;
    bool changed = ftCo_8009AAFC(&gobj, main_stick != 0, stick_x, angle);
    uint32_t result = changed ? (uint32_t) outcome : 0;
    return ((uint64_t) (uint32_t) fp.x2064_ledgeCooldown << 32) | result;
}

int oracle_ledge_slow_variant(float percent, float threshold) {
    Fighter fp = { 0 };
    Fighter_GObj gobj = { &fp };
    fp.dmg.x1830_percent = percent;
    common.x488 = threshold;
    p_ftCommonData = &common;
    selected_motion = -1;
    ftCo_8009AB9C(&gobj);
    return selected_motion == ftCo_MS_CliffClimbSlow;
}
