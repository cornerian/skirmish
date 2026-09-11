/* Host adapter for the complete down-smash input predicate.
 * The item branch and the Ness override are disabled; the motion change only
 * captures its id. */
#include <math.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct { float x, y; } Vec2;
typedef struct { int x8; } FigaTreeEntry;
typedef struct Fighter_GObj Fighter_GObj;
/* lstick[0] and cstick[0..1] sit where the pinned angle helpers compiled by
 * the aerial adapter expect them. */
typedef struct Fighter {
    struct { Vec2 lstick[2]; Vec2 cstick[2]; uint32_t held_buttons[1]; uint32_t pressed_buttons; } input;
    float facing_dir;
    int kind;
    void* item_gobj;
    int allow_interrupt;
    int cmd_vars[1];
    int throw_flags;
    uint8_t x670_timer_lstick_tilt_x;
    uint8_t x671_timer_lstick_tilt_y;
} Fighter;
struct Fighter_GObj { Fighter* user_data; };
typedef Fighter_GObj HSD_GObj;
typedef int FtMotionId;
typedef struct {
    float dash_smash_stick_threshold;
    int dash_smash_window;
    float xB8_radians, xBC_radians, xC0_radians, xC4_radians;
    float xCC, xD0, xD4, xD8;
} ftCommonData;

#define GET_FIGHTER(g) ((g)->user_data)
#define ABS(x) ((x) < 0 ? -(x) : (x))
#define HSD_PAD_A 0x100
#define HSD_PAD_LR 0x60
#define Ft_MF_None 0
#define FTKIND_NESS 8
#define FTKIND_PEACH 9
#define FTKIND_PIKACHU 12
#define FTKIND_GAMEWATCH 24
#define FTKIND_PICHU 25
enum {
    ftCo_MS_AttackS4Hi = 58, ftCo_MS_AttackS4HiS, ftCo_MS_AttackS4S,
    ftCo_MS_AttackS4LwS, ftCo_MS_AttackS4Lw, ftCo_MS_AttackHi4, ftCo_MS_AttackLw4,
    ftCo_MS_LightThrowF4 = 243, ftCo_MS_LightThrowB4, ftCo_MS_LightThrowHi4,
    ftCo_MS_LightThrowLw4
};
enum cmd_var_idx { cmd_unk0_bool };

static _Thread_local ftCommonData common;
extern _Thread_local ftCommonData* p_ftCommonData;
/* Pinned in the forward-smash adapter against the same host layout. */
bool ftCo_800DF3A8(Fighter* fp);
static bool ftCo_800DF3DC(Fighter* fp) { return ftCo_800DF3A8(fp); }
static _Thread_local int captured_motion;
static _Thread_local int captured_flags;
/* Figatree availability, indexed by the sub-motion id the source tests. */
static _Thread_local FigaTreeEntry entries[128];

static FigaTreeEntry* ftData_80085FD4(Fighter* fp, int msid) { (void) fp; return &entries[msid]; }
static void Fighter_ChangeMotionState(Fighter_GObj* gobj, int msid, int flags,
                                      float start, float rate, float blend,
                                      void* callback)
{
    (void) gobj; (void) start; (void) rate; (void) blend; (void) callback;
    captured_motion = msid;
    captured_flags = flags;
}
static void ftAnim_8006EBA4(Fighter_GObj* gobj) { (void) gobj; }
static void ftCo_800957F4(Fighter_GObj* gobj, int msid) { (void) gobj; captured_motion = msid; }
static bool ftCo_80094E54(Fighter* fp) { (void) fp; return false; }

/* The pinned angle helpers are compiled once by the aerial adapter. */
float ftCo_GetLStickAngle(Fighter* fp);
float ftCo_GetCStickAngle(Fighter* fp);

static void ftNs_AttackLw4_Enter(Fighter_GObj* gobj) { (void) gobj; captured_motion = -1; }
static void doEnter(Fighter_GObj* gobj);

#include "attack_lw4_original.inc"

/* values: stick y, C-stick y, previous C-stick y; rules: threshold, window. */
static Fighter setup(const float* values, const float* rules, uint32_t pressed,
                     uint32_t tilt_y_age)
{
    common = (ftCommonData) { .xD4 = rules[0], .xD8 = rules[1] };
    p_ftCommonData = &common;
    return (Fighter) { .input = { .lstick = {{ 0, values[0] }},
                                   .cstick = {{ 0, values[1] }, { 0, values[2] }},
                                   .pressed_buttons = pressed },
                       .facing_dir = 1, .allow_interrupt = 1,
                       .x671_timer_lstick_tilt_y = (uint8_t) tilt_y_age };
}

int oracle_down_smash(const float* values, const float* rules, uint32_t pressed,
                     uint32_t tilt_y_age, int* motion)
{
    Fighter fighter = setup(values, rules, pressed, tilt_y_age);
    Fighter_GObj gobj = { &fighter };
    captured_motion = 0;
    int result = ftCo_AttackLw4_CheckInput(&gobj);
    *motion = captured_motion;
    return result;
}
