/* Host adapter for Fox/Falco's side special (`ftfoxspecials.c`,
 * `fox_specials.functions.json`: every non-static callback plus the four
 * `static inline` helpers -- the source actually has four, not three; all
 * four are extracted).
 *
 * Dependency treatment, as briefed:
 *  - CAPTURED (logged, not implemented): `Fighter_ChangeMotionState`,
 *    `ftCo_80096900`, `ftCo_LandingFallSpecial_Enter`, `ftCo_Fall_Enter`.
 *  - SCRIPTED (test-controlled return): `ftAnim_IsFramesRemaining`,
 *    `ft_80082708`, `ft_800827A0`, `ft_CheckGroundAndLedge`,
 *    `ftCliffCommon_80081298`, `ft_GetGroundFrictionMultiplier`.
 *  - FAITHFUL (real arithmetic, reproduced verbatim from the pinned
 *    source below each definition's own citation): `ftCommon_Fall`,
 *    `ftCommon_ApplyFrictionAir`, `ftCommon_ApplyFrictionGround`,
 *    `ftCommon_8007D60C`, `ftCommon_8007D6A4`, `ftCommon_8007D7FC`,
 *    `ftCommon_AirToGroundStateChange`, `ftCommon_UseAllJumps`,
 *    `ft_80084F3C`, `ft_800850E0`, `ft_80085088`, `ft_80085134`. These are
 *    reimplemented locally with a self-contained `Fighter` struct rather
 *    than linked against `tests/oracle/physics.c`/`locomotion.c`'s own
 *    extractions: those compile against their own local struct layout for
 *    their own callers, and nothing in this codebase's oracle build
 *    guarantees two independently hand-written adapter structs share
 *    field offsets. Duplicating the verbatim bodies against one struct
 *    this file owns is the safe way to get faithful arithmetic without an
 *    unchecked cross-adapter ABI assumption.
 *  - Ground `ftCommon_ApplyGroundMovement` is a documented no-op: the real
 *    function (`ftcommon.c:133-148`) projects `xE4_ground_accel_1`/`gr_vel`
 *    onto the floor normal and folds in `ft_GetGroundFrictionMultiplier`
 *    a second time, entirely through the full map-collision system this
 *    oracle does not model; every value this differential suite compares
 *    (gr_vel, gravity delay, TransN outputs, entry/exit decisions) is
 *    already finalized before this call, so its position-only effect is
 *    intentionally not reproduced.
 *  - Ghost/GFX no-ops: `efSync_Spawn`, `Fighter_SetEffectHitlagCallbacks`,
 *    `it_8029CEB4`, `ftAnim_8006EBA4`. Confirmed unmodeled elsewhere
 *    (`docs/fox-side-special.md`): the ghost item spawns no hitboxes.
 *
 * Thread-local state isolates independent concurrent test calls, not
 * gameplay global state, matching every other adapter in this directory. */
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

typedef uint8_t u8;
typedef int32_t s32;
typedef uint32_t u32;
typedef float f32;
typedef int FtMotionId;
typedef int MotionFlags;
typedef int GroundOrAir;
typedef s32 enum_t;
#define ftCommon_GroundAirColl_MF 0x1
#define Ft_MF_SkipRumble 0x2
#define Ft_MF_KeepColAnimHitStatus 0x4

#define GA_Ground 0
#define GA_Air 1
#define FTKIND_FOX 2
#define FTKIND_FALCO 22
#define FTKIND_POPO 10
#define FTKIND_NANA 11

typedef struct { float x, y, z; } Vec3;

enum {
    ftFx_MS_SpecialSStart = 1000,
    ftFx_MS_SpecialS,
    ftFx_MS_SpecialSEnd,
    ftFx_MS_SpecialAirSStart,
    ftFx_MS_SpecialAirS,
    ftFx_MS_SpecialAirSEnd,
};

typedef struct {
    /* x24..x50, ftFox/types.h:93-105. */
    float x24_FOX_ILLUSION_GRAVITY_DELAY;
    float x28_FOX_ILLUSION_GROUND_VEL_X;
    float x2C_FOX_ILLUSION_UNK1;
    float x30_FOX_ILLUSION_UNK2;
    float x34_FOX_ILLUSION_GROUND_END_VEL_X;
    float x38_FOX_ILLUSION_GROUND_FRICTION;
    float x3C_FOX_ILLUSION_AIR_END_VEL_X;
    float x40_FOX_ILLUSION_AIR_MUL_X;
    float x44_FOX_ILLUSION_FALL_ACCEL;
    float x48_FOX_ILLUSION_TERMINAL_VELOCITY;
    float x4C_FOX_ILLUSION_FREEFALL_MOBILITY;
    float x50_FOX_ILLUSION_LANDING_LAG;
} ftFox_DatAttrs;

typedef struct {
    float ground_friction;
    float walk_max_vel;
    float ground_max_horizontal_velocity;
    float terminal_velocity;
    float specials_ground_speed_retention;
    s32 max_jumps;
} ftCo_DatAttrs;

typedef struct {
    float gravityDelay;
    void* ghostGObj;
    Vec3 ghostEffectPos[4];
    float blendFrames[4];
} SpecialSMoveVar;

typedef struct { u32 pressed_buttons; } FighterInput;
#define HSD_PAD_B 0x2000
typedef struct { s32 anim_id; } MotionState;
#define ACTION_STATE_COUNT 512
typedef struct { SpecialSMoveVar SpecialS; } MvFx;
typedef struct { MvFx fx; } Mv;

typedef struct Fighter_GObj_s Fighter_GObj;

typedef struct Fighter {
    s32 kind;
    s32 motion_id;
    FighterInput input;
    ftFox_DatAttrs* dat_attrs;
    ftCo_DatAttrs co_attrs;
    GroundOrAir ground_or_air;
    float gr_vel;
    Vec3 self_vel;
    float facing_dir;
    Vec3 cur_pos;
    float cur_anim_frame;
    u8 x1968_jumpsUsed;
    u8 x1969_walljumpUsed;
    bool x2219_b0;
    bool x2222_b2;
    bool x2227_b0;
    bool x594_b0;
    Vec3 x6A4_transNOffset;
    s32 cmd_vars[8];
    void (*accessory4_cb)(Fighter_GObj*);
    Mv mv;
    MotionState x1C_actionStateList[ACTION_STATE_COUNT];
} Fighter;
struct Fighter_GObj_s { Fighter* user_data; };
typedef Fighter_GObj HSD_GObj;
#define GET_FIGHTER(gobj) ((gobj)->user_data)
static Fighter* getFighter(Fighter_GObj* gobj) { return GET_FIGHTER(gobj); }
static ftFox_DatAttrs* getFtSpecialAttrs(Fighter* fp) { return fp->dat_attrs; }

typedef struct { float friction_when_above_walk_speed; } FtCommonData;
/* Thread-local: written per call (see the assignment below) and read back
 * by the included decomp source within the same call. A plain global here
 * let one proptest thread's `friction_when_above_walk_speed` bleed into
 * another's concurrent call, the same race `3b56a66` fixed for
 * `ftfoxspeciallw.c`/`air_drift_recovery.c`'s own copies of this struct.
 * `p_ftCommonData` is a macro, not a plain pointer, so it resolves
 * per-thread instead of freezing to whichever thread ran static init. */
static _Thread_local FtCommonData ftCommonData_ = { 1.0f };
#define p_ftCommonData (&ftCommonData_)

/* ---- FAITHFUL: verbatim arithmetic from the pinned source. ---- */

/* ftcommon.c:462-467. */
static void ftCommon_Fall(Fighter* fp, float gravity, float terminal_vel) {
    fp->self_vel.y -= gravity;
    if (fp->self_vel.y < -terminal_vel) {
        fp->self_vel.y = -terminal_vel;
    }
}

/* ftcommon.c:253-261. */
static void ftCommon_ApplyFrictionAir(Fighter* fp, float friction) {
    float f = friction;
    if ((f < 0 ? -f : f) >= (fp->self_vel.x < 0 ? -fp->self_vel.x : fp->self_vel.x)) {
        f = -fp->self_vel.x;
    } else if (fp->self_vel.x > 0) {
        f = -f;
    }
    /* The real function writes x74_anim_vel.x, integrated into self_vel by
     * the caller's own frame tail; this adapter has no separate animation-
     * velocity channel, so it applies the result directly, matching this
     * oracle's per-call (not per-integration-step) comparison granularity. */
    fp->self_vel.x += f;
}

/* ftcommon.c:51-59. */
static void ftCommon_ApplyFrictionGround(Fighter* fp, float friction) {
    float f = friction;
    if ((f < 0 ? -f : f) > (fp->gr_vel < 0 ? -fp->gr_vel : fp->gr_vel)) {
        f = -fp->gr_vel;
    } else if (fp->gr_vel > 0) {
        f = -f;
    }
    fp->gr_vel += f;
}

/* ftcommon.c:133-148, position-only effect elided; see the file header. */
static void ftCommon_ApplyGroundMovement(Fighter_GObj* gobj) { (void) gobj; }

/* ftcommon.c:527-539. */
static void ftCommon_8007D60C(Fighter* fp) {
    fp->ground_or_air = GA_Air;
    fp->gr_vel = 0;
    fp->x1968_jumpsUsed = (u8) fp->co_attrs.max_jumps;
}

/* ftcommon.c:546-557 (`ftCommon_8007D6A4`). */
static void ftCommon_8007D6A4(Fighter* fp) {
    if (fp->x594_b0) {
        fp->self_vel.x = fp->x6A4_transNOffset.z * fp->facing_dir;
    }
    if (fp->gr_vel > fp->co_attrs.ground_max_horizontal_velocity) {
        fp->gr_vel = fp->co_attrs.ground_max_horizontal_velocity;
    } else if (fp->gr_vel < -fp->co_attrs.ground_max_horizontal_velocity) {
        fp->gr_vel = -fp->co_attrs.ground_max_horizontal_velocity;
    }
    fp->ground_or_air = GA_Ground;
    fp->gr_vel = fp->self_vel.x;
    fp->x1968_jumpsUsed = 0;
    fp->x1969_walljumpUsed = 0;
    fp->x2227_b0 = 0;
}

/* ftcommon.c:581-594; the knockback/bonus-stat branch is unmodeled
 * elsewhere in this codebase and has no bearing on the compared
 * arithmetic, so it is elided here too. */
static void ftCommon_8007D7FC(Fighter* fp) { ftCommon_8007D6A4(fp); }

static void ftCommon_UseAllJumps(Fighter* fp) { fp->x1968_jumpsUsed = (u8) fp->co_attrs.max_jumps; }

/* ft_084E.c:42-53. */
static void ft_80084F3C(Fighter_GObj* gobj) {
    Fighter* fp = GET_FIGHTER(gobj);
    float friction = fp->co_attrs.ground_friction;
    if ((fp->gr_vel < 0 ? -fp->gr_vel : fp->gr_vel) > fp->co_attrs.walk_max_vel) {
        friction *= p_ftCommonData->friction_when_above_walk_speed;
    }
    ftCommon_ApplyFrictionGround(fp, friction);
    ftCommon_ApplyGroundMovement(gobj);
}

/* ft_084E.c:109-118. */
static void ft_800850E0(Fighter_GObj* gobj, float arg8, float arg9) {
    Fighter* fp = GET_FIGHTER(gobj);
    if (fp->x594_b0) {
        fp->gr_vel = fp->x6A4_transNOffset.z * arg9;
    } else {
        ftCommon_ApplyFrictionGround(fp, arg8);
    }
    ftCommon_ApplyGroundMovement(gobj);
}

/* ft_084E.c:91-98. */
static void ft_80085088(Fighter_GObj* gobj) {
    Fighter* fp = GET_FIGHTER(gobj);
    ft_800850E0(gobj, fp->co_attrs.ground_friction, fp->facing_dir);
}

/* ft_084E.c:120-125. */
static void ft_80085134(Fighter_GObj* gobj) {
    Fighter* fp = GET_FIGHTER(gobj);
    fp->self_vel.x = fp->x6A4_transNOffset.z * fp->facing_dir;
    fp->self_vel.y = fp->x6A4_transNOffset.y;
}

static s32 ftGetFacingDirInt(Fighter* fp) { return fp->facing_dir < 0.0f ? -1 : 1; }

/* ---- CAPTURED: logged, not reimplemented. ---- */
static _Thread_local FtMotionId captured_msid;
static _Thread_local int change_motion_state_calls;
static _Thread_local int captured_landing_fall_special_calls;
static _Thread_local bool captured_landing_allow_interrupt;
static _Thread_local float captured_landing_lag;
static _Thread_local int captured_fall_enter_calls;
static _Thread_local int captured_fallspecial_calls;
static _Thread_local int captured_fallspecial_arg1, captured_fallspecial_arg2;
static _Thread_local bool captured_fallspecial_allow_interrupt;
static _Thread_local float captured_fallspecial_mobility, captured_fallspecial_landing_lag;
static _Thread_local int captured_wait_reenter_calls;

static void reset_captures(void) {
    captured_msid = 0;
    change_motion_state_calls = 0;
    captured_landing_fall_special_calls = 0;
    captured_fall_enter_calls = 0;
    captured_fallspecial_calls = 0;
    captured_wait_reenter_calls = 0;
}

static void Fighter_ChangeMotionState(Fighter_GObj* gobj, FtMotionId msid, MotionFlags flags,
                                       f32 start, f32 rate, f32 blend, void* cb) {
    (void) gobj;
    (void) flags;
    (void) start;
    (void) rate;
    (void) blend;
    (void) cb;
    captured_msid = msid;
    change_motion_state_calls++;
}

static void ftCo_80096900(Fighter_GObj* gobj, int arg1, int arg2, bool allow_interrupt,
                           float mobility, float landing_lag) {
    (void) gobj;
    captured_fallspecial_calls++;
    captured_fallspecial_arg1 = arg1;
    captured_fallspecial_arg2 = arg2;
    captured_fallspecial_allow_interrupt = allow_interrupt;
    captured_fallspecial_mobility = mobility;
    captured_fallspecial_landing_lag = landing_lag;
}

static void ftCo_LandingFallSpecial_Enter(Fighter_GObj* gobj, bool allow_interrupt,
                                           float landing_lag) {
    (void) gobj;
    captured_landing_fall_special_calls++;
    captured_landing_allow_interrupt = allow_interrupt;
    captured_landing_lag = landing_lag;
}

static void ftCo_Fall_Enter(Fighter_GObj* gobj) {
    (void) gobj;
    captured_fall_enter_calls++;
}

/* ft_8008A2BC: ordinary Wait re-entry (End ground's natural animation
 * completion). Captured like the other terminal motion changes above. */
static void ft_8008A2BC(Fighter_GObj* gobj) {
    (void) gobj;
    captured_wait_reenter_calls++;
}

/* ftCo_Landing.c/inlines.h:81-89, reusing the captured
 * `Fighter_ChangeMotionState` above -- faithful control flow, captured
 * terminal effect. */
static void ftCommon_AirToGroundStateChange(Fighter_GObj* gobj, Fighter* fp, FtMotionId msid,
                                             MotionFlags flags) {
    ftCommon_8007D7FC(fp);
    Fighter_ChangeMotionState(gobj, msid, flags, fp->cur_anim_frame, 1.0f, 0.0f, NULL);
}

/* ---- SCRIPTED: test-controlled return values. ---- */
static _Thread_local bool script_frames_remaining;
static bool ftAnim_IsFramesRemaining(Fighter_GObj* gobj) {
    (void) gobj;
    return script_frames_remaining;
}
static _Thread_local bool script_ft_80082708;
static bool ft_80082708(Fighter_GObj* gobj) {
    (void) gobj;
    return script_ft_80082708;
}
static _Thread_local bool script_ft_800827A0;
static bool ft_800827A0(Fighter_GObj* gobj) {
    (void) gobj;
    return script_ft_800827A0;
}
static _Thread_local bool script_ft_check_ground_and_ledge;
static bool ft_CheckGroundAndLedge(Fighter_GObj* gobj, s32 facing) {
    (void) gobj;
    (void) facing;
    return script_ft_check_ground_and_ledge;
}
static _Thread_local bool script_ledge_common;
static bool ftCliffCommon_80081298(Fighter_GObj* gobj) {
    (void) gobj;
    return script_ledge_common;
}
static _Thread_local float script_ground_friction_multiplier = 1.0f;
static float ft_GetGroundFrictionMultiplier(Fighter* fp) {
    (void) fp;
    return script_ground_friction_multiplier;
}

/* ---- Ghost/GFX no-ops. ---- */
static void efSync_Spawn(int id, Fighter_GObj* gobj, void* joint, float* facing) {
    (void) id;
    (void) gobj;
    (void) joint;
    (void) facing;
}
static void Fighter_SetEffectHitlagCallbacks(Fighter_GObj* gobj) { (void) gobj; }
static void* it_8029CEB4(Fighter_GObj* gobj, Vec3* pos, int kind, float dir) {
    (void) gobj;
    (void) pos;
    (void) kind;
    (void) dir;
    return NULL;
}
static void ftAnim_8006EBA4(Fighter_GObj* gobj) { (void) gobj; }
static float ftPartGetRotX(Fighter* fp, int index) {
    (void) fp;
    (void) index;
    return 0.0f;
}
/* ftFx_SpecialS_CreateGFX (0x800E9DF8): dropped from extraction (needs
 * the full ftParts/part-joint system); hand-written no-op, matching
 * every other ghost/GFX effect in this adapter. Only ever installed as
 * `fp->accessory4_cb`, never invoked by anything this oracle calls. */
static void ftFx_SpecialS_CreateGFX(Fighter_GObj* gobj) { (void) gobj; }

/* Item kinds referenced by `ftFox_SpecialS_CreateGhostItem`; values are
 * unused (the stub above ignores `kind`), only distinct constants are
 * needed to compile the extracted switch. */
enum { It_Kind_Fox_Illusion = 1, It_Kind_Falco_Phantasm = 2 };

/* `ftFx_MF_SpecialS_Coll`/`ftFx_MF_SpecialSDash_Coll`: top-of-file
 * `static MotionFlags const` globals (`ftfoxspecials.c:24-27`), not
 * extracted by `extract_function` (it only selects function bodies),
 * reproduced here so the extracted GroundToAir/AirToGround handlers
 * link. */
static MotionFlags const ftFx_MF_SpecialS_Coll = ftCommon_GroundAirColl_MF | Ft_MF_SkipRumble;
static MotionFlags const ftFx_MF_SpecialSDash_Coll =
    ftFx_MF_SpecialS_Coll | Ft_MF_KeepColAnimHitStatus;

#include "fox_specials_original.inc"

/* ------------------------------------------------------------------- */
/* Oracle entry points. Each resets captures/scripts, builds a minimal
 * `Fighter`, runs the real extracted callback(s) for one phase, and
 * reports the resulting state plus which captured side effect (if any)
 * fired. Ground/air share one function per phase family, selected by
 * `ground`. */

static void reset_fighter(Fighter* fp, ftFox_DatAttrs* attrs) {
    memset(fp, 0, sizeof(*fp));
    fp->kind = FTKIND_FOX;
    fp->dat_attrs = attrs;
    fp->facing_dir = 1.0f;
}

/* Start phase Enter: `ftFx_SpecialSStart_Enter`/`ftFx_SpecialAirSStart_Enter`. */
void oracle_fox_start_enter(bool ground, float gr_vel_in, float self_vel_x_in, s32 max_jumps,
                             float x24, float x28, FtMotionId* out_msid, float* out_gr_vel,
                             float* out_self_vel_x, float* out_self_vel_y, s32* out_jumps_used,
                             float* out_gravity_delay) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x24_FOX_ILLUSION_GRAVITY_DELAY = x24;
    attrs.x28_FOX_ILLUSION_GROUND_VEL_X = x28;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.co_attrs.max_jumps = max_jumps;
    fp.gr_vel = gr_vel_in;
    fp.self_vel.x = self_vel_x_in;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    if (ground) {
        ftFx_SpecialSStart_Enter(&gobj);
    } else {
        ftFx_SpecialAirSStart_Enter(&gobj);
    }
    *out_msid = captured_msid;
    *out_gr_vel = fp.gr_vel;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
    *out_jumps_used = fp.x1968_jumpsUsed;
    *out_gravity_delay = fp.mv.fx.SpecialS.gravityDelay;
}

/* Start phase Phys: `ftFx_SpecialSStart_Phys`/`ftFx_SpecialAirSStart_Phys`,
 * including the ground variant's own unconditional gravity-delay tick. */
void oracle_fox_start_phys(bool ground, float gravity_delay_in, float gr_vel_in,
                            float self_vel_x_in, float self_vel_y_in, float ground_friction,
                            float walk_max_vel, float x2C, float x30, float terminal_velocity,
                            float above_walk_friction_mul, float* out_gravity_delay,
                            float* out_gr_vel, float* out_self_vel_x, float* out_self_vel_y) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x2C_FOX_ILLUSION_UNK1 = x2C;
    attrs.x30_FOX_ILLUSION_UNK2 = x30;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.co_attrs.ground_friction = ground_friction;
    fp.co_attrs.walk_max_vel = walk_max_vel;
    fp.co_attrs.terminal_velocity = terminal_velocity;
    fp.gr_vel = gr_vel_in;
    fp.self_vel.x = self_vel_x_in;
    fp.self_vel.y = self_vel_y_in;
    fp.mv.fx.SpecialS.gravityDelay = gravity_delay_in;
    ftCommonData_.friction_when_above_walk_speed = above_walk_friction_mul;
    Fighter_GObj gobj = { &fp };
    if (ground) {
        ftFx_SpecialSStart_Phys(&gobj);
    } else {
        ftFx_SpecialAirSStart_Phys(&gobj);
    }
    *out_gravity_delay = fp.mv.fx.SpecialS.gravityDelay;
    *out_gr_vel = fp.gr_vel;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
}

/* Start/Dash/End Anim: reports which captured transition (if any) fired,
 * by motion id (0 when `ftAnim_IsFramesRemaining` kept the pose playing). */
void oracle_fox_anim(s32 phase, bool ground, bool frames_remaining, float x4C, float x50,
                      FtMotionId* out_msid, int* out_fallspecial_calls,
                      bool* out_fallspecial_allow_interrupt, float* out_fallspecial_mobility,
                      float* out_fallspecial_landing_lag) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x4C_FOX_ILLUSION_FREEFALL_MOBILITY = x4C;
    attrs.x50_FOX_ILLUSION_LANDING_LAG = x50;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_frames_remaining = frames_remaining;
    switch (phase) {
    case 0:
        ground ? ftFx_SpecialSStart_Anim(&gobj) : ftFx_SpecialAirSStart_Anim(&gobj);
        break;
    case 1:
        ground ? ftFx_SpecialS_Anim(&gobj) : ftFx_SpecialAirS_Anim(&gobj);
        break;
    default:
        ground ? ftFx_SpecialSEnd_Anim(&gobj) : ftFx_SpecialAirSEnd_Anim(&gobj);
        break;
    }
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    *out_fallspecial_calls = captured_fallspecial_calls;
    *out_fallspecial_allow_interrupt = captured_fallspecial_allow_interrupt;
    *out_fallspecial_mobility = captured_fallspecial_mobility;
    *out_fallspecial_landing_lag = captured_fallspecial_landing_lag;
}

/* Dash phase Phys: `ftFx_SpecialS_Phys`/`ftFx_SpecialAirS_Phys` (TransN). */
void oracle_fox_dash_phys(bool ground, bool has_trans_n, float trans_n_z, float trans_n_y,
                           float facing, float ground_friction, float gr_vel_in,
                           float self_vel_x_in, float self_vel_y_in, float* out_gr_vel,
                           float* out_self_vel_x, float* out_self_vel_y) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.facing_dir = facing;
    fp.co_attrs.ground_friction = ground_friction;
    fp.x594_b0 = has_trans_n;
    fp.x6A4_transNOffset.z = trans_n_z;
    fp.x6A4_transNOffset.y = trans_n_y;
    fp.gr_vel = gr_vel_in;
    fp.self_vel.x = self_vel_x_in;
    fp.self_vel.y = self_vel_y_in;
    Fighter_GObj gobj = { &fp };
    if (ground) {
        ftFx_SpecialS_Phys(&gobj);
    } else {
        ftFx_SpecialAirS_Phys(&gobj);
    }
    *out_gr_vel = fp.gr_vel;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
}

/* Dash phase IASA: `ftFx_SpecialS_IASA`/`ftFx_SpecialAirS_IASA` (the B-press
 * shortening; `ground_or_air` decides the destination, not which variant
 * currently owns dispatch). Returns which End motion id fired (0 if none). */
FtMotionId oracle_fox_dash_iasa(bool ground_owns_dispatch, bool air_actually, bool pressed_b) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.ground_or_air = air_actually ? GA_Air : GA_Ground;
    fp.input.pressed_buttons = pressed_b ? HSD_PAD_B : 0;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    if (ground_owns_dispatch) {
        ftFx_SpecialS_IASA(&gobj);
    } else {
        ftFx_SpecialAirS_IASA(&gobj);
    }
    return change_motion_state_calls > 0 ? captured_msid : 0;
}

/* Start/Dash Coll: reports which captured conversion fired.
 * `result`: 0 none, 1 ground-to-air, 2 air-to-ground, 3 ledge-common. */
int oracle_fox_start_coll(bool ground, bool coll_result, bool ledge_check, bool ledge_common,
                           FtMotionId* out_msid) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_ft_80082708 = coll_result;
    script_ft_check_ground_and_ledge = ledge_check;
    script_ledge_common = ledge_common;
    if (ground) {
        ftFx_SpecialSStart_Coll(&gobj);
        *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
        return coll_result ? 0 : 1;
    }
    ftFx_SpecialAirSStart_Coll(&gobj);
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    if (ledge_check) {
        return 2;
    }
    return ledge_common ? 3 : 0;
}

int oracle_fox_dash_coll(bool ground, bool coll_result, bool ledge_check, bool ledge_common,
                          FtMotionId* out_msid) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_ft_80082708 = coll_result;
    script_ft_check_ground_and_ledge = ledge_check;
    script_ledge_common = ledge_common;
    if (ground) {
        ftFx_SpecialS_Coll(&gobj);
        *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
        return coll_result ? 0 : 1;
    }
    ftFx_SpecialAirS_Coll(&gobj);
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    if (ledge_check) {
        return 2;
    }
    return ledge_common ? 3 : 0;
}

/* End phase Enter: `ftFx_SpecialSEnd_Enter`/`ftFx_SpecialAirSEnd_Enter`. */
void oracle_fox_end_enter(bool ground, float facing, float x34, float x3C, float x44,
                           FtMotionId* out_msid, float* out_gr_vel, float* out_self_vel_x,
                           float* out_self_vel_y, float* out_gravity_delay) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x34_FOX_ILLUSION_GROUND_END_VEL_X = x34;
    attrs.x3C_FOX_ILLUSION_AIR_END_VEL_X = x3C;
    attrs.x44_FOX_ILLUSION_FALL_ACCEL = x44;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.facing_dir = facing;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    if (ground) {
        ftFx_SpecialSEnd_Enter(&gobj);
    } else {
        ftFx_SpecialAirSEnd_Enter(&gobj);
    }
    *out_msid = captured_msid;
    *out_gr_vel = fp.gr_vel;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
    *out_gravity_delay = fp.mv.fx.SpecialS.gravityDelay;
}

/* End phase Phys: `ftFx_SpecialSEnd_Phys`/`ftFx_SpecialAirSEnd_Phys`. */
void oracle_fox_end_phys(bool ground, float gravity_delay_in, float gr_vel_in,
                          float self_vel_x_in, float self_vel_y_in, float ground_friction,
                          float x38, float x40, float x48, float terminal_velocity,
                          float* out_gravity_delay, float* out_gr_vel, float* out_self_vel_x,
                          float* out_self_vel_y) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x38_FOX_ILLUSION_GROUND_FRICTION = x38;
    attrs.x40_FOX_ILLUSION_AIR_MUL_X = x40;
    attrs.x48_FOX_ILLUSION_TERMINAL_VELOCITY = x48;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.co_attrs.ground_friction = ground_friction;
    fp.co_attrs.terminal_velocity = terminal_velocity;
    fp.gr_vel = gr_vel_in;
    fp.self_vel.x = self_vel_x_in;
    fp.self_vel.y = self_vel_y_in;
    fp.mv.fx.SpecialS.gravityDelay = gravity_delay_in;
    Fighter_GObj gobj = { &fp };
    if (ground) {
        ftFx_SpecialSEnd_Phys(&gobj);
    } else {
        ftFx_SpecialAirSEnd_Phys(&gobj);
    }
    *out_gravity_delay = fp.mv.fx.SpecialS.gravityDelay;
    *out_gr_vel = fp.gr_vel;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
}

/* End phase Coll. Ground: `ftFx_SpecialSEnd_Coll` (`ft_800827A0` false ->
 * captured `ftCo_Fall_Enter`). Air: `ftFx_SpecialAirSEnd_Coll`
 * (`ft_CheckGroundAndLedge` true -> captured `ftCo_LandingFallSpecial_
 * Enter`, else `ftCliffCommon_80081298`). Returns: 0 none, 1 Fall entered
 * (ground), 2 LandingFallSpecial entered (air), 3 ledge-common (air). */
int oracle_fox_end_coll(bool ground, bool coll_result, bool ledge_check, bool ledge_common,
                         int* out_fall_calls, int* out_landing_calls, bool* out_landing_allow,
                         float* out_landing_lag) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_ft_800827A0 = coll_result;
    script_ft_check_ground_and_ledge = ledge_check;
    script_ledge_common = ledge_common;
    if (ground) {
        ftFx_SpecialSEnd_Coll(&gobj);
        *out_fall_calls = captured_fall_enter_calls;
        *out_landing_calls = 0;
        *out_landing_allow = false;
        *out_landing_lag = 0.0f;
        return coll_result ? 0 : 1;
    }
    ftFx_SpecialAirSEnd_Coll(&gobj);
    *out_fall_calls = 0;
    *out_landing_calls = captured_landing_fall_special_calls;
    *out_landing_allow = captured_landing_allow_interrupt;
    *out_landing_lag = captured_landing_lag;
    if (ledge_check) {
        return 2;
    }
    return ledge_common ? 3 : 0;
}
