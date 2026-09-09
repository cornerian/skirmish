/* Whole selected upstream bodies, unchanged. Layout is a minimal host adapter.
 * All temporary common-data state is thread-local. PSVECCrossProduct's z
 * component uses scalar arithmetic; no paired-single/FMA equivalence claimed. */
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
typedef uint32_t u32;
typedef struct { float x, y, z; } Vec3;
typedef struct { float x, y; } Vec2;
typedef struct {
    float x144_radians, x148, x14C, x150;
    u32 unk_kb_angle_min, unk_kb_angle_max;
    int x7F0, xFC;
    float x1A8;
} ftCommonData;
typedef struct {
    struct { int x1848_kb_angle, x18ac_time_since_hit; } dmg;
    int ground_or_air;
    struct { struct { struct { uint8_t x1A, x1B; } damage; } co; } mv;
    Vec3 x8c_kb_vel;
    struct { Vec2 lstick[1]; } input;
} Fighter;
static _Thread_local ftCommonData* p_ftCommonData;
#define GA_Air 1
#define MTXDegToRad(a) ((a) * 0.01745329252f)
#define ABS(a) (((a) < 0) ? -(a) : (a))
static void PSVECCrossProduct(const Vec3* a, const Vec3* b, Vec3* out) {
    out->x = a->y * b->z - a->z * b->y;
    out->y = a->z * b->x - a->x * b->z;
    out->z = a->x * b->y - a->y * b->x;
}
#include "damage_angle_range_original.inc"
#include "damage_core_original.inc"

float oracle_damage_angle(int32_t angle, float knockback, int airborne,
                          const float* rules, const uint32_t* bounds,
                          int32_t timer, uint8_t* flags) {
    static _Thread_local ftCommonData common;
    common = (ftCommonData){ .x144_radians=rules[0], .x148=rules[1],
        .x14C=rules[2], .x150=rules[3], .unk_kb_angle_min=bounds[0],
        .unk_kb_angle_max=bounds[1], .x7F0=timer };
    p_ftCommonData = &common;
    Fighter fighter = { .dmg.x1848_kb_angle=angle, .ground_or_air=airborne,
        .mv.co.damage={flags[0], flags[1]} };
    float result = ftCo_Damage_CalcAngle(&fighter, knockback);
    flags[0] = fighter.mv.co.damage.x1A;
    flags[1] = fighter.mv.co.damage.x1B;
    return result;
}

void oracle_damage_merge(const float* values, int32_t since_hit,
                         int32_t window, float* output) {
    static _Thread_local ftCommonData common;
    common = (ftCommonData){ .xFC=window };
    p_ftCommonData = &common;
    Fighter fighter = { .dmg.x18ac_time_since_hit=since_hit,
        .x8c_kb_vel={values[0],values[1],0} };
    ftCo_Damage_CalcVel(&fighter, values[2], values[3]);
    output[0] = fighter.x8c_kb_vel.x;
    output[1] = fighter.x8c_kb_vel.y;
}

void oracle_damage_di(const float* values, float max_degrees, float* output) {
    static _Thread_local ftCommonData common;
    common = (ftCommonData){ .x1A8=max_degrees };
    p_ftCommonData = &common;
    Fighter fighter = { .x8c_kb_vel={values[0],values[1],0},
        .input.lstick={{values[2],values[3]}} };
    ftCo_8008E5A4(&fighter);
    output[0] = fighter.x8c_kb_vel.x;
    output[1] = fighter.x8c_kb_vel.y;
}
