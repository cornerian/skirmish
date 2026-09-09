/* Host adapter for the complete original buffered-tech eligibility function. */
#include <stdbool.h>
#include <stdint.h>
typedef uint8_t u8;
typedef struct Fighter Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;
struct Fighter { u8 x680, x684; };
typedef struct { int x1C; float x250; } ftCommonData;
static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local bool input_locked;
#define GET_FIGHTER(gobj) ((gobj)->user_data)
static bool ftCo_800C5240(Fighter_GObj* gobj) { (void) gobj; return input_locked; }
#include "down_attack_original.inc"

int oracle_damage_floor_tech(uint32_t locked, uint8_t age, uint8_t previous,
                             float window, int32_t repeat_lockout) {
    Fighter fighter = { age, previous };
    Fighter_GObj gobj = { &fighter };
    common = (ftCommonData) { .x1C = repeat_lockout, .x250 = window };
    p_ftCommonData = &common;
    input_locked = locked != 0;
    return ftCo_800986B0(&gobj);
}
