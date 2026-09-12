/* Exact ftcoll.c helper/function bodies and KNOCKBACK macro, on compact host
 * structs. The array adapter supplies every accessed field and coefficient. */
#include <stdint.h>
typedef int32_t s32;
typedef uint32_t u32;
typedef struct {
    float xF4, xF8, x108, x110, x114, x118, x11C, x120;
    int x6D4, x6D8[1];
} ftCommonData;
typedef struct {
    struct { float x1830_percent, x1838_percentTemp; } dmg;
    int x2225_b7, x2224_b2;
} Fighter;
typedef struct { u32 x24, x28, x2C; } HitCapsule;
static _Thread_local ftCommonData* p_ftCommonData;

/* `ftColl_80079AB0`'s three `a * b + c`-shaped subexpressions each match a
 * real Gekko `fmadds` (`tools/ppc_fma_audit.py ftColl_80079AB0`;
 * `docs/math.md`), and `fighter::combat::knockback` uses `f32::mul_add` at
 * each. This oracle macro is deliberately left exactly as `ftcoll.c` itself
 * pins it (still compiled `-ffp-contract=off`, like every adapter other
 * than `build.rs`'s `FMA_CONTRACT_FILES`): an earlier attempt to compile
 * this one with `-ffp-contract=fast -mfma` -- restructured into the
 * per-statement form the two adapters in `FMA_CONTRACT_FILES` use -- was
 * reverted after disassembling the result showed GCC choosing a different
 * multiply/add pairing than the retail binary for the two-products-summed
 * `inner` term (`x118 * x110 + x114 * (x118 * x28)`-shaped), which is
 * itself pinned inline in `ftColl_80079AB0`'s own body and so isn't this
 * wrapper's to restructure. `docs/math.md` has the concrete evidence; the
 * differential test compares against this uncontracted oracle with a
 * documented few-ULP tolerance instead of bit-for-bit. */
#define KNOCKBACK(defense, attack, arg3, one, ftd, hit, w, inner)             \
    ((defense) *                                                              \
     ((attack) *                                                              \
      ((arg3) * ((0.01F * (hit)->x24 *                                        \
                  ((ftd)->x11C *                                              \
                       (((ftd)->xF8 - (((w) * (ftd)->xF8) / ((one) + (w)))) * \
                        (inner)) +                                            \
                   (ftd)->x120)) +                                            \
                 (hit)->x2C))))

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#include "combat_knockback_original.inc"
#pragma GCC diagnostic pop

float oracle_combat_knockback(const float rules[8], const u32 hit[3],
    const float damage[2], const float modifiers[4], u32 attack_damage,
    s32 count_override, int override_mode) {
    static _Thread_local ftCommonData common;
    common = (ftCommonData){rules[0], rules[1], rules[2], rules[3], rules[4],
        rules[5], rules[6], rules[7], count_override, {count_override}};
    p_ftCommonData = &common;
    Fighter fp = {{damage[0], damage[1]}, override_mode != 0, override_mode == 2};
    HitCapsule capsule = {hit[0], hit[1], hit[2]};
    return ftColl_80079AB0(&fp, &capsule, attack_damage,
        modifiers[0], modifiers[1], modifiers[2], modifiers[3]);
}
