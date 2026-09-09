/* Exact ftCommon_CalcHitlag body with its three accessed common coefficients.
 * Squat's enum value is 39 in ftCommon/forward.h; SquatWait follows it. */
#include <stdint.h>
typedef int32_t FtMotionId;
#define ftCo_MS_Squat 39
typedef struct { float x198, x19C, x1A0; } ftCommonData;
static _Thread_local ftCommonData* p_ftCommonData;

#include "combat_hitlag_original.inc"

float oracle_combat_hitlag(int32_t damage, int crouching, float multiplier, const float rules[3]) {
    static _Thread_local ftCommonData common;
    common = (ftCommonData){rules[0], rules[1], rules[2]};
    p_ftCommonData = &common;
    return ftCommon_CalcHitlag(damage, crouching ? ftCo_MS_Squat : -1, multiplier);
}
