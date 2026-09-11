/* Host adapter for the pinned Run entry/animation-rate callbacks
 * (`ftCo_Run_Enter`, `ftCo_Run_Enter_Full`, `ftCo_Run_Anim`,
 * `tests/oracle/original/run.c`, `run.functions.json`). `ftCo_Run_IASA`/
 * `_Phys`/`_Coll` and Run's own callers (`fn_800CA5F0`/`fn_800CA644`/
 * `fn_800CA698`) are not part of this selection and are not declared here.
 * `Fighter_ChangeMotionState` captures its motion id, start frame and rate
 * argument; `ftAnim_SetAnimRate` captures its rate;
 * `ft_GetGroundFrictionMultiplier` reads an explicit environment field,
 * matching the walk oracle's own convention (`tests/oracle/walkcommon.c`).
 * Thread-local state isolates independent concurrent test calls, not
 * gameplay global state. */
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
