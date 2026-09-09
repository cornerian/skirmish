/* Exact upstream snapshots, included after build.rs removes their includes.
 * Target adaptations: u32/s32/f32 are fixed-width host types; GameCube `long`
 * is 32-bit, so its token is erased only while including the MSL snapshot.
 * -fwrapv gives HSD_Randi the target's signed multiplication wrap behavior.
 * Global state deliberately follows upstream; callers must serialize access.
 */
#include <stdint.h>

typedef uint32_t u32;
typedef int32_t s32;
typedef float f32;
#include "random_hsd_original.inc"

void oracle_hsd_set_seed(uint32_t value)
{
    seed = value;
    seed_ptr = &seed;
}

uint32_t oracle_hsd_get_seed(void) { return *seed_ptr; }

#define long
#define next oracle_msl_seed
#define rand oracle_msl_rand
#define srand oracle_msl_set_seed
#include "random_msl_original.inc"
#undef srand
#undef rand
#undef next
#undef long

uint32_t oracle_msl_get_seed(void) { return oracle_msl_seed; }

/* Both endpoints are valid positions within one live array. The seed starts
 * at index 1; this checks lower inclusion, upper exclusion and reversed ranges.
 * Outputs: selected seed, post-draw seed, draw, external value, fallback value,
 * and whether the selected seed remains external.
 */
void oracle_hsd_forget(uint32_t fallback, uint32_t external, uint32_t low,
                       uint32_t high, uint32_t output[6])
{
    uint32_t memory[3] = {0, external, 0};
    seed = fallback;
    seed_ptr = &memory[1];
    _HSD_RandForgetMemory(&memory[low], &memory[high]);
    output[0] = *seed_ptr;
    output[2] = HSD_Rand();
    output[1] = *seed_ptr;
    output[3] = memory[1];
    output[4] = seed;
    output[5] = seed_ptr == &memory[1];
    seed_ptr = &seed;
}
