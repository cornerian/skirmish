/* Host adapter for Fox/Falco's down special (Reflector, `ftfoxspeciallw.c`,
 * `ftfoxspeciallw.functions.json`: every non-static callback plus every
 * `static`/`static inline` helper the file itself defines -- 72 functions,
 * effectively the whole file minus its three GFX-only `Create*GFX`
 * accessory callbacks, which are hand-written no-ops below instead (never
 * invoked by anything this oracle calls; only ever installed as
 * `fp->accessory4_cb`).
 *
 * Dependency treatment, as briefed:
 *  - CAPTURED (logged, not implemented): `Fighter_ChangeMotionState`,
 *    `ftColl_CreateReflectHit` (records the call and sets `reflecting`;
 *    the bubble geometry it also assigns has no effect in this engine, see
 *    `docs/fox-down-special.md`), `ftCommon_8007D92C`'s two destinations
 *    (`ftCo_Fall_Enter`/`ft_8008A2BC`).
 *  - SCRIPTED (test-controlled return): `ftAnim_IsFramesRemaining`,
 *    `ft_80082708`, `ft_80081D0C`, `ftCo_80099F1C`, `ftCo_800C97A8`,
 *    `ftCo_Jump_CheckInput`, `ftCo_800CB870` -- each also increments a
 *    thread-local call counter so the RETURN_IF short-circuit order in
 *    `ftFx_SpecialLwLoop_IASA`/`ftFx_SpecialAirLwLoop_IASA` is verifiable,
 *    not just its net effect.
 *  - FAITHFUL (real arithmetic, reproduced verbatim from the pinned
 *    `ftcommon.c`/`ft_084E.c` sources, the same duplication precedent
 *    `fox_specials.c` already established -- this adapter does not link
 *    against `tests/oracle/physics.c`'s own extraction of the identical
 *    upstream file, since nothing in this codebase's oracle build
 *    guarantees two independently hand-written adapter structs share field
 *    offsets): `ftCommon_Fall`, `ftCommon_ApplyFrictionGround`,
 *    `ftCommon_ApplyGroundMovement` (a documented no-op, see
 *    `fox_specials.c`'s own header for why), `ft_80084F3C`,
 *    `ftCommon_8007D60C`, `ftCommon_8007D6A4`, `ftCommon_8007D7FC`,
 *    `ftCommon_AirToGroundStateChange`, `ftCommon_GroundToAirStateChange`,
 *    `ftCommon_ClampAirDrift`, `ftCo_8009A184` (`ftCo_Pass.c:76+`, minus
 *    the GFX/floor-skip bookkeeping `ftCommon_8007D5D4`/`mpUpdateFloorSkip`
 *    it also performs, which have no effect on anything compared here).
 *  - DOCUMENTED GAP: this move's own air Phys calls `ftCommon_8007CF58`
 *    (`ftcommon.c:283-306`), not extracted here. Its under-drift-maximum
 *    branch is exactly `ftCommon_ApplyFrictionAir(fp, ca->aerial_friction)`
 *    (reused verbatim from `fox_specials.c`'s own copy); its over-drift
 *    branch (a common `x1FC`-based deceleration, reached only when
 *    `|self_vel.x| > air_drift_max`) is not modeled by the Rust mirror
 *    either -- see `characters::fox::down`'s own module doc -- so this
 *    adapter reproduces only the modeled branch, unconditionally, matching
 *    the Rust side exactly rather than silently testing an unmodeled path.
 *  - Ghost/GFX no-ops: `ftAnim_8006EBA4`, `Fighter_SetEffectHitlagCallbacks`,
 *    `efSync_Spawn`, `ftFx_SpecialLw_CreateStartGFX`,
 *    `ftFx_SpecialLw_CreateLoopGFX`, `ftFx_SpecialLw_CreateReflectGFX`,
 *    `ftPartSetRotY`, `ftPartGetRotZ`, `lb_8000B1CC`, `lb_800119DC`. The
 *    Turn phase's own model-rotation write (`ftPartSetRotY`) is confirmed
 *    visual-only; only the facing flip it accompanies is compared.
 *
 * Thread-local state isolates independent concurrent test calls, not
 * gameplay global state, matching every other adapter in this directory. */
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

typedef uint8_t u8;
typedef uint32_t u32;
typedef int32_t s32;
typedef float f32;
typedef int FtMotionId;
typedef int MotionFlags;
typedef int GroundOrAir;
#define GA_Ground 0
#define GA_Air 1
#define HSD_PAD_B 0x2000
#define ftCommon_GroundAirColl_MF 0x1
#define Ft_MF_KeepGfx 0x2
#define Ft_MF_SkipColAnim 0x4
#define Ft_MF_UpdateCmd 0x8
/* Discarded by the no-op rotation sink below; only needed to compile the
 * extracted Turn-step arithmetic. */
#define MTXDegToRad(deg) ((float) (deg))
#define FtPart_HipN 0

typedef struct {
    float x, y, z;
} Vec3;

typedef struct {
    u32 x0_bone_id;
    s32 x4_max_damage;
    Vec3 x8_offset;
    float x14_size;
    float x18_damage_mul;
    float x1C_speed_mul;
    u8 x20_behavior;
} ReflectDesc;

typedef struct {
    /* x98..xB0, ftFox/types.h:141-148. */
    float x98_FOX_REFLECTOR_RELEASE_LAG;
    float x9C_FOX_REFLECTOR_TURN_FRAMES;
    float xA0_FOX_REFLECTOR_UNK1;
    s32 xA4_FOX_REFLECTOR_GRAVITY_DELAY;
    float xA8_FOX_REFLECTOR_MOMENTUM_PRESERVE_X;
    float xAC_FOX_REFLECTOR_FALL_ACCEL;
    ReflectDesc xB0_FOX_REFLECTOR_REFLECTION;
} ftFox_DatAttrs;

typedef struct {
    float ground_friction;
    float walk_max_vel;
    float ground_max_horizontal_velocity;
    float terminal_velocity;
    float aerial_friction;
    float air_drift_max;
    s32 max_jumps;
} ftCo_DatAttrs;

typedef struct {
    s32 releaseLag;
    s32 turnFrames;
    bool isRelease;
    s32 gravityDelay;
} ftFoxSpecialLw;
typedef struct {
    ftFoxSpecialLw SpecialLw;
} MvFx;
typedef struct {
    MvFx fx;
} Mv;

typedef struct {
    u32 held_buttons[1];
} FighterInput;

typedef struct Fighter_GObj_s Fighter_GObj;
typedef Fighter_GObj HSD_GObj;
typedef void (*HSD_GObjEvent)(HSD_GObj*);

typedef struct {
    float x1A2C_reflectHitDirection;
} ReflectAttrs;

typedef struct {
    void* joint;
} FtPart;

typedef struct Fighter {
    FighterInput input;
    ftFox_DatAttrs* dat_attrs;
    ftCo_DatAttrs co_attrs;
    GroundOrAir ground_or_air;
    float gr_vel;
    Vec3 self_vel;
    float facing_dir;
    float cur_anim_frame;
    s32 cmd_vars[8];
    Mv mv;
    bool reflecting;
    HSD_GObjEvent reflect_hit_cb;
    ReflectAttrs ReflectAttr;
    FtPart parts[1];
    void (*accessory4_cb)(Fighter_GObj*);
    u8 x1968_jumpsUsed;
    u8 x1969_walljumpUsed;
    bool x2227_b0;
} Fighter;
struct Fighter_GObj_s {
    Fighter* user_data;
};
#define GET_FIGHTER(gobj) ((gobj)->user_data)

static Fighter* getFighter(Fighter_GObj* gobj) {
    return gobj->user_data;
}
static ftCo_DatAttrs* getFtAttrs(Fighter* fp) {
    return &fp->co_attrs;
}
static ftFox_DatAttrs* getFtSpecialAttrs(Fighter* fp) {
    return fp->dat_attrs;
}

/* Arbitrary distinct motion-state ids (the real `ftFx_MS_*` enum lives in
 * `ftFox/forward.h`, not in this file); only distinctness matters for
 * `Fighter_ChangeMotionState`'s captured `msid` to be checkable below. */
enum {
    ftFx_MS_SpecialLwStart = 2000,
    ftFx_MS_SpecialLwLoop,
    ftFx_MS_SpecialLwHit,
    ftFx_MS_SpecialLwEnd,
    ftFx_MS_SpecialLwTurn,
    ftFx_MS_SpecialAirLwStart,
    ftFx_MS_SpecialAirLwLoop,
    ftFx_MS_SpecialAirLwHit,
    ftFx_MS_SpecialAirLwEnd,
    ftFx_MS_SpecialAirLwTurn,
};

typedef struct {
    float friction_when_above_walk_speed;
    /* x1FC (`struct ftCommonData`, `ft/types.h:181`): `ftCommon_8007CF58`'s
     * common over-drift-maximum deceleration step. */
    float x1FC;
} FtCommonData;
/* Thread-local, like every other piece of mutable state this adapter owns
 * (see the header's note): `oracle_down_phys` writes `x1FC`/
 * `friction_when_above_walk_speed` on every call from whichever proptest
 * property-test thread is currently running, and libtest runs the several
 * `#[test]` functions in this file concurrently by default. A plain
 * (non-thread-local) global here let one thread's `over_drift_step` bleed
 * into another thread's concurrent `ftCommon_8007CF58` call, producing the
 * intermittent `self_vel_x` mismatches this test used to show under the
 * default parallel test runner (reproduced reliably; gone under
 * `--test-threads=1`). `p_ftCommonData` is now a macro instead of a plain
 * pointer: a real pointer captured at static-init time would only ever
 * resolve to the initializing thread's TLS instance, silently
 * reintroducing the same cross-thread aliasing for every other thread. */
static _Thread_local FtCommonData ftCommonData_ = { 1.0f, 0.0f };
#define p_ftCommonData (&ftCommonData_)

/* ---- FAITHFUL: verbatim arithmetic from the pinned source, duplicated
 * from `fox_specials.c`'s own copy of the same upstream file rather than
 * linked against it (see the header's dependency note). ---- */

/* ftcommon.c:462-467. */
static void ftCommon_Fall(Fighter* fp, float gravity, float terminal_vel) {
    fp->self_vel.y -= gravity;
    if (fp->self_vel.y < -terminal_vel) {
        fp->self_vel.y = -terminal_vel;
    }
}

/* ftcommon.c:253-261. The real function writes `x74_anim_vel.x`,
 * integrated into `self_vel` by the caller's own frame tail; this adapter
 * has no separate animation-velocity channel, so it applies the result
 * directly, matching this oracle's per-call (not per-integration-step)
 * comparison granularity (the same simplification `fox_specials.c`'s own
 * copy already documents). */
static void ftCommon_ApplyFrictionAir(Fighter* fp, float friction) {
    float f = friction;
    if ((f < 0 ? -f : f) >= (fp->self_vel.x < 0 ? -fp->self_vel.x : fp->self_vel.x)) {
        f = -fp->self_vel.x;
    } else if (fp->self_vel.x > 0) {
        f = -f;
    }
    fp->self_vel.x += f;
}

/* ftcommon.c:283-306, both branches, applied directly to `self_vel.x` like
 * `ftCommon_ApplyFrictionAir` above (same no-separate-animation-channel
 * simplification). This exact body -- including the over-drift-maximum
 * branch and its sign handling -- is independently pinned verbatim from
 * the same pinned snapshot by `tests/oracle/air_drift_recovery.c`
 * (aliased to "ftcommon", `air_drift_recovery.functions.json`), whose own
 * differential tests compare it bit-exactly against both an under- and an
 * over-maximum input; that adapter cannot be linked against directly here
 * since its `Fighter`/`ftCo_DatAttrs` layout is its own, minimal to that
 * one function (see its header for why duplication, not linkage, is used
 * throughout this directory for cross-file common functions). */
static void ftCommon_8007CF58(Fighter* fp) {
    float vel = fp->self_vel.x;
    float drift_max = fp->co_attrs.air_drift_max;
    if ((vel < 0 ? -vel : vel) > drift_max) {
        float accel = p_ftCommonData->x1FC;
        if ((accel < 0 ? -accel : accel) >= (vel < 0 ? -vel : vel)) {
            accel = -vel;
        } else if (vel > 0) {
            accel = -p_ftCommonData->x1FC;
        }
        fp->self_vel.x += accel;
    } else {
        ftCommon_ApplyFrictionAir(fp, fp->co_attrs.aerial_friction);
    }
}

/* ftcoll.c:8007AEF8: per-frame reflect-vs-projectile collision. Skirmish has
 * no projectiles, so this is an unconditional no-op; only the `reflecting`
 * bit this move's own entries set is observable. */
static void ftColl_8007AEF8(Fighter_GObj* gobj) {
    (void) gobj;
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

/* ftcommon.c:133-148, position-only effect elided; see fox_specials.c's own
 * header for why. */
static void ftCommon_ApplyGroundMovement(Fighter_GObj* gobj) {
    (void) gobj;
}

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

/* ftcommon.c:527-539. */
static void ftCommon_8007D60C(Fighter* fp) {
    fp->ground_or_air = GA_Air;
    fp->gr_vel = 0;
    fp->x1968_jumpsUsed = (u8) fp->co_attrs.max_jumps;
}

/* ftcommon.c:546-557, the TransN root-motion branch dropped: this move has
 * none (unlike the side special's Dash phase). */
static void ftCommon_8007D6A4(Fighter* fp) {
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
 * arithmetic. */
static void ftCommon_8007D7FC(Fighter* fp) {
    ftCommon_8007D6A4(fp);
}

/* ftCommon_ClampSelfVelX/ClampAirDrift. */
static void ftCommon_ClampAirDrift(Fighter* fp) {
    float max = fp->co_attrs.air_drift_max;
    if (fp->self_vel.x > max) {
        fp->self_vel.x = max;
    } else if (fp->self_vel.x < -max) {
        fp->self_vel.x = -max;
    }
}

/* ---- CAPTURED: logged, not reimplemented. ---- */
static _Thread_local FtMotionId captured_msid;
static _Thread_local int change_motion_state_calls;
static _Thread_local int reflect_hit_calls;
static _Thread_local int fall_enter_calls;
static _Thread_local int wait_reenter_calls;

static void reset_captures(void) {
    captured_msid = 0;
    change_motion_state_calls = 0;
    reflect_hit_calls = 0;
    fall_enter_calls = 0;
    wait_reenter_calls = 0;
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

/* ftcoll.c:3189-3210: the bubble geometry assignment (bone/size/offset/
 * damage/speed/behavior) has no effect in this engine and is not captured;
 * only the call count and the `reflecting` bit it sets are observable. */
static void ftColl_CreateReflectHit(Fighter_GObj* gobj, ReflectDesc* reflect, HSD_GObjEvent cb) {
    (void) reflect;
    Fighter* fp = gobj->user_data;
    fp->reflecting = true;
    fp->reflect_hit_cb = cb;
    reflect_hit_calls++;
}

static void ftCo_Fall_Enter(Fighter_GObj* gobj) {
    (void) gobj;
    fall_enter_calls++;
}
static void ft_8008A2BC(Fighter_GObj* gobj) {
    (void) gobj;
    wait_reenter_calls++;
}
/* ftcommon.c:596-604. */
static void ftCommon_8007D92C(Fighter_GObj* gobj) {
    Fighter* fp = gobj->user_data;
    if (fp->ground_or_air == GA_Air) {
        ftCo_Fall_Enter(gobj);
    } else {
        ft_8008A2BC(gobj);
    }
}
/* GFX-only (`x2219_b0`/`efLib_DestroyAll`), no observable effect here. */
static void ftCommon_8007DB24(Fighter_GObj* gobj) {
    (void) gobj;
}

/* ftCo_Landing.c/inlines.h:81-89, reusing the captured
 * `Fighter_ChangeMotionState` above -- faithful control flow, captured
 * terminal effect. Copied from `fox_specials.c`'s own identical adapter. */
static void ftCommon_AirToGroundStateChange(Fighter_GObj* gobj, Fighter* fp, FtMotionId msid,
                                             MotionFlags flags) {
    ftCommon_8007D7FC(fp);
    Fighter_ChangeMotionState(gobj, msid, flags, fp->cur_anim_frame, 1.0f, 0.0f, NULL);
}
static void ftCommon_GroundToAirStateChange(Fighter_GObj* gobj, Fighter* fp, FtMotionId msid,
                                             MotionFlags flags) {
    ftCommon_8007D60C(fp);
    Fighter_ChangeMotionState(gobj, msid, flags, fp->cur_anim_frame, 1.0f, 0.0f, NULL);
}

/* ftCo_Pass.c:76-84, minus `ftCommon_8007D5D4`/`mpUpdateFloorSkip`/the
 * stick-tilt-age rearm (GFX/bookkeeping, no effect on anything compared
 * here). `p_ftCommonData->x46C` is the test-controlled drop velocity. */
static _Thread_local float script_pass_velocity_y;
static void ftCo_8009A184(Fighter_GObj* gobj, FtMotionId msid, MotionFlags mf, float anim_start) {
    Fighter* fp = gobj->user_data;
    ftCommon_ClampAirDrift(fp);
    fp->self_vel.y = script_pass_velocity_y;
    Fighter_ChangeMotionState(gobj, msid, mf, anim_start, 1.0f, 0.0f, NULL);
}

/* ---- SCRIPTED: test-controlled return values, each with its own call
 * counter so short-circuit order in the RETURN_IF-style IASA chains is
 * independently verifiable. ---- */
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
static _Thread_local bool script_ft_80081D0C;
static bool ft_80081D0C(Fighter_GObj* gobj) {
    (void) gobj;
    return script_ft_80081D0C;
}
static _Thread_local bool script_pass_predicate;
static _Thread_local int pass_predicate_calls;
static bool ftCo_80099F1C(Fighter_GObj* gobj) {
    (void) gobj;
    pass_predicate_calls++;
    return script_pass_predicate;
}
static _Thread_local bool script_turn_predicate;
static _Thread_local int turn_predicate_calls;
static bool ftCo_800C97A8(Fighter_GObj* gobj) {
    (void) gobj;
    turn_predicate_calls++;
    return script_turn_predicate;
}
static _Thread_local bool script_jump_check;
static _Thread_local int jump_check_calls;
static bool ftCo_Jump_CheckInput(Fighter_GObj* gobj) {
    (void) gobj;
    jump_check_calls++;
    return script_jump_check;
}
static _Thread_local bool script_aerial_jump_check;
static _Thread_local int aerial_jump_check_calls;
static bool ftCo_800CB870(Fighter_GObj* gobj) {
    (void) gobj;
    aerial_jump_check_calls++;
    return script_aerial_jump_check;
}

/* ---- Ghost/GFX no-ops. ---- */
static void ftAnim_8006EBA4(Fighter_GObj* gobj) {
    (void) gobj;
}
static void Fighter_SetEffectHitlagCallbacks(Fighter_GObj* gobj) {
    (void) gobj;
}
static void efSync_Spawn(int id, Fighter_GObj* gobj, void* joint) {
    (void) id;
    (void) gobj;
    (void) joint;
}
static void ftFx_SpecialLw_CreateStartGFX(Fighter_GObj* gobj) {
    (void) gobj;
}
static void ftFx_SpecialLw_CreateLoopGFX(Fighter_GObj* gobj) {
    (void) gobj;
}
static void ftFx_SpecialLw_CreateReflectGFX(Fighter_GObj* gobj) {
    (void) gobj;
}
static void ftPartSetRotY(Fighter* fp, int index, float radians) {
    (void) fp;
    (void) index;
    (void) radians;
}
static float ftPartGetRotZ(Fighter* fp, int index) {
    (void) fp;
    (void) index;
    return 0.0f;
}
static int ftParts_GetBoneIndex(Fighter* fp, int part) {
    (void) fp;
    (void) part;
    return 0;
}
static void lb_8000B1CC(void* joint, void* mtx, Vec3* out) {
    (void) joint;
    (void) mtx;
    out->x = out->y = out->z = 0.0f;
}
static void lb_800119DC(Vec3* pos, int a, int b, float c, float d) {
    (void) pos;
    (void) a;
    (void) b;
    (void) c;
    (void) d;
}

/* Top-of-file `static MotionFlags const` globals (`ftfoxspeciallw.c:29-32`),
 * not extracted by `extract_function` (it only selects function bodies);
 * reproduced here so the extracted Coll handlers link. Values are opaque to
 * every comparison this adapter makes. */
static MotionFlags const ftFx_MF_SpecialLw_Coll = ftCommon_GroundAirColl_MF | Ft_MF_KeepGfx;
static MotionFlags const ftFx_MF_SpecialLwEnd_Coll = Ft_MF_SkipColAnim | Ft_MF_UpdateCmd;

/* Forward declarations for every extracted function, since only complete
 * definitions (not the pinned source's own forward declarations) survive
 * extraction; several are called ahead of their real definition, exactly
 * as in the pinned source. Signatures copied verbatim. */
static inline void ftFox_SpecialLw_SetVars(HSD_GObj* gobj);
void ftFx_SpecialLw_Enter(HSD_GObj* gobj);
void ftFx_SpecialAirLw_Enter(HSD_GObj* gobj);
void ftFx_SpecialLwStart_Anim(HSD_GObj* gobj);
void ftFx_SpecialAirLwStart_Anim(HSD_GObj* gobj);
void ftFx_SpecialLwStart_IASA(HSD_GObj* gobj);
void ftFx_SpecialAirLwStart_IASA(HSD_GObj* gobj);
bool ftFx_SpecialLwStart_CheckPass(HSD_GObj* gobj);
void ftFx_SpecialLwStart_Pass(HSD_GObj* gobj);
void ftFx_SpecialLwStart_Phys(HSD_GObj* gobj);
void ftFx_SpecialAirLwStart_Phys(HSD_GObj* gobj);
void ftFx_SpecialLwStart_Coll(HSD_GObj* gobj);
void ftFx_SpecialAirLwStart_Coll(HSD_GObj* gobj);
void ftFx_SpecialLwStart_GroundToAir(HSD_GObj* gobj);
void ftFx_SpecialAirLwStart_AirToGround(HSD_GObj* gobj);
void ftFx_SpecialLwLoop_Anim(HSD_GObj* gobj);
void ftFx_SpecialAirLwLoop_Anim(HSD_GObj* gobj);
void ftFx_SpecialLwLoop_IASA(HSD_GObj* gobj);
void ftFx_SpecialAirLwLoop_IASA(HSD_GObj* gobj);
static bool ftFx_SpecialLwLoop_CheckPass(HSD_GObj* gobj);
static void ftFx_SpecialLwLoop_Pass(HSD_GObj* gobj);
void ftFx_SpecialLwLoop_Phys(HSD_GObj* gobj);
static inline void ftFox_SpecialLw_InlinePhys(HSD_GObj* gobj);
void ftFx_SpecialAirLwLoop_Phys(HSD_GObj* gobj);
void ftFx_SpecialLwLoop_Coll(HSD_GObj* gobj);
void ftFx_SpecialAirLwLoop_Coll(HSD_GObj* gobj);
static void ftFx_SpecialLwLoop_GroundToAir(HSD_GObj* gobj);
static void ftFx_SpecialAirLwLoop_AirToGround(HSD_GObj* gobj);
static void ftFx_SpecialLw_CreateReflectHit(HSD_GObj* gobj);
static void ftFx_SpecialLwLoop_Enter(HSD_GObj* gobj);
static void ftFx_SpecialAirLwLoop_Enter(HSD_GObj* gobj);
static void ftFx_SpecialLw_Turn(HSD_GObj* gobj);
static inline void ftFox_SpecialLw_Turn_Inline(HSD_GObj* gobj);
void ftFx_SpecialLwTurn_Anim(HSD_GObj* gobj);
void ftFx_SpecialAirLwTurn_Anim(HSD_GObj* gobj);
void ftFx_SpecialLwTurn_IASA(HSD_GObj* gobj);
void ftFx_SpecialAirLwTurn_IASA(HSD_GObj* gobj);
void ftFx_SpecialLwTurn_Phys(HSD_GObj* gobj);
void ftFx_SpecialAirLwTurn_Phys(HSD_GObj* gobj);
void ftFx_SpecialLwTurn_Coll(HSD_GObj* gobj);
void ftFx_SpecialAirLwTurn_Coll(HSD_GObj* gobj);
static inline void ftFox_SpecialLw_SetReflectVars(HSD_GObj* gobj);
void ftFx_SpecialLwTurn_GroundToAir(HSD_GObj* gobj);
void ftFx_SpecialAirLwTurn_GroundToAir(HSD_GObj* gobj);
static inline void ftFox_SpecialLwTurn_SetVarAll(HSD_GObj* gobj);
bool ftFx_SpecialLwTurn_Check(HSD_GObj* gobj);
static inline void ftFox_SpecialLwHit_CreateReflectInline(HSD_GObj* gobj);
bool ftFx_SpecialLwHit_Check(HSD_GObj* gobj);
void ftFx_SpecialLwHit_Anim(HSD_GObj* gobj);
void ftFx_SpecialAirLwHit_Anim(HSD_GObj* gobj);
void ftFx_SpecialLwHit_IASA(HSD_GObj* gobj);
void ftFx_SpecialAirLwHit_IASA(HSD_GObj* gobj);
void ftFx_SpecialLwHit_Phys(HSD_GObj* gobj);
void ftFx_SpecialAirLwHit_Phys(HSD_GObj* gobj);
void ftFx_SpecialLwHit_Coll(HSD_GObj* gobj);
void ftFx_SpecialAirLwHit_Coll(HSD_GObj* gobj);
void ftFx_SpecialLwHit_GroundToAir(HSD_GObj* gobj);
void ftFx_SpecialAirLwHit_AirToGround(HSD_GObj* gobj);
void ftFx_SpecialLwHit_SetCall(HSD_GObj* gobj);
void ftFx_SpecialLwHit_Enter(HSD_GObj* gobj);
void ftFx_SpecialLwEnd_Anim(HSD_GObj* gobj);
void ftFx_SpecialAirLwEnd_Anim(HSD_GObj* gobj);
void ftFx_SpecialLwEnd_IASA(HSD_GObj* gobj);
void ftFx_SpecialAirLwEnd_IASA(HSD_GObj* gobj);
void ftFx_SpecialLwEnd_Phys(HSD_GObj* gobj);
void ftFx_SpecialAirLwEnd_Phys(HSD_GObj* gobj);
void ftFx_SpecialLwEnd_Coll(HSD_GObj* gobj);
void ftFx_SpecialAirLwEnd_Coll(HSD_GObj* gobj);
void ftFx_SpecialLwEnd_GroundToAir(HSD_GObj* gobj);
void ftFx_SpecialAirLwEnd_AirToGround(HSD_GObj* gobj);
void ftFx_SpecialLwEnd_Enter(HSD_GObj* gobj);
void ftFx_SpecialAirLwEnd_Enter(HSD_GObj* gobj);

#include "ftfoxspeciallw_original.inc"

/* ------------------------------------------------------------------- */
/* Oracle entry points. Each resets captures/scripts, builds a minimal
 * `Fighter`, runs the real extracted callback(s), and reports the
 * resulting state plus which captured side effect (if any) fired. */

static void reset_fighter(Fighter* fp, ftFox_DatAttrs* attrs) {
    memset(fp, 0, sizeof(*fp));
    fp->dat_attrs = attrs;
    fp->facing_dir = 1.0f;
}

/* `ftFx_SpecialLw_Enter`/`ftFx_SpecialAirLw_Enter` (through
 * `ftFox_SpecialLw_SetVars`). */
void oracle_down_enter(bool ground, float self_vel_x_in, float self_vel_y_in, float x98,
                        float x9c, s32 xa4, float xa8, FtMotionId* out_msid,
                        float* out_self_vel_x, float* out_self_vel_y, s32* out_release_lag,
                        int* out_is_release, s32* out_gravity_delay) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x98_FOX_REFLECTOR_RELEASE_LAG = x98;
    attrs.x9C_FOX_REFLECTOR_TURN_FRAMES = x9c;
    attrs.xA4_FOX_REFLECTOR_GRAVITY_DELAY = xa4;
    attrs.xA8_FOX_REFLECTOR_MOMENTUM_PRESERVE_X = xa8;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.self_vel.x = self_vel_x_in;
    fp.self_vel.y = self_vel_y_in;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    if (ground) {
        ftFx_SpecialLw_Enter(&gobj);
    } else {
        ftFx_SpecialAirLw_Enter(&gobj);
    }
    *out_msid = captured_msid;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
    *out_release_lag = fp.mv.fx.SpecialLw.releaseLag;
    *out_is_release = fp.mv.fx.SpecialLw.isRelease;
    *out_gravity_delay = fp.mv.fx.SpecialLw.gravityDelay;
}

/* Every phase's Phys callback: `phase` 0 Start, 1 Loop, 2 Turn, 3 Hit,
 * 4 End; `ground` selects the grounded/aerial variant. */
void oracle_down_phys(int phase, bool ground, s32 gravity_delay_in, float gr_vel_in,
                       float self_vel_x_in, float self_vel_y_in, float fall_accel,
                       float terminal_velocity, float ground_friction, float walk_max_vel,
                       float aerial_friction, float air_drift_max, float over_drift_step,
                       float above_walk_friction_mul, s32* out_gravity_delay, float* out_gr_vel,
                       float* out_self_vel_x, float* out_self_vel_y) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.xAC_FOX_REFLECTOR_FALL_ACCEL = fall_accel;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.co_attrs.ground_friction = ground_friction;
    fp.co_attrs.walk_max_vel = walk_max_vel;
    fp.co_attrs.terminal_velocity = terminal_velocity;
    fp.co_attrs.aerial_friction = aerial_friction;
    fp.co_attrs.air_drift_max = air_drift_max;
    fp.gr_vel = gr_vel_in;
    fp.self_vel.x = self_vel_x_in;
    fp.self_vel.y = self_vel_y_in;
    fp.mv.fx.SpecialLw.gravityDelay = gravity_delay_in;
    ftCommonData_.friction_when_above_walk_speed = above_walk_friction_mul;
    ftCommonData_.x1FC = over_drift_step;
    Fighter_GObj gobj = { &fp };
    switch (phase * 2 + (ground ? 0 : 1)) {
    case 0:
        ftFx_SpecialLwStart_Phys(&gobj);
        break;
    case 1:
        ftFx_SpecialAirLwStart_Phys(&gobj);
        break;
    case 2:
        ftFx_SpecialLwLoop_Phys(&gobj);
        break;
    case 3:
        ftFx_SpecialAirLwLoop_Phys(&gobj);
        break;
    case 4:
        ftFx_SpecialLwTurn_Phys(&gobj);
        break;
    case 5:
        ftFx_SpecialAirLwTurn_Phys(&gobj);
        break;
    case 6:
        ftFx_SpecialLwHit_Phys(&gobj);
        break;
    case 7:
        ftFx_SpecialAirLwHit_Phys(&gobj);
        break;
    case 8:
        ftFx_SpecialLwEnd_Phys(&gobj);
        break;
    default:
        ftFx_SpecialAirLwEnd_Phys(&gobj);
        break;
    }
    *out_gravity_delay = fp.mv.fx.SpecialLw.gravityDelay;
    *out_gr_vel = fp.gr_vel;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
}

/* Start/Loop/Turn/Hit's per-frame Anim bookkeeping (isRelease/releaseLag)
 * plus each phase's own transition condition: `phase` 0 Start (no
 * transition of its own beyond clip end, reported through `out_msid` via
 * the captured Loop entry plus `out_reflect_hit_calls`), 1 Loop (clip-end-
 * independent release exit to End), 2 Turn (the per-step countdown, flip-
 * once and `turnFrames <= 0` exit through `ftFx_SpecialLwHit_Check`), 3 Hit
 * (clip-end through the same `_Check`, `frames_remaining` gates it). */
void oracle_down_anim(int phase, bool ground, bool held_b, s32 release_lag_in, int is_release_in,
                       s32 turn_frames_in, s32 cmd0_in, float x9c, bool frames_remaining,
                       FtMotionId* out_msid, s32* out_release_lag, int* out_is_release,
                       s32* out_turn_frames, s32* out_cmd0, bool* out_reflecting,
                       int* out_reflect_hit_calls) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x9C_FOX_REFLECTOR_TURN_FRAMES = x9c;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.input.held_buttons[0] = held_b ? HSD_PAD_B : 0;
    fp.mv.fx.SpecialLw.releaseLag = release_lag_in;
    fp.mv.fx.SpecialLw.isRelease = is_release_in;
    fp.mv.fx.SpecialLw.turnFrames = turn_frames_in;
    fp.cmd_vars[0] = cmd0_in;
    fp.ground_or_air = ground ? GA_Ground : GA_Air;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_frames_remaining = frames_remaining;
    switch (phase * 2 + (ground ? 0 : 1)) {
    case 0:
        ftFx_SpecialLwStart_Anim(&gobj);
        break;
    case 1:
        ftFx_SpecialAirLwStart_Anim(&gobj);
        break;
    case 2:
        ftFx_SpecialLwLoop_Anim(&gobj);
        break;
    case 3:
        ftFx_SpecialAirLwLoop_Anim(&gobj);
        break;
    case 4:
        ftFx_SpecialLwTurn_Anim(&gobj);
        break;
    case 5:
        ftFx_SpecialAirLwTurn_Anim(&gobj);
        break;
    case 6:
        ftFx_SpecialLwHit_Anim(&gobj);
        break;
    default:
        ftFx_SpecialAirLwHit_Anim(&gobj);
        break;
    }
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    *out_release_lag = fp.mv.fx.SpecialLw.releaseLag;
    *out_is_release = fp.mv.fx.SpecialLw.isRelease;
    *out_turn_frames = fp.mv.fx.SpecialLw.turnFrames;
    *out_cmd0 = fp.cmd_vars[0];
    *out_reflecting = fp.reflecting;
    *out_reflect_hit_calls = reflect_hit_calls;
}

/* End's clip-end dispatch (`ftCommon_8007D92C`, captured Wait/Fall). */
void oracle_down_end_anim(bool ground, bool frames_remaining, int* out_wait_calls,
                           int* out_fall_calls) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.ground_or_air = ground ? GA_Ground : GA_Air;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_frames_remaining = frames_remaining;
    if (ground) {
        ftFx_SpecialLwEnd_Anim(&gobj);
    } else {
        ftFx_SpecialAirLwEnd_Anim(&gobj);
    }
    *out_wait_calls = wait_reenter_calls;
    *out_fall_calls = fall_enter_calls;
}

/* Loop's IASA short-circuit chain: turn-check, then (ground only)
 * jump-cancel, else (air) the aerial jump-cancel; platform drop is the
 * ground-only remainder, verified separately below. Returns which branch
 * consumed the frame: 0 none, 1 turn, 2 jump-cancel/aerial-jump. */
int oracle_down_loop_iasa(bool ground, bool turn_result, bool jump_result, FtMotionId* out_msid,
                           int* out_turn_calls, int* out_jump_calls, int* out_pass_calls,
                           bool* out_reflecting) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x9C_FOX_REFLECTOR_TURN_FRAMES = 6.0f;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.ground_or_air = ground ? GA_Ground : GA_Air;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    turn_predicate_calls = 0;
    jump_check_calls = 0;
    aerial_jump_check_calls = 0;
    pass_predicate_calls = 0;
    script_turn_predicate = turn_result;
    script_jump_check = jump_result;
    script_aerial_jump_check = jump_result;
    script_pass_predicate = false;
    if (ground) {
        ftFx_SpecialLwLoop_IASA(&gobj);
    } else {
        ftFx_SpecialAirLwLoop_IASA(&gobj);
    }
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    *out_turn_calls = turn_predicate_calls;
    *out_jump_calls = ground ? jump_check_calls : aerial_jump_check_calls;
    *out_pass_calls = pass_predicate_calls;
    *out_reflecting = fp.reflecting;
    if (turn_result) {
        return 1;
    }
    return (ground ? jump_result : jump_result) ? 2 : 0;
}

/* `ftFx_SpecialLwHit_Check`: End (releaseLag<=0 && isRelease) or Loop
 * (fresh reflect hit) otherwise. Shared by Turn's `turnFrames<=0` exit and
 * Hit's own clip end; tested directly here. */
int oracle_down_hit_check(bool ground, s32 release_lag_in, int is_release_in, FtMotionId* out_msid,
                           bool* out_reflecting, int* out_reflect_hit_calls) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.mv.fx.SpecialLw.releaseLag = release_lag_in;
    fp.mv.fx.SpecialLw.isRelease = is_release_in;
    fp.ground_or_air = ground ? GA_Ground : GA_Air;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    int result = ftFx_SpecialLwHit_Check(&gobj);
    *out_msid = captured_msid;
    *out_reflecting = fp.reflecting;
    *out_reflect_hit_calls = reflect_hit_calls;
    return result;
}

/* `ftFox_SpecialLwTurn_SetVarAll` (through `ftFx_SpecialLwTurn_Check`,
 * scripting `ftCo_800C97A8` true): the synchronous first Turn step. */
void oracle_down_turn_check(bool ground, float x9c, float facing_in, FtMotionId* out_msid,
                             bool* out_reflecting, float* out_facing, s32* out_turn_frames,
                             s32* out_cmd0) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x9C_FOX_REFLECTOR_TURN_FRAMES = x9c;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.facing_dir = facing_in;
    fp.ground_or_air = ground ? GA_Ground : GA_Air;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_turn_predicate = true;
    ftFx_SpecialLwTurn_Check(&gobj);
    *out_msid = captured_msid;
    *out_reflecting = fp.reflecting;
    *out_facing = fp.facing_dir;
    *out_turn_frames = fp.mv.fx.SpecialLw.turnFrames;
    *out_cmd0 = fp.cmd_vars[0];
}

/* Start/Loop's platform drop (`ftCo_80099F1C` scripted true ->
 * `ftCo_8009A184` + a fresh reflect hit): `phase` 0 Start, 1 Loop. */
void oracle_down_pass(int phase, float cur_anim_frame, float pass_velocity_y, FtMotionId* out_msid,
                       float* out_self_vel_y, bool* out_reflecting, int* out_reflect_hit_calls) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.cur_anim_frame = cur_anim_frame;
    fp.ground_or_air = GA_Ground;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_pass_velocity_y = pass_velocity_y;
    if (phase == 0) {
        ftFx_SpecialLwStart_Pass(&gobj);
    } else {
        ftFx_SpecialLwLoop_Pass(&gobj);
    }
    *out_msid = captured_msid;
    *out_self_vel_y = fp.self_vel.y;
    *out_reflecting = fp.reflecting;
    *out_reflect_hit_calls = reflect_hit_calls;
}

/* Every phase's ground<->air Coll conversion: `phase` 0 Start, 1 Loop,
 * 2 Turn, 3 Hit, 4 End; `ground_side` selects which Coll callback runs
 * (true: the grounded variant's own `ft_80082708`-gated conversion,
 * false: the aerial variant's own `ft_80081D0C`-gated one). */
int oracle_down_conversion(int phase, bool ground_side, bool coll_result, FtMotionId* out_msid,
                            bool* out_reflecting, int* out_reflect_hit_calls) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_ft_80082708 = coll_result;
    script_ft_80081D0C = coll_result;
    void (*ground_coll[5])(Fighter_GObj*) = {
        ftFx_SpecialLwStart_Coll, ftFx_SpecialLwLoop_Coll,  ftFx_SpecialLwTurn_Coll,
        ftFx_SpecialLwHit_Coll,   ftFx_SpecialLwEnd_Coll,
    };
    void (*air_coll[5])(Fighter_GObj*) = {
        ftFx_SpecialAirLwStart_Coll, ftFx_SpecialAirLwLoop_Coll, ftFx_SpecialAirLwTurn_Coll,
        ftFx_SpecialAirLwHit_Coll,   ftFx_SpecialAirLwEnd_Coll,
    };
    if (ground_side) {
        ground_coll[phase](&gobj);
    } else {
        air_coll[phase](&gobj);
    }
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    *out_reflecting = fp.reflecting;
    *out_reflect_hit_calls = reflect_hit_calls;
    return ground_side ? (coll_result ? 0 : 1) : (coll_result ? 1 : 0);
}
