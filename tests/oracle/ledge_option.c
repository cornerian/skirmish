/* Host adapter for the complete ftCo_8009AAFC ledge stick-region callback. */
#include <stdbool.h>
#include <stdint.h>

typedef struct Fighter {
    float facing_dir;
    int x2064_ledgeCooldown;
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
    int ledge_cooldown;
} ftCommonData;

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local int outcome;

static void ftCo_8009AB9C(Fighter_GObj* gobj);
void ftCo_Fall_Enter(Fighter_GObj* gobj);

#include "ledge_option_original.inc"

static void ftCo_8009AB9C(Fighter_GObj* gobj) {
    (void) gobj;
    outcome = 1;
}

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
