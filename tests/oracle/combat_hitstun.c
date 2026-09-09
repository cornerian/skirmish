/* Initial counter assignment excerpt from ftCo_8008DCE0. The counter remains
 * float storage as in ftCommon/types.h. This is not the full state transition. */
#include <stdint.h>
typedef struct { float x154; } ftCommonData;
static _Thread_local ftCommonData* p_ftCommonData;
#include "combat_hitstun_original.inc"

int32_t oracle_combat_initial_hitstun(float kb_applied, float scale) {
    static _Thread_local ftCommonData common;
    common = (ftCommonData){scale};
    p_ftCommonData = &common;
    struct { struct { struct { struct { float x0; } damage; } co; } mv; } state = {0}, *fp = &state;
    float scaled_kb_154;
    /* Verbatim counter-initialization block from the pinned snapshot. */
    scaled_kb_154 = kb_applied * p_ftCommonData->x154;
    fp->mv.co.damage.x0 = (int) scaled_kb_154;
    if (!fp->mv.co.damage.x0) {
        fp->mv.co.damage.x0 = 1;
    }
    return (int32_t)fp->mv.co.damage.x0;
}
