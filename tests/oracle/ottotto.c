/* Host adapter for the pinned Ottotto/OttottoWait dispatch:
 * `ftCo_8009A3C8` (the edge-entry gate other `_Coll` callbacks call),
 * `ftCo_8009A410` (Ottotto entry), `ftCo_Ottotto_IASA` (the complete input
 * chain, traced call-by-call like the dash batch's `oracle_dash_frame`),
 * `ftCo_Ottotto_Coll`/`ftCo_OttottoWait_Coll` (the shared fall/exit
 * decision) and `ftCo_8009A6B8` (OttottoWait entry). Every callee is a
 * scripted stub; `ft_800827A0` (mode 2, already pinned separately as
 * `mpColl_8004A45C_Floor` in `tests/oracle/edge_floor.c`) is scripted
 * ground-lost/ground-kept here, and `mpFloorGetLeft`/`mpFloorGetRight`
 * answer the supplied floor-end X directly. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef float f32;
typedef uint8_t u8;
typedef uint32_t u32;
typedef struct {
    float x, y, z;
} Vec3;
typedef struct Fighter_GObj Fighter_GObj;
typedef Fighter_GObj HSD_GObj;

typedef struct {
    struct {
        int index;
    } floor;
    u32 env_flags;
} CollData;
typedef struct {
    void *x18;
} Sfx;
typedef struct {
    Sfx *x4C_sfx;
} FtData;

typedef struct Fighter {
    CollData coll_data;
    bool x2228_b2;
    Vec3 self_vel;
    float gr_vel;
    float facing_dir;
    Vec3 cur_pos;
    FtData *ft_data;
} Fighter;
struct Fighter_GObj {
    Fighter *user_data;
};

typedef struct {
    float x478, x47C;
} ftCommonData;

#define GET_FIGHTER(g) ((g)->user_data)
#define RETURN_IF(cond) \
    do {                \
        if (cond)       \
            return;     \
    } while (0)
#define Ft_MF_None 0
#define ABS(a) (((a) < 0) ? -(a) : (a))

enum { ftCo_MS_Ottotto = 245, ftCo_MS_OttottoWait = 246 };
enum { Collide_Edge = 1 << 2 };

/* Call-order trace codes for `ftCo_Ottotto_IASA`, in the exact pinned
 * source order. Shared with `tests/ottotto_differential.rs`. */
enum {
    CALL_SPECIAL_S = 1,
    CALL_ATTACK100 = 2,
    CALL_800D6824 = 3,
    CALL_800D68C0 = 4,
    CALL_CATCH = 5,
    CALL_ATTACK_S4 = 6,
    CALL_ATTACK_HI4 = 7,
    CALL_ATTACK_LW4 = 8,
    CALL_ATTACK_S3 = 9,
    CALL_ATTACK_HI3 = 10,
    CALL_ATTACK_LW3 = 11,
    CALL_ATTACK1 = 12,
    CALL_SHIELD_A4C = 13,
    CALL_TAUNT = 14,
    CALL_JUMP = 15,
    CALL_DASH = 16,
    CALL_SQUAT = 17,
    CALL_TURN = 18,
    CALL_WALK_OTTOTTO = 19,
};

static _Thread_local ftCommonData common;
_Thread_local ftCommonData *p_ftCommonData;
static _Thread_local uint32_t answers;
static _Thread_local int captured_motion;
static _Thread_local uint8_t call_log[32];
static _Thread_local uint8_t call_count;
static _Thread_local uint8_t fired_code;
static _Thread_local bool fall_entered;
static _Thread_local bool wait_entered;
static _Thread_local Vec3 scripted_edge;
static _Thread_local bool scripted_ground_ok;

static void record_call(uint8_t code, bool result)
{
    if (call_count < sizeof call_log) {
        call_log[call_count++] = code;
    }
    if (result && fired_code == 0) {
        fired_code = code;
    }
}
#define ANSWER(code) ((answers & (1u << (code))) != 0)

static void Fighter_ChangeMotionState(Fighter_GObj *gobj, int msid, int flags, float start,
                                      float rate, float blend, void *callback)
{
    (void) gobj;
    (void) flags;
    (void) start;
    (void) rate;
    (void) blend;
    (void) callback;
    captured_motion = msid;
}
static void ft_80088770(Fighter *fp) { (void) fp; }
static void ft_800887CC(Fighter *fp) { (void) fp; }
static void ft_80088328(Fighter *fp, void *sfx, int a, int b)
{
    (void) fp;
    (void) sfx;
    (void) a;
    (void) b;
}
static void ftCo_Fall_Enter(Fighter_GObj *gobj)
{
    (void) gobj;
    fall_entered = true;
}
static void ft_8008A2BC(Fighter_GObj *gobj)
{
    (void) gobj;
    wait_entered = true;
}
static bool ft_800827A0(Fighter_GObj *gobj)
{
    (void) gobj;
    return scripted_ground_ok;
}
static void mpFloorGetLeft(int line_id, Vec3 *out)
{
    (void) line_id;
    *out = scripted_edge;
}
static void mpFloorGetRight(int line_id, Vec3 *out)
{
    (void) line_id;
    *out = scripted_edge;
}

/* Every `ftCo_Ottotto_IASA` callee: scripted answer, logged in call order. */
static bool ftCo_SpecialS_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_SPECIAL_S); record_call(CALL_SPECIAL_S, r); (void) g; return r; }
static bool ftCo_Attack100_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_ATTACK100); record_call(CALL_ATTACK100, r); (void) g; return r; }
static bool ftCo_800D6824(Fighter_GObj *g) { bool r = ANSWER(CALL_800D6824); record_call(CALL_800D6824, r); (void) g; return r; }
static bool ftCo_800D68C0(Fighter_GObj *g) { bool r = ANSWER(CALL_800D68C0); record_call(CALL_800D68C0, r); (void) g; return r; }
static bool ftCo_Catch_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_CATCH); record_call(CALL_CATCH, r); (void) g; return r; }
static bool ftCo_AttackS4_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_ATTACK_S4); record_call(CALL_ATTACK_S4, r); (void) g; return r; }
static bool ftCo_AttackHi4_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_ATTACK_HI4); record_call(CALL_ATTACK_HI4, r); (void) g; return r; }
static bool ftCo_AttackLw4_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_ATTACK_LW4); record_call(CALL_ATTACK_LW4, r); (void) g; return r; }
static bool ftCo_AttackS3_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_ATTACK_S3); record_call(CALL_ATTACK_S3, r); (void) g; return r; }
static bool ftCo_AttackHi3_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_ATTACK_HI3); record_call(CALL_ATTACK_HI3, r); (void) g; return r; }
static bool ftCo_AttackLw3_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_ATTACK_LW3); record_call(CALL_ATTACK_LW3, r); (void) g; return r; }
static bool ftCo_Attack1_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_ATTACK1); record_call(CALL_ATTACK1, r); (void) g; return r; }
static bool ftCo_80091A4C(Fighter_GObj *g) { bool r = ANSWER(CALL_SHIELD_A4C); record_call(CALL_SHIELD_A4C, r); (void) g; return r; }
static bool ftCo_800DE9D8(Fighter_GObj *g) { bool r = ANSWER(CALL_TAUNT); record_call(CALL_TAUNT, r); (void) g; return r; }
static bool ftCo_Jump_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_JUMP); record_call(CALL_JUMP, r); (void) g; return r; }
static bool ftCo_Dash_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_DASH); record_call(CALL_DASH, r); (void) g; return r; }
static bool ftCo_800D5FB0(Fighter_GObj *g) { bool r = ANSWER(CALL_SQUAT); record_call(CALL_SQUAT, r); (void) g; return r; }
static bool ftCo_Turn_CheckInput(Fighter_GObj *g) { bool r = ANSWER(CALL_TURN); record_call(CALL_TURN, r); (void) g; return r; }
static bool ftCo_Walk_CheckInput_Ottotto(Fighter_GObj *g) { bool r = ANSWER(CALL_WALK_OTTOTTO); record_call(CALL_WALK_OTTOTTO, r); (void) g; return r; }

/* Forward declarations so the extracted functions can call each other
 * regardless of extraction order (`ftCo_8009A3C8` calls `ftCo_8009A410`,
 * which the original source only forward-declares above its own
 * definition; that declaration is not itself extracted). */
bool ftCo_8009A3C8(Fighter_GObj *gobj);
static void ftCo_8009A410(Fighter_GObj *gobj);
void ftCo_Ottotto_IASA(Fighter_GObj *gobj);
void ftCo_Ottotto_Coll(Fighter_GObj *gobj);
static void ftCo_8009A6B8(Fighter_GObj *gobj);
void ftCo_OttottoWait_Coll(Fighter_GObj *gobj);

#include "ottotto_original.inc"

static void reset(uint32_t scripted)
{
    answers = scripted;
    captured_motion = 0;
    call_count = 0;
    fired_code = 0;
    fall_entered = false;
    wait_entered = false;
    for (size_t i = 0; i < sizeof call_log; i++) {
        call_log[i] = 0;
    }
}

/* `ftCo_8009A3C8`: `Collide_Edge` set (or not) and the Sandbag exemption. */
int oracle_ottotto_entry_gate(int edge_set, int sandbag, int *out_motion)
{
    reset(0);
    Fighter fighter = { .coll_data = { .env_flags = edge_set ? Collide_Edge : 0 },
                        .x2228_b2 = sandbag != 0 };
    Fighter_GObj gobj = { &fighter };
    int result = ftCo_8009A3C8(&gobj);
    *out_motion = captured_motion;
    return result;
}

/* `ftCo_Ottotto_IASA`'s complete dispatch order. `answers` bit N set means
 * callee N (the `CALL_*` codes above) answers true. Outputs the full
 * ordered trace and which code (0 = none) fired first. */
void oracle_ottotto_iasa(uint32_t scripted, uint8_t *out_calls, uint8_t *out_count,
                         uint8_t *out_fired)
{
    reset(scripted);
    Fighter fighter = { 0 };
    Fighter_GObj gobj = { &fighter };
    ftCo_Ottotto_IASA(&gobj);
    for (uint8_t i = 0; i < call_count && i < 16; i++) {
        out_calls[i] = call_log[i];
    }
    *out_count = call_count < 16 ? call_count : 16;
    *out_fired = fired_code;
}

/* `ftCo_Ottotto_Coll`/`ftCo_OttottoWait_Coll`: `ground_ok` scripts
 * `ft_800827A0`; the floor-end query answers `edge_x` on whichever side
 * `facing_dir` selects. Returns 0 (stay), 1 (fall via `ftCo_Fall_Enter`) or
 * 2 (exit to Wait via `ft_8008A2BC`). */
int oracle_ottotto_coll(int is_wait_variant, int ground_ok, float facing_dir, float cur_x,
                        float edge_x, float x478, float x47C)
{
    reset(0);
    scripted_ground_ok = ground_ok != 0;
    scripted_edge = (Vec3) { edge_x, 0.0F, 0.0F };
    common = (ftCommonData) { .x478 = x478, .x47C = x47C };
    p_ftCommonData = &common;
    Fighter fighter = { .facing_dir = facing_dir, .cur_pos = { cur_x, 0.0F, 0.0F } };
    Fighter_GObj gobj = { &fighter };
    if (is_wait_variant) {
        ftCo_OttottoWait_Coll(&gobj);
    } else {
        ftCo_Ottotto_Coll(&gobj);
    }
    if (fall_entered) {
        return 1;
    }
    if (wait_entered) {
        return 2;
    }
    return 0;
}

/* `ftCo_8009A410`/`ftCo_8009A6B8` entry motion ids, for completeness.
 * `ftCo_8009A6B8` unconditionally evaluates `fp->ft_data->x4C_sfx->x18` as
 * `ft_80088328`'s argument (the stub itself ignores it, but the expression
 * is still evaluated), so `ft_data`/`x4C_sfx` need real, if dummy, storage. */
void oracle_ottotto_enter(int *ottotto_motion, int *ottotto_wait_motion)
{
    reset(0);
    Sfx sfx = { 0 };
    FtData ft_data = { &sfx };
    Fighter fighter = { .ft_data = &ft_data };
    Fighter_GObj gobj = { &fighter };
    ftCo_8009A410(&gobj);
    *ottotto_motion = captured_motion;
    captured_motion = 0;
    ftCo_8009A6B8(&gobj);
    *ottotto_wait_motion = captured_motion;
}
