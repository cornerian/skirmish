/* Whole selected upstream bodies, unchanged. Layout is a minimal host adapter.
 * All temporary common-data state is thread-local. PSVECCrossProduct's z
 * component uses scalar arithmetic; no paired-single/FMA equivalence claimed. */
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <assert.h>
typedef uint32_t u32;
typedef struct { float x, y, z; } Vec3;
typedef struct { float x, y; } Vec2;
typedef struct {
    float x144_radians, x148, x14C, x150;
    u32 unk_kb_angle_min, unk_kb_angle_max;
    int x7F0, xFC;
    float x1A8, x1AC, x130, x4BC;
    float sdi_min_stick_mag, sdi_pos_scale;
    int sdi_stick_window;
    float kb_squat_mul, kb_ice_mul, kb_smashcharge_mul, metal_armor, kb_min;
} ftCommonData;
typedef struct {
    struct { int x1848_kb_angle, x18ac_time_since_hit;
        float kb_applied, armor0, armor1; } dmg;
    int ground_or_air;
    struct { struct { struct { uint8_t x1A, x1B; bool x4; } damage; } co; } mv;
    Vec3 x8c_kb_vel;
    struct { Vec2 lstick[1], cstick[1]; uint32_t held_buttons[1]; } input;
    Vec3 cur_pos, x34_scale;
    bool allow_sdi, is_metal;
    uint8_t x670_timer_lstick_tilt_x, x671_timer_lstick_tilt_y;
    int player_id, x221F_b4, motion_id;
    struct { int state; } smash_attrs;
} Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;
#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define VEC2_SQ_LEN(v) ((v).x * (v).x + (v).y * (v).y)
#define SQ(v) ((v) * (v))
#define HSD_PAD_LR 0x60
enum { ftCo_MS_Squat=1, ftCo_MS_SquatWait, ftCo_MS_DamageIce,
       SmashState_Charging };
static const struct { float x0; } scale_rules = {0};
#define Fighter_804D6524 (&scale_rules)
static _Thread_local unsigned displacement_calls;
static void pl_800401F0(int player, int follower, float x, float y) {
    (void)player; (void)follower; (void)x; (void)y; displacement_calls++;
}
/* These branches are explicitly excluded from the bounded wrappers. */
static bool ftCo_800DF608(Fighter* fp) { (void)fp; return false; }
static void ftColl_8007B7A4(Fighter_GObj* fp, float value) {
    (void)fp; (void)value; assert(!"excluded collision callback");
}
static float ftCo_CalcYScaledKnockback(float kb, float scale, float coefficient) {
    (void)kb; (void)scale; (void)coefficient;
    assert(!"excluded scale modifier"); return 0;
}
static _Thread_local ftCommonData* p_ftCommonData;
#define GA_Air 1
#define MTXDegToRad(a) ((a) * 0.01745329252f)
#define ABS(a) (((a) < 0) ? -(a) : (a))
static void PSVECCrossProduct(const Vec3* a, const Vec3* b, Vec3* out) {
    out->x = a->y * b->z - a->z * b->y;
    out->y = a->z * b->x - a->x * b->z;
    out->z = a->x * b->y - a->y * b->x;
}
#include "damage_core_original.inc"

/* `ftCo_Damage_CalcAngle` (its own `oracle_damage_angle_fma`) moved to
 * `tests/oracle/damage_calc_angle.c`, compiled with FMA contraction so its
 * `x148 * ratio + 1` matches the real Gekko `fmadds`; see that file's own
 * comment and `docs/math.md`. It was the only one of this file's functions
 * that used `ftColl_8007AC68`, so `damage_angle_range_original.inc` is no
 * longer included here either. */

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

float oracle_damage_armor(float kb, const float* armor, float minimum) {
    static _Thread_local ftCommonData common;
    common = (ftCommonData){ .kb_min=minimum };
    p_ftCommonData = &common;
    Fighter fighter = { .dmg={.kb_applied=kb,.armor0=armor[0],.armor1=armor[1]},
        .x34_scale={1,1,1} };
    ftCo_Damage_CalcKnockback(&fighter);
    return fighter.dmg.kb_applied;
}

int oracle_damage_displacement(const float* values, uint8_t* timers,
                               int allowed, int window, int exit, float* output) {
    static _Thread_local ftCommonData common;
    common = (ftCommonData){ .sdi_min_stick_mag=values[4], .sdi_pos_scale=values[5],
        .x4BC=values[5], .sdi_stick_window=window };
    p_ftCommonData = &common;
    displacement_calls=0;
    Fighter fighter = { .cur_pos={values[0],values[1],0},
        .input.lstick={{values[2],values[3]}}, .allow_sdi=allowed,
        .x670_timer_lstick_tilt_x=timers[0], .x671_timer_lstick_tilt_y=timers[1] };
    Fighter_GObj object = { &fighter };
    if (exit) ftCo_Damage_OnExitHitlag(&object);
    else ftCo_Damage_OnEveryHitlag(&object);
    output[0]=fighter.cur_pos.x;
    output[1]=fighter.cur_pos.y;
    timers[0]=fighter.x670_timer_lstick_tilt_x;
    timers[1]=fighter.x671_timer_lstick_tilt_y;
    return (int)displacement_calls;
}
