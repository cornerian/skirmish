/* Whole original ftcommon functions; arrays provide the caller-owned entity
 * list, Player_GetEntity mapping and already-resolved floor-neighbor results.
 * The same-player predicate is the complete ftLib_80086FD4 non-null branch.
 * Only pointer/layout/global storage is adapted; no nudge arithmetic is copied.
 * Each call has its own thread-local world, allowing parallel Rust tests. */
#include <stdint.h>
#include <stdlib.h>
typedef uint8_t u8;
typedef int32_t s32;
typedef int bool;
typedef struct { float x, y; } Vec2;
typedef struct { float x, y, z; } Vec3;
typedef struct HSD_GObj HSD_GObj;
typedef struct {
    Vec3 cur_pos, xD4_unk_vel;
    float facing_dir;
    Vec2 x2C4, xF8_playerNudgeVel;
    int player_id, ground_or_air;
    struct { struct { int index; } floor; } coll_data;
    HSD_GObj* victim_gobj;
    bool x221F_b3, x221F_b4, x2219_b1, x2219_b5, x221D_b5;
} Fighter;
struct HSD_GObj { Fighter* user_data; HSD_GObj* next; };
typedef struct { HSD_GObj* fighters; } Entities;
typedef struct { float x450, x454, x458, x45C, x460; } Common;
#define false 0
#define true 1
#define GA_Ground 0
#define ABS(x) ((x) < 0 ? -(x) : (x))
static _Thread_local Entities entities;
static _Thread_local Common common;
static _Thread_local HSD_GObj* leaders[256];
static _Thread_local const int32_t* floor_neighbors;
#define HSD_GObj_Entities (&entities)
#define p_ftCommonData (&common)
static int ftLib_80086FD4(HSD_GObj* a, HSD_GObj* b) {
    return a == b || a->user_data->player_id == b->user_data->player_id;
}
static HSD_GObj* Player_GetEntity(int player) { return leaders[player]; }
static int mpLineGetPrev(int line) { return floor_neighbors[2*line]; }
static int mpLineGetNext(int line) { return floor_neighbors[2*line+1]; }
#include "nudge_original.inc"

/* Numeric row: position XYZ, deferred XYZ, facing, offset, half-width.
 * Integer row: player, floor(-1=air), owner(-1=leader), flags:
 * inactive1, holds-victim2, nudge-disabled4, hitlag8, overlap-disabled16.
 * Caller supplies valid body/owner/floor indices and at most 16 entities. */
void oracle_nudge(uint32_t count, uint32_t subject, const float* numbers,
                  const int32_t* metadata, const int32_t* neighbors,
                  const float rules[5], float output[5]) {
    if (count > 16 || subject >= count) abort();
    Fighter fighters[16] = {0};
    HSD_GObj objects[16] = {0};
    floor_neighbors = neighbors;
    common = (Common){rules[0], rules[1], rules[2], rules[3], rules[4]};
    for (int i=0; i<256; i++) leaders[i] = NULL;
    for (uint32_t i=0; i<count; i++) {
        const float* n = numbers+9*i;
        const int32_t* m = metadata+4*i;
        Fighter* f = &fighters[i];
        f->cur_pos = (Vec3){n[0],n[1],n[2]};
        f->xD4_unk_vel = (Vec3){n[3],n[4],n[5]};
        f->facing_dir = n[6]; f->x2C4 = (Vec2){n[7],n[8]};
        f->player_id = m[0]; f->ground_or_air = m[1] < 0;
        f->coll_data.floor.index = m[1];
        f->x221F_b4 = m[2] >= 0;
        f->x221F_b3 = !!(m[3]&1); f->x2219_b1 = !!(m[3]&4);
        f->x2219_b5 = !!(m[3]&8); f->x221D_b5 = !!(m[3]&16);
        f->victim_gobj = m[3]&2 ? &objects[i] : NULL;
        f->xF8_playerNudgeVel = (Vec2){123.0f,-456.0f};
        objects[i] = (HSD_GObj){f, i+1 < count ? &objects[i+1] : NULL};
        if (m[2] >= 0) leaders[m[0]] = &objects[m[2]];
    }
    entities.fighters = objects;
    ftCommon_8007E0E4(&objects[subject]);
    output[0] = fighters[subject].xF8_playerNudgeVel.x;
    output[1] = fighters[subject].xF8_playerNudgeVel.y;
    Vec3 position;
    ftCommon_8007F8B4(&fighters[subject], &position);
    output[2] = position.x; output[3] = position.y; output[4] = position.z;
}
