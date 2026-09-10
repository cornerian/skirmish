/* Whole pinned function bodies with minimal host layout. Landing transition
 * callbacks only capture the original selected motion/lag arguments. */
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
typedef uint8_t u8;
typedef int FtMotionId;
typedef struct { float x,y; } Vec2;
typedef struct {
    struct { Vec2 lstick[2],cstick[2]; } input;
    float facing_dir;
    int cmd_vars[1],motion_id;
    uint8_t x67F;
    struct { float landingairn_lag,landingairf_lag,landingairb_lag,
        landingairhi_lag,landingairlw_lag; } co_attrs;
} Fighter;
typedef struct {Fighter* user_data;} Fighter_GObj;
typedef struct {float xDC,xE0,x20_radians,xE8,x248,x7F4; int xE4;} ftCommonData;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local float captured_lag;
static _Thread_local int captured_motion;
#define GET_FIGHTER(gobj) ((gobj)->user_data)
/* Runtime/platform.h definition, including signed-zero behavior. */
#define ABS(x) ((x) < 0 ? -(x) : (x))
enum {ftCo_MS_None=-1,ftCo_MS_AttackAirN=65,ftCo_MS_AttackAirF,
    ftCo_MS_AttackAirB,ftCo_MS_AttackAirHi,ftCo_MS_AttackAirLw,
    ftCo_MS_LandingAirN,ftCo_MS_LandingAirF,ftCo_MS_LandingAirB,
    ftCo_MS_LandingAirHi,ftCo_MS_LandingAirLw};
static void ftCo_LandingAir_EnterWithMsidLag(Fighter_GObj* gobj,FtMotionId msid,float lag) {
    (void)gobj; captured_lag=lag;captured_motion=msid;
}
static void ftCo_Landing_Enter_Basic(Fighter_GObj* gobj) {(void)gobj;captured_motion=-1;}
#include "aerial_angles_original.inc"
#include "aerial_input_original.inc"
#include "aerial_select_original.inc"
#include "aerial_landing_original.inc"

static Fighter setup(const float* values,const float* rules) {
    static _Thread_local ftCommonData common;
    common=(ftCommonData){.xDC=rules[0],.xE0=rules[1],.x20_radians=rules[2]};
    p_ftCommonData=&common;
    return (Fighter){.input={.lstick={{values[0],values[1]}},
        .cstick={{values[2],values[3]},{values[4],values[5]}}},.facing_dir=values[6]};
}
int oracle_aerial_select(const float* values,const float* rules) {
    Fighter fighter=setup(values,rules);
    return ftCo_AttackAir_GetMsidFromCStick(&fighter)-ftCo_MS_AttackAirN;
}
int oracle_aerial_fresh(const float* values,const float* rules) {
    Fighter fighter=setup(values,rules);
    return ftCo_800DF478(&fighter);
}
float oracle_aerial_angle(const float* stick,int cstick) {
    Fighter fighter={.input={.lstick={{stick[0],stick[1]}},.cstick={{stick[0],stick[1]}}}};
    return cstick?ftCo_GetCStickAngle(&fighter):ftCo_GetLStickAngle(&fighter);
}
float oracle_aerial_lag(float base,uint8_t age,int window,float divisor,int direction) {
    static _Thread_local ftCommonData common;
    common=(ftCommonData){.xE4=window,.xE8=divisor};p_ftCommonData=&common;
    Fighter fighter={.cmd_vars={1},.motion_id=ftCo_MS_AttackAirN+direction,
        .x67F=age,.co_attrs={base,base,base,base,base}};
    Fighter_GObj object={&fighter};
    captured_lag=-999;
    ftCo_LandingAir_EnterWithLag(&object);
    return captured_lag;
}
int oracle_knockdown_cstick_up(float previous,float current,float threshold) {
    static _Thread_local ftCommonData common;
    common=(ftCommonData){.x7F4=threshold};p_ftCommonData=&common;
    Fighter fighter={.input={.cstick={{0,current},{0,previous}}}};
    return ftCo_800DF644(&fighter);
}
int oracle_knockdown_cstick_horizontal(const float* values,float threshold,float angle) {
    static _Thread_local ftCommonData common;
    common=(ftCommonData){.x248=threshold,.x20_radians=angle};p_ftCommonData=&common;
    Fighter fighter={.input={.cstick={{values[2],values[3]},{values[0],values[1]}}}};
    return ftCo_800DF678(&fighter);
}
