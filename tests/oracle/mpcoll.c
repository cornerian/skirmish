/* Selected original mpcoll.c bodies, unchanged. Minimal host field layout.
 * JObj world queries copy supplied native samples; bounding calls expose the
 * original rectangle. No graph traversal or stage callback graph is modeled. */
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
typedef uint8_t u8;
typedef uint32_t u32;
typedef int32_t s32;
typedef int enum_t;
typedef struct { float x, y; } Vec2;
typedef struct { float x, y, z; } Vec3;
typedef Vec3 HSD_JObj;
typedef struct { Vec2 top, bottom, left, right; } ftECB;
typedef struct { float top, bottom; Vec2 right, left; } ftCollisionBox;
typedef struct {
    ftECB ecb, prev_ecb, desired_ecb, xE4_ecb, x64_ecb;
    Vec3 cur_pos, prev_pos, last_pos;
    struct { u8 b0, b5, b6; } x34_flags;
    u32 x130_flags;
    struct {
        int kind;
        HSD_JObj* x10C_joint[6];
        float x124, x128, x12C;
        float up, down, front, back, angle;
    } ecb_source;
    int facing_dir;
    float ledge_snap_x, ledge_snap_y, ledge_snap_height;
} CollData;
#define CollData_X130_Clear (1u << 5)
#define CollData_X130_Locked (1u << 4)
#define CollisionFlagAir_CanGrabLedge 4u
#define ECBSource_JObj 1
#define ABS(a) (((a) < 0) ? -(a) : (a))
#define MIN(a,b) (((a) < (b)) ? (a) : (b))
#define HSD_ASSERTREPORT(line,condition,message) do { if (!(condition)) abort(); } while (0)
static _Thread_local float bounds[4];
static void mpBoundingCheck(float left, float bottom, float right, float top) {
    bounds[0]=left; bounds[1]=bottom; bounds[2]=right; bounds[3]=top;
}
static void lb_8000B1CC(const HSD_JObj* joint, const void* unused, Vec3* output) { *output=*joint; }
static void lbVector_Diff(const Vec3* a, const Vec3* b, Vec3* output) {
    output->x=a->x-b->x; output->y=a->y-b->y; output->z=a->z-b->z;
}
#include "mpcoll_original.inc"
#include "mpcoll_plan.inc"

static ftECB shape(const float* p) { return (ftECB){{p[0],p[1]},{p[2],p[3]},{p[4],p[5]},{p[6],p[7]}}; }
static void save_shape(ftECB s, float* p) {
    p[0]=s.top.x; p[1]=s.top.y; p[2]=s.bottom.x; p[3]=s.bottom.y;
    p[4]=s.left.x; p[5]=s.left.y; p[6]=s.right.x; p[7]=s.right.y;
}
void oracle_ecb_apply(u32 mode, float* storage, u32* flags, const float* args, u32 options, float* extra) {
    CollData cd = {
        .ecb=shape(storage), .prev_ecb=shape(storage+8), .desired_ecb=shape(storage+16),
        .xE4_ecb=shape(storage+24), .x64_ecb=shape(storage+32),
        .cur_pos={storage[40],storage[41],storage[42]},
        .prev_pos={storage[43],storage[44],storage[45]},
        .last_pos={storage[46],storage[47],storage[48]},
        .x34_flags={flags[3],flags[4],flags[2]},
        .x130_flags=(flags[0] ? CollData_X130_Clear : 0) | (flags[1] ? CollData_X130_Locked : 0),
    };
    switch (mode) {
    case 0: mpColl_80042384(&cd); break;
    case 1:
        cd.ecb_source.up=args[0]; cd.ecb_source.down=args[1]; cd.ecb_source.front=args[2];
        cd.ecb_source.back=args[3]; cd.ecb_source.angle=args[4]; cd.facing_dir=(int32_t)options;
        mpColl_LoadECB(&cd); break;
    case 2: {
        HSD_JObj samples[6];
        for (int i=0;i<6;i++) { samples[i]=(Vec3){args[i*2],args[i*2+1],0}; cd.ecb_source.x10C_joint[i]=samples+i; }
        cd.ecb_source.kind=ECBSource_JObj; cd.ecb_source.x124=args[12]; cd.ecb_source.x128=args[13]; cd.ecb_source.x12C=args[14];
        mpColl_LoadECB_inline(&cd, (enum_t)options); break;
    }
    case 3: {
        ftCollisionBox external={args[1],args[3],{args[6],args[7]},{args[4],args[5]}};
        mpColl_80042C58(&cd,&external); break;
    }
    case 4: mpCollInterpolateECB(&cd,args[0]); break;
    case 5: mpCollSqueezeHorizontal(&cd,false,args[0],args[1]); break;
    case 6: mpCollSqueezeVertical(&cd,options!=0,args[0],args[1]); break;
    case 7:
        cd.ledge_snap_x=args[0]; cd.ledge_snap_y=args[1]; cd.ledge_snap_height=args[2];
        mpCollCheckBounding(&cd, options); for (int i=0;i<4;i++) extra[i]=bounds[i]; break;
    case 8: oracle_ecb_plan(&cd,extra); break;
    default: abort();
    }
    save_shape(cd.ecb,storage); save_shape(cd.prev_ecb,storage+8); save_shape(cd.desired_ecb,storage+16);
    save_shape(cd.xE4_ecb,storage+24); save_shape(cd.x64_ecb,storage+32);
    storage[40]=cd.cur_pos.x; storage[41]=cd.cur_pos.y; storage[42]=cd.cur_pos.z;
    flags[0]=(cd.x130_flags&CollData_X130_Clear)!=0; flags[1]=(cd.x130_flags&CollData_X130_Locked)!=0;
    flags[2]=cd.x34_flags.b6; flags[3]=cd.x34_flags.b0; flags[4]=cd.x34_flags.b5;
}
