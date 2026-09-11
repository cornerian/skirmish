/* Host adapter for the pinned RunBrake callbacks (`ftCo_RunBrake_Anim`,
 * `ftCo_RunBrake_IASA`, `tests/oracle/original/runbrake.c`,
 * `runbrake.functions.json`). `ftCo_RunBrake_CheckInput`/`_Enter`/`_Phys`/
 * `_Coll` are not part of this selection and are not declared here.
 *
 * `ftAnim_SetAnimRate` captures its rate and whether it was called this
 * call; `ftAnim_IsFramesRemaining` is a scriptable stub; `ft_8008A2BC`
 * records whether the Wait fallback was reached. `ftCo_RunBrake_IASA`'s own
 * three checks (`fn_800CAF78` the shared jump dispatch, `fn_800C9CEC` the
 * turn-run entry -- gated by `cmd_vars[0]`, `ftCo_800D5FB0` the squat entry)
 * are stubbed as scriptable booleans ("script the answers"): this adapter
 * proves the *gate order* (jump, then cmd_vars[0]-gated turn, then squat),
 * not those callbacks' own internal correctness (covered by their own
 * oracles elsewhere). Thread-local state isolates independent concurrent
 * test calls, not gameplay global state. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef int32_t s32;
typedef float f32;

typedef struct {
    s32 x0;
    f32 frames;
} RunBrakeCoData;

typedef struct Fighter {
    f32 gr_vel;
    s32 cmd_vars[2];
    struct {
        struct {
            RunBrakeCoData runbrake;
        } co;
    } mv;
} Fighter;
typedef struct {
    Fighter* user_data;
} Fighter_GObj;
typedef struct {
    f32 x42C;
} ftCommonData;

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define ABS(value) ((value) < 0 ? -(value) : (value))
#define RETURN_IF(x) \
    do { \
        if (x) { \
            return; \
        } \
    } while (0)

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

static _Thread_local int captured_wait_called;
static void ft_8008A2BC(Fighter_GObj* gobj) {
    (void) gobj;
    captured_wait_called = 1;
}

static _Thread_local int scripted_jump_result;
static _Thread_local int captured_jump_called;
static bool fn_800CAF78(Fighter_GObj* gobj) {
    (void) gobj;
    captured_jump_called = 1;
    return scripted_jump_result;
}

static _Thread_local int scripted_turn_result;
static _Thread_local int captured_turn_called;
static bool fn_800C9CEC(Fighter_GObj* gobj) {
    (void) gobj;
    captured_turn_called = 1;
    return scripted_turn_result;
}

static _Thread_local int scripted_squat_result;
static _Thread_local int captured_squat_called;
static bool ftCo_800D5FB0(Fighter_GObj* gobj) {
    (void) gobj;
    captured_squat_called = 1;
    return scripted_squat_result;
}

#include "runbrake_original.inc"

/* `ftCo_RunBrake_Anim`'s complete per-frame decision
 * (`ftCo_RunBrake.c:49-77`): the velocity-gated marker freeze
 * (`cmd_vars[1]`/`x42C`/`runbrake.x0`), the unconditional `frames`
 * countdown, and the exit test (`!IsFramesRemaining || !frames`). */
typedef struct {
    s32 set_rate_called;
    f32 rate;
    s32 cmd_vars1_after;
    s32 x0_after;
    f32 frames_after;
    s32 wait_called;
} RunBrakeAnimResult;

RunBrakeAnimResult oracle_run_brake_anim(int32_t cmd_vars1, int32_t runbrake_x0, float gr_vel,
                                          float freeze_speed, float runbrake_frames,
                                          int32_t is_frames_remaining) {
    Fighter fp = { 0 };
    Fighter_GObj gobj = { &fp };
    fp.cmd_vars[1] = cmd_vars1;
    fp.mv.co.runbrake.x0 = runbrake_x0;
    fp.gr_vel = gr_vel;
    fp.mv.co.runbrake.frames = runbrake_frames;
    common.x42C = freeze_speed;
    p_ftCommonData = &common;
    scripted_frames_remaining = is_frames_remaining;
    captured_set_rate_called = 0;
    captured_anim_rate = 0.0f;
    captured_wait_called = 0;
    ftCo_RunBrake_Anim(&gobj);
    RunBrakeAnimResult result = {
        .set_rate_called = captured_set_rate_called,
        .rate = captured_anim_rate,
        .cmd_vars1_after = fp.cmd_vars[1],
        .x0_after = fp.mv.co.runbrake.x0,
        .frames_after = fp.mv.co.runbrake.frames,
        .wait_called = captured_wait_called,
    };
    return result;
}

/* `ftCo_RunBrake_IASA`'s gate order (`ftCo_RunBrake.c:80-87`): jump first,
 * then the turn-run entry gated by `cmd_vars[0]`, then squat. Each
 * scripted answer is a boolean; `*_called` reports whether the chain
 * reached that check at all (a `RETURN_IF` chain, so a `true` answer
 * short-circuits every later check). */
typedef struct {
    s32 jump_called;
    s32 turn_called;
    s32 squat_called;
} RunBrakeIasaResult;

RunBrakeIasaResult oracle_run_brake_iasa(int32_t jump_result, int32_t cmd_vars0,
                                          int32_t turn_result, int32_t squat_result) {
    Fighter fp = { 0 };
    Fighter_GObj gobj = { &fp };
    fp.cmd_vars[0] = cmd_vars0;
    scripted_jump_result = jump_result;
    scripted_turn_result = turn_result;
    scripted_squat_result = squat_result;
    captured_jump_called = 0;
    captured_turn_called = 0;
    captured_squat_called = 0;
    ftCo_RunBrake_IASA(&gobj);
    RunBrakeIasaResult result = {
        .jump_called = captured_jump_called,
        .turn_called = captured_turn_called,
        .squat_called = captured_squat_called,
    };
    return result;
}
