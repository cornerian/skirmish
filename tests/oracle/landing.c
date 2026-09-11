/* Host adapter for the ordinary Landing entry family and its input chain:
 * ftCo_Landing_Enter, ftCo_Landing_Enter_Basic, ftCo_LandingFallSpecial_Enter_
 * Basic, ftCo_LandingFallSpecial_Enter and ftCo_Landing_IASA. Every callee
 * that is not one of those pinned functions is a simple stub: ftCo_800C5240
 * (the hammer check) always answers false so ftCo_HammerLanding_Enter is
 * never reached at runtime but still needs to compile; ftCommon_8007D7FC is a
 * no-op; Fighter_ChangeMotionState captures the requested motion, flags,
 * start and rate; every CheckInput callee in the IASA chain logs a call code
 * and answers from a per-frame `answers` bitmask (see `record_call`/`ANSWER`
 * below, the same pattern as `tests/oracle/dash.c`). The fighter-kind switch
 * in ftCo_Landing_Enter (Mario/Dr. Mario, Peach, Marth/Roy, Game & Watch, Ice
 * Climbers, Kirby, Mewtwo) is reproduced verbatim from the pinned body, so
 * the host `Fighter` exposes the exact union fields it reads and writes; kind
 * stays the default 0, which matches none of those cases, so every branch
 * compiles but never executes.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct Fighter_GObj Fighter_GObj;
typedef Fighter_GObj HSD_GObj;
typedef int FtMotionId;
typedef int MotionFlags;

typedef struct {
    float normal_landing_lag;
} ftCo_DatAttrs;

typedef struct Fighter {
    int kind;
    float x2EC;
    ftCo_DatAttrs co_attrs;
    float cur_anim_frame;
    float frame_speed_mul;
    struct {
        struct {
            struct {
                int allow_interrupt;
            } landing;
        } co;
    } mv;
    /* Character-specific fields ftCo_Landing_Enter's switch reads and
     * writes. kind (0) never selects any of these branches at runtime; they
     * exist only so the verbatim switch body compiles. */
    union {
        struct {
            int x2234_tornadoCharge;
            int x2238_isCapeBoost;
        } mr;
        struct {
            int specialairn_used;
        } pe;
        struct {
            int x222C;
        } ms;
        struct {
            int x2234;
        } gw;
        struct {
            int x224C;
        } pp;
        struct {
            int xCC;
            int xC4;
            int x64;
        } kb;
        struct {
            int x223C_isConfusionBoost;
        } mt;
    } u;
} Fighter;
struct Fighter_GObj {
    Fighter* user_data;
};

#define GET_FIGHTER(g) ((g)->user_data)
#define PAD_STACK(n)
#define RETURN_IF(cond) \
    do {                \
        if (cond)       \
            return;     \
    } while (0)
#define Ft_MF_None 0

/* None of these equal the default fighter kind (0) the oracle harness uses,
 * so ftCo_Landing_Enter's switch always falls to its default case. */
enum {
    FTKIND_MARIO = 1,
    FTKIND_DRMARIO = 2,
    FTKIND_PEACH = 3,
    FTKIND_MARS = 4,
    FTKIND_EMBLEM = 5,
    FTKIND_GAMEWATCH = 6,
    FTKIND_POPO = 7,
    FTKIND_NANA = 8,
    FTKIND_KIRBY = 9,
    FTKIND_MEWTWO = 10,
};

/* Sentinel motion IDs distinguishing the two Landing motions in this
 * harness; they are not claimed to be the authentic FtMotionId values. */
enum {
    ftCo_MS_Landing = 42,
    ftCo_MS_LandingFallSpecial = 43,
};

/* Call-order trace codes for ftCo_Landing_IASA, shared with
 * tests/landing_differential.rs. */
enum {
    CALL_SPECIAL_S = 1,
    CALL_ATTACK100 = 2,
    CALL_800D6824 = 3,
    CALL_800D68C0 = 4,
    CALL_CATCH = 5,
    CALL_ATTACK_S4 = 6,
    CALL_ATTACK_HI4 = 7,
    CALL_ATTACK_LW4 = 8,
    CALL_ATTACK_S3 = 9,
    CALL_ATTACK_HI3 = 10,
    CALL_ATTACK_LW3 = 11,
    CALL_JAB = 12,
    CALL_SHIELD = 13,
    CALL_TAUNT = 14,
    CALL_JUMP = 15,
    CALL_DASH = 16,
    CALL_SQUAT = 17,
    CALL_TURN = 18,
    CALL_WALK = 19,
};

static _Thread_local uint32_t answers;
static _Thread_local int captured_motion;
static _Thread_local int captured_flags;
static _Thread_local float captured_start;
static _Thread_local float captured_rate;
static _Thread_local uint8_t call_log[20];
static _Thread_local uint8_t call_count;
static _Thread_local uint8_t fired_code;

/* Pushes `code` to the trace unconditionally; when `result` is also true,
 * this call is the one that made `ftCo_Landing_IASA` return (first-wins). */
static void record_call(uint8_t code, bool result)
{
    if (call_count < sizeof call_log) {
        call_log[call_count++] = code;
    }
    if (result && fired_code == 0) {
        fired_code = code;
    }
}
#define ANSWER(code) ((answers & (1u << (code))) != 0)

static void reset_captures(uint32_t scripted)
{
    answers = scripted;
    captured_motion = 0;
    captured_flags = 0;
    captured_start = 0.0F;
    captured_rate = 0.0F;
    call_count = 0;
    fired_code = 0;
    for (size_t i = 0; i < sizeof call_log; i++) {
        call_log[i] = 0;
    }
}

static bool ftCo_800C5240(Fighter_GObj* gobj)
{
    (void) gobj;
    return false;
}
static void ftCo_HammerLanding_Enter(Fighter_GObj* gobj) { (void) gobj; }
static void ftCommon_8007D7FC(Fighter* fp) { (void) fp; }
static void ftPe_8011D598(Fighter_GObj* gobj) { (void) gobj; }
static void Fighter_ChangeMotionState(Fighter_GObj* gobj, FtMotionId msid,
                                      MotionFlags flags, float start,
                                      float rate, float blend, void* callback)
{
    (void) gobj;
    (void) blend;
    (void) callback;
    captured_motion = msid;
    captured_flags = flags;
    captured_start = start;
    captured_rate = rate;
}

static bool ftCo_SpecialS_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_SPECIAL_S);
    record_call(CALL_SPECIAL_S, result);
    return result;
}
static bool ftCo_Attack100_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_ATTACK100);
    record_call(CALL_ATTACK100, result);
    return result;
}
static bool ftCo_800D6824(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_800D6824);
    record_call(CALL_800D6824, result);
    return result;
}
static bool ftCo_800D68C0(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_800D68C0);
    record_call(CALL_800D68C0, result);
    return result;
}
static bool ftCo_Catch_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_CATCH);
    record_call(CALL_CATCH, result);
    return result;
}
static bool ftCo_AttackS4_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_ATTACK_S4);
    record_call(CALL_ATTACK_S4, result);
    return result;
}
static bool ftCo_AttackHi4_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_ATTACK_HI4);
    record_call(CALL_ATTACK_HI4, result);
    return result;
}
static bool ftCo_AttackLw4_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_ATTACK_LW4);
    record_call(CALL_ATTACK_LW4, result);
    return result;
}
static bool ftCo_AttackS3_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_ATTACK_S3);
    record_call(CALL_ATTACK_S3, result);
    return result;
}
static bool ftCo_AttackHi3_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_ATTACK_HI3);
    record_call(CALL_ATTACK_HI3, result);
    return result;
}
static bool ftCo_AttackLw3_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_ATTACK_LW3);
    record_call(CALL_ATTACK_LW3, result);
    return result;
}
static bool ftCo_Attack1_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_JAB);
    record_call(CALL_JAB, result);
    return result;
}
static bool ftCo_80091A4C(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_SHIELD);
    record_call(CALL_SHIELD, result);
    return result;
}
static bool ftCo_800DE9D8(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_TAUNT);
    record_call(CALL_TAUNT, result);
    return result;
}
static bool ftCo_Jump_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_JUMP);
    record_call(CALL_JUMP, result);
    return result;
}
static bool ftCo_Dash_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_DASH);
    record_call(CALL_DASH, result);
    return result;
}
static bool ftCo_SquatWait_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_SQUAT);
    record_call(CALL_SQUAT, result);
    return result;
}
static bool ftCo_Turn_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_TURN);
    record_call(CALL_TURN, result);
    return result;
}
static bool ftCo_Walk_CheckInput(Fighter_GObj* gobj)
{
    bool result = ANSWER(CALL_WALK);
    record_call(CALL_WALK, result);
    return result;
}

#include "landing_original.inc"

/* Runs ftCo_Landing_IASA once. `frame` is cur_anim_frame, `rate` is
 * frame_speed_mul, `lag` is co_attrs.normal_landing_lag, `allow` is
 * mv.co.landing.allow_interrupt, `answers` scripts every CheckInput callee
 * (bit N set means call code N answers true). Fills `out_calls` (sized at
 * least 20) with the ordered call trace and `out_count` with how many were
 * logged; returns the call code that made the chain fire (0 = the chain
 * never fired, whether locked out or simply exhausted with every answer
 * false). */
int oracle_landing_iasa(float frame, float rate, float lag, int allow,
                        uint32_t answers_in, uint8_t* out_calls,
                        uint8_t* out_count)
{
    reset_captures(answers_in);
    Fighter fighter = {
        .cur_anim_frame = frame,
        .frame_speed_mul = rate,
        .co_attrs = { .normal_landing_lag = lag },
        .mv = { .co = { .landing = { .allow_interrupt = allow } } },
    };
    Fighter_GObj gobj = { &fighter };
    ftCo_Landing_IASA(&gobj);
    for (uint8_t i = 0; i < call_count && i < 20; i++) {
        out_calls[i] = call_log[i];
    }
    *out_count = call_count < 20 ? call_count : 20;
    return fired_code;
}

/* Drives one of the four entry points. `kind_of_entry`: 0 =
 * ftCo_Landing_Enter_Basic, 1 = ftCo_LandingFallSpecial_Enter_Basic, 2 =
 * ftCo_LandingFallSpecial_Enter(gobj, allow, lag_for_fallspecial). `allow`
 * and `lag_for_fallspecial` are only consulted for kind_of_entry == 2; the
 * `_Basic` entries hardcode their own allow flag and rate (1.0). `x2EC` is
 * fixed at a representative nonzero value (see LANDING_X2EC) so
 * ftCo_LandingFallSpecial_Enter's `(0.1 + x2EC) / lag` rate is exercised
 * with a nonzero numerator, matching the fixed constant the Rust
 * differential test mirrors. Outputs the resulting motion (the sentinel
 * ftCo_MS_Landing/ftCo_MS_LandingFallSpecial above), the captured
 * allow_interrupt and the captured animation rate. */
#define LANDING_X2EC 0.37F

int oracle_landing_enter(int kind_of_entry, int allow, float lag_for_fallspecial,
                         int* out_msid, int* out_allow, float* out_rate)
{
    reset_captures(0);
    Fighter fighter = { .x2EC = LANDING_X2EC };
    Fighter_GObj gobj = { &fighter };
    switch (kind_of_entry) {
    case 0:
        ftCo_Landing_Enter_Basic(&gobj);
        break;
    case 1:
        ftCo_LandingFallSpecial_Enter_Basic(&gobj);
        break;
    default:
        ftCo_LandingFallSpecial_Enter(&gobj, allow, lag_for_fallspecial);
        break;
    }
    *out_msid = captured_motion;
    *out_allow = fighter.mv.co.landing.allow_interrupt;
    *out_rate = captured_rate;
    return captured_flags == Ft_MF_None && captured_start == 0.0F;
}
