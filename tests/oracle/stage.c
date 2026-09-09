/* Original mplib functions over native-sized, caller-filled array adapters.
 * build.rs replaces one 32-bit pointer-byte subtraction with element index*8.
 * PSVECNormalize is explicitly mapped to original C_VECNormalize: this oracle
 * checks scalar normalization, not PowerPC's reciprocal-root approximation.
 * Allocations/globals are scoped to one call and thread; no shared stage state. */
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef int16_t s16;
typedef int32_t s32;
typedef uint32_t u32;
typedef uint8_t u8;
typedef float f32;
typedef struct { float x, y; } Vec2;
typedef struct { float x, y, z; } Vec3;
typedef Vec3 Vec;
typedef void Fighter_GObj;
typedef struct {
    uint16_t v0_idx, v1_idx;
    s16 prev_id0, next_id0, prev_id1, next_id1;
    uint16_t hi_flags, lo_flags;
} MapLine;
typedef struct { MapLine* x0; u32 flags; } CollLine;
typedef struct { Vec2 pos; } CollVtx;
typedef struct {
    s16 floor_start, floor_count, ceiling_start, ceiling_count;
    s16 right_wall_start, right_wall_count, left_wall_start, left_wall_count;
    s16 dynamic_start, dynamic_count;
} MapJoint;
typedef struct CollJoint {
    struct CollJoint* next;
    MapJoint* inner;
    u32 flags;
    Vec2 bounding_min, bounding_max;
} CollJoint;

#define CollLine_Floor 1
#define CollLine_Ceiling 2
#define CollLine_RightWall 4
#define CollLine_LeftWall 8
#define LINE_FLAG_EMPTY (1 << 7)
#define LINE_FLAG_ENABLED (1 << 16)
#define LINE_FLAG_HIDDEN (1 << 18)
#define CollJoint_B10 (1 << 10)
#define CollJoint_TooFar (1 << 12)
#define CollJoint_Enabled (1 << 16)
#define CollJoint_Hidden (1 << 18)
#define F32_MAX FLT_MAX
#define ABS(x) ((x) < 0 ? -(x) : (x))
#define SQ(x) ((x) * (x))
#define PAD_STACK(n)
#define ASSERTMSGLINE(line, condition, message) assert(condition)
#define LINEID_CHECK(line, id) assert((id) >= 0)
#define C_VECNormalize oracle_stage_normalize
#include "sdk_vec_original.inc"
#define PSVECNormalize oracle_stage_normalize

static _Thread_local bool didCheckBounding;
static _Thread_local CollVtx* groundCollVtx;
static _Thread_local CollLine* groundCollLine;
static _Thread_local CollJoint* groundCollJoint;
static _Thread_local CollJoint* jointListStart;
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-variable"
#include "mplib_original.inc"
#pragma GCC diagnostic pop

typedef struct {
    float start[2], end[2];
    u32 flags, material;
    s32 previous[2], next[2];
} OracleLine;
typedef struct {
    s32 id;
    u32 flags;
    float bounds_min[2], bounds_max[2];
    /* floor, ceiling, left wall, right wall, dynamic: start then length. */
    s32 ranges[10];
} OracleJoint;
typedef struct {
    float position[3], normal[3];
    s32 line_id;
    u32 flags, callback_count;
    s32 callback_ids[128];
} OracleContact;

static _Thread_local uint64_t accepted;
static _Thread_local OracleContact* active_output;
static bool accept_floor(Fighter_GObj* unused, int line_id) {
    assert(active_output->callback_count < 128);
    active_output->callback_ids[active_output->callback_count++] = line_id;
    return ((accepted >> line_id) & 1) != 0;
}

static MapLine* setup_lines(const OracleLine* lines, int count) {
    assert(count >= 0 && count <= 64);
    MapLine* map = calloc(count ? count : 1, sizeof(*map));
    groundCollLine = calloc(count ? count : 1, sizeof(*groundCollLine));
    groundCollVtx = calloc(count ? 2 * count : 1, sizeof(*groundCollVtx));
    assert(map && groundCollLine && groundCollVtx);
    for (int i = 0; i < count; ++i) {
        map[i] = (MapLine){2*i, 2*i+1,
            lines[i].previous[0], lines[i].next[0],
            lines[i].previous[1], lines[i].next[1], 0, lines[i].material};
        groundCollLine[i] = (CollLine){&map[i], lines[i].flags};
        memcpy(&groundCollVtx[2*i].pos, lines[i].start, sizeof(Vec2));
        memcpy(&groundCollVtx[2*i+1].pos, lines[i].end, sizeof(Vec2));
    }
    return map;
}

static void free_lines(MapLine* map) {
    free(map);
    free(groundCollLine);
    free(groundCollVtx);
    groundCollLine = NULL;
    groundCollVtx = NULL;
}

int oracle_stage_intersection(int kind, const float endpoints[8], float out[2]) {
    if (kind == 0)
        return mpLineIntersection(endpoints[0], endpoints[1], endpoints[2], endpoints[3],
            endpoints[4], endpoints[5], endpoints[6], endpoints[7], out, out+1);
    if (kind == 1)
        return mpLineIntersectionH(out, out+1, endpoints[0], endpoints[1], endpoints[2],
            endpoints[4], endpoints[5], endpoints[6], endpoints[7]);
    return mpLineIntersectionV(out, out+1, endpoints[0], endpoints[1], endpoints[3],
        endpoints[4], endpoints[5], endpoints[6], endpoints[7]);
}

void oracle_stage_remap(const float previous[4], const float current[4],
                        const float point[2], float out[2]) {
    mpRemap2d(out, out + 1, previous[0], previous[1], previous[2], previous[3],
              current[0], current[1], current[2], current[3], point[0], point[1]);
}

void oracle_stage_endpoints(const OracleLine* lines, int count, int id,
    float out[4], s32 neighbors[2]) {
    MapLine* map = setup_lines(lines, count);
    neighbors[0] = mpLineGetPrev(id);
    neighbors[1] = mpLineGetNext(id);
    mpLib_8004ED5C(id, out, out+1, out+2, out+3);
    free_lines(map);
}

int oracle_stage_project(const OracleLine* lines, int count, int kind, int id,
    const float point[2], float* delta, u32* flags, float normal_out[3]) {
    MapLine* map = setup_lines(lines, count);
    Vec3 vec = {point[0], point[1], 0}, normal;
    int result;
    if (kind == CollLine_Floor)
        result = mpLib_8004DD90_Floor(id, &vec, delta, flags, &normal);
    else if (kind == CollLine_Ceiling)
        result = mpLib_8004E090_Ceiling(id, &vec, delta, flags, &normal);
    else if (kind == CollLine_LeftWall)
        result = mpLib_8004E398_LeftWall(id, &vec, delta, flags, &normal);
    else
        result = mpLib_8004E684_RightWall(id, &vec, delta, flags, &normal);
    if (result != -1) memcpy(normal_out, &normal, sizeof(normal));
    free_lines(map);
    return result;
}

int oracle_stage_query(const OracleLine* lines, int line_count,
    const OracleJoint* joints, int joint_count, int kind, const float query[5],
    const s32 skip[3], int prechecked, uint64_t accept_mask, OracleContact* out) {
    assert(joint_count >= 0 && joint_count <= 32);
    int capacity = 1;
    for (int i = 0; i < joint_count; ++i) {
        assert(joints[i].id >= 0 && joints[i].id < 64);
        if (joints[i].id >= capacity) capacity = joints[i].id + 1;
    }
    MapLine* map = setup_lines(lines, line_count);
    MapJoint* inners = calloc(capacity, sizeof(*inners));
    groundCollJoint = calloc(capacity, sizeof(*groundCollJoint));
    assert(inners && groundCollJoint);
    jointListStart = joint_count ? &groundCollJoint[joints[0].id] : NULL;
    for (int i = 0; i < joint_count; ++i) {
        int id = joints[i].id;
        const s32* r = joints[i].ranges;
        inners[id] = (MapJoint){r[0],r[1],r[2],r[3],r[6],r[7],r[4],r[5],r[8],r[9]};
        CollJoint* j = &groundCollJoint[id];
        j->inner = &inners[id];
        j->flags = joints[i].flags;
        j->next = i + 1 < joint_count ? &groundCollJoint[joints[i+1].id] : NULL;
        memcpy(&j->bounding_min, joints[i].bounds_min, sizeof(Vec2));
        memcpy(&j->bounding_max, joints[i].bounds_max, sizeof(Vec2));
    }
    didCheckBounding = prechecked != 0;
    accepted = accept_mask;
    active_output = out;
    out->callback_count = 0;
    Vec3 position, normal;
    int result;
    if (kind == CollLine_Floor)
        result = mpCheckFloor(query[0],query[1],query[2],query[3],query[4],
            &position,&out->line_id,&out->flags,&normal,skip[0],skip[1],skip[2],accept_floor,NULL);
    else if (kind == CollLine_Ceiling)
        result = mpCheckCeiling(query[0],query[1],query[2],query[3],
            &position,&out->line_id,&out->flags,&normal,skip[1],skip[2]);
    else if (kind == CollLine_LeftWall)
        result = mpCheckLeftWall(query[0],query[1],query[2],query[3],
            &position,&out->line_id,&out->flags,&normal,skip[1],skip[2]);
    else
        result = mpCheckRightWall(query[0],query[1],query[2],query[3],
            &position,&out->line_id,&out->flags,&normal,skip[1],skip[2]);
    if (result) {
        memcpy(out->position, &position, sizeof(position));
        memcpy(out->normal, &normal, sizeof(normal));
    }
    free_lines(map);
    free(inners);
    free(groundCollJoint);
    groundCollJoint = NULL;
    jointListStart = NULL;
    active_output = NULL;
    return result;
}
