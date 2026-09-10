/* Host adapter for the complete multijump root-turn callback. */
#include <stdint.h>

typedef float f32;
typedef int32_t s32;
typedef struct HSD_JObj {
    f32 rotation_y;
} HSD_JObj;
typedef struct {
    HSD_JObj* joint;
} FtPart;
typedef struct Fighter {
    struct {
        struct {
            struct {
                s32 x0;
            } jumpaerial;
        } co;
    } mv;
    f32 facing_dir;
    FtPart* parts;
} Fighter;

#define PAD_STACK(size) ((void) (size))
#define MTXDegToRad(value) ((value) * 0.01745329252f)

static void HSD_JObjAddRotationY(HSD_JObj* joint, f32 value) {
    joint->rotation_y += value;
}

#include "multi_jump_original.inc"

void oracle_multi_jump_turn(s32* remaining, f32* facing, f32* yaw, s32 total) {
    HSD_JObj joint = { *yaw };
    FtPart part = { &joint };
    Fighter fighter = {
        .mv.co.jumpaerial.x0 = *remaining,
        .facing_dir = *facing,
        .parts = &part,
    };
    ft_800CB6EC(&fighter, total);
    *remaining = fighter.mv.co.jumpaerial.x0;
    *facing = fighter.facing_dir;
    *yaw = joint.rotation_y;
}
