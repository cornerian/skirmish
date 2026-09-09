/* Host adapter for the complete neutral-special input predicate. */
#include <stdbool.h>
#include <stdint.h>

typedef struct Vec2 {
    float x;
    float y;
} Vec2;

typedef struct Fighter {
    struct {
        Vec2 lstick[1];
        uint32_t pressed_buttons;
    } input;
} Fighter;

typedef struct ftCommonData {
    float x218;
    float x21C;
} ftCommonData;

#define HSD_PAD_B 0x0200
#define ABS(value) ((value) < 0 ? -(value) : (value))

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;

#include "special_input_original.inc"

int oracle_neutral_special_input(uint32_t pressed_buttons, float stick_x,
                                 float stick_y, float horizontal_threshold,
                                 float vertical_threshold) {
    Fighter fp = { 0 };
    fp.input.pressed_buttons = pressed_buttons;
    fp.input.lstick[0] = (Vec2) { stick_x, stick_y };
    common.x218 = horizontal_threshold;
    common.x21C = vertical_threshold;
    p_ftCommonData = &common;
    return ftCo_800D67C4(&fp);
}
