/* Complete original clash helpers, type-3 victim insertion and rebound entry/
 * first-physics callbacks. Host pointers stand for stable nonzero native IDs.
 * Only ordinary non-Slash hits are supported: the source sound callback takes
 * its nonrandom branch, so the no-result audio/effect stubs consume no RNG.
 * We do not emulate the parent collision scan or motion/animation graph. */
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
typedef uint8_t u8;
typedef int8_t s8;
typedef struct { float x,y,z; } Vec3;
typedef struct { void* victim; uint32_t x4; } HitVictim;
typedef struct {
    int state;
    uint32_t x4;
    float damage;
    bool x40_b1;
    uint32_t x40_b4;
    Vec3 hurt_coll_pos;
    HitVictim victims_1[12];
    uint8_t x44;
} HitCapsule;
typedef struct Fighter Fighter;
typedef struct { Fighter* user_data; } Fighter_GObj;
typedef Fighter_GObj HSD_GObj;
struct Fighter {
    int ground_or_air;
    HitCapsule x914[4];
    Vec3 cur_pos;
    struct { int int_value; float x191C, facing_dir; } dmg;
    struct { float clank_animation_length; } co_attrs;
    struct { struct { struct { float anim_speed, x0; } rebound; } co; } mv;
    float xE8_ground_accel_2, friction;
    int friction_calls, motion;
};
typedef struct { int x3CC; float x3D0,x3D4,x3D8,x3DC; } ftCommonData;
static _Thread_local ftCommonData common;
static _Thread_local ftCommonData* p_ftCommonData;
static _Thread_local s8 ftColl_804D6560[4];
#define ARRAY_SIZE(a) (sizeof(a)/sizeof((a)[0]))
#define PAD_STACK(n)
#define HitCapsule_Disabled 0
#define GA_Ground 0
#define GET_FIGHTER(g) ((g)->user_data)
#define Ft_MF_None 0
#define ftCo_MS_ReboundStop 237
static void efSync_Spawn(int id,void* unused,Vec3* point) {(void)id;(void)unused;(void)point;}
static void ftColl_800784B4(Fighter* f,HitCapsule* a,HitCapsule* b) {(void)f;(void)a;(void)b;}
static void Fighter_ChangeMotionState(Fighter_GObj* g,int motion,int flags,float start,float rate,float blend,void* other) {
    (void)flags;(void)start;(void)rate;(void)blend;(void)other;g->user_data->motion=motion;
}
static float ft_GetGroundFrictionMultiplier(Fighter* f) {return f->friction;}
static void ft_80084F3C(Fighter_GObj* g) {g->user_data->friction_calls++;}
#include "clank_victims_original.inc"
#include "clank_response_original.inc"
#include "clank_push_original.inc"
#include "clank_rebound_original.inc"

typedef struct {
    uint32_t enabled,group,rebound,next;
    float damage;
    uint32_t victims[12][2];
} BridgeHit;
typedef struct {
    uint32_t id,grounded;
    float x;
    int32_t damage;
    float duration,towards;
    BridgeHit hits[4];
} BridgeFighter;
static void* pointer(uint32_t id,Fighter* f,const BridgeFighter* b) {
    if(id==b[0].id)return &f[0];
    if(id==b[1].id)return &f[1];
    return (void*)(uintptr_t)id;
}
static uint32_t identity(void* p,Fighter* f,const BridgeFighter* b) {
    if(p==&f[0])return b[0].id;
    if(p==&f[1])return b[1].id;
    return (uint32_t)(uintptr_t)p;
}
int oracle_clank_pair(BridgeFighter* b,const uint32_t* slots,uint32_t* candidates,int gap,float scale,float base) {
    Fighter f[2]={0};
    common=(ftCommonData){.x3CC=gap,.x3D0=scale,.x3D4=base};p_ftCommonData=&common;
    for(size_t side=0;side<2;side++) {
        f[side].ground_or_air=b[side].grounded?0:1;
        f[side].cur_pos.x=b[side].x;
        f[side].dmg.int_value=b[side].damage;
        f[side].dmg.x191C=b[side].duration;
        f[side].dmg.facing_dir=b[side].towards;
        for(size_t slot=0;slot<4;slot++) {
            HitCapsule* h=&f[side].x914[slot];BridgeHit* bridge=&b[side].hits[slot];
            h->state=bridge->enabled;h->x4=bridge->group;h->damage=bridge->damage;
            h->x40_b1=bridge->rebound;h->x44=bridge->next;
            for(size_t v=0;v<12;v++) {
                h->victims_1[v]=(HitVictim){pointer(bridge->victims[v][0],f,b),bridge->victims[v][1]};
            }
        }
    }
    for(size_t i=0;i<4;i++)ftColl_804D6560[i]=candidates[i];
    int result=ftColl_8007699C(&f[0],&f[0].x914[slots[0]],&f[1],&f[1].x914[slots[1]]);
    for(size_t side=0;side<2;side++) {
        b[side].damage=f[side].dmg.int_value;b[side].duration=f[side].dmg.x191C;b[side].towards=f[side].dmg.facing_dir;
        for(size_t slot=0;slot<4;slot++) {
            HitCapsule* h=&f[side].x914[slot];BridgeHit* bridge=&b[side].hits[slot];bridge->next=h->x44;
            for(size_t v=0;v<12;v++) {
                bridge->victims[v][0]=identity(h->victims_1[v].victim,f,b);
                bridge->victims[v][1]=h->victims_1[v].x4;
            }
        }
    }
    for(size_t i=0;i<4;i++)candidates[i]=ftColl_804D6560[i];
    return result;
}
int oracle_clank_victim(uint32_t* entries,uint32_t* next,uint32_t id) {
    HitCapsule h={0};h.x44=*next;
    for(size_t i=0;i<12;i++)h.victims_1[i]=(HitVictim){(void*)(uintptr_t)entries[2*i],entries[2*i+1]};
    int result=lbColl_80008688(&h,3,(void*)(uintptr_t)id);
    for(size_t i=0;i<12;i++){entries[2*i]=(uint32_t)(uintptr_t)h.victims_1[i].victim;entries[2*i+1]=h.victims_1[i].x4;}
    *next=h.x44;return result;
}
void oracle_clank_rebound(const float* v,float* out) {
    common=(ftCommonData){.x3D8=v[3],.x3DC=v[4]};p_ftCommonData=&common;
    Fighter f={.dmg={.x191C=v[0],.facing_dir=v[1]},.co_attrs.clank_animation_length=v[2],.friction=v[5]};
    Fighter_GObj g={&f};ftCo_80099D9C(&g);
    out[0]=f.mv.co.rebound.anim_speed;out[1]=f.mv.co.rebound.x0;out[2]=f.xE8_ground_accel_2;
    ftCo_Rebound_Phys(&g);out[3]=f.mv.co.rebound.x0;out[4]=f.friction_calls;
    ftCo_Rebound_Phys(&g);out[5]=f.friction_calls;out[6]=f.motion;
}
