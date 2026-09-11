/* Host adapter for the complete smash charge state machine from ft_0DF0.c:
 * arming, clearing, the released damage conversion, the per-frame charge
 * tick and the input phase. Animation-rate changes are captured; the shake
 * table, sound and graphics callbacks are inert. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef uint32_t u32;
typedef float f32;
typedef struct { float x, y; } Vec2;
typedef enum SmashState {
    SmashState_None,
    SmashState_PreCharge,
    SmashState_Charging,
    SmashState_Release,
} SmashState;
typedef struct SmashAttr {
    SmashState state;
    float x2118_frames;
    float x211C_holdFrame;
    float x2120_damageMul;
    float x2124_frameSpeedMul;
    u32 x2128;
    int x212C;
    int x212D;
    int x2130_sfxBool;
} SmashAttr;
typedef struct Fighter {
    struct { Vec2 lstick[2]; Vec2 cstick[2]; uint32_t held_buttons[1]; } input;
    float frame_speed_mul;
    SmashAttr smash_attrs;
} Fighter;
typedef struct Fighter_GObj { Fighter* user_data; } Fighter_GObj;
typedef struct { float x7C8; } ftCommonData;
struct Fighter_ShakeTable_t { Vec2* x0; int x4; };

#define GET_FIGHTER(g) ((g)->user_data)
#define HSD_PAD_A 0x100
#define PAD_STACK(n)

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
/* Read-only, so a plain static table is safe across test threads. */
static Vec2 shake_offsets[4];
static struct Fighter_ShakeTable_t shake_table = { shake_offsets, 4 };
#define Fighter_SmashChargeShakeTable (&shake_table)
static _Thread_local int rate_sets;
static _Thread_local float last_rate;

static void ftAnim_SetAnimRate(Fighter_GObj* gobj, float rate)
{
    (void) gobj;
    rate_sets++;
    last_rate = rate;
}
static void ftCo_800C0200(Fighter* fp, u32 id) { (void) fp; (void) id; }
static void ftCo_800BFFD0(Fighter* fp, u32 id, int arg) { (void) fp; (void) id; (void) arg; }
static void ftCommon_8007ECD4(Fighter* fp, int arg) { (void) fp; (void) arg; }
static void ftCommon_8007EBAC(Fighter* fp, u32 a, u32 b) { (void) fp; (void) a; (void) b; }
static void ft_PlaySFX(Fighter* fp, int a, int b, int c) { (void) fp; (void) a; (void) b; (void) c; }

#include "smash_charge_original.inc"

/* Arm with ftCo_800DEE84, then per frame run the charge tick and the input
 * phase with the held bit from `held` (one bit per frame, bit 0 first) and
 * record the state after each frame. Returns the released damage
 * conversion of `damage`; `frames_out` receives the charged frame count. */
float oracle_charge_lifecycle(float hold, float mul, float rate, uint32_t held, int frames,
                              float damage, int* states, float* frames_out, int* rate_sets_out,
                              float* last_rate_out)
{
    common = (ftCommonData) { .x7C8 = 1e9f };
    p_ftCommonData = &common;
    Fighter fighter = { .frame_speed_mul = rate };
    Fighter_GObj gobj = { &fighter };
    rate_sets = 0;
    last_rate = -1;
    ftCo_800DEE84(&gobj, 0x7B, hold, mul);
    for (int frame = 0; frame < frames; frame++) {
        ftCo_800DEF38(&gobj);
        fighter.input.held_buttons[0] = (held >> frame) & 1 ? HSD_PAD_A : 0;
        ftCo_800DF0D0(&gobj);
        states[frame] = (int) fighter.smash_attrs.state;
    }
    *frames_out = fighter.smash_attrs.x2118_frames;
    *rate_sets_out = rate_sets;
    *last_rate_out = last_rate;
    return ftCo_800DEEB8(&fighter, damage);
}

int oracle_charge_input(int state, uint32_t held, float rate, float* rate_out)
{
    Fighter fighter = { .input = { .held_buttons = { held } }, .frame_speed_mul = rate,
                        .smash_attrs = { .state = (SmashState) state, .x2124_frameSpeedMul = rate } };
    Fighter_GObj gobj = { &fighter };
    rate_sets = 0;
    last_rate = -1;
    ftCo_800DF0D0(&gobj);
    *rate_out = last_rate;
    return (int) fighter.smash_attrs.state;
}

int oracle_charge_tick(int state, float frames, float hold, float* frames_out)
{
    common = (ftCommonData) { .x7C8 = 1e9f };
    p_ftCommonData = &common;
    Fighter fighter = { .smash_attrs = { .state = (SmashState) state, .x2118_frames = frames,
                                         .x211C_holdFrame = hold, .x212D = 4 } };
    Fighter_GObj gobj = { &fighter };
    ftCo_800DEF38(&gobj);
    *frames_out = fighter.smash_attrs.x2118_frames;
    return (int) fighter.smash_attrs.state;
}

float oracle_charge_damage(int state, float damage, float frames, float hold, float mul)
{
    Fighter fighter = { .smash_attrs = { .state = (SmashState) state, .x2118_frames = frames,
                                         .x211C_holdFrame = hold, .x2120_damageMul = mul } };
    return ftCo_800DEEB8(&fighter, damage);
}

int oracle_charge_clear(void)
{
    Fighter fighter = { .smash_attrs = { .state = SmashState_Charging, .x2118_frames = 3 } };
    Fighter_GObj gobj = { &fighter };
    ftCo_800DEEA8(&gobj);
    return (int) fighter.smash_attrs.state;
}
