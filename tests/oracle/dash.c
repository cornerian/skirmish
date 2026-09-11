/* Host adapter for the complete Dash-phase and AttackDash input dispatchers:
 * ftCo_Dash_CheckInput, ftCo_Dash_Enter, ftCo_Dash_IASA, the dash-attack
 * entry family (ftCo_AttackDash_CheckInput/decideFighter/doEnter/SetMv0/
 * IASA/Phys) and the shared catch (ftCo_800D8A38, ftCo_800D8AE0) and guard
 * (ftCo_80091AD8, ftCo_80091A4C, ftCo_80091B9C) predicates. Every callee that
 * is not one of those pinned functions returns a scripted answer decoded
 * from a bitmask, except two genuinely unconditional void actions
 * (ftCo_Turn_Enter_Smash and the catch transition), which are simply
 * recorded when reached, and the item/Kirby branches, which are disabled by
 * construction (item_gobj is always NULL, kind is never FTKIND_KIRBY) and
 * never execute at runtime. `inlineA0` is reproduced verbatim (a single
 * field read) because the source defines it once, above the pinned guard
 * functions, and `extract_function` only pulls the named function itself.
 *
 * ftCo_Dash_IASA's own dispatch order is traced by `oracle_dash_frame`:
 * ftCo_800D8A38 (catch), ftCo_Dash_CheckInput, ftCo_80091AD8/ftCo_80091A4C
 * (shield) and ftCo_80091B9C stay the exact pinned bodies, but are compiled
 * under renamed symbols and reintroduced through thin wrappers (matching
 * their forward-declared original names) that log each consultation before
 * returning the real result. ftCo_Dash_CheckInput needs its own alias
 * extraction (`dash_checkinput`) from the same `dash.c` original, because
 * its definition and its two call sites inside ftCo_Dash_IASA live in the
 * same snapshot; renaming one via a single #include-scoped macro would have
 * renamed the other. ftCo_AttackDash_CheckInput is wrapped the same way,
 * even though its definition and call site are in different snapshots, for
 * a uniform pattern. The remaining non-pinned callees (SpecialS, the
 * forward-smash/roll/taunt/jump/run checks) are simple `answers`-bitmask
 * stubs, also logged.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef float f32;
typedef uint8_t u8;
typedef int32_t s32;
typedef struct {
    float x;
} Vec1;
typedef struct Fighter_GObj Fighter_GObj;
typedef Fighter_GObj HSD_GObj;
typedef int FtMotionId;

typedef struct Fighter {
    struct {
        Vec1 lstick[1];
        uint32_t held_buttons[1];
        uint32_t pressed_buttons;
    } input;
    float facing_dir;
    float gr_vel;
    int kind;
    void *item_gobj;
    void *x197C;
    int allow_interrupt;
    int cmd_vars[1];
    uint8_t x670_timer_lstick_tilt_x;
    float cur_anim_frame;
    struct {
        float dash_initial_velocity;
        float ground_friction;
    } co_attrs;
    struct {
        /* mv.co is a real union in the source: ftCo_800D8AE0 reads/writes
         * mv.co.common.x0, which is the exact same storage as
         * mv.co.attackdash.x0 while an AttackDash instance owns it. */
        union {
            struct {
                float x0;
                int x4;
            } dash;
            struct {
                float x0;
            } attackdash;
            struct {
                float x0;
            } common;
            struct {
                int x20;
                float x24;
            } guard;
        } co;
    } mv;
    int trigger_analog_timer;
    float shield_health;
} Fighter;
struct Fighter_GObj {
    Fighter *user_data;
};

typedef struct {
    float dash_smash_stick_threshold;
    int dash_smash_window;
    float x44, x48, x4C, x50, x54, x68;
    int powershield_input_window;
} ftCommonData;

#define GET_FIGHTER(g) ((g)->user_data)
#define PAD_STACK(n)
#define RETURN_IF(cond) \
    do {                \
        if (cond)       \
            return;     \
    } while (0)
#define HSD_PAD_A 0x100
#define HSD_PAD_L 0x40
#define HSD_PAD_R 0x20
#define HSD_PAD_LR (HSD_PAD_L | HSD_PAD_R)
#define Ft_MF_None 0
#define FTKIND_KIRBY 999

enum { ftCo_MS_Dash = 20, ftCo_MS_Run = 21, ftCo_MS_AttackDash = 50,
       ftCo_MS_Catch = 212, ftCo_MS_CatchDash = 214,
       ftCo_MS_LightThrowF4 = 243, ftCo_MS_LightThrowDash = 246 };
/* Not a real motion state: a sentinel so the caller can tell
 * ftCo_Turn_Enter_Smash apart from an ordinary Fighter_ChangeMotionState. */
enum { TURN_ENTER_SMASH_SENTINEL = -1 };

/* Call-order trace codes, shared with tests/dash_differential.rs. */
enum {
    CALL_SPECIAL_S = 1,
    CALL_CATCH = 2,
    CALL_ATTACK_S4 = 3,
    CALL_ROLL = 4,
    CALL_ATTACKDASH_CHECKINPUT = 5,
    CALL_DASH_CHECKINPUT = 6,
    CALL_SHIELD_AD8 = 7,
    CALL_SHIELD_A4C = 8,
    CALL_SHIELD_B9C = 9,
    CALL_TAUNT = 10,
    CALL_JUMP_CAF78 = 11,
    CALL_RUN_CA5F0 = 12,
    CALL_FRICTION = 13,
};

static _Thread_local ftCommonData common;
_Thread_local ftCommonData *p_ftCommonData;
static _Thread_local uint32_t answers;
static _Thread_local int captured_motion;
static _Thread_local int captured_flags;
static _Thread_local int guard_action; /* 0 none, 1 powershield, 2 guard */
static _Thread_local bool wait_iasa_opened;
static _Thread_local float captured_phys_friction;
static _Thread_local float captured_phys_facing;
static _Thread_local uint8_t call_log[32];
static _Thread_local uint8_t call_count;
static _Thread_local uint8_t fired_code;

/* Pushes `code` to the trace unconditionally; when `is_gate`, a true
 * `result` also (first-wins) records the code that decided this frame. */
static void record_call(uint8_t code, bool is_gate, bool result)
{
    if (call_count < sizeof call_log) {
        call_log[call_count++] = code;
    }
    if (is_gate && result && fired_code == 0) {
        fired_code = code;
    }
}

enum {
    ANSWER_SPECIAL_S = 1 << CALL_SPECIAL_S,
    ANSWER_ATTACK_S4 = 1 << CALL_ATTACK_S4,
    ANSWER_ROLL = 1 << CALL_ROLL,
    ANSWER_TAUNT = 1 << CALL_TAUNT,
    ANSWER_JUMP_CAF78 = 1 << CALL_JUMP_CAF78,
    ANSWER_RUN_CA5F0 = 1 << CALL_RUN_CA5F0,
};
#define ANSWER(bit) ((answers & (bit)) != 0)

static void Fighter_ChangeMotionState(Fighter_GObj *gobj, int msid, int flags,
                                      float start, float rate, float blend,
                                      void *callback)
{
    (void) gobj;
    (void) start;
    (void) rate;
    (void) blend;
    (void) callback;
    captured_motion = msid;
    captured_flags = flags;
}
static void ftAnim_8006EBA4(Fighter_GObj *gobj) { (void) gobj; }
static void ftCommon_800804A0(Fighter *fp, float value) { (void) fp; (void) value; }
static void ft_PlaySFX(Fighter *fp, int a, int b, int c)
{
    (void) fp;
    (void) a;
    (void) b;
    (void) c;
}
static void ftCo_Turn_Enter_Smash(Fighter_GObj *gobj)
{
    (void) gobj;
    captured_motion = TURN_ENTER_SMASH_SENTINEL;
}
static bool ftCo_SpecialS_CheckInput(Fighter_GObj *gobj)
{
    (void) gobj;
    bool result = ANSWER(ANSWER_SPECIAL_S);
    record_call(CALL_SPECIAL_S, true, result);
    return result;
}
static bool ftCo_80094E54(Fighter *fp) { (void) fp; return false; }
static void ftCo_800957F4(Fighter_GObj *gobj, int msid)
{
    (void) gobj;
    captured_motion = msid;
}
static bool ftCo_AttackS4_8008C114(Fighter_GObj *gobj)
{
    (void) gobj;
    bool result = ANSWER(ANSWER_ATTACK_S4);
    record_call(CALL_ATTACK_S4, true, result);
    return result;
}
static bool ftCo_80099264(Fighter_GObj *gobj)
{
    (void) gobj;
    bool result = ANSWER(ANSWER_ROLL);
    record_call(CALL_ROLL, true, result);
    return result;
}
/* The taunt check (D-pad up): ftCo_800DE9B8/ftCo_800DE9D8, ftCo_AppealS.c:
 * 35,43. Scripted false everywhere else in this adapter (taunts are
 * unmodeled); block_42 always returns without touching gr_vel in that case,
 * and the shared friction tail is reached only past a taunt entry. */
static bool ftCo_800DE9D8(Fighter_GObj *gobj)
{
    (void) gobj;
    bool result = ANSWER(ANSWER_TAUNT);
    record_call(CALL_TAUNT, true, result);
    return result;
}
static bool fn_800CAF78(Fighter_GObj *gobj)
{
    (void) gobj;
    bool result = ANSWER(ANSWER_JUMP_CAF78);
    record_call(CALL_JUMP_CAF78, true, result);
    return result;
}
static bool fn_800CA5F0(Fighter_GObj *gobj)
{
    (void) gobj;
    bool result = ANSWER(ANSWER_RUN_CA5F0);
    record_call(CALL_RUN_CA5F0, true, result);
    return result;
}
/* The shared friction tail's only call, so it is the hook point for logging
 * that the tail was reached. */
static float ft_GetGroundFrictionMultiplier(Fighter *fp)
{
    (void) fp;
    record_call(CALL_FRICTION, false, true);
    return 1.0F;
}

/* Item and Kirby branches are unreachable (item_gobj is always NULL, kind is
 * never FTKIND_KIRBY) but still need to compile. */
static int it_8026B30C(void *item_gobj) { (void) item_gobj; return 0; }
static void ftCo_Attack_800CCF58(HSD_GObj *gobj, int arg) { (void) gobj; (void) arg; }
static void ftKb_SpecialN_800F1F68(Fighter_GObj *gobj) { (void) gobj; }

/* ftCo_800D8A38/ftCo_800D8AE0's own item/tether gates. ftCo_80095254 and
 * fn_800D8E94/fn_800D952C are hardcoded to let the ordinary LR-held/A-press
 * (or buffer) logic decide, matching the pinned `catch.c` adapter's own
 * precedent; ftCo_800952DC likewise for ftCo_800D8AE0. */
static bool ftCo_80095254(Fighter_GObj *gobj) { (void) gobj; return false; }
static bool ftCo_800952DC(Fighter_GObj *gobj) { (void) gobj; return false; }
static bool fn_800D8E94(Fighter_GObj *gobj) { (void) gobj; return true; }
static bool fn_800D952C(Fighter_GObj *gobj) { (void) gobj; return true; }
static void ftCo_800D8C54(Fighter_GObj *gobj, FtMotionId msid)
{
    (void) gobj;
    captured_motion = msid;
}

/* Guard predicates share this inline gate, copied verbatim from the pinned
 * source's own `inlineA0` (defined once, above the pinned guard functions,
 * so `extract_function` cannot pull it in with them). */
static inline bool inlineA0(Fighter *fp)
{
    return fp->input.held_buttons[0] & HSD_PAD_LR ? true : false;
}
static void ftCo_800939B4(Fighter_GObj *gobj)
{
    (void) gobj;
    guard_action = 1;
}
static void ftCo_800923B4(Fighter_GObj *gobj)
{
    (void) gobj;
    guard_action = 2;
}

static void ftCo_Wait_IASA(Fighter_GObj *gobj)
{
    (void) gobj;
    wait_iasa_opened = true;
}

static void ft_80085030(Fighter_GObj *gobj, float gr_friction, float facing_dir)
{
    (void) gobj;
    captured_phys_friction = gr_friction;
    captured_phys_facing = facing_dir;
}

/* Forward declarations so the extracted functions can call each other and
 * across originals regardless of extraction order; `decideFighter` and
 * `doEnter` are static in the source. The five renamed-and-wrapped pinned
 * functions below (catch, AttackDash's CheckInput, Dash's own CheckInput,
 * both guard entries and the guard buffer arm) also need their original
 * names declared here, since `dash_original.inc`'s `ftCo_Dash_IASA` body
 * calls them by those names. */
void ftCo_Dash_Enter(Fighter_GObj *gobj, int arg1);
bool ftCo_AttackDash_CheckInput(HSD_GObj *gobj);
void ftCo_AttackDash_SetMv0(HSD_GObj *gobj);
static void decideFighter(Fighter_GObj *gobj);
static void doEnter(Fighter_GObj *gobj);
bool ftCo_800D8A38(Fighter_GObj *gobj);
bool ftCo_Dash_CheckInput(Fighter_GObj *gobj);
bool ftCo_80091AD8(Fighter_GObj *gobj, int mv_x20);
bool ftCo_80091A4C(Fighter_GObj *gobj);
void ftCo_80091B9C(Fighter_GObj *gobj);

#define ftCo_800D8A38 pinned_ftCo_800D8A38
#include "dash_catch_original.inc"
#undef ftCo_800D8A38

#define ftCo_80091AD8 pinned_ftCo_80091AD8
#define ftCo_80091A4C pinned_ftCo_80091A4C
#define ftCo_80091B9C pinned_ftCo_80091B9C
#include "dash_guard_original.inc"
#undef ftCo_80091AD8
#undef ftCo_80091A4C
#undef ftCo_80091B9C

#define ftCo_Dash_CheckInput pinned_ftCo_Dash_CheckInput
#include "dash_checkinput_original.inc"
#undef ftCo_Dash_CheckInput

#include "dash_original.inc"

#define ftCo_AttackDash_CheckInput pinned_ftCo_AttackDash_CheckInput
#include "attack_dash_original.inc"
#undef ftCo_AttackDash_CheckInput

/* Thin wrappers over the pinned bodies above, reintroduced under their
 * original names (which `ftCo_Dash_IASA`'s pinned body already calls) so
 * every consultation is logged before returning the exact real result. */
bool ftCo_800D8A38(Fighter_GObj *gobj)
{
    bool result = pinned_ftCo_800D8A38(gobj);
    record_call(CALL_CATCH, true, result);
    return result;
}
bool ftCo_Dash_CheckInput(Fighter_GObj *gobj)
{
    bool result = pinned_ftCo_Dash_CheckInput(gobj);
    record_call(CALL_DASH_CHECKINPUT, true, result);
    return result;
}
bool ftCo_80091AD8(Fighter_GObj *gobj, int mv_x20)
{
    bool result = pinned_ftCo_80091AD8(gobj, mv_x20);
    record_call(CALL_SHIELD_AD8, true, result);
    return result;
}
bool ftCo_80091A4C(Fighter_GObj *gobj)
{
    bool result = pinned_ftCo_80091A4C(gobj);
    record_call(CALL_SHIELD_A4C, true, result);
    return result;
}
void ftCo_80091B9C(Fighter_GObj *gobj)
{
    pinned_ftCo_80091B9C(gobj);
    record_call(CALL_SHIELD_B9C, false, true);
}
bool ftCo_AttackDash_CheckInput(HSD_GObj *gobj)
{
    bool result = pinned_ftCo_AttackDash_CheckInput(gobj);
    record_call(CALL_ATTACKDASH_CHECKINPUT, true, result);
    return result;
}

static void reset_captures(uint32_t scripted)
{
    answers = scripted;
    captured_motion = 0;
    captured_flags = 0;
    guard_action = 0;
    wait_iasa_opened = false;
    captured_phys_friction = 0.0F;
    captured_phys_facing = 0.0F;
    call_count = 0;
    fired_code = 0;
    for (size_t i = 0; i < sizeof call_log; i++) {
        call_log[i] = 0;
    }
}

/* rules: dash_smash_stick_threshold, dash_smash_window. state: stick x,
 * facing, tilt_x_age. Returns whether the dash restarted or a smash Turn
 * fired; *motion reports which (ftCo_MS_Dash or the Turn sentinel). */
int oracle_dash_check_input(const float *rules, float stick_x, float facing,
                            uint32_t tilt_x_age, float gr_vel, int *motion,
                            int *dash_x4)
{
    common = (ftCommonData) {
        .dash_smash_stick_threshold = rules[0],
        .dash_smash_window = (int) rules[1],
    };
    p_ftCommonData = &common;
    reset_captures(0);
    Fighter fighter = {
        .input = { .lstick = { { stick_x } } },
        .facing_dir = facing,
        .gr_vel = gr_vel,
        .x670_timer_lstick_tilt_x = (uint8_t) tilt_x_age,
        .co_attrs = { .dash_initial_velocity = 2.0F },
    };
    Fighter_GObj gobj = { &fighter };
    int result = pinned_ftCo_Dash_CheckInput(&gobj);
    *motion = captured_motion;
    *dash_x4 = fighter.mv.co.dash.x4;
    return result;
}

/* rules: x44, x48, x4C, x54, x68, dash_smash_stick_threshold,
 * dash_smash_window, powershield_input_window. state: dash_from_input (x4),
 * frame, gr_vel, facing, stick_x, tilt_x_age, trigger_analog_timer,
 * cmd_vars0 (Run's turn flag, unused by Dash), shield_health. buttons:
 * pressed, held (HSD_PAD_A/HSD_PAD_LR bits). answers: the scripted bitmask
 * above (bit N set means callee N's stub answers true; codes 2/5/6/7/8/9
 * are the real pinned bodies instead, driven by `pressed`/`held`/
 * `shield_health`/`stick_x`/`tilt_x_age`, not by `answers`). Outputs the
 * branch fired via the motion, guard_action and gr_vel_after out-parameters,
 * plus the full ordered call trace (out_calls, sized at least 16; out_count
 * many entries) and which code (0 = none) ultimately fired. */
int oracle_dash_frame(const float *rules, const float *state, uint32_t pressed,
                      uint32_t held, uint32_t scripted, int *motion,
                      int *guard_action_out, float *gr_vel_after,
                      int *dash_x4_after, uint8_t *out_calls,
                      uint8_t *out_count, uint8_t *out_fired)
{
    common = (ftCommonData) {
        .x44 = rules[0], .x48 = rules[1], .x4C = rules[2], .x54 = rules[3],
        .x68 = rules[4], .dash_smash_stick_threshold = rules[5],
        .dash_smash_window = (int) rules[6],
        .powershield_input_window = (int) rules[7],
    };
    p_ftCommonData = &common;
    reset_captures(scripted);
    Fighter fighter = {
        .input = { .lstick = { { state[4] } },
                  .held_buttons = { held },
                  .pressed_buttons = pressed },
        .facing_dir = state[3],
        .gr_vel = state[2],
        .x670_timer_lstick_tilt_x = (uint8_t) state[5],
        .cur_anim_frame = state[1],
        .trigger_analog_timer = (int) state[6],
        .shield_health = state[8],
        .cmd_vars = { (int) state[7] },
        .mv = { .co = { .dash = { .x4 = (int) state[0] } } },
        .co_attrs = { .dash_initial_velocity = 2.0F },
    };
    Fighter_GObj gobj = { &fighter };
    ftCo_Dash_IASA(&gobj);
    *motion = captured_motion;
    *guard_action_out = guard_action;
    *gr_vel_after = fighter.gr_vel;
    *dash_x4_after = fighter.mv.co.dash.x4;
    for (uint8_t i = 0; i < call_count && i < 16; i++) {
        out_calls[i] = call_log[i];
    }
    *out_count = call_count < 16 ? call_count : 16;
    *out_fired = fired_code;
    return captured_motion != 0 || guard_action != 0;
}

/* Runs ftCo_AttackDash_IASA once. buffer is mv.co.attackdash.x0; lr_held is
 * HSD_PAD_LR in held_buttons[0]; allow_interrupt selects the Wait-chain
 * exposure. Outputs whether CatchDash fired, the buffer afterward and
 * whether the Wait chain opened. */
int oracle_attack_dash_frame(uint32_t lr_held, float buffer, int allow_interrupt,
                             float *buffer_after, int *wait_opened)
{
    reset_captures(0);
    Fighter fighter = {
        .input = { .held_buttons = { lr_held ? HSD_PAD_LR : 0 } },
        .allow_interrupt = allow_interrupt,
        .mv = { .co = { .attackdash = { .x0 = buffer } } },
    };
    Fighter_GObj gobj = { &fighter };
    ftCo_AttackDash_IASA(&gobj);
    *buffer_after = fighter.mv.co.attackdash.x0;
    *wait_opened = wait_iasa_opened;
    return captured_motion == ftCo_MS_CatchDash;
}

/* ftCo_AttackDash_Phys, with ft_80085030 stubbed to capture its friction and
 * facing arguments (root-motion selection stays in the Rust resource). */
void oracle_attack_dash_phys(float x50, float ground_friction, float facing,
                             float *friction_out, float *facing_out)
{
    common = (ftCommonData) { .x50 = x50 };
    p_ftCommonData = &common;
    reset_captures(0);
    Fighter fighter = {
        .facing_dir = facing,
        .co_attrs = { .ground_friction = ground_friction },
    };
    Fighter_GObj gobj = { &fighter };
    ftCo_AttackDash_Phys(&gobj);
    *friction_out = captured_phys_friction;
    *facing_out = captured_phys_facing;
}
