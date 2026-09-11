/* Host adapter for Fox/Falco's up special (`ftfoxspecialhi.c`,
 * `ftfoxspecialhi.functions.json`: every non-static callback plus the
 * three static/static-inline helpers (`ftFox_SpecialHi_RotateModel`,
 * `ftFox_SpecialHi_IsBound`, `ftFox_SpecialHiBound_SetVars`) -- only the
 * two GFX-only `_CreateChargeGFX`/`_CreateLaunchGFX` callbacks are dropped
 * (they need the full `ftParts` bone-joint system; hand-written no-ops
 * below, matching `fox_specials.c`'s own precedent for its one skipped
 * GFX helper).
 *
 * Two dependencies live in OTHER pinned files and are reused via
 * `adapters.json` aliases rather than re-snapshotted: `lbVector_AngleXY`
 * (`up_special_angle` -> the existing `lbvector` snapshot) and
 * `ftCo_8009A134` (`up_special_platform` -> the existing `pass` snapshot,
 * the platform-skip check `ftFox_SpecialHi_IsBound`/the grounded launch
 * decision both call). Both `.inc`s are included before
 * `ftfoxspecialhi_original.inc` so their symbols are defined when the
 * pinned body calls them.
 *
 * Dependency treatment, as briefed:
 *  - CAPTURED (logged, not implemented): `Fighter_ChangeMotionState`
 *    (also records the `start` frame argument, needed for the frame-13
 *    Fall->Landing regression), `ftCo_80096900` (FallSpecial),
 *    `ft_8008A2BC` (Wait re-entry), `ftCo_Fall_Enter`, `ft_80084DB0`
 *    (Fall's own "ordinary air" Phys -- the shared aerial gravity/fast-
 *    fall pipeline this move does not override, so only dispatch is
 *    confirmed, not its arithmetic), `ftCommon_8007CF58` (Bound air
 *    drift clamp -- lives in `ftcommon.c`, out of this batch's pinned
 *    scope like `ft_80084DB0`; `up.rs`'s own `Movement::drift_clamp` is
 *    exercised by native tests instead), `mpUpdateFloorSkip` (the
 *    platform-skip side effect inside `ftCo_8009A134`).
 *  - SCRIPTED (test-controlled return): `ftAnim_IsFramesRemaining`,
 *    `ft_80082708`, `ft_800827A0`, `ft_CheckGroundAndLedge`,
 *    `ftCliffCommon_80081298`, `mpColl_IsOnPlatform`.
 *  - FAITHFUL (real arithmetic, reproduced verbatim from the pinned
 *    source below each definition's own citation, against this file's
 *    own self-contained `Fighter` struct rather than linked against
 *    another adapter's independent layout -- see `fox_specials.c`'s own
 *    header for why): `ftCommon_Fall`, `ftCommon_ApplyFrictionAir`,
 *    `ftCommon_ApplyFrictionGround`, `ftCommon_UpdateFacing`,
 *    `ftCommon_8007D60C`, `ftCommon_8007D6A4`, `ftCommon_8007D7FC`,
 *    `ftCommon_8007DB24`, `ftCommon_ClampSelfVelX`,
 *    `ftCommon_ClampAirDrift`, `ftCommon_AirToGroundStateChange`,
 *    `ft_80084F3C`, `ft_800851C0`, `ft_80084104`, `stickGetDir`,
 *    `getFtSpecialAttrs`, `getFtColl`, `ftGetGroundAir`,
 *    `ftGetFacingDirInt`.
 *  - Ground `ftCommon_ApplyGroundMovement` is a documented no-op, exactly
 *    like `fox_specials.c`'s own treatment: the real function projects
 *    onto the floor normal through the full map-collision system this
 *    oracle does not model, and every value compared here is already
 *    finalized before this call.
 *  - Ghost/GFX no-ops: `efSync_Spawn`, `Fighter_SetEffectHitlagCallbacks`,
 *    `ftAnim_8006EBA4`, `ftPartSetRotX`, `ftParts_GetBoneIndex`,
 *    `ftFx_SpecialHi_CreateChargeGFX`, `ftFx_SpecialHi_CreateLaunchGFX`.
 *    The visual model-rotation bone stays unmodeled elsewhere
 *    (`docs/fox-up-special.md`); this adapter only confirms
 *    `rotateModel` itself, not the bone write.
 *
 * Thread-local state isolates independent concurrent test calls, not
 * gameplay global state, matching every other adapter in this directory. */
#include <math.h>
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

#define GA_Ground 0
#define GA_Air 1
/* `ftfoxspecialhi.c`'s own top-of-file macros (not function bodies, so
 * `extract_function` does not pull them) and `forward.h:251`. */
#define HALF_PI32 (1.5707963705062866f)
#define CLIFFCATCH_BOTH 0
typedef float vf32;

typedef struct { float x, y, z; } Vec3;
typedef struct { Vec3 normal; } Surface;

#define Collide_LeftWallMask 0x3Fu
#define Collide_RightWallMask 0xFC0u
#define Collide_CeilingMask 0x6000u
#define Collide_FloorMask 0x18000u

typedef struct {
    u32 env_flags;
    Surface floor;
    Surface ceiling;
    Surface left_facing_wall;
    Surface right_facing_wall;
} CollData;

enum {
    ftFx_MS_SpecialHiHold = 2000,
    ftFx_MS_SpecialHiHoldAir,
    ftFx_MS_SpecialHi,
    ftFx_MS_SpecialAirHi,
    ftFx_MS_SpecialHiLanding,
    ftFx_MS_SpecialHiFall,
    ftFx_MS_SpecialHiBound,
};

/* `ftFox/types.h:109-132`, x54..x94. */
typedef struct {
    float x54_FOX_FIREFOX_GRAVITY_DELAY;
    float x58_FOX_FIREFOX_VEL_X;
    float x5C_FOX_FIREFOX_AIR_MOMENTUM_PRESERVE_X;
    float x60_FOX_FIREFOX_FALL_ACCEL;
    float x64_FOX_FIREFOX_DIRECTION_STICK_RANGE_MIN;
    float x68_FOX_FIREFOX_DURATION;
    s32 x6C_FOX_FIREFOX_BOUNCE_VAR;
    float x70_FOX_FIREFOX_DURATION_END;
    float x74_FOX_FIREFOX_SPEED;
    float x78_FOX_FIREFOX_REVERSE_ACCEL;
    float x7C_FOX_FIREFOX_GROUND_MOMENTUM_END;
    float x80_FOX_FIREFOX_UNK2;
    float x84_FOX_FIREFOX_BOUND_VEL_X;
    float x88_FOX_FIREFOX_FACING_STICK_RANGE_MIN;
    float x8C_FOX_FIREFOX_FREEFALL_MOBILITY;
    float x90_FOX_FIREFOX_LANDING_LAG;
    float x94_FOX_FIREFOX_BOUND_ANGLE;
} ftFox_DatAttrs;

typedef struct {
    float ground_friction;
    float walk_max_vel;
    float terminal_velocity;
    float air_drift_max;
    s32 max_jumps;
} ftCo_DatAttrs;

typedef struct {
    float gravityDelay;
    float rotateModel;
    float travelFrames;
    float unk;
    float unk2;
} SpecialHiMoveVar;
typedef struct { SpecialHiMoveVar SpecialHi; } MvFx;
typedef struct { MvFx fx; } Mv;

typedef struct { Vec3 lstick[1]; } FighterInput;

typedef struct Fighter_GObj_s Fighter_GObj;

typedef struct Fighter {
    FighterInput input;
    ftFox_DatAttrs* dat_attrs;
    ftCo_DatAttrs co_attrs;
    GroundOrAir ground_or_air;
    float gr_vel;
    Vec3 self_vel;
    float facing_dir;
    Vec3 cur_pos;
    Vec3 x6A4_transNOffset;
    float cur_anim_frame;
    u8 x1968_jumpsUsed;
    bool x2219_b0;
    bool x2223_b4;
    CollData coll_data;
    s32 cmd_vars[8];
    void (*accessory4_cb)(Fighter_GObj*);
    void (*x21F8)(Fighter_GObj*);
    Mv mv;
} Fighter;
struct Fighter_GObj_s { Fighter* user_data; };
typedef Fighter_GObj HSD_GObj;
#define GET_FIGHTER(gobj) ((gobj)->user_data)
static Fighter* getFighter(Fighter_GObj* gobj) { return GET_FIGHTER(gobj); }
static ftFox_DatAttrs* getFtSpecialAttrs(Fighter* fp) { return fp->dat_attrs; }
static CollData* getFtColl(Fighter* fp) { return &fp->coll_data; }
static bool ftGetGroundAir(Fighter* fp) { return fp->ground_or_air; }
static s32 ftGetFacingDirInt(Fighter* fp) { return fp->facing_dir < 0.0f ? -1 : 1; }

/* ft/inlines.h:123-130. */
static float stickGetDir(float x1, float x2) { return x1 < x2 ? -x1 : x1; }

/* `FTFOX_SPECIALHI_COLL_FLAG` (top-of-file macro, not a function body, so
 * `extract_function` does not pull it); flag bit identity is not asserted
 * on by this differential suite (only the destination msid is), so
 * arbitrary distinct values are sufficient to link. */
enum {
    Ft_MF_KeepGfx = 1 << 0,
    Ft_MF_SkipMatAnim = 1 << 1,
    Ft_MF_UpdateCmd = 1 << 2,
    Ft_MF_SkipColAnim = 1 << 3,
    Ft_MF_SkipItemVis = 1 << 4,
    Ft_MF_Unk19 = 1 << 5,
    Ft_MF_SkipModelPartVis = 1 << 6,
    Ft_MF_SkipModelFlags = 1 << 7,
    Ft_MF_Unk27 = 1 << 8,
    Ft_MF_SkipHit = 1 << 9,
};
#define FTFOX_SPECIALHI_COLL_FLAG                                            \
    (Ft_MF_KeepGfx | Ft_MF_SkipMatAnim | Ft_MF_UpdateCmd | Ft_MF_SkipColAnim | \
     Ft_MF_SkipItemVis | Ft_MF_Unk19 | Ft_MF_SkipModelPartVis |               \
     Ft_MF_SkipModelFlags | Ft_MF_Unk27)

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

/* ftcommon.c:621-629. */
static void ftCommon_UpdateFacing(Fighter* fp) {
    fp->facing_dir = fp->input.lstick[0].x >= 0.0f ? 1.0f : -1.0f;
}

/* ftcommon.c:527-539. */
static void ftCommon_8007D60C(Fighter* fp) {
    fp->ground_or_air = GA_Air;
    fp->gr_vel = 0.0f;
    fp->x1968_jumpsUsed = (u8) fp->co_attrs.max_jumps;
}

/* ftcommon.c:546-557; the dead `x594_b0`/ground-speed-clamp branches this
 * move never sets are elided (the clamp is immediately overwritten by the
 * unconditional `gr_vel = self_vel.x` two lines later in the real body, so
 * omitting it changes no observable output). */
static void ftCommon_8007D6A4(Fighter* fp) {
    fp->ground_or_air = GA_Ground;
    fp->gr_vel = fp->self_vel.x;
    fp->x1968_jumpsUsed = 0;
}

/* ftcommon.c:581-594; the knockback/bonus-stat branch is unmodeled
 * elsewhere and has no bearing on the compared arithmetic. */
static void ftCommon_8007D7FC(Fighter* fp) { ftCommon_8007D6A4(fp); }

/* ftcommon.c:650-655; `efLib_DestroyAll`'s GFX teardown is a no-op here. */
static void ftCommon_8007DB24(Fighter_GObj* gobj) {
    Fighter* fp = GET_FIGHTER(gobj);
    fp->x2219_b0 = false;
}

/* ftcommon.c:447-455. */
static void ftCommon_ClampSelfVelX(Fighter* fp, float max) {
    if (fp->self_vel.x < -max) {
        fp->self_vel.x = -max;
    } else if (fp->self_vel.x > max) {
        fp->self_vel.x = max;
    }
}

/* ftcommon.c:457-460. */
static void ftCommon_ClampAirDrift(Fighter* fp) {
    ftCommon_ClampSelfVelX(fp, fp->co_attrs.air_drift_max);
}

static _Thread_local FtMotionId captured_msid;
static _Thread_local float captured_start;
static _Thread_local int change_motion_state_calls;
static void Fighter_ChangeMotionState(Fighter_GObj* gobj, FtMotionId msid, MotionFlags flags,
                                       f32 start, f32 rate, f32 blend, void* cb) {
    (void) gobj;
    (void) flags;
    (void) rate;
    (void) blend;
    (void) cb;
    captured_msid = msid;
    captured_start = start;
    change_motion_state_calls++;
}

/* ftCo_Landing.c/inlines.h:81-89, faithful control flow with the captured
 * terminal effect above. */
static void ftCommon_AirToGroundStateChange(Fighter_GObj* gobj, Fighter* fp, FtMotionId msid,
                                             MotionFlags flags) {
    ftCommon_8007D7FC(fp);
    Fighter_ChangeMotionState(gobj, msid, flags, fp->cur_anim_frame, 1.0f, 0.0f, NULL);
}

/* ft_084E.c:42-53. */
typedef struct { float friction_when_above_walk_speed; } FtCommonData;
/* Thread-local, matching every other piece of mutable state this adapter
 * owns (see the `script_*`/`*_calls` globals above) and the same fix
 * `ftfoxspeciallw.c`'s own `ftCommonData_` needed for its per-call
 * `x1FC` (`3b56a66`): this constant is never written after static
 * initialization here, so the plain-global version could not actually
 * race across threads today, but keeping the name/shape identical to a
 * struct that could someday gain a per-call field -- and matching the
 * established convention -- is cheaper than relying on that being true
 * forever. `p_ftCommonData` is a macro, not a plain pointer, for the same
 * reason as the down-special adapter's own: a real pointer captured at
 * static-init time would only ever resolve to the initializing thread's
 * TLS instance. */
static _Thread_local FtCommonData ftCommonData_ = { 1.0f };
#define p_ftCommonData (&ftCommonData_)
static void ft_80084F3C(Fighter_GObj* gobj) {
    Fighter* fp = GET_FIGHTER(gobj);
    float friction = fp->co_attrs.ground_friction;
    if ((fp->gr_vel < 0 ? -fp->gr_vel : fp->gr_vel) > fp->co_attrs.walk_max_vel) {
        friction *= p_ftCommonData->friction_when_above_walk_speed;
    }
    ftCommon_ApplyFrictionGround(fp, friction);
    ftCommon_ApplyGroundMovement(gobj);
}

/* ft_084E.c:138-142. */
static void ft_800851C0(Fighter_GObj* gobj) {
    Fighter* fp = GET_FIGHTER(gobj);
    fp->self_vel.y = fp->x6A4_transNOffset.y;
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
static _Thread_local bool script_check_ground_and_ledge;
static bool ft_CheckGroundAndLedge(Fighter_GObj* gobj, s32 facing) {
    (void) gobj;
    (void) facing;
    return script_check_ground_and_ledge;
}
static _Thread_local bool script_ledge_common;
static bool ftCliffCommon_80081298(Fighter_GObj* gobj) {
    (void) gobj;
    return script_ledge_common;
}
static _Thread_local bool script_on_platform;
static bool mpColl_IsOnPlatform(CollData* coll) {
    (void) coll;
    return script_on_platform;
}
static _Thread_local int floor_skip_calls;
static void mpUpdateFloorSkip(CollData* coll) {
    (void) coll;
    floor_skip_calls++;
}

/* `ft_80084104` (`ft_081B.c:1043-1049`): faithful three-line body against
 * the scripted/captured pieces it calls, both already established above. */
static void ftCo_Fall_Enter(Fighter_GObj* gobj);
static void ft_80084104(Fighter_GObj* gobj) {
    bool skip = ft_800827A0(gobj);
    if (!skip) {
        ftCo_Fall_Enter(gobj);
    }
}

/* ---- CAPTURED: logged, not reimplemented. ---- */
static _Thread_local int fallspecial_calls;
static _Thread_local bool fallspecial_allow_interrupt;
static _Thread_local float fallspecial_mobility, fallspecial_landing_lag;
static _Thread_local int wait_reenter_calls;
static _Thread_local int fall_enter_calls;
static _Thread_local int ft_80084db0_calls;
static _Thread_local int drift_clamp_calls;
/* `ftFox_SpecialHi_RotateModel` is itself pinned (extracted verbatim), so
 * it cannot be instrumented directly; it unconditionally calls
 * `ftPartSetRotX` every time it runs, which this adapter does own, so
 * counting calls there reliably detects "did RotateModel run" -- in
 * particular disambiguating `ftFx_SpecialAirHi_Coll`'s `facingDir` redirect
 * from doing nothing, which a before/after comparison of `facing_dir`/
 * `rotateModel` cannot do for every input (both can legitimately
 * recompute to their prior value). */
static _Thread_local int rotate_model_calls;

static void reset_captures(void) {
    captured_msid = 0;
    captured_start = 0.0f;
    change_motion_state_calls = 0;
    fallspecial_calls = 0;
    wait_reenter_calls = 0;
    fall_enter_calls = 0;
    floor_skip_calls = 0;
    ft_80084db0_calls = 0;
    drift_clamp_calls = 0;
    rotate_model_calls = 0;
}

static void ftCo_80096900(Fighter_GObj* gobj, int arg1, int arg2, bool allow_interrupt,
                           float mobility, float landing_lag) {
    (void) gobj;
    (void) arg1;
    (void) arg2;
    fallspecial_calls++;
    fallspecial_allow_interrupt = allow_interrupt;
    fallspecial_mobility = mobility;
    fallspecial_landing_lag = landing_lag;
}
static void ft_8008A2BC(Fighter_GObj* gobj) {
    (void) gobj;
    wait_reenter_calls++;
}
static void ftCo_Fall_Enter(Fighter_GObj* gobj) {
    (void) gobj;
    fall_enter_calls++;
}
static void ft_80084DB0(Fighter_GObj* gobj) {
    (void) gobj;
    ft_80084db0_calls++;
}
/* `ftcommon.c:283+`, out of this batch's pinned scope (see the file
 * header); captures dispatch only. */
static bool ftCommon_8007CF58(Fighter* fp) {
    (void) fp;
    drift_clamp_calls++;
    return false;
}

/* ---- Ghost/GFX no-ops. ---- */
static void efSync_Spawn(int id, Fighter_GObj* gobj, void* pos, void* dir) {
    (void) id;
    (void) gobj;
    (void) pos;
    (void) dir;
}
static void Fighter_SetEffectHitlagCallbacks(Fighter* fp) { (void) fp; }
static void ftAnim_8006EBA4(Fighter_GObj* gobj) { (void) gobj; }
static void ftPartSetRotX(Fighter* fp, int index, float angle) {
    (void) fp;
    (void) index;
    (void) angle;
    rotate_model_calls++;
}
enum { FtPart_XRotN = 0 };
static int ftParts_GetBoneIndex(Fighter* fp, int part) {
    (void) fp;
    (void) part;
    return 0;
}
/* GFX-only, dropped from extraction (need the full `ftParts` bone-joint
 * system); only ever installed as `fp->accessory4_cb`, matching
 * `fox_specials.c`'s identical treatment of its own `_CreateGFX`. */
static void ftFx_SpecialHi_CreateChargeGFX(Fighter_GObj* gobj) { (void) gobj; }
static void ftFx_SpecialHi_CreateLaunchGFX(Fighter_GObj* gobj) { (void) gobj; }

/* `ftcommon.c:1422-1427`: never called by anything in this file, only
 * assigned into `x21F8`; a distinct address lets the oracle confirm the
 * assignment happened without reimplementing its own body. */
static void ftCommon_8007F76C(Fighter_GObj* gobj) { (void) gobj; }

/* Forward declarations: the pinned bodies below call each other in an
 * order that does not match their concatenation order (`build.rs` joins
 * `ftfoxspecialhi.functions.json` in list order, which follows the
 * original file's own top-to-bottom layout for readability rather than a
 * dependency-safe order), so every pinned symbol gets a prototype here
 * regardless of where its own definition lands in the generated .inc. */
static void ftFox_SpecialHi_RotateModel(HSD_GObj* gobj);
static bool ftFox_SpecialHi_IsBound(HSD_GObj* gobj);
static void ftFox_SpecialHiBound_SetVars(HSD_GObj* gobj);
void ftFx_SpecialHi_Enter(HSD_GObj* gobj);
void ftFx_SpecialAirHiStart_Enter(HSD_GObj* gobj);
void ftFx_SpecialHiHold_Anim(HSD_GObj* gobj);
void ftFx_SpecialHiHoldAir_Anim(HSD_GObj* gobj);
void ftFx_SpecialHiHold_IASA(HSD_GObj* gobj);
void ftFx_SpecialHiHoldAir_IASA(HSD_GObj* gobj);
void ftFx_SpecialHiHold_Phys(HSD_GObj* gobj);
void ftFx_SpecialHiHoldAir_Phys(HSD_GObj* gobj);
void ftFx_SpecialHiHold_Coll(HSD_GObj* gobj);
void ftFx_SpecialHiHoldAir_Coll(HSD_GObj* gobj);
void ftFx_SpecialHiHold_GroundToAir(HSD_GObj* gobj);
void ftFx_SpecialHiHoldAir_AirToGround(HSD_GObj* gobj);
void ftFx_SpecialHi_Anim(HSD_GObj* gobj);
void ftFx_SpecialAirHi_Anim(HSD_GObj* gobj);
void ftFx_SpecialHi_IASA(HSD_GObj* gobj);
void ftFx_SpecialAirHi_IASA(HSD_GObj* gobj);
void ftFx_SpecialHi_Phys(HSD_GObj* gobj);
void ftFx_SpecialAirHi_Phys(HSD_GObj* gobj);
void ftFx_SpecialHi_Coll(HSD_GObj* gobj);
void ftFx_SpecialAirHi_Coll(HSD_GObj* gobj);
void ftFx_SpecialHi_GroundToAir(HSD_GObj* gobj);
void ftFx_SpecialAirHi_AirToGround(HSD_GObj* gobj);
void ftFx_SpecialAirHi_Enter(HSD_GObj* gobj);
void ftFx_SpecialHiLanding_Anim(HSD_GObj* gobj);
void ftFx_SpecialHiFall_Anim(HSD_GObj* gobj);
void ftFx_SpecialHiLanding_IASA(HSD_GObj* gobj);
void ftFx_SpecialHiFall_IASA(HSD_GObj* gobj);
void ftFx_SpecialHiLanding_Phys(HSD_GObj* gobj);
void ftFx_SpecialHiFall_Phys(HSD_GObj* gobj);
void ftFx_SpecialHiLanding_Coll(HSD_GObj* gobj);
void ftFx_SpecialHiFall_Coll(HSD_GObj* gobj);
void ftFx_SpecialHiFall_Enter(HSD_GObj* gobj);
void ftFx_SpecialHiFall_AirToGround(HSD_GObj* gobj);
void ftFx_SpecialHiLanding_GroundToAir(HSD_GObj* gobj);
void ftFx_SpecialHiBound_Anim(HSD_GObj* gobj);
void ftFx_SpecialHiBound_IASA(HSD_GObj* gobj);
void ftFx_SpecialHiBound_Phys(HSD_GObj* gobj);
void ftFx_SpecialHiBound_Coll(HSD_GObj* gobj);
void ftFx_SpecialHiBound_Enter(HSD_GObj* gobj);

/* `lbVector_AngleXY` (the "ground_launch"-style `up_special_angle` alias
 * of the existing `lbvector` snapshot) needs its own file-local
 * `lbVector_Len_xy_accurate` helper, static in the original file and not
 * itself selected; `sqrtf_accurate` is `sqrtf` in this decomp's own
 * `placeholder.h` (the non-PPC build `lbvector.c` actually includes), so
 * this reproduces the exact same arithmetic `ground_launch.c` already
 * relies on for the sibling `lbVector_Angle`. */
#define sqrtf_accurate sqrtf
static inline float lbVector_Len_xy_accurate(Vec3* vec) {
    return sqrtf_accurate(vec->x * vec->x + vec->y * vec->y);
}
#include "up_special_angle_original.inc"

/* `ftCo_8009A134` (the `up_special_platform` alias of the existing `pass`
 * snapshot): needs only the scripted/captured platform helpers above. */
#include "up_special_platform_original.inc"

#include "ftfoxspecialhi_original.inc"

/* ------------------------------------------------------------------- */
/* Oracle entry points. Each resets captures/scripts, builds a minimal
 * `Fighter`, runs the real extracted callback(s) for one phase, and
 * reports the resulting state plus which captured side effect (if any)
 * fired. Ground/air share one function per phase family, selected by
 * `ground`, matching `fox_specials.c`'s own convention. */

static void reset_fighter(Fighter* fp, ftFox_DatAttrs* attrs) {
    memset(fp, 0, sizeof(*fp));
    fp->dat_attrs = attrs;
    fp->facing_dir = 1.0f;
}

/* Hold Enter: `ftFx_SpecialHi_Enter`/`ftFx_SpecialAirHiStart_Enter`. */
void oracle_hold_enter(bool ground, float gr_vel_in, float self_vel_x_in, s32 max_jumps,
                        float x54, float x58, FtMotionId* out_msid, float* out_gr_vel,
                        float* out_self_vel_x, float* out_self_vel_y,
                        float* out_gravity_delay) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x54_FOX_FIREFOX_GRAVITY_DELAY = x54;
    attrs.x58_FOX_FIREFOX_VEL_X = x58;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.co_attrs.max_jumps = max_jumps;
    fp.gr_vel = gr_vel_in;
    fp.self_vel.x = self_vel_x_in;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    if (ground) {
        ftFx_SpecialHi_Enter(&gobj);
    } else {
        ftFx_SpecialAirHiStart_Enter(&gobj);
    }
    *out_msid = captured_msid;
    *out_gr_vel = fp.gr_vel;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
    *out_gravity_delay = fp.mv.fx.SpecialHi.gravityDelay;
}

/* Hold Phys: `ftFx_SpecialHiHold_Phys`/`ftFx_SpecialHiHoldAir_Phys`. */
void oracle_hold_phys(bool ground, float gravity_delay_in, float gr_vel_in,
                       float self_vel_x_in, float self_vel_y_in, float ground_friction,
                       float walk_max_vel, float x5c, float x60, float terminal_velocity,
                       float* out_gravity_delay, float* out_gr_vel, float* out_self_vel_x,
                       float* out_self_vel_y) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x5C_FOX_FIREFOX_AIR_MOMENTUM_PRESERVE_X = x5c;
    attrs.x60_FOX_FIREFOX_FALL_ACCEL = x60;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.co_attrs.ground_friction = ground_friction;
    fp.co_attrs.walk_max_vel = walk_max_vel;
    fp.co_attrs.terminal_velocity = terminal_velocity;
    fp.gr_vel = gr_vel_in;
    fp.self_vel.x = self_vel_x_in;
    fp.self_vel.y = self_vel_y_in;
    fp.mv.fx.SpecialHi.gravityDelay = gravity_delay_in;
    Fighter_GObj gobj = { &fp };
    if (ground) {
        ftFx_SpecialHiHold_Phys(&gobj);
    } else {
        ftFx_SpecialHiHoldAir_Phys(&gobj);
    }
    *out_gravity_delay = fp.mv.fx.SpecialHi.gravityDelay;
    *out_gr_vel = fp.gr_vel;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
}

/* Hold Anim end: `ftFx_SpecialHiHold_Anim`/`ftFx_SpecialHiHoldAir_Anim`,
 * exercising the whole launch decision graph below them (ground: the
 * grounded-launch-or-decline `ftFx_SpecialAirHi_AirToGround`; air: the
 * pure aerial launch `ftFx_SpecialAirHi_Enter`). */
void oracle_hold_anim(bool ground, bool frames_remaining, bool air_actual, float stick_x,
                       float stick_y, float x64, float x68, float x74, float x88,
                       float floor_nx, float floor_ny, bool on_platform, float facing_in,
                       s32 max_jumps, FtMotionId* out_msid, float* out_travel_frames,
                       float* out_rotate_model, float* out_facing, float* out_gr_vel,
                       float* out_self_vel_x, float* out_self_vel_y, int* out_jumps_used) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x64_FOX_FIREFOX_DIRECTION_STICK_RANGE_MIN = x64;
    attrs.x68_FOX_FIREFOX_DURATION = x68;
    attrs.x74_FOX_FIREFOX_SPEED = x74;
    attrs.x88_FOX_FIREFOX_FACING_STICK_RANGE_MIN = x88;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.ground_or_air = air_actual ? GA_Air : GA_Ground;
    fp.input.lstick[0].x = stick_x;
    fp.input.lstick[0].y = stick_y;
    fp.coll_data.env_flags = Collide_FloorMask;
    fp.coll_data.floor.normal.x = floor_nx;
    fp.coll_data.floor.normal.y = floor_ny;
    fp.facing_dir = facing_in;
    fp.co_attrs.max_jumps = max_jumps;
    script_on_platform = on_platform;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_frames_remaining = frames_remaining;
    if (ground) {
        ftFx_SpecialHiHold_Anim(&gobj);
    } else {
        ftFx_SpecialHiHoldAir_Anim(&gobj);
    }
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    *out_travel_frames = fp.mv.fx.SpecialHi.travelFrames;
    *out_rotate_model = fp.mv.fx.SpecialHi.rotateModel;
    *out_facing = fp.facing_dir;
    *out_gr_vel = fp.gr_vel;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
    *out_jumps_used = fp.x1968_jumpsUsed;
}

/* Hold Coll: ground loss (`ftFx_SpecialHiHold_Coll`) and air landing/ledge
 * (`ftFx_SpecialHiHoldAir_Coll`). Reports: 0 none, 1 ground-to-air (ground),
 * 2 air-to-ground (air), 3 ledge-common (air). */
int oracle_hold_coll(bool ground, bool coll_result, bool ledge_check, bool ledge_common,
                      FtMotionId* out_msid) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_ft_80082708 = coll_result;
    script_check_ground_and_ledge = ledge_check;
    script_ledge_common = ledge_common;
    if (ground) {
        ftFx_SpecialHiHold_Coll(&gobj);
        *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
        return coll_result ? 0 : 1;
    }
    ftFx_SpecialHiHoldAir_Coll(&gobj);
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    if (ledge_check) {
        return 2;
    }
    return ledge_common ? 3 : 0;
}

/* Travel Anim end: `ftFx_SpecialHi_Anim`/`ftFx_SpecialAirHi_Anim`. Reports
 * which destination fired: 0 none, 1 Landing (ground-actual), 2 Fall
 * (air-actual). */
int oracle_travel_anim(bool ground, float travel_frames_in, bool grounded_actual,
                        float* out_travel_frames, FtMotionId* out_msid, float* out_start) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.ground_or_air = grounded_actual ? GA_Ground : GA_Air;
    fp.mv.fx.SpecialHi.travelFrames = travel_frames_in;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    if (ground) {
        ftFx_SpecialHi_Anim(&gobj);
    } else {
        ftFx_SpecialAirHi_Anim(&gobj);
    }
    *out_travel_frames = fp.mv.fx.SpecialHi.travelFrames;
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    *out_start = captured_start;
    if (change_motion_state_calls == 0) {
        return 0;
    }
    return grounded_actual ? 1 : 2;
}

/* Travel Phys: `ftFx_SpecialHi_Phys`/`ftFx_SpecialAirHi_Phys`. */
void oracle_travel_phys(bool ground, float unk_in, float x70, float x78, float gr_vel_in,
                         float self_vel_x_in, float self_vel_y_in, float facing,
                         float rotate_model, float* out_unk, float* out_gr_vel,
                         float* out_self_vel_x, float* out_self_vel_y) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x70_FOX_FIREFOX_DURATION_END = x70;
    attrs.x78_FOX_FIREFOX_REVERSE_ACCEL = x78;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.gr_vel = gr_vel_in;
    fp.self_vel.x = self_vel_x_in;
    fp.self_vel.y = self_vel_y_in;
    fp.facing_dir = facing;
    fp.mv.fx.SpecialHi.unk = unk_in;
    fp.mv.fx.SpecialHi.rotateModel = rotate_model;
    Fighter_GObj gobj = { &fp };
    if (ground) {
        ftFx_SpecialHi_Phys(&gobj);
    } else {
        ftFx_SpecialAirHi_Phys(&gobj);
    }
    *out_unk = fp.mv.fx.SpecialHi.unk;
    *out_gr_vel = fp.gr_vel;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
}

/* Travel ground Coll: `ftFx_SpecialHi_Coll`. Reports: 0 none (floor
 * contact only rotates the model), 1 ground-to-air. */
int oracle_travel_coll_ground(float unk2_in, bool coll_result, bool floor_contact,
                               float floor_nx, float floor_ny, float facing,
                               float* out_unk2, float* out_rotate_model, FtMotionId* out_msid) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.mv.fx.SpecialHi.unk2 = unk2_in;
    fp.facing_dir = facing;
    fp.coll_data.env_flags = floor_contact ? Collide_FloorMask : 0;
    fp.coll_data.floor.normal.x = floor_nx;
    fp.coll_data.floor.normal.y = floor_ny;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_ft_80082708 = coll_result;
    ftFx_SpecialHi_Coll(&gobj);
    *out_unk2 = fp.mv.fx.SpecialHi.unk2;
    *out_rotate_model = fp.mv.fx.SpecialHi.rotateModel;
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    return coll_result ? 0 : 1;
}

/* Travel air Coll: `ftFx_SpecialAirHi_Coll`, the IsBound/redirect
 * decision. `unk2_in`/`x6c`/`on_platform` feed `ftFox_SpecialHi_IsBound`;
 * `check_ground_ledge`/`ledge_common` script the two collision checks;
 * `env_flags`/the four surface normals and `self_vel` feed the angle
 * gates. Reports: 0 nothing (including a caught ledge, `ledge_common`
 * true, which also leaves facing/rotateModel unchanged and so is not
 * separately distinguishable from the plain-nothing case), 1 Bound
 * entered, 2 redirect (facingDir). */
int oracle_travel_coll_air(float unk2_in, s32 x6c, float x94, bool on_platform,
                            bool check_ground_ledge, bool ledge_common, u32 env_flags,
                            float floor_nx, float floor_ny, float ceil_nx, float ceil_ny,
                            float lwall_nx, float lwall_ny, float rwall_nx, float rwall_ny,
                            float self_vel_x, float self_vel_y, float* out_facing,
                            float* out_rotate_model) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x6C_FOX_FIREFOX_BOUNCE_VAR = x6c;
    attrs.x94_FOX_FIREFOX_BOUND_ANGLE = x94;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.mv.fx.SpecialHi.unk2 = unk2_in;
    fp.coll_data.env_flags = env_flags;
    fp.coll_data.floor.normal.x = floor_nx;
    fp.coll_data.floor.normal.y = floor_ny;
    fp.coll_data.ceiling.normal.x = ceil_nx;
    fp.coll_data.ceiling.normal.y = ceil_ny;
    fp.coll_data.left_facing_wall.normal.x = lwall_nx;
    fp.coll_data.left_facing_wall.normal.y = lwall_ny;
    fp.coll_data.right_facing_wall.normal.x = rwall_nx;
    fp.coll_data.right_facing_wall.normal.y = rwall_ny;
    fp.self_vel.x = self_vel_x;
    fp.self_vel.y = self_vel_y;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_check_ground_and_ledge = check_ground_ledge;
    script_ledge_common = ledge_common;
    script_on_platform = on_platform;
    ftFx_SpecialAirHi_Coll(&gobj);
    *out_facing = fp.facing_dir;
    *out_rotate_model = fp.mv.fx.SpecialHi.rotateModel;
    if (change_motion_state_calls > 0 && captured_msid == ftFx_MS_SpecialHiBound) {
        return 1;
    }
    if (rotate_model_calls > 0) {
        /* `rotate_model_calls` (see its declaration) reliably detects the
         * `facingDir` redirect regardless of whether `facing_dir`/
         * `rotateModel` happen to recompute to their prior values. */
        return 2;
    }
    return 0;
}

/* Landing/Fall Anim end: `ftFx_SpecialHiLanding_Anim` (-> Wait) /
 * `ftFx_SpecialHiFall_Anim` (-> FallSpecial). `landing`: which phase. */
void oracle_landing_fall_anim(bool landing, bool frames_remaining, float x8c, float x90,
                               int* out_wait_calls, int* out_fallspecial_calls,
                               float* out_mobility, float* out_landing_lag) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x8C_FOX_FIREFOX_FREEFALL_MOBILITY = x8c;
    attrs.x90_FOX_FIREFOX_LANDING_LAG = x90;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_frames_remaining = frames_remaining;
    if (landing) {
        ftFx_SpecialHiLanding_Anim(&gobj);
    } else {
        ftFx_SpecialHiFall_Anim(&gobj);
    }
    *out_wait_calls = wait_reenter_calls;
    *out_fallspecial_calls = fallspecial_calls;
    *out_mobility = fallspecial_mobility;
    *out_landing_lag = fallspecial_landing_lag;
}

/* Landing/Fall Phys: `ftFx_SpecialHiLanding_Phys` (ground friction) /
 * `ftFx_SpecialHiFall_Phys` (dispatch-only, see the file header). */
void oracle_landing_fall_phys(bool landing, float gr_vel_in, float x7c, float ground_friction,
                               float* out_gr_vel, int* out_fall_phys_calls) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x7C_FOX_FIREFOX_GROUND_MOMENTUM_END = x7c;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.co_attrs.ground_friction = ground_friction;
    fp.gr_vel = gr_vel_in;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    if (landing) {
        ftFx_SpecialHiLanding_Phys(&gobj);
    } else {
        ftFx_SpecialHiFall_Phys(&gobj);
    }
    *out_gr_vel = fp.gr_vel;
    *out_fall_phys_calls = ft_80084db0_calls;
}

/* Landing Coll: `ftFx_SpecialHiLanding_Coll` -> FallSpecial. */
void oracle_landing_coll(bool coll_result, float x8c, float x90, int* out_fallspecial_calls,
                          float* out_mobility, float* out_landing_lag) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x8C_FOX_FIREFOX_FREEFALL_MOBILITY = x8c;
    attrs.x90_FOX_FIREFOX_LANDING_LAG = x90;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_ft_80082708 = coll_result;
    ftFx_SpecialHiLanding_Coll(&gobj);
    *out_fallspecial_calls = fallspecial_calls;
    *out_mobility = fallspecial_mobility;
    *out_landing_lag = fallspecial_landing_lag;
}

/* Fall Coll: `ftFx_SpecialHiFall_Coll`, the frame-13 Landing regression
 * (`ftFx_SpecialHiFall_Enter` on ground/ledge success). Reports: 0 none,
 * 1 Landing entered at `*out_start`, 2 ledge-common. */
int oracle_fall_coll(bool check_ground_ledge, bool ledge_common, FtMotionId* out_msid,
                      float* out_start) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_check_ground_and_ledge = check_ground_ledge;
    script_ledge_common = ledge_common;
    ftFx_SpecialHiFall_Coll(&gobj);
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    *out_start = captured_start;
    if (check_ground_ledge) {
        return 1;
    }
    return ledge_common ? 2 : 0;
}

/* Bound Enter: `ftFx_SpecialHiBound_Enter` (+ `ftFox_SpecialHiBound_
 * SetVars`). */
void oracle_bound_enter(float self_vel_x_in, float x84, bool floor_contact, float floor_nx,
                         float floor_ny, FtMotionId* out_msid, float* out_self_vel_x,
                         s32* out_cmd_var0) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x84_FOX_FIREFOX_BOUND_VEL_X = x84;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.self_vel.x = self_vel_x_in;
    fp.coll_data.env_flags = floor_contact ? Collide_FloorMask : 0;
    fp.coll_data.floor.normal.x = floor_nx;
    fp.coll_data.floor.normal.y = floor_ny;
    fp.cmd_vars[0] = 7;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    ftFx_SpecialHiBound_Enter(&gobj);
    *out_msid = captured_msid;
    *out_self_vel_x = fp.self_vel.x;
    *out_cmd_var0 = fp.cmd_vars[0];
}

/* Bound Anim: `ftFx_SpecialHiBound_Anim`. Reports: 0 nothing (kept
 * playing), 1 FallSpecial (cmd_vars[0] set mid-air, or anim end while
 * air), 2 Wait (anim end while ground). */
int oracle_bound_anim(s32 cmd_var0, bool ground_actual, bool frames_remaining, float x8c,
                       float x90, s32 max_jumps, int* out_wait_calls,
                       int* out_fallspecial_calls, int* out_jumps_used) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x8C_FOX_FIREFOX_FREEFALL_MOBILITY = x8c;
    attrs.x90_FOX_FIREFOX_LANDING_LAG = x90;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.cmd_vars[0] = cmd_var0;
    fp.ground_or_air = ground_actual ? GA_Ground : GA_Air;
    fp.co_attrs.max_jumps = max_jumps;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_frames_remaining = frames_remaining;
    ftFx_SpecialHiBound_Anim(&gobj);
    *out_wait_calls = wait_reenter_calls;
    *out_fallspecial_calls = fallspecial_calls;
    *out_jumps_used = fp.x1968_jumpsUsed;
    if (fallspecial_calls > 0) {
        return 1;
    }
    return wait_reenter_calls > 0 ? 2 : 0;
}

/* Bound Phys: `ftFx_SpecialHiBound_Phys`. Air: `ft_800851C0` (self_vel.y
 * = transN.y) + the captured drift clamp. Ground: `ft_80084F3C`. */
void oracle_bound_phys(bool ground_actual, float trans_n_y, float gr_vel_in,
                        float ground_friction, float walk_max_vel, float* out_self_vel_y,
                        float* out_gr_vel, int* out_drift_clamp_calls) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.ground_or_air = ground_actual ? GA_Ground : GA_Air;
    fp.x6A4_transNOffset.y = trans_n_y;
    fp.gr_vel = gr_vel_in;
    fp.co_attrs.ground_friction = ground_friction;
    fp.co_attrs.walk_max_vel = walk_max_vel;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    ftFx_SpecialHiBound_Phys(&gobj);
    *out_self_vel_y = fp.self_vel.y;
    *out_gr_vel = fp.gr_vel;
    *out_drift_clamp_calls = drift_clamp_calls;
}

/* Bound Coll: `ftFx_SpecialHiBound_Coll`. Reports: 0 none, 1 land-and-stay
 * (air, `ftCommon_8007D7FC`), 2 ledge-common (air), 3 `ft_80084104`'s
 * `ftCo_Fall_Enter` (ground, `!ft_800827A0`). */
int oracle_bound_coll(bool ground_actual, bool check_ground_ledge, bool ledge_common,
                       bool ft_800827a0_result, float* out_gr_vel) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.ground_or_air = ground_actual ? GA_Ground : GA_Air;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_check_ground_and_ledge = check_ground_ledge;
    script_ledge_common = ledge_common;
    script_ft_800827A0 = ft_800827a0_result;
    ftFx_SpecialHiBound_Coll(&gobj);
    *out_gr_vel = fp.gr_vel;
    if (!ground_actual) {
        if (check_ground_ledge) {
            return 1;
        }
        return ledge_common ? 2 : 0;
    }
    return fall_enter_calls > 0 ? 3 : 0;
}
