/* Complete selected upstream function bodies. Host layout is deliberately
 * minimal; graphics/audio/statistics stubs have no inputs feeding arithmetic.
 * Common data and all per-call objects are thread-local or stack owned. */
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
typedef uint8_t u8;
typedef int8_t s8;
typedef struct { float x,y,z; } Vec3;
typedef struct { float end; } HSD_JObj;
typedef struct { int x11; } BoneIds;
typedef struct { BoneIds* x8; } FighterData;
typedef struct {
    float x260_startShieldHealth, x2D8, x2D4, x264;
    float analog_shoulder_deadzone, x278, x2F0, x2EC;
    float x28C, x2E8, x2E4, x290, x294, x2BC, x298;
    float sdi_min_stick_mag, sdi_pos_scale, x4BC, x4C0, x308;
    int sdi_stick_window;
} ftCommonData;
typedef struct Fighter Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;
typedef Fighter_GObj HSD_GObj;
typedef void (*HSD_GObjEvent)(Fighter_GObj*);
struct Fighter {
    int kind, player_id, x221F_b4, ground_or_air;
    bool x221B_b0, x221A_b7, x221C_b2, x2219_b0, allow_sdi;
    float shield_health, lightshield_amount, specialn_facing_dir, gr_vel, rate;
    int x19A4;
    HSD_GObjEvent hitlag_cb, post_hitlag_cb;
    struct { float initial_shield_size; } co_attrs;
    struct { struct { struct { float x2C, x10; } guard; } co; } mv;
    struct { float triggers[1]; Vec3 lstick[1]; uint32_t pressed_buttons; } input;
    struct { struct { Vec3 normal; } floor; } coll_data;
    Vec3 cur_pos;
    uint8_t x670_timer_lstick_tilt_x;
    struct { HSD_JObj* joint; } parts[1];
    FighterData* ft_data;
    HSD_JObj animation;
    float grab_timer;
    s8 x1A50, x1A51;
    bool x2224_b6, x2224_b5;
    int x1A52, x1A53;
};
typedef struct { int x0_bone_id; float x10_size; Vec3 x4_offset; } AbsorbDesc;
typedef AbsorbDesc ShieldDesc;
#define GET_FIGHTER(g) ((g)->user_data)
#define GET_JOBJ(g) (&(g)->user_data->animation)
#define PAD_STACK(n)
#define FTKIND_YOSHI 14
#define GA_Ground 0
#define ftCo_MS_GuardSetOff 182
#define Ft_MF_None 0
#define HSD_PAD_AB 0x300
#define HSD_PAD_XY 0xc00
#define HSD_PAD_LR 0x60
static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static void pl_8003E0E8(int a,int b) {(void)a;(void)b;}
static void ftCo_80098B20(Fighter_GObj* g) {(void)g;}
static void ft_PlaySFX(Fighter* f,int a,int b,int c) {(void)f;(void)a;(void)b;(void)c;}
static void Fighter_ChangeMotionState(Fighter_GObj* g,int a,int b,float c,float d,float e,void* f) {(void)g;(void)a;(void)b;(void)c;(void)d;(void)e;(void)f;}
static void ftCo_80092158_inline(Fighter_GObj* g,int a,HSD_JObj* j) {(void)g;(void)a;(void)j;}
static float lbGetJObjEndFrame(HSD_JObj* j) { return j->end; }
static void ftAnim_SetAnimRate(Fighter_GObj* g,float rate) {g->user_data->rate=rate;}
static void ftColl_8007B1B8(Fighter_GObj* g,ShieldDesc* s,HSD_GObjEvent cb) {(void)g;(void)s;(void)cb;}
static void HSD_JObjSetScale(HSD_JObj* j,Vec3* v) {(void)j;(void)v;}
static void ftCo_80092E50(Fighter_GObj* g) {(void)g;}
static void pl_800402D0(int a,int b,bool result) {(void)a;(void)b;(void)result;}
void ftCo_80093240(Fighter_GObj*);
void ftCo_800932DC(Fighter_GObj*);
#include "shield_guard_original.inc"
#include "shield_mash_original.inc"
#include "shield_environment_original.inc"

int oracle_shield_environment_damage(float damage) { return getEnvDmg(damage); }

float oracle_shield_radius(const float* v) {
    common=(ftCommonData){.x260_startShieldHealth=v[1],.x2D4=v[3],.x2D8=v[4],.x264=v[5]}; p_ftCommonData=&common;
    Fighter f={.shield_health=v[0],.lightshield_amount=v[2],.co_attrs.initial_shield_size=v[6]};
    return inlineB0(&f);
}
int oracle_shield_drain(const float* v,float* out) {
    common=(ftCommonData){.analog_shoulder_deadzone=v[2],.x278=v[6],.x2EC=v[4],.x2F0=v[5]}; p_ftCommonData=&common;
    Fighter f={.shield_health=v[0],.input.triggers={v[1]},.lightshield_amount=v[3],.x221B_b0=true,.mv.co.guard.x10=v[7]};
    Fighter_GObj g={&f}; int broken=ftCo_800925A4(&g);
    out[0]=f.shield_health;out[1]=f.lightshield_amount;out[2]=f.mv.co.guard.x10;return broken;
}
void oracle_shield_response(int damage,const float* v,float* out) {
    common=(ftCommonData){.x28C=v[3],.x290=v[4],.x2E4=v[1],.x2E8=v[2],.x294=v[6],.x2BC=v[7],.x298=v[8],.x260_startShieldHealth=60,.x264=1}; p_ftCommonData=&common;
    BoneIds ids={0}; FighterData data={&ids};
    Fighter f={.x19A4=damage,.lightshield_amount=v[0],.specialn_facing_dir=v[9],.animation.end=v[5],.ft_data=&data};
    Fighter_GObj g={&f}; out[0]=ftCo_80092ED8(damage,v[0]);ftCo_80092F2C(&g,false);out[1]=f.rate;out[2]=f.gr_vel;
}
void oracle_shield_displacement(const float* v,uint8_t* timer,int window,int exit,float* out) {
    common=(ftCommonData){.sdi_min_stick_mag=v[6],.sdi_pos_scale=v[7],.x4BC=v[7],.x4C0=v[8],.sdi_stick_window=window}; p_ftCommonData=&common;
    Fighter f={.cur_pos={v[0],v[1],0},.coll_data.floor.normal={v[2],v[3],0},.input.lstick={{v[4],0,0}},.allow_sdi=(int)v[5],.x670_timer_lstick_tilt_x=*timer};
    Fighter_GObj g={&f};if(exit)ftCo_800932DC(&g);else ftCo_80093240(&g);
    out[0]=f.cur_pos.x;out[1]=f.cur_pos.y;*timer=f.x670_timer_lstick_tilt_x;
}
int oracle_shield_mash(float* timer,int8_t* directions,const float* stick,uint16_t pressed,float threshold,float amount) {
    common=(ftCommonData){.x308=threshold};p_ftCommonData=&common;
    Fighter f={.grab_timer=*timer,.x1A50=directions[0],.x1A51=directions[1],.input={.lstick={{stick[0],stick[1],0}},.pressed_buttons=pressed}};
    int result=ftCommon_GrabMash(&f,amount);*timer=f.grab_timer;directions[0]=f.x1A50;directions[1]=f.x1A51;return result;
}
