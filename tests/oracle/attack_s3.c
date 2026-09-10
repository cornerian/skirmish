/* Host adapter for the complete forward-tilt input and angle selection. The
 * item branches are disabled and the motion change only captures its id. */
#include <math.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct { float x, y; } Vec2;
typedef struct { int x8; } FigaTreeEntry;
typedef struct Fighter_GObj Fighter_GObj;
typedef struct Fighter {
    struct { Vec2 lstick[1]; uint32_t held_buttons[1]; uint32_t pressed_buttons; } input;
    float facing_dir;
    int kind;
    void* item_gobj;
    int allow_interrupt;
    int cmd_vars[1];
    struct { struct { struct { int x0; } attacklw3; } co; } mv;
    void (*x21EC)(Fighter_GObj*);
} Fighter;
struct Fighter_GObj { Fighter* user_data; };
typedef Fighter_GObj HSD_GObj;
typedef int FtMotionId;
typedef struct {
    float x20_radians, x98, x9C_radians, xA0_radians, xA4_radians, xA8_radians;
    float attackhi3_stick_threshold_y, xB0;
} ftCommonData;

#define GET_FIGHTER(g) ((g)->user_data)
#define ABS(x) ((x) < 0 ? -(x) : (x))
#define HSD_PAD_A 0x100
#define HSD_PAD_LR 0x60
#define Ft_MF_None 0
#define Ft_MF_SkipAttackCount 0x400
#define FTKIND_GAMEWATCH 24
enum {
    ftCo_MS_AttackS3Hi = 51, ftCo_MS_AttackS3HiS, ftCo_MS_AttackS3S,
    ftCo_MS_AttackS3LwS, ftCo_MS_AttackS3Lw, ftCo_MS_AttackHi3, ftCo_MS_AttackLw3,
    ftCo_MS_LightThrowF = 240, ftCo_MS_LightThrowHi, ftCo_MS_LightThrowLw
};
enum cmd_var_idx { cmd_unk0_bool };

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local int captured_motion;
static _Thread_local int captured_flags;
/* Figatree availability, indexed by the sub-motion id the source tests. */
static _Thread_local FigaTreeEntry entries[64];

static bool ftpickupitem_80094790(Fighter_GObj* gobj) { (void) gobj; return false; }
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
static int it_8026B30C(void* item) { (void) item; return 0; }
static void ftCo_800957F4(Fighter_GObj* gobj, int msid) { (void) gobj; captured_motion = msid; }
static void ftCo_Attack_800CCF58(Fighter_GObj* gobj, int arg) { (void) gobj; (void) arg; }
static void ftCo_Attack_800CDD14(Fighter_GObj* gobj) { (void) gobj; }
static bool ftCo_80094E54(Fighter* fp) { (void) fp; return false; }

/* The pinned ftCommon_GetLStickAngle body is compiled once by the aerial
 * adapter; both host layouts place lstick[0] first. */
float ftCo_GetLStickAngle(Fighter* fp);

static void decideAngle(Fighter_GObj* gobj);
#include "attack_s3_original.inc"

/* values: stick x, stick y, facing; rules: x98, x20, x9C, xA0, xA4, xA8;
 * available bits: 1 High, 2 HighSlight, 4 LowSlight, 8 Low. */
int oracle_forward_tilt(const float* values, const float* rules, uint32_t pressed,
                        uint32_t available, int* motion)
{
    common = (ftCommonData) { .x20_radians = rules[1], .x98 = rules[0],
                              .x9C_radians = rules[2], .xA0_radians = rules[3],
                              .xA4_radians = rules[4], .xA8_radians = rules[5] };
    p_ftCommonData = &common;
    for (int i = 0; i < 64; i++) entries[i].x8 = 0;
    entries[ftCo_MS_AttackS3S].x8 = (available & 1) != 0;
    entries[ftCo_MS_AttackS3LwS].x8 = (available & 2) != 0;
    entries[ftCo_MS_AttackHi3].x8 = (available & 4) != 0;
    entries[ftCo_MS_AttackLw3].x8 = (available & 8) != 0;
    Fighter fighter = { .input = { .lstick = {{ values[0], values[1] }}, .pressed_buttons = pressed },
                        .facing_dir = values[2], .allow_interrupt = 1 };
    Fighter_GObj gobj = { &fighter };
    captured_motion = 0;
    int result = ftCo_AttackS3_CheckInput(&gobj);
    *motion = captured_motion;
    return result;
}
