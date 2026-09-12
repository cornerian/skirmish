/* Host adapter for the match-start warp-in (Entry/EntryStart/EntryEnd,
 * `ft_0C31.c`, `entry.functions.json`, `docs/match-start.md`).
 *
 * Only `!x221F_b4` (Skirmish's single-fighter-per-port scope) is exercised;
 * the secondary-entity branch's helper stubs below (`Player_GetEntityAtIndex`
 * returning NULL) exist only so the verbatim extracted code compiles.
 * `Fighter_ChangeMotionState` captures the destination motion id;
 * `ftCommon_8007D92C` (EntryEnd's real exit dispatcher, outside this file)
 * and every JObj/effect/audio/accessory call are no-ops that only record
 * whether they fired, matching the taunt oracle's own stubbing convention.
 * `p_ftCommonData` is the pinned global timer-configuration struct
 * (`ft/types.h:473-476`); thread-local so concurrent test calls do not
 * interfere. */
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

typedef uint8_t u8;
typedef int32_t s32;
typedef uint32_t u32;
typedef float f32;
typedef int FtMotionId;
typedef struct {
    float x, y, z;
} Vec3;
typedef struct {
    float w, x, y, z;
} Quaternion;

enum { ftCo_MS_Entry = 322, ftCo_MS_EntryStart = 323, ftCo_MS_EntryEnd = 324 };

typedef struct {
    s32 x6B0, x6B4, x6B8;
    s32 x6BC;
    s32 x6C0;
    f32 x6C4;
    s32 x6C8;
} ftCommonData;
static _Thread_local ftCommonData common_data;
#define p_ftCommonData (&common_data)

typedef struct {
    s32 timer;
    Vec3 x8;
    Vec3 x14;
    f32 x4;
    f32 x20;
    f32 x24;
    f32 x28;
} EntryMv;

typedef struct {
    f32 trophy_scale;
} CoAttrs;

typedef struct Fighter_GObj Fighter_GObj;
typedef Fighter_GObj HSD_GObj;

typedef struct Fighter {
    struct {
        struct {
            EntryMv entry;
        } co;
    } mv;
    Vec3 cur_pos;
    Vec3 x34_scale;
    CoAttrs co_attrs;
    f32 facing_dir;
    s32 player_id;
    bool x221F_b4;
    bool x2219_b1;
    bool x221E_b1, x221E_b2, x221F_b1;
    void* x20A0_accessory;
    void (*accessory1_cb)(Fighter_GObj*);
    u8 x60C[64];
} Fighter;
struct Fighter_GObj {
    Fighter* user_data;
    void* hsd_obj;
};

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define GET_JOBJ(gobj) ((void*) 0)
/* Referenced only as an accessory-callback constant passed to
 * `ftCommon_SetAccessory`, which this adapter stubs to a no-op; the real
 * value (a per-character JObj-loading callback) never matters here. */
#define Fighter_804D6514 ((void*) 0)

static _Thread_local FtMotionId captured_msid;
static _Thread_local int change_motion_state_calls;
static void Fighter_ChangeMotionState(Fighter_GObj* gobj, FtMotionId msid, int flags, f32 start,
                                       f32 rate, f32 blend, void* cb) {
    (void) flags;
    (void) start;
    (void) rate;
    (void) blend;
    (void) cb;
    (void) gobj;
    captured_msid = msid;
    change_motion_state_calls++;
}

static void HSD_JObjSetScale(void* jobj, Vec3* scale) {
    (void) jobj;
    (void) scale;
}
static void HSD_JObjSetRotation(void* jobj, Quaternion* rotation) {
    (void) jobj;
    (void) rotation;
}
static void HSD_JObjSetTranslate(void* jobj, Vec3* translate) {
    (void) jobj;
    (void) translate;
}
static void HSD_JObjSetTranslateWithMtxDirtyOutOfLine(void* jobj, Vec3* translate) {
    (void) jobj;
    (void) translate;
}
static void ftCommon_SetAccessory(Fighter* fp, void* cb) {
    (void) fp;
    (void) cb;
}
static f32 ftCommon_800804EC(Fighter* fp) {
    (void) fp;
    return 0.0f;
}
static void efAsync_Spawn(Fighter_GObj* gobj, void* unk, int kind, int id, void* jobj, void* pos) {
    (void) gobj;
    (void) unk;
    (void) kind;
    (void) id;
    (void) jobj;
    (void) pos;
}
static void lbAudioAx_80024304(int id) { (void) id; }
static void ftCo_800BFFD0(Fighter* fp, int a, int b) {
    (void) fp;
    (void) a;
    (void) b;
}
static Fighter_GObj* Player_GetEntityAtIndex(s32 player_id, int index) {
    (void) player_id;
    (void) index;
    return (Fighter_GObj*) 0;
}
static _Thread_local bool flags_bit4;
static bool Player_GetFlagsBit4(s32 player_id) {
    (void) player_id;
    return flags_bit4;
}
static _Thread_local int invincibility_calls;
static _Thread_local s32 invincibility_value;
static void ftColl_8007B760(Fighter_GObj* gobj, s32 frames) {
    (void) gobj;
    invincibility_calls++;
    invincibility_value = frames;
}
static _Thread_local int exit_calls;
static void ftCommon_8007D92C(Fighter_GObj* gobj) {
    (void) gobj;
    exit_calls++;
}

/* Referenced only as function-pointer values (`accessory1_cb`); never
 * invoked by anything this oracle calls (both are purely cosmetic warp-star
 * follow callbacks). */
static void fn_800C69F4(Fighter_GObj* gobj) { (void) gobj; }
static void fn_800C6F34(Fighter_GObj* gobj) { (void) gobj; }

void ftCo_800C6408(Fighter_GObj* gobj);
void ftCo_800C6B6C(Fighter_GObj* gobj);

#include "entry_original.inc"

static void reset_fighter(Fighter* fp) {
    memset(fp, 0, sizeof(*fp));
}

/* `ftCo_Entry_Anim`. `timer` is `mv.co.entry.timer` in/out. `trophy_scale`
 * and `scale_y` feed the EntryStart entry (`ftCo_800C6408`) when this call
 * transitions (`x34_scale.y` is `scale_y`, `co_attrs.trophy_scale` is
 * `trophy_scale`); `start_frames` is `p_ftCommonData->x6BC`. Returns whether
 * `Fighter_ChangeMotionState` fired this call (into EntryStart);
 * `*out_x20`/`*out_x24` are only meaningful when it did. */
int oracle_entry_anim(s32* timer, f32 trophy_scale, f32 scale_y, s32 start_frames, s32* out_timer,
                       f32* out_x20, f32* out_x24) {
    Fighter fp;
    reset_fighter(&fp);
    fp.mv.co.entry.timer = *timer;
    fp.x34_scale.y = scale_y;
    fp.co_attrs.trophy_scale = trophy_scale;
    common_data.x6BC = start_frames;
    Fighter_GObj gobj = {&fp};
    change_motion_state_calls = 0;
    ftCo_Entry_Anim(&gobj);
    *out_timer = fp.mv.co.entry.timer;
    *out_x20 = fp.mv.co.entry.x20;
    *out_x24 = fp.mv.co.entry.x24;
    return change_motion_state_calls > 0 && captured_msid == ftCo_MS_EntryStart;
}

/* `ftCo_EntryStart_Anim`, followed by `ftCo_EntryStart_Phys` on whatever
 * state is current afterward (matching the source's own per-frame Anim-
 * then-Phys order; a transition frame runs EntryEnd's own Phys here, not
 * EntryStart's). `x4`/`x20` are the fighter's fixed spawn anchor and
 * amplitude; `end_frames` is `p_ftCommonData->x6C0` (only read on a
 * transition). Returns whether this call transitioned into EntryEnd. */
int oracle_entry_start_frame(s32* timer, f32 x4, f32 x20, s32 start_frames, s32 end_frames,
                              s32* out_timer, f32* out_x28, f32* out_y) {
    Fighter fp;
    reset_fighter(&fp);
    fp.mv.co.entry.timer = *timer;
    fp.mv.co.entry.x4 = x4;
    fp.mv.co.entry.x20 = x20;
    common_data.x6BC = start_frames;
    common_data.x6C0 = end_frames;
    Fighter_GObj gobj = {&fp};
    change_motion_state_calls = 0;
    ftCo_EntryStart_Anim(&gobj);
    int transitioned = change_motion_state_calls > 0 && captured_msid == ftCo_MS_EntryEnd;
    if (transitioned) {
        ftCo_EntryEnd_Phys(&gobj);
    } else {
        ftCo_EntryStart_Phys(&gobj);
    }
    *out_timer = fp.mv.co.entry.timer;
    *out_x28 = fp.mv.co.entry.x28;
    *out_y = fp.cur_pos.y;
    return transitioned;
}

/* `ftCo_EntryEnd_Anim` then `ftCo_EntryEnd_Phys` (unconditionally: an exit
 * frame's Phys does not run under the real per-frame schedule once the
 * fighter has left Entry-family motion, so the caller should ignore
 * `*out_y`/`*out_x28` when `*out_exited` is set). `flag_bit4` scripts
 * `Player_GetFlagsBit4`; `invincibility_frames` is `x6C8`. */
int oracle_entry_end_frame(s32* timer, f32 x4, f32 x20, s32 start_frames, bool flag_bit4,
                            s32 invincibility_frames, s32* out_timer, f32* out_x28, f32* out_y,
                            int* out_invincibility_applied, s32* out_invincibility_value) {
    Fighter fp;
    reset_fighter(&fp);
    fp.mv.co.entry.timer = *timer;
    fp.mv.co.entry.x4 = x4;
    fp.mv.co.entry.x20 = x20;
    common_data.x6BC = start_frames;
    common_data.x6C8 = invincibility_frames;
    flags_bit4 = flag_bit4;
    Fighter_GObj gobj = {&fp};
    exit_calls = 0;
    invincibility_calls = 0;
    ftCo_EntryEnd_Anim(&gobj);
    int exited = exit_calls > 0;
    if (!exited) {
        ftCo_EntryEnd_Phys(&gobj);
    }
    *out_timer = fp.mv.co.entry.timer;
    *out_x28 = fp.mv.co.entry.x28;
    *out_y = fp.cur_pos.y;
    *out_invincibility_applied = invincibility_calls;
    *out_invincibility_value = invincibility_value;
    return exited;
}

/* Chains a fresh Entry->EntryStart transition frame's `ftCo_Entry_Anim`
 * (timer==0, so it transitions) directly into `ftCo_EntryStart_Phys` within
 * the same call, exactly matching the real per-frame Anim-then-Phys order
 * for the transition frame itself -- unlike `oracle_entry_start_frame`,
 * which re-runs `ftCo_EntryStart_Anim`'s own decrement first, appropriate
 * for a *steady-state* EntryStart frame, not the transition frame. Added
 * for the Yoshi's Story/Fountain of Dreams real-replay parity loop
 * (docs/parity.md): the transition frame (`t = 1 / start_frames`) is the
 * one case the existing proptests never chained, and it is where
 * `fox-ys.slp`/`fox-fod.slp` show a real-replay position.y gap this
 * function proves is not a Skirmish arithmetic bug (see
 * `entry_transition_frame_matches_the_oracle_bit_exactly` below). */
void oracle_entry_transition_frame(f32 trophy_scale, f32 scale_y, f32 x4, s32 start_frames,
                                    s32* out_timer, f32* out_x20, f32* out_y) {
    Fighter fp;
    reset_fighter(&fp);
    fp.mv.co.entry.timer = 0;
    fp.mv.co.entry.x4 = x4;
    fp.x34_scale.y = scale_y;
    fp.co_attrs.trophy_scale = trophy_scale;
    common_data.x6BC = start_frames;
    Fighter_GObj gobj = {&fp};
    ftCo_Entry_Anim(&gobj);
    ftCo_EntryStart_Phys(&gobj);
    *out_timer = fp.mv.co.entry.timer;
    *out_x20 = fp.mv.co.entry.x20;
    *out_y = fp.cur_pos.y;
}

/* `ftCo_800C6408` standalone (the EntryStart entry/init). */
void oracle_entry_start_enter(f32 trophy_scale, f32 scale_y, s32 start_frames, s32* out_timer,
                               f32* out_x24, f32* out_x20) {
    Fighter fp;
    reset_fighter(&fp);
    fp.x34_scale.y = scale_y;
    fp.co_attrs.trophy_scale = trophy_scale;
    common_data.x6BC = start_frames;
    Fighter_GObj gobj = {&fp};
    ftCo_800C6408(&gobj);
    *out_timer = fp.mv.co.entry.timer;
    *out_x24 = fp.mv.co.entry.x24;
    *out_x20 = fp.mv.co.entry.x20;
}

/* `ftCo_800C6B6C` standalone (the EntryEnd entry/init). */
void oracle_entry_end_enter(f32 x4, f32 x20, s32 end_frames, s32* out_timer, f32* out_y) {
    Fighter fp;
    reset_fighter(&fp);
    fp.mv.co.entry.x4 = x4;
    fp.mv.co.entry.x20 = x20;
    common_data.x6C0 = end_frames;
    Fighter_GObj gobj = {&fp};
    ftCo_800C6B6C(&gobj);
    *out_timer = fp.mv.co.entry.timer;
    *out_y = fp.cur_pos.y;
}
