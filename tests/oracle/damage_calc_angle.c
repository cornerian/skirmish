/* `ftCo_Damage_CalcAngle` alone, compiled with FMA contraction enabled (see
 * `build.rs`'s `FMA_CONTRACT_FILES`) so its `x148 * ratio + 1` matches the
 * real Gekko `fmadds` (`tools/ppc_fma_audit.py ftCo_Damage_CalcAngle`;
 * `docs/math.md`) instead of the default two-rounding expansion.
 *
 * This used to be one of the six functions bundled into `damage_core.c`
 * (compiled from the same `combat_hitstun` original snapshot, alongside
 * `ftCo_Damage_CalcVel`, `ftCo_8008E5A4`, `ftCo_Damage_CalcKnockback`,
 * `ftCo_Damage_OnEveryHitlag` and `ftCo_Damage_OnExitHitlag`), which is why
 * it is split out here instead of flipping `damage_core.c`'s own build
 * flags: several of those other five also compile to real fused ops
 * (`ftCo_8008E5A4`, `ftCo_Damage_OnExitHitlag`) that this batch's Rust port
 * does not yet mirror, so contracting that whole translation unit would
 * risk changing their oracle output out from under already-passing
 * comparisons. `ftColl_8007AC68` is duplicated here (under a private name,
 * via the `ftColl_8007AC68` macro below) rather than sharing the copy
 * `damage_core.c` already links, to avoid a duplicate-symbol link error
 * between the two static libraries.
 */
#include <stdint.h>
#include <stdbool.h>

typedef uint32_t u32;
typedef struct {
    float x144_radians, x148, x14C, x150;
    u32 unk_kb_angle_min, unk_kb_angle_max;
    int x7F0;
} ftCommonData;
typedef struct {
    struct { int x1848_kb_angle; } dmg;
    int ground_or_air;
    struct { struct { struct { uint8_t x1A, x1B; } damage; } co; } mv;
} Fighter;
static _Thread_local ftCommonData* p_ftCommonData;
#define GA_Air 1
#define MTXDegToRad(a) ((a) * 0.01745329252f)

#define ftColl_8007AC68 ftColl_8007AC68_calc_angle_fma_local
#include "damage_angle_range_original.inc"
#include "damage_calc_angle_original.inc"
#undef ftColl_8007AC68

float oracle_damage_angle_fma(int32_t angle, float knockback, int airborne,
                              const float* rules, const uint32_t* bounds,
                              int32_t timer, uint8_t* flags) {
    static _Thread_local ftCommonData common;
    common = (ftCommonData){ .x144_radians = rules[0], .x148 = rules[1],
        .x14C = rules[2], .x150 = rules[3], .unk_kb_angle_min = bounds[0],
        .unk_kb_angle_max = bounds[1], .x7F0 = timer };
    p_ftCommonData = &common;
    Fighter fighter = { .dmg.x1848_kb_angle = angle, .ground_or_air = airborne,
        .mv.co.damage = { flags[0], flags[1] } };
    float result = ftCo_Damage_CalcAngle(&fighter, knockback);
    flags[0] = fighter.mv.co.damage.x1A;
    flags[1] = fighter.mv.co.damage.x1B;
    return result;
}
