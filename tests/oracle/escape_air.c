/* Host adapter for the complete air-dodge trigger, launch and decay bodies.
 * The item-pickup buffer, motion change, script start and statistics
 * callbacks are stubs that only capture the selected motion. */
#include <math.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef uint8_t u8;
typedef struct { float x, y; } Vec2;
typedef struct { float x, y, z; } Vec3;
typedef struct Fighter {
    struct { Vec2 lstick[1]; uint32_t pressed_buttons; } input;
    Vec3 self_vel;
    int cmd_vars[1];
    struct { struct { struct { Vec3 self_vel; int timer; } escapeair; } co; } mv;
} Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;
typedef struct {
    Vec2 escapeair_deadzone;
    int x334;
    float escapeair_force, escapeair_decay, x340, x344;
} ftCommonData;

/* Runtime/platform.h definition, including signed-zero behavior. */
#define ABS(x) ((x) < 0 ? -(x) : (x))
#define HSD_PAD_L 0x40
#define HSD_PAD_R 0x20
#define Ft_MF_None 0
enum { ftCo_MS_EscapeAir = 236 };
enum cmd_var_idx { cmd_skip_decay };

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local int captured_motion;
static _Thread_local int fell;

static bool ftCo_800D705C(Fighter_GObj* gobj) { (void) gobj; return false; }
static void Fighter_ChangeMotionState(Fighter_GObj* gobj, int msid, int flags,
                                      float start, float rate, float blend,
                                      void* callback)
{
    (void) gobj; (void) flags; (void) start; (void) rate; (void) blend;
    (void) callback;
    captured_motion = msid;
}
static void ftAnim_8006EBA4(Fighter_GObj* gobj) { (void) gobj; }
static void ftCommon_8007EBAC(Fighter* fp, uint32_t a, uint32_t b)
{
    (void) fp; (void) a; (void) b;
}
static void ft_80084DB0(Fighter_GObj* gobj) { (void) gobj; fell = 1; }
void ftCo_80099A9C(Fighter_GObj* gobj, int timer);

#include "escape_air_angle_original.inc"
#include "escape_air_original.inc"

/* Complete ftCo_80099A58 over an arbitrary pressed-button word. */
int oracle_air_dodge_request(uint32_t pressed, int timer, int* motion,
                             int* timer_out)
{
    common = (ftCommonData) { .x334 = timer };
    p_ftCommonData = &common;
    Fighter fighter = { .input = { .pressed_buttons = pressed } };
    Fighter_GObj gobj = { &fighter };
    captured_motion = 0;
    int result = ftCo_80099A58(&gobj);
    *motion = captured_motion;
    *timer_out = fighter.mv.co.escapeair.timer;
    return result;
}

/* Complete ftCo_80099A9C launch velocity over arbitrary binary32 input. */
void oracle_air_dodge_launch(const float* stick, const float* deadzone,
                             float force, float* velocity)
{
    common = (ftCommonData) {
        .escapeair_deadzone = { deadzone[0], deadzone[1] },
        .escapeair_force = force,
    };
    p_ftCommonData = &common;
    Fighter fighter = {
        .input = { .lstick = {{ stick[0], stick[1] }} },
        .self_vel = { 5.0f, -7.0f, 0.0f },
    };
    Fighter_GObj gobj = { &fighter };
    captured_motion = 0;
    ftCo_80099A9C(&gobj, 0);
    velocity[0] = fighter.self_vel.x;
    velocity[1] = fighter.self_vel.y;
    velocity[2] = (float) captured_motion;
}

/* Complete ftCo_EscapeAir_Phys: decay both axes, or defer to ordinary fall. */
int oracle_air_dodge_decay(const float* velocity, float decay, int skip,
                           float* out)
{
    common = (ftCommonData) { .escapeair_decay = decay };
    p_ftCommonData = &common;
    Fighter fighter = { .self_vel = { velocity[0], velocity[1], 0.0f },
                        .cmd_vars = { skip } };
    Fighter_GObj gobj = { &fighter };
    fell = 0;
    ftCo_EscapeAir_Phys(&gobj);
    out[0] = fighter.self_vel.x;
    out[1] = fighter.self_vel.y;
    return fell;
}
