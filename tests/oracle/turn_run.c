/* Host adapter for the pinned TurnRun callbacks (`ftCo_TurnRun_Phys`,
 * `ftCo_TurnRun_Enter`, `ftCo_TurnRun_Anim`, `fn_800C9CEC`, `fn_800C9D40`,
 * `tests/oracle/original/turn_run.c`, `turn_run.functions.json`).
 * `ftCo_TurnRun_IASA`/`_Coll` are not part of this selection and are not
 * declared here.
 *
 * `mv.co` is modeled as a real C union (`CoData`) so `turnrun.accel_mul`
 * and `walk.middle_anim_frame` alias the same storage exactly as the
 * pinned source's own union does (`ftCo_TurnRun.c:67`,
 * `ftCommon/types.h:53,71`) -- writing the entry facing through
 * `turnrun.accel_mul` at `ftCo_TurnRun_Enter` and reading it back through
 * `walk.middle_anim_frame` at `ftCo_TurnRun_Anim` is the exact mechanism
 * `docs/run.md`'s corrected flip check depends on.
 *
 * `Fighter_ChangeMotionState` captures its motion id/flags/start/speed;
 * `ftAnim_SetAnimRate` captures its rate and whether it was called this
 * call (the marker-inactive branch never calls it); `ftAnim_IsFramesRemaining`
 * and `fn_800CA644` are scriptable stubs returning a caller-supplied
 * boolean and recording whether they were reached; `ft_8008A2BC` records
 * whether the Wait fallback was reached; `ft_PlaySFX` is a no-op (this
 * adapter's `Fighter.x197C` is always NULL, so the pinned source's own
 * guard never calls it, but the symbol must still resolve to compile the
 * extracted body). Thread-local state isolates independent concurrent test
 * calls, not gameplay global state. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef uint8_t u8;
typedef int32_t s32;
typedef float f32;
typedef int FtMotionId;
typedef int ftCommon_MotionState;
typedef unsigned int MotionFlags;

#define ftCo_MS_TurnRun 22
#define Ft_MF_SkipAnimVel 0x8

typedef struct {
    float x, y, z;
} Vec3;
typedef struct {
    float ground_friction;
} ftCo_DatAttrs;
typedef struct {
    f32 x;
    f32 y;
} LStickSample;
typedef struct {
    LStickSample lstick[1];
} InputState;

/* Aliases `turnrun.accel_mul` (the per-entry facing) with
 * `walk.middle_anim_frame`, matching the pinned source's own union. */
typedef union {
    struct {
        f32 accel_mul;
        s32 x14;
    } turnrun;
    struct {
        f32 middle_anim_frame;
    } walk;
} CoData;

typedef struct Fighter {
    float gr_vel;
    float xE4_ground_accel_1;
    Vec3 self_vel;
    Vec3 x74_anim_vel;
    struct {
        struct {
            Vec3 normal;
        } floor;
    } coll_data;
    struct {
        CoData co;
    } mv;
    ftCo_DatAttrs co_attrs;
    f32 facing_dir;
    f32 cur_anim_frame;
    s32 cmd_vars[2];
    InputState input;
    void* x197C;
} Fighter;
typedef struct {
    Fighter* user_data;
} Fighter_GObj;
typedef struct {
    float run_dash_turn_friction_multiplier;
    f32 x38_someLStickXThreshold;
} ftCommonData;

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local float supplied_acceleration;
static _Thread_local float supplied_target;

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define PAD_STACK(size) ((void) (size))
#define ABS(value) ((value) < 0 ? -(value) : (value))

static void getAccelAndTarget(Fighter* fp, float* acceleration, float* target) {
    (void) fp;
    *acceleration = supplied_acceleration;
    *target = supplied_target;
}

static void ftCommon_ApplyFrictionGround(Fighter* fp, float friction) {
    if (ABS(friction) > ABS(fp->gr_vel)) {
        friction = -fp->gr_vel;
    } else if (fp->gr_vel > 0) {
        friction = -friction;
    }
    fp->xE4_ground_accel_1 = friction;
}

static void ftCommon_ApplyGroundMovement(Fighter_GObj* gobj) {
    Fighter* fp = GET_FIGHTER(gobj);
    fp->x74_anim_vel.x = fp->coll_data.floor.normal.y * fp->xE4_ground_accel_1;
    fp->x74_anim_vel.y = -fp->coll_data.floor.normal.x * fp->xE4_ground_accel_1;
    fp->x74_anim_vel.z = 0.0F;
    fp->self_vel.x = fp->coll_data.floor.normal.y * fp->gr_vel;
    fp->self_vel.y = -fp->coll_data.floor.normal.x * fp->gr_vel;
    fp->self_vel.z = 0.0F;
}

static _Thread_local ftCommon_MotionState captured_msid;
static _Thread_local MotionFlags captured_flags;
static _Thread_local float captured_start;
static _Thread_local float captured_speed;
static void Fighter_ChangeMotionState(Fighter_GObj* gobj, ftCommon_MotionState msid,
                                       MotionFlags flags, float anim_start, float anim_speed,
                                       float anim_blend, Fighter_GObj* arg3) {
    (void) gobj;
    (void) anim_blend;
    (void) arg3;
    captured_msid = msid;
    captured_flags = flags;
    captured_start = anim_start;
    captured_speed = anim_speed;
}

static _Thread_local int captured_play_sfx_called;
static void ft_PlaySFX(Fighter* fp, s32 sfx_id, s32 arg2, s32 arg3) {
    (void) fp;
    (void) sfx_id;
    (void) arg2;
    (void) arg3;
    captured_play_sfx_called = 1;
}

static _Thread_local float captured_anim_rate;
static _Thread_local int captured_set_rate_called;
static void ftAnim_SetAnimRate(Fighter_GObj* gobj, float rate) {
    (void) gobj;
    captured_anim_rate = rate;
    captured_set_rate_called = 1;
}

static _Thread_local int scripted_frames_remaining;
static bool ftAnim_IsFramesRemaining(Fighter_GObj* gobj) {
    (void) gobj;
    return scripted_frames_remaining;
}

static _Thread_local int scripted_fn_800ca644_result;
static _Thread_local int captured_fn_800ca644_called;
static bool fn_800CA644(Fighter_GObj* gobj) {
    (void) gobj;
    captured_fn_800ca644_called = 1;
    return scripted_fn_800ca644_result;
}

static _Thread_local int captured_wait_called;
static void ft_8008A2BC(Fighter_GObj* gobj) {
    (void) gobj;
    captured_wait_called = 1;
}

#include "turn_run_original.inc"

/* state: ground velocity/acceleration, self XY, animation XY, floor normal XY.
 * parameters: supplied acceleration/target, entry facing, ground friction,
 * common friction multiplier.
 */
void oracle_turn_run(float state[8], const float parameters[5]) {
    Fighter fighter = { 0 };
    Fighter_GObj object = { &fighter };
    fighter.gr_vel = state[0];
    fighter.xE4_ground_accel_1 = state[1];
    fighter.self_vel.x = state[2];
    fighter.self_vel.y = state[3];
    fighter.x74_anim_vel.x = state[4];
    fighter.x74_anim_vel.y = state[5];
    fighter.coll_data.floor.normal.x = state[6];
    fighter.coll_data.floor.normal.y = state[7];
    fighter.co_attrs.ground_friction = parameters[3];
    fighter.mv.co.turnrun.accel_mul = parameters[2];
    common.run_dash_turn_friction_multiplier = parameters[4];
    p_ftCommonData = &common;
    supplied_acceleration = parameters[0];
    supplied_target = parameters[1];
    ftCo_TurnRun_Phys(&object);
    state[0] = fighter.gr_vel;
    state[1] = fighter.xE4_ground_accel_1;
    state[2] = fighter.self_vel.x;
    state[3] = fighter.self_vel.y;
    state[4] = fighter.x74_anim_vel.x;
    state[5] = fighter.x74_anim_vel.y;
    state[6] = fighter.coll_data.floor.normal.x;
    state[7] = fighter.coll_data.floor.normal.y;
}

/* `ftCo_TurnRun_Enter`'s literal field assignments (`ftCo_TurnRun.c:44-55`):
 * `cmd_vars[1] = 0`, `turnrun.accel_mul = facing_dir` (the entry facing,
 * aliasing `walk.middle_anim_frame`), `ChangeMotionState(..., ftCo_MS_TurnRun,
 * Ft_MF_SkipAnimVel, anim_start, 1.0F, 0.0F, NULL)`, `turnrun.x14 = 0`. */
typedef struct {
    s32 msid;
    MotionFlags flags;
    f32 start;
    f32 speed;
    s32 cmd_vars1_after;
    f32 accel_mul_after;
    s32 x14_after;
} TurnRunEnterResult;

TurnRunEnterResult oracle_turn_run_enter(float anim_start, float facing_dir) {
    Fighter fp = { 0 };
    Fighter_GObj gobj = { &fp };
    fp.facing_dir = facing_dir;
    fp.cmd_vars[1] = 1;
    fp.mv.co.turnrun.x14 = 1;
    captured_msid = -1;
    captured_flags = 0;
    captured_start = 0.0f;
    captured_speed = 0.0f;
    captured_play_sfx_called = 0;
    ftCo_TurnRun_Enter(&gobj, anim_start);
    TurnRunEnterResult result = {
        .msid = captured_msid,
        .flags = captured_flags,
        .start = captured_start,
        .speed = captured_speed,
        .cmd_vars1_after = fp.cmd_vars[1],
        .accel_mul_after = fp.mv.co.turnrun.accel_mul,
        .x14_after = fp.mv.co.turnrun.x14,
    };
    return result;
}

/* `ftCo_TurnRun_Anim`'s complete per-frame decision (`ftCo_TurnRun.c:57-77`):
 * the freeze/resume/flip sequence gated by `cmd_vars[1]`/`turnrun.x14`, then
 * the unconditional animation-end check (`!IsFramesRemaining &&
 * !fn_800CA644 => ft_8008A2BC`). `entry_facing` models `turnrun.accel_mul`
 * (read back through the `middle_anim_frame` alias); `facing_dir` is the
 * fighter's *current* facing, mutated in place by a flip. */
typedef struct {
    s32 set_rate_called;
    f32 rate;
    s32 cmd_vars1_after;
    s32 x14_after;
    f32 facing_after;
    s32 fn_800ca644_called;
    s32 wait_called;
} TurnRunAnimResult;

TurnRunAnimResult oracle_turn_run_anim(int32_t cmd_vars1, int32_t turnrun_x14,
                                        float entry_facing, float facing_dir, float gr_vel,
                                        int32_t is_frames_remaining,
                                        int32_t fn_800ca644_result) {
    Fighter fp = { 0 };
    Fighter_GObj gobj = { &fp };
    fp.cmd_vars[1] = cmd_vars1;
    fp.mv.co.turnrun.x14 = turnrun_x14;
    fp.mv.co.turnrun.accel_mul = entry_facing;
    fp.facing_dir = facing_dir;
    fp.gr_vel = gr_vel;
    scripted_frames_remaining = is_frames_remaining;
    scripted_fn_800ca644_result = fn_800ca644_result;
    captured_set_rate_called = 0;
    captured_anim_rate = 0.0f;
    captured_fn_800ca644_called = 0;
    captured_wait_called = 0;
    ftCo_TurnRun_Anim(&gobj);
    TurnRunAnimResult result = {
        .set_rate_called = captured_set_rate_called,
        .rate = captured_anim_rate,
        .cmd_vars1_after = fp.cmd_vars[1],
        .x14_after = fp.mv.co.turnrun.x14,
        .facing_after = fp.facing_dir,
        .fn_800ca644_called = captured_fn_800ca644_called,
        .wait_called = captured_wait_called,
    };
    return result;
}

/* `fn_800C9CEC`/`fn_800C9D40` (`ftCo_TurnRun.c:19-42`): identical
 * `lstick.x * facing_dir <= x38` gate, differing only in the animation
 * start frame passed to `ftCo_TurnRun_Enter` on success (`cur_anim_frame`
 * for `fn_800C9CEC`, always `0.0F` for `fn_800C9D40`). */
typedef struct {
    s32 returned;
    s32 entered;
    f32 start_used;
} TurnRunCheckResult;

static TurnRunCheckResult check(int cec, float lstick_x, float facing_dir, float threshold,
                                 float cur_anim_frame) {
    Fighter fp = { 0 };
    Fighter_GObj gobj = { &fp };
    fp.input.lstick[0].x = lstick_x;
    fp.facing_dir = facing_dir;
    fp.cur_anim_frame = cur_anim_frame;
    common.x38_someLStickXThreshold = threshold;
    p_ftCommonData = &common;
    captured_start = -1.0f;
    captured_msid = -1;
    bool returned = cec ? fn_800C9CEC(&gobj) : fn_800C9D40(&gobj);
    TurnRunCheckResult result = {
        .returned = returned,
        .entered = captured_msid != -1,
        .start_used = captured_start,
    };
    return result;
}

TurnRunCheckResult oracle_fn_800c9cec(float lstick_x, float facing_dir, float threshold,
                                       float cur_anim_frame) {
    return check(1, lstick_x, facing_dir, threshold, cur_anim_frame);
}

TurnRunCheckResult oracle_fn_800c9d40(float lstick_x, float facing_dir, float threshold,
                                       float cur_anim_frame) {
    return check(0, lstick_x, facing_dir, threshold, cur_anim_frame);
}
