/* Host adapter for the complete ftCo_800DA824 grab-escape timer formula. */
#include <stdint.h>

typedef int32_t s32;
typedef uint8_t u8;
typedef float f32;
typedef struct Fighter {
    u8 player_id;
    struct {
        f32 x1830_percent;
    } dmg;
} Fighter;
typedef struct {
    f32 x354;
    f32 x358;
    f32 x35C;
    f32 x360;
    f32 x364;
    f32 x368;
} ftCommonData;

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local s32 scripted_standing;
static _Thread_local s32 scripted_handicap;

static s32 Player_80033BB8(int slot) {
    (void) slot;
    return scripted_standing;
}
static s32 Player_GetHandicap(int slot) {
    (void) slot;
    return scripted_handicap;
}

#include "escape_formula_original.inc"

float oracle_escape_formula(float base, float handicap_scale,
                            float handicap_max, float rank_scale,
                            float rank_max, float percent_scale,
                            float percent, int32_t standing,
                            int32_t handicap) {
    common.x354 = base;
    common.x358 = handicap_scale;
    common.x35C = handicap_max;
    common.x360 = rank_scale;
    common.x364 = rank_max;
    common.x368 = percent_scale;
    p_ftCommonData = &common;
    scripted_standing = standing;
    scripted_handicap = handicap;
    Fighter fp = { 0 };
    fp.dmg.x1830_percent = percent;
    return ftCo_800DA824(&fp);
}
