/* Host adapter for the grounded side-special common dispatch
 * (`ftCo_SpecialS.c`, `special_s.functions.json`: `ftCo_SpecialS_HasInput`,
 * `ftCo_SpecialS_CheckInput`, `doEnter`).
 *
 * `ftData_SpecialS[]` (the per-character table) is stubbed: only index
 * `FTKIND_FOX` is populated, with a logging stub recording that Fox's own
 * entry fired (this adapter has no per-character Start/Dash/End bodies of
 * its own -- that is `fox_specials.c`'s job); every other index is NULL,
 * matching "the character has no such move". `ftCommon_UpdateFacing` is
 * captured (records whether it fired); `ft_GetGroundFrictionMultiplier` is
 * scripted, matching `fox_specials.c`'s own treatment of it. */
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

typedef uint8_t u8;
typedef int32_t s32;
typedef uint32_t u32;
typedef float f32;
#define ABS(x) ((x) < 0 ? -(x) : (x))
#define HSD_PAD_B 0x2000
#define FTKIND_FOX 2
#define FTKIND_COUNT 30

typedef struct { float x, y; } Vec2;
typedef struct {
    u32 pressed_buttons;
    Vec2 lstick[1];
} FighterInput;

typedef struct {
    float specials_ground_speed_retention;
} ftCo_DatAttrs;

typedef struct Fighter {
    s32 kind;
    FighterInput input;
    float facing_dir;
    u8 x688;
    ftCo_DatAttrs co_attrs;
    float gr_vel;
} Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;

typedef struct { float x218, x220; } FtCommonData;
static FtCommonData ftCommonData_;
static FtCommonData* p_ftCommonData = &ftCommonData_;

static _Thread_local int fox_entry_calls;
static void ftData_SpecialS_Fox(Fighter_GObj* gobj) {
    (void) gobj;
    fox_entry_calls++;
}
typedef void (*SpecialSEntry)(Fighter_GObj*);
static SpecialSEntry ftData_SpecialS[FTKIND_COUNT];

static _Thread_local int update_facing_calls;
static void ftCommon_UpdateFacing(Fighter* fp) {
    fp->facing_dir = -fp->facing_dir;
    update_facing_calls++;
}

static _Thread_local float script_ground_friction_multiplier;
static float ft_GetGroundFrictionMultiplier(Fighter* fp) {
    (void) fp;
    return script_ground_friction_multiplier;
}

#include "special_s_original.inc"

/* `has_fox_entry` scripts whether Fox has a side special at all (always
 * true for the differential test's own purposes; exercised false to
 * confirm the "no such move" branch). Returns whether the check fired;
 * `*out_entry_calls` is 1 if Fox's own entry ran, `*out_update_facing` is
 * whether `ftCommon_UpdateFacing` fired, `*out_gr_vel`/`*out_facing` are
 * the post-`doEnter` state. */
int oracle_special_s_check_input(bool has_fox_entry, u32 pressed_buttons, float stick_x,
                                  float turn_threshold, float facing, u8 x688, float retention,
                                  float friction_multiplier, float gr_vel_in, int* out_entry_calls,
                                  int* out_update_facing, float* out_gr_vel, float* out_facing) {
    memset(ftData_SpecialS, 0, sizeof(ftData_SpecialS));
    ftData_SpecialS[FTKIND_FOX] = has_fox_entry ? ftData_SpecialS_Fox : NULL;
    p_ftCommonData->x220 = turn_threshold;
    Fighter fp;
    memset(&fp, 0, sizeof(fp));
    fp.kind = FTKIND_FOX;
    fp.input.pressed_buttons = pressed_buttons;
    fp.input.lstick[0].x = stick_x;
    fp.facing_dir = facing;
    fp.x688 = x688;
    fp.co_attrs.specials_ground_speed_retention = retention;
    fp.gr_vel = gr_vel_in;
    Fighter_GObj gobj = { &fp };
    fox_entry_calls = 0;
    update_facing_calls = 0;
    script_ground_friction_multiplier = friction_multiplier;
    int fired = ftCo_SpecialS_CheckInput(&gobj);
    *out_entry_calls = fox_entry_calls;
    *out_update_facing = update_facing_calls;
    *out_gr_vel = fp.gr_vel;
    *out_facing = fp.facing_dir;
    return fired;
}

int oracle_special_s_has_input(u32 pressed_buttons, float stick_x, float threshold) {
    Fighter fp;
    memset(&fp, 0, sizeof(fp));
    fp.input.pressed_buttons = pressed_buttons;
    fp.input.lstick[0].x = stick_x;
    p_ftCommonData->x218 = threshold;
    return ftCo_SpecialS_HasInput(&fp);
}
