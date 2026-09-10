/* Host adapter for complete captured-fighter bone alignment. */
#include <stdbool.h>
#include <stddef.h>

typedef unsigned char u8;
typedef struct {
    float x;
    float y;
    float z;
} Vec3;
typedef struct {
    Vec3 world;
} HSD_JObj;
typedef struct Fighter_GObj Fighter_GObj;
typedef struct {
    HSD_JObj* joint;
} FighterPart;
typedef struct Fighter {
    Fighter_GObj* victim_gobj;
    struct {
        struct {
            struct {
                HSD_JObj* x18;
            } capturedamage;
        } co;
    } mv;
    FighterPart parts[1];
    Vec3 cur_pos;
    Vec3 x34_scale;
} Fighter;
struct Fighter_GObj {
    Fighter* user_data;
};
typedef struct {
    float x3C4;
} ftCommonData;

static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;

#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define PAD_STACK(size) ((void) (size))
#define FtPart_XRotN 0

static int ftParts_GetBoneIndex(Fighter* fp, int part) {
    (void) fp;
    return part;
}

static void lb_8000B1CC(HSD_JObj* joint, void* matrix, Vec3* output) {
    (void) matrix;
    *output = joint->world;
}

#include "capture_alignment_original.inc"

int oracle_capture_alignment(const float position[3],
                             const float holder_anchor[3],
                             const float victim_anchor[3], float threshold,
                             float scale_y, float output[3]) {
    HSD_JObj holder_joint = {
        { holder_anchor[0], holder_anchor[1], holder_anchor[2] }
    };
    HSD_JObj victim_joint = {
        { victim_anchor[0], victim_anchor[1], victim_anchor[2] }
    };
    Fighter holder = { 0 };
    Fighter victim = { 0 };
    Fighter_GObj holder_gobj = { &holder };
    Fighter_GObj victim_gobj = { &victim };
    holder.mv.co.capturedamage.x18 = &holder_joint;
    victim.victim_gobj = &holder_gobj;
    victim.parts[0].joint = &victim_joint;
    victim.cur_pos = (Vec3) { position[0], position[1], position[2] };
    victim.x34_scale.y = scale_y;
    common.x3C4 = threshold;
    p_ftCommonData = &common;
    int lifted = fn_800DAD18(&victim_gobj);
    output[0] = victim.cur_pos.x;
    output[1] = victim.cur_pos.y;
    output[2] = victim.cur_pos.z;
    return lifted;
}
