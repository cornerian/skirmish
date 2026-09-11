/* Host adapter for the pinned walk-kind/animation-rate/retype helpers
 * (`ftWalkCommon_GetWalkType`, `ftWalkCommon_800DFC70`,
 * `ftWalkCommon_800DFCA4`, `ftWalkCommon_800DFDDC`, `ftWalkCommon_800DFEC8`).
 * `ftWalkCommon_GetWalkType_800DFBF8_fake` is a `static inline` duplicate of
 * the extracted, non-static `ftWalkCommon_GetWalkType` (byte-identical
 * bodies); rather than re-extracting the inline, this adapter forward
 * declares the public function and defines the `_fake` name as a thin
 * wrapper around it, exactly reproducing what `ftWalkCommon_800DFCA4`/
 * `800DFEC8` call. `Fighter_ChangeMotionState`, `ftAnim_8006EBA4` and
 * `OSReport`/`HSD_ASSERT` are stubbed (the latter two are unreachable: the
 * `800DFEC8` switch's default case can only be hit if the caller's own
 * `fp->mv.co.walk.msid` disagrees with the motion actually driving the
 * walk type, which every entry point below keeps consistent).
 * `ftAnim_SetAnimRate` and `ftAnim_8006F484` capture/supply their scalar
 * argument and scripted return; `ft_GetGroundFrictionMultiplier` reads an
 * explicit environment field. Thread-local state isolates independent
 * concurrent test calls, not gameplay global state. */
#include <stdbool.h>
#include <stdint.h>
#include <stdarg.h>

typedef uint8_t u8;
typedef int32_t s32;
typedef float f32;
typedef int FtMotionId;
typedef int ftCommon_MotionState;
typedef unsigned int MotionFlags;
typedef int FtWalkType;
#define FtWalkType_Slow 0
#define FtWalkType_Middle 1
#define FtWalkType_Fast 2

typedef struct { f32 x; } Vec1;

typedef struct {
    f32 accel_mul;
    f32 x0;
    ftCommon_MotionState msid;
    f32 slow_anim_frame, middle_anim_frame, fast_anim_frame;
    f32 slow_anim_rate, middle_anim_rate, fast_anim_rate;
} WalkCoData;

typedef struct {
    f32 gr_vel;
    f32 facing_dir;
    f32 cur_anim_frame;
    FtMotionId motion_id;
    struct { Vec1 lstick[1]; } input;
    struct { struct { WalkCoData walk; } co; } mv;
    struct { f32 walk_max_vel; } co_attrs;
    /* Environment inputs the real engine would supply through separate
     * globals/queries; explicit here for the oracle. */
    f32 ground_friction_multiplier;
    f32 current_anim_length;
} Fighter;

typedef struct { Fighter *user_data; } HSD_GObj;
typedef HSD_GObj Fighter_GObj;

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define ABS(x) ((x) < 0 ? -(x) : (x))

typedef struct {
    f32 walk_stick_threshold;
    f32 walk_middle_animation_stick_threshold;
    f32 walk_fast_stick_threshold;
} ftCommonData;
static _Thread_local ftCommonData common;
static _Thread_local ftCommonData *p_ftCommonData;

static float ft_GetGroundFrictionMultiplier(Fighter *fp) {
    return fp->ground_friction_multiplier;
}

static void Fighter_ChangeMotionState(Fighter_GObj *gobj, ftCommon_MotionState msid,
                                       MotionFlags flags, float anim_start, float rate,
                                       int unk1, int unk2) {
    (void) gobj;
    (void) msid;
    (void) flags;
    (void) anim_start;
    (void) rate;
    (void) unk1;
    (void) unk2;
}
static void ftAnim_8006EBA4(Fighter_GObj *gobj) { (void) gobj; }

static _Thread_local float captured_rate;
static void ftAnim_SetAnimRate(Fighter_GObj *gobj, float rate) {
    (void) gobj;
    captured_rate = rate;
}

static float ftAnim_8006F484(Fighter_GObj *gobj) {
    return GET_FIGHTER(gobj)->current_anim_length;
}

static void OSReport(const char *fmt, ...) { (void) fmt; }
static void HSD_ASSERT(int line, int cond) { (void) line; (void) cond; }

FtWalkType ftWalkCommon_GetWalkType(HSD_GObj *gobj);
static FtWalkType ftWalkCommon_GetWalkType_800DFBF8_fake(HSD_GObj *gobj) {
    return ftWalkCommon_GetWalkType(gobj);
}

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-variable"
#include "walkcommon_original.inc"
#pragma GCC diagnostic pop

/* Returns 0/1/2 for Slow/Middle/Fast (`ftWalkCommon_GetWalkType`, identical
 * to the `_fake` inline `ftWalkCommon_800DFCA4`/`800DFEC8` call). */
int oracle_walk_type(float gr_vel, float accel_mul, float middle, float fast, float walk_max) {
    Fighter fp = {0};
    Fighter_GObj gobj = { &fp };
    common = (ftCommonData) {
        .walk_stick_threshold = 0,
        .walk_middle_animation_stick_threshold = middle,
        .walk_fast_stick_threshold = fast,
    };
    p_ftCommonData = &common;
    fp.gr_vel = gr_vel;
    fp.mv.co.walk.accel_mul = accel_mul;
    fp.co_attrs.walk_max_vel = walk_max;
    return (int) ftWalkCommon_GetWalkType(&gobj);
}

/* `ftWalkCommon_800DFDDC`'s animation-rate selection. `kind` is 0/1/2
 * (Slow/Middle/Fast); `fp->motion_id - fp->mv.co.walk.msid` is set up to
 * equal it exactly. */
float oracle_walk_rate(float gr_vel, float facing, int kind, const float rates[3],
                        float x0, float friction_mul) {
    Fighter fp = {0};
    Fighter_GObj gobj = { &fp };
    fp.gr_vel = gr_vel;
    fp.facing_dir = facing;
    fp.mv.co.walk.msid = 0;
    fp.motion_id = kind;
    fp.mv.co.walk.slow_anim_rate = rates[0];
    fp.mv.co.walk.middle_anim_rate = rates[1];
    fp.mv.co.walk.fast_anim_rate = rates[2];
    fp.mv.co.walk.x0 = x0;
    fp.ground_friction_multiplier = friction_mul;
    captured_rate = 0.0f;
    ftWalkCommon_800DFDDC(&gobj);
    return captured_rate;
}

typedef struct {
    int changed;
    int new_kind;
    int start_frame;
} WalkRetypeResult;

static _Thread_local int captured_retype_changed;
static _Thread_local float captured_retype_start_frame;
static void capture_retype_start_frame(HSD_GObj *gobj, float frame) {
    (void) gobj;
    captured_retype_changed = 1;
    captured_retype_start_frame = frame;
}

/* `ftWalkCommon_800DFEC8`'s start-frame remap when the walk kind changes,
 * plus an independent (but input-identical, hence exact) call into
 * `ftWalkCommon_GetWalkType` to report the target kind: `800DFEC8` never
 * exposes its locally computed `walk_action_type` to the caller, and this
 * oracle's own captured callback stands in for the real `ftCo_Walk_Enter`
 * re-entry `arg_cb`, which is not part of the pinned selection. */
WalkRetypeResult oracle_walk_retype(int cur_kind, float cur_frame, float cur_len,
                                     const float lengths[3], float gr_vel, float accel_mul,
                                     float middle_threshold, float fast_threshold,
                                     float walk_max) {
    Fighter fp = {0};
    Fighter_GObj gobj = { &fp };
    common = (ftCommonData) {
        .walk_stick_threshold = 0,
        .walk_middle_animation_stick_threshold = middle_threshold,
        .walk_fast_stick_threshold = fast_threshold,
    };
    p_ftCommonData = &common;
    fp.gr_vel = gr_vel;
    fp.mv.co.walk.accel_mul = accel_mul;
    fp.co_attrs.walk_max_vel = walk_max;
    fp.mv.co.walk.msid = 0;
    fp.motion_id = cur_kind;
    fp.cur_anim_frame = cur_frame;
    fp.mv.co.walk.slow_anim_frame = lengths[0];
    fp.mv.co.walk.middle_anim_frame = lengths[1];
    fp.mv.co.walk.fast_anim_frame = lengths[2];
    fp.current_anim_length = cur_len;
    int new_kind = (int) ftWalkCommon_GetWalkType(&gobj);
    captured_retype_changed = 0;
    captured_retype_start_frame = 0.0f;
    ftWalkCommon_800DFEC8(&gobj, capture_retype_start_frame);
    WalkRetypeResult result = {
        .changed = captured_retype_changed,
        .new_kind = new_kind,
        .start_frame = (int) captured_retype_start_frame,
    };
    return result;
}
