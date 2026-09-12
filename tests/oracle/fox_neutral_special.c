/* Host adapter for Fox/Falco's neutral special (Blaster, `ftfoxspecialn.c`,
 * `ftfoxspecialn.functions.json`): the Start/Loop/End state machine's own
 * per-frame Anim/IASA bookkeeping, ground/air Enter, and
 * `PrepareBlasterShot`/`FireBlasterShot` (the shot's own launch angle and
 * the arguments it hands to the item spawn entry point, captured below --
 * the laser item's own real spawn/motion/lifetime/reflect logic is a
 * separate adapter, `fox_laser.c`, extracted from `itfoxlaser.c` itself).
 *
 * Dependency treatment, as briefed (matching `ftfoxspeciallw.c`'s own
 * precedent):
 *  - CAPTURED (logged, not implemented): `Fighter_ChangeMotionState`,
 *    `it_8029C6A4` (the laser item spawn entry point -- captures the exact
 *    angle/speed/kind/position this state machine hands it; the item's own
 *    behavior once spawned is `fox_laser.c`'s job, not this adapter's),
 *    `ft_8008A2BC` (Wait re-entry), `ftCo_Fall_Enter`, `ftCo_80096900`
 *    (`FallSpecial` entry -- records its `landing_lag` argument),
 *    `ftCommon_8007D7FC` (common ground-entry reset -- this move's own
 *    `Enter` unconditionally re-zeroes `self_vel`/`gr_vel` immediately
 *    afterward regardless of what this dependency does, so only the call
 *    itself is observable here; its own arithmetic is independently pinned
 *    by the down-special adapter's `oracle_down_enter`, which depends on it
 *    for real).
 *  - SCRIPTED (test-controlled return): `ftAnim_IsFramesRemaining`, each
 *    with its own call counter.
 *  - HAND-DUPLICATED, verbatim, against a pinned header snapshot (these are
 *    `static inline` functions with no `.c` file of their own to extract
 *    from -- see `tests/fox_neutral_special_differential.rs`'s own
 *    `adapter_statements_are_verbatim_in_the_pinned_sources` test):
 *    `ftFox_SpecialN_CheckLoopInput` (`ftFox/inlines.h:7-14`).
 *  - Ghost/GFX/bookkeeping no-ops: `lb_8000B1CC` (hold-joint transform --
 *    irrelevant here since the real spawn position never reads it, see
 *    `docs/fox-neutral-special.md`), `ftParts_GetBoneIndex`,
 *    `ftAnim_8006EBA4`, `it_802ADDD0`, `it_802AE538`, `it_802AE608`,
 *    `it_802AE8A8` (cosmetic gun spawn, returns a fixed non-null sentinel),
 *    `it_8026BAE8`, `it_802AE1D0`, `ftLib_GetKind`, `ft_PlaySFX`,
 *    `ftpickupitem_80094818`, `OSReport`, `__assert`,
 *    `ftFox_SpecialN_OnChangeAction` (assigned as a callback, never invoked
 *    by anything this oracle calls).
 *
 * Thread-local state isolates independent concurrent test calls, matching
 * every other adapter in this directory. */
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

typedef uint8_t u8;
typedef uint16_t u16;
typedef uint32_t u32;
typedef int32_t s32;
typedef float f32;
typedef double f64;
typedef int FtMotionId;
typedef u32 MotionFlags;
typedef int GroundOrAir;
#define GA_Ground 0
#define GA_Air 1
#define HSD_PAD_B 0x2000
#define PAD_STACK(n)

/* forward.h:172,181,233. Opaque to every comparison this adapter makes
 * (only ever handed to the captured `Fighter_ChangeMotionState`). */
static MotionFlags const Ft_MF_KeepGfx = 1 << 1;
static MotionFlags const Ft_MF_SkipModel = 1 << 4;
static MotionFlags const Ft_MF_SkipAttackCount = 1 << 25;

typedef struct {
    float x, y, z;
} Vec3;

typedef struct Fighter_GObj_s Fighter_GObj;
typedef Fighter_GObj HSD_GObj;
typedef void (*HSD_GObjEvent)(HSD_GObj*);

/* ftFox/types.h:79-89 (offsets relative to the struct's own base). */
typedef struct {
    float x10_FOX_BLASTER_ANGLE;
    float x14_FOX_BLASTER_VEL;
    float x18_FOX_BLASTER_LANDING_LAG;
    s32 x1C_FOX_BLASTER_SHOT_ITKIND;
    s32 x20_FOX_BLASTER_GUN_ITKIND;
} ftFox_DatAttrs;

typedef struct {
    bool isBlasterLoop;
} FtFoxSpecialN;
typedef struct {
    FtFoxSpecialN SpecialN;
} MvFx;
typedef struct {
    MvFx fx;
} Mv;

/* The cosmetic hand-held gun item's own gobj handle
 * (`fp->u.fx.x222C_blasterGObj`); a plain struct stands in for the real
 * per-character union, matching every other adapter's minimal-fields-only
 * convention. */
typedef struct {
    HSD_GObj* x222C_blasterGObj;
} UFx;
typedef struct {
    UFx fx;
} FighterU;

typedef struct {
    u32 held_buttons[1];
    u32 pressed_buttons;
} FighterInput;

typedef struct {
    void* joint;
} FtPart;

typedef struct Fighter {
    FighterInput input;
    ftFox_DatAttrs* dat_attrs;
    GroundOrAir ground_or_air;
    float gr_vel;
    Vec3 self_vel;
    Vec3 cur_pos;
    float facing_dir;
    s32 cmd_vars[8];
    Mv mv;
    FighterU u;
    HSD_GObjEvent take_dmg_cb;
    HSD_GObjEvent death2_cb;
    HSD_GObjEvent x21EC;
    void (*accessory4_cb)(Fighter_GObj*);
    FtPart parts[1];
} Fighter;
struct Fighter_GObj_s {
    Fighter* user_data;
};
#define GET_FIGHTER(gobj) ((gobj)->user_data)

static Fighter* getFighter(Fighter_GObj* gobj) {
    return gobj->user_data;
}
static ftFox_DatAttrs* getFtSpecialAttrs(Fighter* fp) {
    return fp->dat_attrs;
}

/* Arbitrary distinct motion-state ids (the real `ftFx_MS_*` enum lives in
 * `ftFox/forward.h`, not in this file); only distinctness matters for
 * `Fighter_ChangeMotionState`'s captured `msid` to be checkable below. */
enum {
    ftFx_MS_SpecialNStart = 3000,
    ftFx_MS_SpecialNLoop,
    ftFx_MS_SpecialNEnd,
    ftFx_MS_SpecialAirNStart,
    ftFx_MS_SpecialAirNLoop,
    ftFx_MS_SpecialAirNEnd,
};
/* Not the real per-game enum values (SFX-only, no gameplay effect --
 * `ft_PlaySFX` below is a no-op and neither branch is ever compared). */
enum { FTKIND_FOX = 1, FTKIND_FALCO = 2 };
#define SFX_VOLUME_MAX 127
#define SFX_PAN_MID 64
#define FtPart_RThumbNb 0

/* ---- CAPTURED: logged, not reimplemented. ---- */
static _Thread_local FtMotionId captured_msid;
static _Thread_local int change_motion_state_calls;
static _Thread_local int wait_reenter_calls;
static _Thread_local int fall_enter_calls;
static _Thread_local int fall_special_calls;
static _Thread_local float fall_special_landing_lag;
static _Thread_local int common_8007d7fc_calls;

/* The laser item spawn entry point: captures the exact launch angle/speed/
 * kind/position this state machine hands it. The item's own real behavior
 * once spawned (motion, lifetime, shield bounce, Reflector hand-off) is
 * `fox_laser.c`'s job (extracted directly from `itfoxlaser.c`), not this
 * adapter's. */
static _Thread_local int it_8029C6A4_calls;
static _Thread_local f64 captured_launch_angle;
static _Thread_local f32 captured_launch_speed;
static _Thread_local s32 captured_launch_kind;

static void reset_captures(void) {
    captured_msid = 0;
    change_motion_state_calls = 0;
    wait_reenter_calls = 0;
    fall_enter_calls = 0;
    fall_special_calls = 0;
    fall_special_landing_lag = 0.0f;
    common_8007d7fc_calls = 0;
    it_8029C6A4_calls = 0;
    captured_launch_angle = 0.0;
    captured_launch_speed = 0.0f;
    captured_launch_kind = 0;
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

static void it_8029C6A4(f32 angle, f32 vel, HSD_GObj* parent, Vec3* vec, int kind) {
    (void) parent;
    (void) vec;
    it_8029C6A4_calls++;
    captured_launch_angle = angle;
    captured_launch_speed = vel;
    captured_launch_kind = kind;
}

static void ft_8008A2BC(Fighter_GObj* gobj) {
    (void) gobj;
    wait_reenter_calls++;
}
static void ftCo_Fall_Enter(Fighter_GObj* gobj) {
    (void) gobj;
    fall_enter_calls++;
}
static void ftCo_80096900(Fighter_GObj* gobj, int mobility, int unused, bool allow_interrupt,
                           int unused2, f32 landing_lag) {
    (void) gobj;
    (void) mobility;
    (void) unused;
    (void) allow_interrupt;
    (void) unused2;
    fall_special_calls++;
    fall_special_landing_lag = landing_lag;
}
/* `ftfoxspecialn.c`'s own `Enter` re-zeroes `self_vel`/`gr_vel`
 * unconditionally right after this call, so only the call count is
 * observable from here; the real arithmetic (ground_or_air/jump-counter
 * reset) is independently pinned by the down-special adapter's own
 * `ftCommon_8007D7FC` duplication, which depends on it for its own
 * comparisons. */
static void ftCommon_8007D7FC(Fighter* fp) {
    (void) fp;
    common_8007d7fc_calls++;
}

/* ---- SCRIPTED: test-controlled return values. ---- */
static _Thread_local bool script_frames_remaining;
static _Thread_local int frames_remaining_calls;
static bool ftAnim_IsFramesRemaining(Fighter_GObj* gobj) {
    (void) gobj;
    frames_remaining_calls++;
    return script_frames_remaining;
}

/* ---- Ghost/GFX/bookkeeping no-ops. ---- */
static void lb_8000B1CC(void* joint, Vec3* offset, Vec3* out) {
    (void) joint;
    (void) offset;
    out->x = out->y = out->z = 0.0f;
}
static int ftParts_GetBoneIndex(Fighter* fp, int part) {
    (void) fp;
    (void) part;
    return 0;
}
static void ftAnim_8006EBA4(Fighter_GObj* gobj) {
    (void) gobj;
}
static void it_802ADDD0(HSD_GObj* gobj, int visible) {
    (void) gobj;
    (void) visible;
}
static void it_802AE538(HSD_GObj* gobj) {
    (void) gobj;
}
static void it_802AE608(HSD_GObj* gobj) {
    (void) gobj;
}
static Fighter_GObj cosmetic_gun_gobj_storage;
static HSD_GObj* it_802AE8A8(f32 facing_dir, HSD_GObj* parent, Vec3* pos, int bone, int kind) {
    (void) facing_dir;
    (void) parent;
    (void) pos;
    (void) bone;
    (void) kind;
    /* A fixed non-null sentinel: the real function's own failure path
     * (`assert_line`) is not exercised by anything this port's own state
     * machine can reach (the cosmetic gun item is unmodeled but always
     * assumed to spawn successfully, matching `docs/fox-neutral-special.md`'s
     * own "confirmed hitbox-free, entirely unmodeled" treatment). */
    return &cosmetic_gun_gobj_storage;
}
static void it_8026BAE8(HSD_GObj* gobj, f32 scale) {
    (void) gobj;
    (void) scale;
}
static void it_802AE1D0(HSD_GObj* gobj) {
    (void) gobj;
}
static int ftLib_GetKind(HSD_GObj* gobj) {
    (void) gobj;
    return FTKIND_FOX;
}
static void ft_PlaySFX(Fighter* fp, u32 id, int volume, int pan) {
    (void) fp;
    (void) id;
    (void) volume;
    (void) pan;
}
static void ftpickupitem_80094818(Fighter_GObj* gobj, int arg) {
    (void) gobj;
    (void) arg;
}
static int OSReport(const char* fmt, ...) {
    (void) fmt;
    return 0;
}
static void __assert(const char* file, int line, const char* expr) {
    (void) file;
    (void) line;
    (void) expr;
}
static void ftFx_SpecialN_OnChangeAction(Fighter_GObj* gobj) {
    (void) gobj;
}
/* Assigned to `take_dmg_cb`/`death2_cb` by `ftFox_SpecialN_SetCall`, never
 * invoked by anything this oracle calls. */
static void ftFx_Init_800E5588(Fighter_GObj* gobj) {
    (void) gobj;
}

/* ---- HAND-DUPLICATED verbatim: `ftFox/inlines.h:7-14`, a `static inline`
 * helper with no `.c` file of its own to extract from. Checked against the
 * pinned `tests/oracle/original/ftfox_inlines.h` snapshot by
 * `tests/fox_neutral_special_differential.rs`'s own
 * `adapter_statements_are_verbatim_in_the_pinned_sources` test. ---- */
/* BEGIN VERBATIM CHECK LOOP INPUT */
static inline void ftFox_SpecialN_CheckLoopInput(HSD_GObj* gobj)
{
    Fighter* fp = GET_FIGHTER(gobj);

    if (fp->cmd_vars[0] != 0 && (fp->input.pressed_buttons & HSD_PAD_B)) {
        fp->mv.fx.SpecialN.isBlasterLoop = true;
    }
}
/* END VERBATIM CHECK LOOP INPUT */

/* Forward declarations for every extracted function (see
 * `ftfoxspeciallw.c`'s own header note on why: only complete definitions,
 * not the pinned source's own forward declarations, survive extraction,
 * and several are called ahead of their real definition exactly as in the
 * pinned source). Signatures copied verbatim. */
static inline void ftFox_SpecialN_GetHoldJoint(HSD_GObj* gobj, Vec3* pos, f32 z_offset);
void ftFx_SpecialN_FtGetHoldJoint(HSD_GObj* gobj, Vec3* pos);
static inline void ftFox_SpecialN_SetNULL(HSD_GObj* gobj);
static inline f64 ftFox_SpecialN_PrepareBlasterShot(HSD_GObj* gobj, Fighter* fp,
                                                    ftFox_DatAttrs* da, Vec3* pos);
static inline void ftFox_SpecialN_FireBlasterShot(HSD_GObj* gobj, Fighter* fp, ftFox_DatAttrs* da,
                                                  Vec3* pos, f64 launch_angle);
static inline void ftFox_SpecialN_SetCall(HSD_GObj* gobj);
static inline void ftFox_SpecialN_SpawnBlaster(HSD_GObj* gobj, Fighter* fp, ftFox_DatAttrs* da,
                                               int assert_line);
static inline void ftFox_SpecialN_InitializeState(HSD_GObj* gobj, Fighter* fp);
void ftFx_SpecialN_Enter(HSD_GObj* gobj);
void ftFx_SpecialAirN_Enter(HSD_GObj* gobj);
static inline void ftFox_SpecialN_UpdateBlaster(Fighter* fp);
static inline void ftFox_SpecialN_StartAnimation(HSD_GObj* gobj, FtMotionId loop_msid);
void ftFx_SpecialNStart_Anim(HSD_GObj* gobj);
static inline void ftFox_SpecialN_BeginLoopTransition(HSD_GObj* gobj, Fighter* fp,
                                                      FtMotionId loop_msid);
static inline void ftFox_SpecialN_FinishLoopTransition(Fighter* fp);
static inline void ftFox_SpecialN_FinishEndTransition(Fighter* fp);
void ftFx_SpecialNLoop_Anim(HSD_GObj* gobj);
static inline void ftFox_SpecialN_RemoveBlasterNULL(HSD_GObj* gobj);
static inline bool ftFox_SpecialN_UpdateEndAnimation(HSD_GObj* gobj, Fighter* fp);
void ftFx_SpecialNEnd_Anim(HSD_GObj* gobj);
void ftFx_SpecialAirNStart_Anim(HSD_GObj* gobj);
void ftFx_SpecialAirNLoop_Anim(HSD_GObj* gobj);
void ftFx_SpecialAirNEnd_Anim(HSD_GObj* gobj);
void ftFx_SpecialNStart_IASA(HSD_GObj* gobj);
void ftFx_SpecialNLoop_IASA(HSD_GObj* gobj);
void ftFx_SpecialNEnd_IASA(HSD_GObj* gobj);
void ftFx_SpecialAirNStart_IASA(HSD_GObj* gobj);
void ftFx_SpecialAirNLoop_IASA(HSD_GObj* gobj);
void ftFx_SpecialAirNEnd_IASA(HSD_GObj* gobj);
void ftFx_SpecialN_CreateBlasterShot(HSD_GObj* gobj);

/* Top-of-file globals (`ftfoxspecialn.c:156-157`), not extracted by
 * `extract_function` (it only selects function bodies); reproduced here so
 * the extracted SFX calls link. Opaque to every comparison this adapter
 * makes (SFX-only, `ft_PlaySFX` above is a no-op). */
u32 foxSFX[2] = { 110103, 110106 };
u32 falcoSFX[2] = { 100099, 100102 };

#include "ftfoxspecialn_original.inc"

/* ------------------------------------------------------------------- */
/* Oracle entry points. */

static void reset_fighter(Fighter* fp, ftFox_DatAttrs* attrs) {
    memset(fp, 0, sizeof(*fp));
    fp->dat_attrs = attrs;
    fp->facing_dir = 1.0f;
}

/* `ftFx_SpecialN_Enter`/`ftFx_SpecialAirN_Enter`: motion transition,
 * `cmd_vars`/`isBlasterLoop` reset, and (ground only) the explicit
 * `self_vel`/`gr_vel` zero this move's own Enter performs itself
 * (independent of whatever `ftCommon_8007D7FC` does, see its own note
 * above). */
void oracle_neutral_enter(bool ground, f32 gr_vel_in, f32 self_vel_x_in, f32 self_vel_y_in,
                           f32 self_vel_z_in, FtMotionId* out_msid, s32* out_cmd_vars0,
                           s32* out_cmd_vars1, s32* out_cmd_vars2, s32* out_cmd_vars3,
                           int* out_is_blaster_loop, f32* out_gr_vel, f32* out_self_vel_x,
                           f32* out_self_vel_y, f32* out_self_vel_z) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.gr_vel = gr_vel_in;
    fp.self_vel.x = self_vel_x_in;
    fp.self_vel.y = self_vel_y_in;
    fp.self_vel.z = self_vel_z_in;
    fp.mv.fx.SpecialN.isBlasterLoop = true;
    fp.cmd_vars[0] = fp.cmd_vars[1] = fp.cmd_vars[2] = fp.cmd_vars[3] = 7;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    if (ground) {
        ftFx_SpecialN_Enter(&gobj);
    } else {
        ftFx_SpecialAirN_Enter(&gobj);
    }
    *out_msid = captured_msid;
    *out_cmd_vars0 = fp.cmd_vars[0];
    *out_cmd_vars1 = fp.cmd_vars[1];
    *out_cmd_vars2 = fp.cmd_vars[2];
    *out_cmd_vars3 = fp.cmd_vars[3];
    *out_is_blaster_loop = fp.mv.fx.SpecialN.isBlasterLoop;
    *out_gr_vel = fp.gr_vel;
    *out_self_vel_x = fp.self_vel.x;
    *out_self_vel_y = fp.self_vel.y;
    *out_self_vel_z = fp.self_vel.z;
}

/* `ftFox_SpecialN_CheckLoopInput`: the exact turnaround-latch predicate,
 * over arbitrary `cmd_vars[0]`/fresh-press/current-latch inputs. */
bool oracle_neutral_check_loop_input(s32 cmd0_in, bool pressed_b, bool is_loop_in) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.cmd_vars[0] = cmd0_in;
    fp.input.pressed_buttons = pressed_b ? HSD_PAD_B : 0;
    fp.mv.fx.SpecialN.isBlasterLoop = is_loop_in;
    Fighter_GObj gobj = { &fp };
    ftFox_SpecialN_CheckLoopInput(&gobj);
    return fp.mv.fx.SpecialN.isBlasterLoop;
}

/* Start's own transition-on-clip-end (`ftFox_SpecialN_StartAnimation`):
 * never fires a shot itself (only Loop's own Anim callback re-derives the
 * `cmd_vars[2]` fire gate). */
int oracle_neutral_start_anim(bool ground, bool frames_remaining, FtMotionId* out_msid) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_frames_remaining = frames_remaining;
    if (ground) {
        ftFx_SpecialNStart_Anim(&gobj);
    } else {
        ftFx_SpecialAirNStart_Anim(&gobj);
    }
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    return change_motion_state_calls;
}

/* Loop's own per-frame Anim callback: the loop-vs-end transition decision
 * (`isBlasterLoop`, reset to false on a repeat), and the same-frame fire
 * check (`cmd_vars[2]`), item spawn captured. */
void oracle_neutral_loop_anim(bool ground, bool frames_remaining, int is_blaster_loop_in,
                               s32 cmd_vars2_in, f32 facing_dir_in, f32 angle_attr_in,
                               f32 vel_attr_in, s32 kind_in, FtMotionId* out_msid,
                               int* out_is_blaster_loop, int* out_fired, f64* out_angle,
                               f32* out_speed, s32* out_kind) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x10_FOX_BLASTER_ANGLE = angle_attr_in;
    attrs.x14_FOX_BLASTER_VEL = vel_attr_in;
    attrs.x1C_FOX_BLASTER_SHOT_ITKIND = kind_in;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.facing_dir = facing_dir_in;
    fp.mv.fx.SpecialN.isBlasterLoop = is_blaster_loop_in;
    fp.cmd_vars[2] = cmd_vars2_in;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_frames_remaining = frames_remaining;
    if (ground) {
        ftFx_SpecialNLoop_Anim(&gobj);
    } else {
        ftFx_SpecialAirNLoop_Anim(&gobj);
    }
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    *out_is_blaster_loop = fp.mv.fx.SpecialN.isBlasterLoop;
    *out_fired = it_8029C6A4_calls;
    *out_angle = captured_launch_angle;
    *out_speed = captured_launch_speed;
    *out_kind = captured_launch_kind;
}

/* End's own ground clip-end dispatch: direct `Wait` re-entry, no landing
 * lag of its own. */
void oracle_neutral_end_anim_ground(bool frames_remaining, int* out_wait_calls,
                                     int* out_change_motion_state_calls) {
    ftFox_DatAttrs attrs = { 0 };
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_frames_remaining = frames_remaining;
    ftFx_SpecialNEnd_Anim(&gobj);
    *out_wait_calls = wait_reenter_calls;
    *out_change_motion_state_calls = change_motion_state_calls;
}

/* End's own air clip-end dispatch: `x18 == 0` -> ordinary `Fall`;
 * otherwise `FallSpecial` with the attribute's own landing lag. */
void oracle_neutral_end_anim_air(bool frames_remaining, f32 landing_lag_in, int* out_fall_calls,
                                  int* out_fall_special_calls, f32* out_fall_special_lag) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x18_FOX_BLASTER_LANDING_LAG = landing_lag_in;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    Fighter_GObj gobj = { &fp };
    reset_captures();
    script_frames_remaining = frames_remaining;
    ftFx_SpecialAirNEnd_Anim(&gobj);
    *out_fall_calls = fall_enter_calls;
    *out_fall_special_calls = fall_special_calls;
    *out_fall_special_lag = fall_special_landing_lag;
}

/* `ftFx_SpecialN_CreateBlasterShot` directly (the `accessory4_cb` fire
 * path installed by Start's own clip-end transition): `PrepareBlasterShot`/
 * `FireBlasterShot` with the item spawn captured. */
void oracle_neutral_create_blaster_shot(s32 cmd_vars2_in, f32 facing_dir_in, f32 angle_attr_in,
                                         f32 vel_attr_in, s32 kind_in, int* out_fired,
                                         f64* out_angle, f32* out_speed, s32* out_kind) {
    ftFox_DatAttrs attrs = { 0 };
    attrs.x10_FOX_BLASTER_ANGLE = angle_attr_in;
    attrs.x14_FOX_BLASTER_VEL = vel_attr_in;
    attrs.x1C_FOX_BLASTER_SHOT_ITKIND = kind_in;
    Fighter fp;
    reset_fighter(&fp, &attrs);
    fp.facing_dir = facing_dir_in;
    fp.cmd_vars[2] = cmd_vars2_in;
    Fighter_GObj gobj = { &fp };
    reset_captures();
    ftFx_SpecialN_CreateBlasterShot(&gobj);
    *out_fired = it_8029C6A4_calls;
    *out_angle = captured_launch_angle;
    *out_speed = captured_launch_speed;
    *out_kind = captured_launch_kind;
}
