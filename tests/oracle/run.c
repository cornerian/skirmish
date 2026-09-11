/* Host adapter for the pinned Run entry/animation-rate/IASA callbacks
 * (`ftCo_Run_Enter`, `ftCo_Run_Enter_Full`, `ftCo_Run_Anim`,
 * `ftCo_Run_IASA`, `tests/oracle/original/run.c`, `run.functions.json`).
 * `ftCo_Run_Phys`/`_Coll` and Run's own callers (`fn_800CA5F0`/
 * `fn_800CA644`/`fn_800CA698`) are not part of this selection and are not
 * declared here. `Fighter_ChangeMotionState` captures its motion id, start
 * frame and rate argument; `ftAnim_SetAnimRate` captures its rate;
 * `ft_GetGroundFrictionMultiplier` reads an explicit environment field,
 * matching the walk oracle's own convention (`tests/oracle/walkcommon.c`).
 * `ftCo_Run_IASA`'s own eleven checks are stubbed as scriptable booleans
 * ("script the answers", the same convention `tests/oracle/runbrake.c`
 * uses for `ftCo_RunBrake_IASA`): this adapter proves the *gate order*
 * around `run.x0` (`ftCo_Run.c:125-126`) -- while `run.x0 > 0.0F` the
 * RunTurn (`fn_800C9D40`) and RunBrake (`ftCo_RunBrake_CheckInput`) checks
 * are both skipped -- not those callbacks' own internal correctness
 * (covered by their own oracles elsewhere). Thread-local state isolates
 * independent concurrent test calls, not gameplay global state. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef uint8_t u8;
typedef int32_t s32;
typedef float f32;
typedef int FtMotionId;
typedef int ftCommon_MotionState;
typedef unsigned int MotionFlags;

/* Any distinct value stands in for the real `ftCo_MS_Run` enumerator; the
 * oracle only proves `Fighter_ChangeMotionState` is called with this exact,
 * fixed constant, not its real numeric encoding (owned by `forward.h`'s
 * `ftCommon_MotionState`, cited in `docs/run.md`). */
#define ftCo_MS_Run 21
#define Ft_MF_None 0

typedef struct {
    f32 x0;
    f32 x4;
} RunCoData;

typedef struct {
    f32 run_animation_scaling;
} ftCo_DatAttrs;

typedef struct {
    /* Unnamed common-data float consumed by the `ftCo_80091A4C` branch;
     * unused by this adapter's own scripted stub. */
    f32 x410;
} ftCommonData;

typedef struct {
    f32 gr_vel;
    f32 facing_dir;
    struct {
        struct {
            RunCoData run;
        } co;
    } mv;
    ftCo_DatAttrs co_attrs;
    /* Environment input the real engine would supply through a separate
     * query; explicit here for the oracle. */
    f32 ground_friction_multiplier;
} Fighter;

typedef struct {
    Fighter *user_data;
} HSD_GObj;
typedef HSD_GObj Fighter_GObj;

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define ABS(x) ((x) < 0 ? -(x) : (x))
#define PAD_STACK(size) ((void) (size))
#define RETURN_IF(x) \
    do { \
        if (x) { \
            return; \
        } \
    } while (0)

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData *p_ftCommonData;

static float ft_GetGroundFrictionMultiplier(Fighter *fp) {
    return fp->ground_friction_multiplier;
}

static _Thread_local ftCommon_MotionState captured_msid;
static _Thread_local float captured_start;
static _Thread_local float captured_speed;
static void Fighter_ChangeMotionState(Fighter_GObj *gobj, ftCommon_MotionState msid,
                                       MotionFlags flags, float anim_start, float anim_speed,
                                       float anim_blend, Fighter_GObj *arg3) {
    (void) gobj;
    (void) flags;
    (void) anim_blend;
    (void) arg3;
    captured_msid = msid;
    captured_start = anim_start;
    captured_speed = anim_speed;
}

static _Thread_local float captured_anim_rate;
static void ftAnim_SetAnimRate(Fighter_GObj *gobj, float rate) {
    (void) gobj;
    captured_anim_rate = rate;
}

/* `ftCo_Run_IASA`'s own eleven checks (`ftCo_Run.c:101-128`), each a
 * scriptable boolean stub recording whether it was reached ("script the
 * answers"). Named after their call order in the source, not their real
 * symbols beyond `fn_800CAF78`/`fn_800C9D40`/`ftCo_RunBrake_CheckInput`
 * (the ones this batch's lockout gate actually concerns). */
#define IASA_STUB(name) \
    static _Thread_local int scripted_##name; \
    static _Thread_local int called_##name; \
    static bool name(Fighter_GObj *gobj) { \
        (void) gobj; \
        called_##name = 1; \
        return scripted_##name; \
    }
IASA_STUB(ftCo_SpecialS_CheckInput)
IASA_STUB(ftCo_Attack100_CheckInput)
IASA_STUB(ftCo_800D6824)
IASA_STUB(ftCo_800D68C0)
IASA_STUB(ftCo_800D8A38)
IASA_STUB(ftCo_AttackDash_CheckInput)
IASA_STUB(ftCo_80091A4C)
IASA_STUB(ftCo_800DE9D8)
IASA_STUB(fn_800CAF78)
IASA_STUB(fn_800C9D40)
IASA_STUB(ftCo_RunBrake_CheckInput)
#undef IASA_STUB

static _Thread_local int called_ftCo_AttackDash_SetMv0;
static void ftCo_AttackDash_SetMv0(Fighter_GObj *gobj) {
    (void) gobj;
    called_ftCo_AttackDash_SetMv0 = 1;
}

static _Thread_local int called_ftCo_80091B90;
static void ftCo_80091B90(Fighter_GObj *gobj, float arg) {
    (void) gobj;
    (void) arg;
    called_ftCo_80091B90 = 1;
}

static _Thread_local int called_ftCo_80091B9C;
static void ftCo_80091B9C(Fighter_GObj *gobj) {
    (void) gobj;
    called_ftCo_80091B9C = 1;
}

/* Forward declaration: the selection lists `ftCo_Run_Enter` (which calls
 * `ftCo_Run_Enter_Full`) before `ftCo_Run_Enter_Full` itself, matching the
 * pinned source's own definition order (`ftCo_Run.c:61`, `66`). */
void ftCo_Run_Enter_Full(Fighter_GObj *gobj, float arg0, float anim_start, float anim_speed);

#include "run_original.inc"

/* `ftCo_Run_Anim`'s animation-rate selection and `run.x0` countdown.
 * `run_x4`/`friction_mul` model `fp->mv.co.run.x4` and
 * `ft_GetGroundFrictionMultiplier`. Returns the captured
 * `ftAnim_SetAnimRate` rate and `run.x0` after its own countdown
 * (`ftCo_Run.c:96-98`: decremented by 1 only while `> 0.0F`, never
 * otherwise clamped). */
typedef struct {
    f32 rate;
    f32 run_x0_after;
} RunAnimResult;

RunAnimResult oracle_run_anim(float gr_vel, float run_x4, float facing, float scaling,
                               float friction_mul, float run_x0) {
    Fighter fp = {0};
    Fighter_GObj gobj = {&fp};
    fp.gr_vel = gr_vel;
    fp.facing_dir = facing;
    fp.mv.co.run.x4 = run_x4;
    fp.mv.co.run.x0 = run_x0;
    fp.co_attrs.run_animation_scaling = scaling;
    fp.ground_friction_multiplier = friction_mul;
    captured_anim_rate = 0.0f;
    ftCo_Run_Anim(&gobj);
    RunAnimResult result = {
        .rate = captured_anim_rate,
        .run_x0_after = fp.mv.co.run.x0,
    };
    return result;
}

/* `ftCo_Run_Enter_Full` via the public two-argument `ftCo_Run_Enter`
 * (`arg0 = x0`; `ftCo_Run_Enter` itself always supplies `anim_start = 0.0F`,
 * `anim_speed = 1.0F` -- see `oracle_run_enter_full` below for the direct
 * three-argument entry point `fn_800CA698`/`ftCo_RunDirect.c` uses, unreached
 * in this codebase). `gr_vel` is pre-set on the fighter so `run.x4 = gr_vel`
 * (`ftCo_Run.c:73`) is observable in the result. */
typedef struct {
    s32 msid;
    f32 start;
    f32 speed;
    f32 x0;
    f32 x4;
} RunEnterResult;

RunEnterResult oracle_run_enter(float x0, float gr_vel) {
    Fighter fp = {0};
    Fighter_GObj gobj = {&fp};
    fp.gr_vel = gr_vel;
    captured_msid = -1;
    captured_start = 0.0f;
    captured_speed = 0.0f;
    ftCo_Run_Enter(&gobj, x0);
    RunEnterResult result = {
        .msid = captured_msid,
        .start = captured_start,
        .speed = captured_speed,
        .x0 = fp.mv.co.run.x0,
        .x4 = fp.mv.co.run.x4,
    };
    return result;
}

/* `ftCo_Run_Enter_Full`'s own direct three-argument entry point
 * (`fn_800CA698`, unreached in this codebase -- see `docs/run.md`), exposed
 * for completeness/documentation rather than a Rust-side comparison. */
RunEnterResult oracle_run_enter_full(float x0, float anim_start, float anim_speed, float gr_vel) {
    Fighter fp = {0};
    Fighter_GObj gobj = {&fp};
    fp.gr_vel = gr_vel;
    captured_msid = -1;
    captured_start = 0.0f;
    captured_speed = 0.0f;
    ftCo_Run_Enter_Full(&gobj, x0, anim_start, anim_speed);
    RunEnterResult result = {
        .msid = captured_msid,
        .start = captured_start,
        .speed = captured_speed,
        .x0 = fp.mv.co.run.x0,
        .x4 = fp.mv.co.run.x4,
    };
    return result;
}

/* `ftCo_Run_IASA`'s gate order (`ftCo_Run.c:101-128`), focused on the
 * `run.x0` lockout: `jump_called` (`fn_800CAF78`) is reached whenever every
 * earlier scripted check returns false; `turn_called` (`fn_800C9D40`) and
 * `brake_called` (`ftCo_RunBrake_CheckInput`) are only reached while
 * `run_x0 <= 0.0F`. */
typedef struct {
    s32 special_called;
    s32 attack100_called;
    s32 x6824_called;
    s32 x68c0_called;
    s32 x8a38_called;
    s32 attackdash_called;
    s32 attackdash_setmv0_called;
    s32 x91a4c_called;
    s32 x91b90_called;
    s32 x91b9c_called;
    s32 de9d8_called;
    s32 jump_called;
    s32 turn_called;
    s32 brake_called;
} RunIasaResult;

RunIasaResult oracle_run_iasa(float run_x0, int32_t special, int32_t attack100, int32_t x6824,
                               int32_t x68c0, int32_t x8a38, int32_t attackdash, int32_t x91a4c,
                               int32_t de9d8, int32_t jump, int32_t turn, int32_t brake) {
    Fighter fp = {0};
    Fighter_GObj gobj = {&fp};
    fp.mv.co.run.x0 = run_x0;
    common.x410 = 0.0f;
    p_ftCommonData = &common;
    scripted_ftCo_SpecialS_CheckInput = special;
    scripted_ftCo_Attack100_CheckInput = attack100;
    scripted_ftCo_800D6824 = x6824;
    scripted_ftCo_800D68C0 = x68c0;
    scripted_ftCo_800D8A38 = x8a38;
    scripted_ftCo_AttackDash_CheckInput = attackdash;
    scripted_ftCo_80091A4C = x91a4c;
    scripted_ftCo_800DE9D8 = de9d8;
    scripted_fn_800CAF78 = jump;
    scripted_fn_800C9D40 = turn;
    scripted_ftCo_RunBrake_CheckInput = brake;
    called_ftCo_SpecialS_CheckInput = 0;
    called_ftCo_Attack100_CheckInput = 0;
    called_ftCo_800D6824 = 0;
    called_ftCo_800D68C0 = 0;
    called_ftCo_800D8A38 = 0;
    called_ftCo_AttackDash_CheckInput = 0;
    called_ftCo_AttackDash_SetMv0 = 0;
    called_ftCo_80091A4C = 0;
    called_ftCo_80091B90 = 0;
    called_ftCo_80091B9C = 0;
    called_ftCo_800DE9D8 = 0;
    called_fn_800CAF78 = 0;
    called_fn_800C9D40 = 0;
    called_ftCo_RunBrake_CheckInput = 0;
    ftCo_Run_IASA(&gobj);
    RunIasaResult result = {
        .special_called = called_ftCo_SpecialS_CheckInput,
        .attack100_called = called_ftCo_Attack100_CheckInput,
        .x6824_called = called_ftCo_800D6824,
        .x68c0_called = called_ftCo_800D68C0,
        .x8a38_called = called_ftCo_800D8A38,
        .attackdash_called = called_ftCo_AttackDash_CheckInput,
        .attackdash_setmv0_called = called_ftCo_AttackDash_SetMv0,
        .x91a4c_called = called_ftCo_80091A4C,
        .x91b90_called = called_ftCo_80091B90,
        .x91b9c_called = called_ftCo_80091B9C,
        .de9d8_called = called_ftCo_800DE9D8,
        .jump_called = called_fn_800CAF78,
        .turn_called = called_fn_800C9D40,
        .brake_called = called_ftCo_RunBrake_CheckInput,
    };
    return result;
}
