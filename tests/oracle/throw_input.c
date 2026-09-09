/* Host adapter for the three complete ftCo_800DD1E4 stick predicates. */
#include <stdbool.h>

typedef struct {
    float x;
    float y;
} Vec2;
typedef struct {
    Vec2 lstick[2];
} FighterInput;
typedef struct Fighter {
    FighterInput input;
} Fighter;
typedef struct {
    float x98;
    float attackhi3_stick_threshold_y;
    float xB0;
} ftCommonData;

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;

#include "throw_input_original.inc"

static Fighter fighter(float current, float previous, int axis) {
    Fighter fp = { 0 };
    if (axis == 0) {
        fp.input.lstick[0].x = current;
        fp.input.lstick[1].x = previous;
    } else {
        fp.input.lstick[0].y = current;
        fp.input.lstick[1].y = previous;
    }
    return fp;
}

int oracle_throw_horizontal(float current, float previous, float threshold) {
    Fighter fp = fighter(current, previous, 0);
    common.x98 = threshold;
    p_ftCommonData = &common;
    return ftCo_800DD1E4_inline1(&fp);
}

int oracle_throw_up(float current, float previous, float threshold) {
    Fighter fp = fighter(current, previous, 1);
    common.attackhi3_stick_threshold_y = threshold;
    p_ftCommonData = &common;
    return ftCo_800DD1E4_inline2(&fp);
}

int oracle_throw_down(float current, float previous, float threshold) {
    Fighter fp = fighter(current, previous, 1);
    common.xB0 = threshold;
    p_ftCommonData = &common;
    return ftCo_800DD1E4_inline3(&fp);
}
