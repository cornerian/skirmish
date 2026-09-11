/* Host adapter for the aerial special common dispatch
 * (`ftCo_SpecialAir.c`, `special_air.functions.json`:
 * `ftCo_SpecialAir_CheckInput`).
 *
 * The four character tables (`ftData_SpecialAirHi/Lw/S/N`) are stubbed:
 * Fox's own index is populated in every table with a distinct logging
 * stub (Fox has all four moves in the source), recording which one fired.
 * `ftCommon_UpdateFacing` is captured. */
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

typedef uint8_t u8;
typedef int32_t s32;
typedef uint32_t u32;
typedef float f32;
typedef bool BOOL;
#define ABS(x) ((x) < 0 ? -(x) : (x))
#define HSD_PAD_B 0x2000
#define FTKIND_FOX 2
#define FTKIND_COUNT 30

typedef struct { float x, y; } Vec2;
typedef struct {
    u32 pressed_buttons;
    Vec2 lstick[1];
} FighterInput;

typedef struct Fighter {
    s32 kind;
    FighterInput input;
    float facing_dir;
    float x676_x;
    bool x2227_b5;
    bool x2228_b7;
} Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;

typedef struct { float x218, x220, x21C, x224; } FtCommonData;
static FtCommonData ftCommonData_;
static FtCommonData* p_ftCommonData = &ftCommonData_;

static _Thread_local int fired_table;
static void ftData_SpecialAirHi_Fox(Fighter_GObj* gobj) {
    (void) gobj;
    fired_table = 1;
}
static void ftData_SpecialAirLw_Fox(Fighter_GObj* gobj) {
    (void) gobj;
    fired_table = 2;
}
static void ftData_SpecialAirS_Fox(Fighter_GObj* gobj) {
    (void) gobj;
    fired_table = 3;
}
static void ftData_SpecialAirN_Fox(Fighter_GObj* gobj) {
    (void) gobj;
    fired_table = 4;
}
typedef void (*AirEntry)(Fighter_GObj*);
static AirEntry ftData_SpecialAirHi[FTKIND_COUNT];
static AirEntry ftData_SpecialAirLw[FTKIND_COUNT];
static AirEntry ftData_SpecialAirS[FTKIND_COUNT];
static AirEntry ftData_SpecialAirN[FTKIND_COUNT];

static _Thread_local int update_facing_calls;
static void ftCommon_UpdateFacing(Fighter* fp) {
    fp->facing_dir = -fp->facing_dir;
    update_facing_calls++;
}

#include "special_air_original.inc"

/* `has_hi`/`has_lw`/`has_s`/`has_n` script whether Fox's own table entry
 * exists in each of the four tables (all true models the real game; each
 * individually false exercises the "character has no such move" branch).
 * Returns which table fired: 0 none, 1 Hi, 2 Lw, 3 S, 4 N. `*out_facing`
 * and `*out_update_facing` report the side branch's own turn check. */
int oracle_special_air_check_input(bool has_hi, bool has_lw, bool has_s, bool has_n,
                                    u32 pressed_buttons, float stick_x, float stick_y,
                                    float vertical_threshold, float side_threshold,
                                    float turn_threshold, float neutral_threshold, float facing,
                                    float x676_x, bool x2228_b7, float* out_facing,
                                    int* out_update_facing) {
    memset(ftData_SpecialAirHi, 0, sizeof(ftData_SpecialAirHi));
    memset(ftData_SpecialAirLw, 0, sizeof(ftData_SpecialAirLw));
    memset(ftData_SpecialAirS, 0, sizeof(ftData_SpecialAirS));
    memset(ftData_SpecialAirN, 0, sizeof(ftData_SpecialAirN));
    ftData_SpecialAirHi[FTKIND_FOX] = has_hi ? ftData_SpecialAirHi_Fox : NULL;
    ftData_SpecialAirLw[FTKIND_FOX] = has_lw ? ftData_SpecialAirLw_Fox : NULL;
    ftData_SpecialAirS[FTKIND_FOX] = has_s ? ftData_SpecialAirS_Fox : NULL;
    ftData_SpecialAirN[FTKIND_FOX] = has_n ? ftData_SpecialAirN_Fox : NULL;
    p_ftCommonData->x21C = vertical_threshold;
    p_ftCommonData->x218 = side_threshold;
    p_ftCommonData->x220 = turn_threshold;
    p_ftCommonData->x224 = neutral_threshold;
    Fighter fp;
    memset(&fp, 0, sizeof(fp));
    fp.kind = FTKIND_FOX;
    fp.input.pressed_buttons = pressed_buttons;
    fp.input.lstick[0].x = stick_x;
    fp.input.lstick[0].y = stick_y;
    fp.facing_dir = facing;
    fp.x676_x = x676_x;
    fp.x2228_b7 = x2228_b7;
    Fighter_GObj gobj = { &fp };
    fired_table = 0;
    update_facing_calls = 0;
    ftCo_SpecialAir_CheckInput(&gobj);
    *out_facing = fp.facing_dir;
    *out_update_facing = update_facing_calls;
    return fired_table;
}
