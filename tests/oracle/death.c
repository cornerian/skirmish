/* Host adapter for the complete blast-line and top-death selector. */
#include <stdbool.h>
#include <stdint.h>

typedef struct Vec3 {
    float x;
    float y;
    float z;
} Vec3;

typedef struct Fighter {
    bool x222A_b1;
    bool x2228_b5;
    bool x2228_b2;
    bool x2219_b2;
    bool x2219_b1;
    Vec3 cur_pos;
    int ground_or_air;
    bool x2222_b3;
    Vec3 x8c_kb_vel;
    int player_id;
    int motion_id;
} Fighter;

typedef struct Fighter_GObj {
    Fighter* user_data;
} Fighter_GObj;

typedef struct ftCommonData {
    float x4F0;
    int x520;
} ftCommonData;

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define GA_Ground 0
#define GA_Air 1
#define ftCo_MS_DamageIce 999

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local float blast_left;
static _Thread_local float blast_right;
static _Thread_local float blast_bottom;
static _Thread_local float blast_top;
static _Thread_local bool force_normal_top;
static _Thread_local bool camera_disables_screen;
static _Thread_local uint32_t hsd_seed;
static _Thread_local int selected;

static float Stage_GetBlastZoneRightOffset(void) { return blast_right; }
static float Stage_GetBlastZoneLeftOffset(void) { return blast_left; }
static float Stage_GetBlastZoneTopOffset(void) { return blast_top; }
static float Stage_GetBlastZoneBottomOffset(void) { return blast_bottom; }
static bool Player_GetMoreFlagsBit5(int player) {
    (void) player;
    return force_normal_top;
}
static bool Camera_8003010C(void) { return camera_disables_screen; }
static int HSD_Randi(int maximum) {
    hsd_seed = hsd_seed * UINT32_C(214013) + UINT32_C(2531011);
    return maximum * (int) (hsd_seed >> 16) / 65536;
}
static void ftCo_800D3680(Fighter_GObj* gobj) { (void) gobj; selected = 1; }
static void ftCo_800D3950(Fighter_GObj* gobj) { (void) gobj; selected = 2; }
static void ftCo_800D3BC8(Fighter_GObj* gobj) { (void) gobj; selected = 3; }
static void ftCo_800D3E40(Fighter_GObj* gobj) { (void) gobj; selected = 4; }
static void ftCo_800D40B8(Fighter_GObj* gobj) { (void) gobj; selected = 5; }
static void ftCo_800D41C4(Fighter_GObj* gobj) { (void) gobj; selected = 6; }
static void ftCo_800D4780(Fighter_GObj* gobj) { (void) gobj; selected = 7; }
static void ftCo_800D47B8(Fighter_GObj* gobj) { (void) gobj; selected = 8; }

#include "death_original.inc"

int oracle_blast_death(uint32_t exclusions, float x, float y, float left,
                       float right, float bottom, float top, bool grounded,
                       bool forced_top_eligible, float knockback_y,
                       float top_threshold, bool normal_top,
                       bool disable_screen, int screen_chance, bool ice,
                       uint32_t* seed) {
    Fighter fighter = { 0 };
    Fighter_GObj gobj = { &fighter };
    fighter.x222A_b1 = exclusions & 1;
    fighter.x2228_b5 = exclusions & 2;
    fighter.x2228_b2 = exclusions & 4;
    fighter.x2219_b2 = exclusions & 8;
    fighter.x2219_b1 = exclusions & 16;
    fighter.cur_pos = (Vec3) { x, y, 0 };
    fighter.ground_or_air = grounded ? GA_Ground : GA_Air;
    fighter.x2222_b3 = forced_top_eligible;
    fighter.x8c_kb_vel.y = knockback_y;
    fighter.motion_id = ice ? ftCo_MS_DamageIce : 0;
    common.x4F0 = top_threshold;
    common.x520 = screen_chance;
    p_ftCommonData = &common;
    blast_left = left;
    blast_right = right;
    blast_bottom = bottom;
    blast_top = top;
    force_normal_top = normal_top;
    camera_disables_screen = disable_screen;
    hsd_seed = *seed;
    selected = 0;
    bool result = ftCo_800D3158(&gobj);
    *seed = hsd_seed;
    return result ? selected : 0;
}
