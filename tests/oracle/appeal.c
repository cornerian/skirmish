/* Host adapter for the taunt (AppealSR/AppealSL) entry chain and IASA
 * (`ftCo_AppealS.c`, `appeal.functions.json`).
 *
 * `ftCo_800DEA28` is deliberately NOT extracted (it is outside this batch's
 * selection): it is hand-written here instead, reproducing its real
 * per-character switch with every side effect stubbed to a no-op. Every
 * oracle call below uses `kind == 0`, an ordinary fighter matching none of
 * `FTKIND_CLINK`/`FTKIND_DRMARIO`/`FTKIND_GANON`, so it always falls to the
 * `default` branch exactly like the real function and only the extracted
 * `ftCo_800DEBD0` -> `ftCo_800DEAE8` chain is ever exercised -- matching
 * `docs/taunt.md`'s "unmodeled" character side effects.
 *
 * `ftData_80085FD4` is a scripted per-animation-id availability table (>=
 * 512 entries, comfortably past every id this oracle uses); `Fighter_
 * ChangeMotionState` only captures the selected motion id and the fighter's
 * `allow_interrupt` at the moment of the call (real motion-blend/frame-rate
 * arguments are not compared). `DbLevel` is fixed at 0 (below
 * `DbLKind_DebugRom`), so the Peach/Zelda debug-only hooks in `ftCo_
 * 800DEBD0` never fire either. Every `ftCo_AppealS_IASA` `CheckInput`
 * dependency is stubbed to log its own call (in the source's own RETURN_IF
 * order) and answer from a caller-supplied bitmask, so `oracle_taunt_iasa`
 * can both script which check "wins" and confirm the exact consultation
 * order against a Rust mirror. Thread-local state isolates independent
 * concurrent test calls, not gameplay global state. */
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

typedef uint8_t u8;
typedef int32_t s32;
typedef uint32_t u32;
typedef float f32;
typedef int FtMotionId;
typedef struct { float x, y, z; } Vec3;

#define HSD_PAD_DPADUP 0x8

#define RETURN_IF(cond) \
    do { \
        if (cond) { \
            return; \
        } \
    } while (0)

#define FTKIND_CLINK 100
#define FTKIND_DRMARIO 101
#define FTKIND_GANON 102
#define FTKIND_PEACH 103
#define FTKIND_ZELDA 104
#define FTKIND_KIRBY 105
#define DbLKind_DebugRom 1

enum { ftCo_MS_AppealSR = 264, ftCo_MS_AppealSL = 265 };

typedef struct { u32 pressed_buttons; } FighterInput;

typedef struct { s32 anim_id; } MotionState;

/* Sized comfortably past every animation id this oracle uses (>= 512, per
 * the task note). */
#define ANIM_TABLE_SIZE 512
#define ACTION_STATE_COUNT 512

typedef struct { s32 x8; } AnimData;

typedef struct Fighter {
    FighterInput input;
    bool allow_interrupt;
    float facing_dir;
    s32 kind;
    s32 player_id;
    s32 x221F_b4;
    s32 x18;
    MotionState x1C_actionStateList[ACTION_STATE_COUNT];
    MotionState x20_actionStateList[ACTION_STATE_COUNT];
} Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;

#define GET_FIGHTER(gobj) ((gobj)->user_data)

static int DbLevel = 0;

static _Thread_local AnimData anim_table[ANIM_TABLE_SIZE];
static AnimData* ftData_80085FD4(Fighter* fp, s32 anim_id) {
    (void) fp;
    if (anim_id < 0 || anim_id >= ANIM_TABLE_SIZE) {
        return &anim_table[0];
    }
    return &anim_table[anim_id];
}

static _Thread_local FtMotionId captured_msid;
static _Thread_local bool captured_allow_interrupt;
static _Thread_local int change_motion_state_calls;
static void Fighter_ChangeMotionState(Fighter_GObj* gobj, FtMotionId msid, int flags, f32 start,
                                       f32 rate, f32 blend, void* cb) {
    (void) flags;
    (void) start;
    (void) rate;
    (void) blend;
    (void) cb;
    Fighter* fp = GET_FIGHTER(gobj);
    captured_msid = msid;
    captured_allow_interrupt = fp->allow_interrupt;
    change_motion_state_calls++;
}

/* Every per-character side effect `ftCo_800DEA28`/`ftCo_800DEBD0` can reach
 * is a no-op; only their control flow (which branch is taken, and in what
 * order) matters here. */
static void ftCl_Init_80149318(Fighter_GObj* gobj) { (void) gobj; }
static void ftDr_Init_80149910(Fighter_GObj* gobj) { (void) gobj; }
static void lb_8000B1CC(void* joint, void* unused, Vec3* pos) {
    (void) joint;
    (void) unused;
    (void) pos;
}
static void lb_800119DC(Vec3* pos, int a, f32 b, f32 c, f32 d) {
    (void) pos;
    (void) a;
    (void) b;
    (void) c;
    (void) d;
}
static void pl_80040120(s32 player_id, s32 x221F_b4) {
    (void) player_id;
    (void) x221F_b4;
}
static void ftPe_Init_8011B93C(Fighter_GObj* gobj) { (void) gobj; }
static void ftZd_Init_801395C8(Fighter_GObj* gobj) { (void) gobj; }
static void ftKb_SpecialN_800F5D04(Fighter_GObj* gobj, bool a) {
    (void) gobj;
    (void) a;
}

/* `ftCo_800DEBD0` is extracted below (`appeal_original.inc`); declared here
 * (non-static, matching its real linkage) so this hand-written stand-in for
 * `ftCo_800DEA28` can call it before that definition appears. */
void ftCo_800DEBD0(Fighter_GObj* gobj);

static void ftCo_800DEA28(Fighter_GObj* gobj) {
    Fighter* fp = GET_FIGHTER(gobj);
    switch (fp->kind) {
    case FTKIND_CLINK:
        ftCl_Init_80149318(gobj);
        break;
    case FTKIND_DRMARIO:
        ftDr_Init_80149910(gobj);
        break;
    case FTKIND_GANON: {
        Vec3 pos;
        lb_8000B1CC(0, 0, &pos);
        lb_800119DC(&pos, 80, 1.0f, 0.003f, 1.0471976f);
        ftCo_800DEBD0(gobj);
    }
    default:
        ftCo_800DEBD0(gobj);
        break;
    }
    pl_80040120(fp->player_id, fp->x221F_b4);
}

/* Every `ftCo_AppealS_IASA` dependency: logs its own call (source order) to
 * `iasa_calls` and answers from the matching bit of `iasa_mask`. */
static _Thread_local u32 iasa_mask;
static _Thread_local int iasa_calls[16];
static _Thread_local int iasa_call_count;
#define CHECK_STUB(name, index) \
    static bool name(Fighter_GObj* gobj) { \
        (void) gobj; \
        iasa_calls[iasa_call_count++] = index; \
        return (iasa_mask >> index) & 1u; \
    }
CHECK_STUB(ftCo_SpecialS_CheckInput, 0)
CHECK_STUB(ftCo_Attack100_CheckInput, 1)
CHECK_STUB(ftCo_800D6824, 2)
CHECK_STUB(ftCo_800D68C0, 3)
CHECK_STUB(ftCo_Catch_CheckInput, 4)
CHECK_STUB(ftCo_AttackS4_CheckInput, 5)
CHECK_STUB(ftCo_AttackHi4_CheckInput, 6)
CHECK_STUB(ftCo_AttackLw4_CheckInput, 7)
CHECK_STUB(ftCo_AttackS3_CheckInput, 8)
CHECK_STUB(ftCo_AttackHi3_CheckInput, 9)
CHECK_STUB(ftCo_AttackLw3_CheckInput, 10)
CHECK_STUB(ftCo_Attack1_CheckInput, 11)
CHECK_STUB(ftCo_80099794, 12)
CHECK_STUB(ftCo_80091A4C, 13)

#include "appeal_original.inc"

static void reset_fighter(Fighter* fp) {
    memset(fp, 0, sizeof(*fp));
    /* Keeps both AppealSR (264) and AppealSL (265) inside x1C_actionStateList
     * for every call: msid1 >= fp->x18 selects x20_actionStateList instead,
     * which this oracle never populates. */
    fp->x18 = 100000;
}

/* Complete `ftCo_800DE9D8` (which dispatches through `ftCo_800DEA28` ->
 * `ftCo_800DEBD0` -> `ftCo_800DEAE8` for an ordinary, kind-0 fighter):
 * `pressed` scripts the D-pad-up press, `facing` is `fp->facing_dir`, and
 * `left_available` scripts AppealSL's own figatree-availability flag
 * (`ftData_80085FD4(fp, ms->anim_id)->x8`). Returns whether the taunt fired;
 * `*out_msid` is the motion id `Fighter_ChangeMotionState` was called with
 * (0 if it was never called), and `*out_allow_interrupt` is `fp->
 * allow_interrupt` at that same call (must be false, matching `fp->
 * allow_interrupt = false;` in `ftCo_800DEAE8`, ahead of the call). */
int oracle_taunt_enter(bool pressed, f32 facing, bool left_available, FtMotionId* out_msid,
                        bool* out_allow_interrupt) {
    Fighter fp;
    reset_fighter(&fp);
    fp.input.pressed_buttons = pressed ? HSD_PAD_DPADUP : 0;
    fp.facing_dir = facing;
    fp.x1C_actionStateList[ftCo_MS_AppealSR].anim_id = 239;
    fp.x1C_actionStateList[ftCo_MS_AppealSL].anim_id = 240;
    anim_table[240].x8 = left_available ? 1 : 0;
    Fighter_GObj gobj = { &fp };
    captured_msid = 0;
    captured_allow_interrupt = true;
    change_motion_state_calls = 0;
    int fired = ftCo_800DE9D8(&gobj);
    *out_msid = change_motion_state_calls > 0 ? captured_msid : 0;
    *out_allow_interrupt = captured_allow_interrupt;
    return fired;
}

/* Complete `ftCo_AppealS_IASA`: `allow_interrupt` scripts the entry gate,
 * `answers` is the per-check response bitmask, `out_calls`/`out_count`
 * (capacity 16) report every check reached in the source's own RETURN_IF
 * order. Returns whether any check fired (the RETURN_IF chain returned
 * early on it) -- false when `allow_interrupt` is false (no check runs at
 * all) or every check answers false. */
int oracle_taunt_iasa(bool allow_interrupt, u32 answers, int* out_calls, int* out_count) {
    Fighter fp;
    reset_fighter(&fp);
    fp.allow_interrupt = allow_interrupt;
    Fighter_GObj gobj = { &fp };
    iasa_mask = answers;
    iasa_call_count = 0;
    ftCo_AppealS_IASA(&gobj);
    *out_count = iasa_call_count;
    for (int i = 0; i < iasa_call_count; i++) {
        out_calls[i] = iasa_calls[i];
    }
    return iasa_call_count > 0 && ((answers >> iasa_calls[iasa_call_count - 1]) & 1u);
}
