/* Host oracle for ftCo_8008DCE0's pointer-free motion selection. */
#include <stdint.h>
typedef int32_t s32;
typedef int32_t enum_t;
typedef struct { float x, y, z; } Vec3;
typedef struct {
    float x154, x158, x15C, x160, x100;
} ftCommonData;

/* BEGIN VERBATIM MOTION TABLE */
int ftCo_803C5520[2][4][3] = {
    {
        { 81, 78, 75 },
        { 82, 79, 76 },
        { 83, 80, 77 },
        { 89, 88, 87 },
    },
    {
        { 84, 84, 84 },
        { 85, 85, 85 },
        { 86, 86, 86 },
        { 89, 88, 87 },
    },
};
/* END VERBATIM MOTION TABLE */

int32_t oracle_damage_motion(float knockback, float scale,
                             const float* thresholds, int airborne, int height)
{
    ftCommonData common = {
        .x154 = scale,
        .x158 = thresholds[0],
        .x15C = thresholds[1],
        .x160 = thresholds[2],
        .x100 = 0,
    };
    ftCommonData* p_ftCommonData = &common;
    float kb_applied = knockback;
    float scaled_kb_154;
    struct { float v; } scaled_kb;
    Vec3 pos;
    s32 kb_level_base;
    enum_t kb_level;
    int arg1 = -1;
    (void) pos;
/* BEGIN VERBATIM KNOCKBACK SCALE */
    scaled_kb_154 = kb_applied * p_ftCommonData->x154;
/* END VERBATIM KNOCKBACK SCALE */
/* BEGIN VERBATIM LEVEL SELECTION */
    {
        Vec3* normal;
        if (scaled_kb_154 < p_ftCommonData->x158) {
            kb_level_base = 0;
            goto block_9;
        } else {
            if (!(scaled_kb_154 < p_ftCommonData->x15C)) {
                goto block_6;
            }
            kb_level_base = 1;
            goto block_9;
        }
    block_6:
        if (!(scaled_kb_154 < p_ftCommonData->x160)) {
            goto block_8;
        }
        kb_level_base = 2;
        goto block_9;
    block_8:
        kb_level_base = 3;
    block_9:
        kb_level = kb_level_base;
        if (arg1 == -1) {
            goto block_11;
        }
        kb_level = 3;
    block_11:
        scaled_kb.v = kb_applied * p_ftCommonData->x100;
/* END VERBATIM LEVEL SELECTION */
    }
    return ftCo_803C5520[airborne][kb_level][height];
}
