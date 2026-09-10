/* Host adapter for the complete FallSpecial platform-landing predicate. */
#include <stdbool.h>
#include <stdint.h>

typedef struct { float x, y; } Vec2;
typedef struct Fighter { struct { Vec2 lstick[1]; } input; } Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;
typedef struct { float x25C; } ftCommonData;

#define GET_FIGHTER(g) ((g)->user_data)
#define LINE_FLAG_PLATFORM 0x100

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local uint32_t line_flags;
static uint32_t mpLineGetFlags(int line_id) { (void) line_id; return line_flags; }

#include "fall_special_original.inc"

int oracle_fall_special_platform_landing(int line_id, uint32_t flags,
                                         float stick_y, float threshold)
{
    common = (ftCommonData) { .x25C = threshold };
    p_ftCommonData = &common;
    line_flags = flags;
    Fighter fighter = { .input = { .lstick = {{ 0.0f, stick_y }} } };
    Fighter_GObj gobj = { &fighter };
    return ftCo_80096CC8(&gobj, line_id);
}
