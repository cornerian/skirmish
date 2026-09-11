/* Host adapter for the complete jab-combo input chain: ftCo_Attack1_CheckInput
 * (Wait's fresh-press entry to the first jab or a remembered follow-up), the
 * first/second/third jab entries and their buffered-follow-up IASA checks,
 * and the rapid-jab entry count, start/loop transition and loop continuation
 * check from ftCo_Attack100.c. Both snapshots are compiled in this one
 * translation unit because their pinned bodies call each other directly.
 * Item throws/drops, the Game & Watch, Pikachu/Pichu and Marth overrides are
 * disabled (kind stays the default 0, item_gobj stays NULL); their branches
 * still compile so the pinned bodies are unmodified. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct Fighter_GObj Fighter_GObj;
typedef Fighter_GObj HSD_GObj;
typedef int MotionFlags;
typedef int enum_t;
typedef int FtMotionId;
typedef uint8_t u8;

typedef struct {
    float jab_2_input_window;
    float jab_3_input_window;
    int rapid_jab_window;
} ftCo_DatAttrs;

typedef struct Fighter {
    struct {
        uint32_t pressed_buttons;
        uint32_t held_buttons[1];
        uint32_t released_buttons;
    } input;
    void* item_gobj;
    float hitlag_mul;
    int x2218_b1;
    int x2218_b2;
    int unk_msid;
    struct {
        struct {
            struct { int x0; } attack1;
            struct { int x0; int x4; } attack100;
        } co;
    } mv;
    int x1A54;
    ftCo_DatAttrs co_attrs;
    int kind;
    void (*x21EC)(Fighter_GObj*);
    int allow_interrupt;
    int throw_flags;
    int throw_flags_b3;
    float cur_anim_frame;
    float frame_speed_mul;
} Fighter;
struct Fighter_GObj { Fighter* user_data; };

#define GET_FIGHTER(g) ((g)->user_data)
#define HSD_PAD_A 0x100
#define HSD_PAD_LR 0x60
#define Ft_MF_None 0
#define Ft_MF_SkipAttackCount 0x400
#define RETURN_IF(x) if (x) return;
#define PAD_STACK(n)

/* Fighter kinds that switch a branch away from the ordinary path; none of
 * them equal the default fighter kind (0) used by the sequence oracle, so
 * every override branch below compiles but never runs. */
#define FTKIND_PIKACHU 12
#define FTKIND_PICHU 25
#define FTKIND_GAMEWATCH 24
#define FTKIND_MARS 18

enum {
    ftCo_MS_Wait = 14,
    ftCo_MS_Attack11 = 44,
    ftCo_MS_Attack12 = 45,
    ftCo_MS_Attack13 = 46,
    ftCo_MS_Attack100Start = 47,
    ftCo_MS_Attack100Loop = 48,
    ftCo_MS_Attack100End = 49,
    ftCo_MS_LightThrowF = 240,
    ftCo_MS_LightThrowDrop = 241,
};

static _Thread_local int captured_motion;
static _Thread_local int captured_flags;
/* ft_800892A0/ft_80089824 call counts, for tests that want to see the loop's
 * frame-zero restart run exactly twice (as the down tilt's exit does). */
static _Thread_local int restart_calls;

/* Fighter_ChangeMotionState captures the requested motion and flags; the
 * jab timer reset rule is fighter.c:1143 `if ((msid != 0xE) && (msid !=
 * 0xF) && (msid != 0x10) && (msid != 0x11)) fp->hitlag_mul = 0.0f;` (0xE..
 * 0x11 are Wait and the two walks, 14..=17), so only a jab that ends into
 * Wait or a walk keeps its follow-up window. */
static void Fighter_ChangeMotionState(Fighter_GObj* gobj, int msid, int flags,
                                      float start, float rate, float blend,
                                      void* callback)
{
    (void) start; (void) rate; (void) blend; (void) callback;
    Fighter* fp = GET_FIGHTER(gobj);
    captured_motion = msid;
    captured_flags = flags;
    if (!(msid >= 14 && msid <= 17)) {
        fp->hitlag_mul = 0.0f;
    }
}
static void ftAnim_8006EBA4(Fighter_GObj* gobj) { (void) gobj; }
static bool ftpickupitem_80094790(Fighter_GObj* gobj) { (void) gobj; return false; }
static int it_8026B30C(void* item) { (void) item; return 0; }
static void ftCo_800957F4(Fighter_GObj* gobj, int msid) { (void) gobj; captured_motion = msid; }
static void ftCo_Attack_800CCF58(Fighter_GObj* gobj, bool arg) { (void) gobj; (void) arg; }
static void ftCo_Attack_800CDD14(Fighter_GObj* gobj) { (void) gobj; }
static void ftGw_Attack11_Enter(Fighter_GObj* gobj) { (void) gobj; captured_motion = -2; }
static void ftGw_Attack100Start_Enter(Fighter_GObj* gobj) { (void) gobj; captured_motion = -2; }
static void ft_800892A0(Fighter_GObj* gobj) { (void) gobj; restart_calls++; }
static void ft_80089824(Fighter_GObj* gobj) { (void) gobj; restart_calls++; }
/* ft_8008A2BC: "Fall off a floor" always lands the first three jabs and the
 * rapid jab's end back in Wait in this grounded-only harness. */
static void ft_8008A2BC(Fighter_GObj* gobj)
{
    Fighter_ChangeMotionState(gobj, ftCo_MS_Wait, Ft_MF_None, 0.0f, 1.0f, 0.0f, NULL);
}
static void onPkPc21EC(Fighter_GObj* gobj) { ft_800892A0(gobj); ft_80089824(gobj); }

static bool ftCo_AttackS4_CheckInput(Fighter_GObj* gobj) { (void) gobj; return false; }
static bool ftCo_AttackHi4_CheckInput(Fighter_GObj* gobj) { (void) gobj; return false; }
static bool ftCo_AttackLw4_CheckInput(Fighter_GObj* gobj) { (void) gobj; return false; }
static bool ftCo_AttackS3_CheckInput(Fighter_GObj* gobj) { (void) gobj; return false; }
static bool ftCo_AttackHi3_CheckInput(Fighter_GObj* gobj) { (void) gobj; return false; }
static bool ftCo_AttackLw3_CheckInput(Fighter_GObj* gobj) { (void) gobj; return false; }
static bool ftCo_Jump_CheckInput(Fighter_GObj* gobj) { (void) gobj; return false; }
static bool ftCo_Dash_CheckInput(Fighter_GObj* gobj) { (void) gobj; return false; }
static bool ftCo_800D5FB0(Fighter_GObj* gobj) { (void) gobj; return false; }
static bool ftCo_Turn_CheckInput(Fighter_GObj* gobj) { (void) gobj; return false; }
static bool ftCo_Walk_CheckInput(Fighter_GObj* gobj) { (void) gobj; return false; }

/* Declared ahead so the extracted bodies (concatenated in functions.json
 * order, not original file order) can call each other and the sibling
 * translation-unit-mate attack100 functions. */
static void decideAttack11(Fighter_GObj* gobj);
static void checkAttack11(Fighter_GObj* gobj);
static void doAttack12(Fighter_GObj* gobj);
static bool checkAttack12(Fighter_GObj* gobj);
static void doAttack13(Fighter_GObj* gobj);
static bool checkAttack13(Fighter_GObj* gobj);
bool ftCo_Attack1_CheckInput(Fighter_GObj* gobj);
bool ftCo_Attack_800D6A50(Fighter_GObj* gobj);
void fn_800D6AC4(Fighter_GObj* gobj);
void fn_800D6B8C(Fighter_GObj* gobj);

/* ftCo_Attack13_IASA reaches ftCo_Wait_IASA while allow_interrupt; that is
 * where the real Wait chain's jab check (ftCo_Attack1_CheckInput) lives, so
 * the stub reproduces exactly that hop and nothing else of the Wait chain. */
static void ftCo_Wait_IASA(Fighter_GObj* gobj) { ftCo_Attack1_CheckInput(gobj); }

#include "attack1_original.inc"
#include "attack100_original.inc"

/* Drives the pinned bodies above frame by frame from Wait (motion 14,
 * every field zero). `attrs` is [jab_2_input_window, jab_3_input_window,
 * rapid_jab_window]. Each event word applies, in order: bit0 A pressed,
 * bit1 A released, bit2 set x2218_b1 (the script's follow-up-ready command,
 * applied fresh every frame), bit3 the rapid command is issued this frame,
 * bit4 the rapid command's value, bit5 allow_interrupt for this frame, bit6
 * animation end this frame, bit7 loop_check (throw_flags_b3) this frame,
 * bit8 the loop figatree's frame zero (cur_anim_frame = 0, else 1). Records
 * motion, the window (hitlag_mul), a b1/b2/attack1.x0/attack100.x0/x4
 * bitfield and the press count after every frame. Returns the final
 * motion. */
int oracle_jab_sequence(const float* attrs, int frames, const uint32_t* events,
                        int* out_motion, float* out_window, int* out_flags,
                        int* out_presses)
{
    Fighter fighter = { 0 };
    fighter.co_attrs.jab_2_input_window = attrs[0];
    fighter.co_attrs.jab_3_input_window = attrs[1];
    fighter.co_attrs.rapid_jab_window = (int) attrs[2];
    fighter.unk_msid = ftCo_MS_Wait;
    Fighter_GObj gobj = { &fighter };
    int motion = ftCo_MS_Wait;
    int a_down = 0;
    restart_calls = 0;
    for (int frame = 0; frame < frames; frame++) {
        uint32_t e = events[frame];
        int pressed = e & 1;
        int released = (e >> 1) & 1;
        if (pressed) {
            a_down = 1;
        }
        if (released) {
            a_down = 0;
        }
        fighter.input.pressed_buttons = pressed ? HSD_PAD_A : 0;
        fighter.input.released_buttons = released ? HSD_PAD_A : 0;
        fighter.input.held_buttons[0] = a_down ? HSD_PAD_A : 0;
        fighter.x2218_b1 = (int) ((e >> 2) & 1);
        if ((e >> 3) & 1) {
            fighter.x2218_b2 = (int) ((e >> 4) & 1);
        }
        fighter.allow_interrupt = (int) ((e >> 5) & 1);
        fighter.throw_flags_b3 = (int) ((e >> 7) & 1);

        captured_motion = -1;
        if (motion == ftCo_MS_Attack100Loop) {
            fighter.cur_anim_frame = ((e >> 8) & 1) ? 0.0f : 1.0f;
            fighter.frame_speed_mul = 1.0f;
            ftCo_Attack100Loop_Anim(&gobj);
        } else if ((e >> 6) & 1) {
            switch (motion) {
            case ftCo_MS_Attack11:
            case ftCo_MS_Attack12:
            case ftCo_MS_Attack13:
            case ftCo_MS_Attack100End:
                ft_8008A2BC(&gobj);
                break;
            case ftCo_MS_Attack100Start:
                /* As ftCo_Attack100Start_Anim does on animation end. */
                Fighter_ChangeMotionState(&gobj, ftCo_MS_Attack100Loop,
                                          Ft_MF_SkipAttackCount, 0.0f, 1.0f,
                                          0.0f, NULL);
                break;
            default:
                break;
            }
        }
        if (captured_motion != -1) {
            motion = captured_motion;
            captured_motion = -1;
        }

        switch (motion) {
        case ftCo_MS_Wait:
            ftCo_Attack1_CheckInput(&gobj);
            break;
        case ftCo_MS_Attack11:
            ftCo_Attack11_IASA(&gobj);
            break;
        case ftCo_MS_Attack12:
            ftCo_Attack12_IASA(&gobj);
            break;
        case ftCo_MS_Attack13:
            ftCo_Attack13_IASA(&gobj);
            break;
        case ftCo_MS_Attack100Loop:
            ftCo_Attack100Loop_IASA(&gobj);
            break;
        default:
            break;
        }
        if (captured_motion != -1) {
            motion = captured_motion;
        }

        out_motion[frame] = motion;
        out_window[frame] = fighter.hitlag_mul;
        out_flags[frame] = (fighter.x2218_b1 & 1) | ((fighter.x2218_b2 & 1) << 1) |
                           ((fighter.mv.co.attack1.x0 & 1) << 2) |
                           ((fighter.mv.co.attack100.x0 & 1) << 3) |
                           ((fighter.mv.co.attack100.x4 & 1) << 4);
        out_presses[frame] = fighter.x1A54;
    }
    return motion;
}
