/* Complete upstream wall-tech jump selector with minimal input/common data. */
#include <stdbool.h>
#include <stdint.h>

typedef uint8_t u8;
typedef struct {
    struct { struct { float x, y; } lstick[1]; } input;
    u8 x67E;
} Fighter;
typedef struct { float x250, tap_jump_threshold; } ftCommonData;
static _Thread_local ftCommonData* p_ftCommonData;

#include "passive_wall_original.inc"

int oracle_wall_tech_jumps(u8 age, float stick_y, float window,
                           float threshold) {
    static _Thread_local ftCommonData common;
    common = (ftCommonData) { window, threshold };
    p_ftCommonData = &common;
    Fighter fighter = { .input.lstick={{0, stick_y}}, .x67E=age };
    return ftCo_800C1E0C(&fighter);
}
