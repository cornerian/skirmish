/* The unchanged landing transition exposes the animation-rate expression;
 * side effects unrelated to that scalar result are excluded by the host. */
#include <stddef.h>
#include <stdint.h>
typedef uint8_t u8;
typedef int FtMotionId;
typedef struct {int unused;} Fighter;
typedef struct {Fighter* user_data;} Fighter_GObj;
#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define Ft_MF_None 0
static _Thread_local float end_frame,result;
static void ftCommon_8007D7FC(Fighter* fp) {(void)fp;}
static void Fighter_ChangeMotionState(Fighter_GObj* gobj,int msid,int flags,
    float frame,float speed,float value,void* ptr) {
    (void)gobj;(void)msid;(void)flags;(void)frame;(void)speed;(void)value;(void)ptr;
}
static float ftAnim_8006F484(Fighter_GObj* gobj) {(void)gobj;return end_frame;}
static void ftAnim_SetAnimRate(Fighter_GObj* gobj,float rate) {(void)gobj;result=rate;}
#include "aerial_rate_original.inc"
float oracle_aerial_rate(float end,float lag) {
    Fighter fighter={0};Fighter_GObj object={&fighter};end_frame=end;
    ftCo_LandingAir_EnterWithMsidLag(&object,70,lag);
    return result;
}
