/* Host adapter for the real per-frame ledge-catch query: mpColl_80044164
 * (Collide_LeftLedgeGrab, mpFloorGetLeft) and mpColl_800443C4
 * (Collide_RightLedgeGrab, mpFloorGetRight), mp/mpcoll.c. Both are reached
 * every airborne frame from mpColl_80046904's `!touched_floor &&
 * CollisionFlagAir_CanGrabLedge` branch (mpcoll.c:2516-2547), which
 * ft_80083090_inline/ft_800831CC (ft_081B.c:637-694) drive for ordinary
 * Fall/DamageFall/aerial collision every frame, with `ftCliffCommon_80081298`
 * attaching on a successful grab.
 *
 * The floor-database box search (mpLib_80051BA8_Floor) and the two
 * mpCheckMultiple line-of-sight obstruction checks are stubbed out instead of
 * ported: `game::ledge::scan` already enumerates one known, isolated,
 * disconnected ledge endpoint at a time (an eligible line has no connected
 * neighbor -- see its doc comment), so the real box search could only ever
 * find that same endpoint, with nothing else in the box to obstruct line of
 * sight to it. This adapter forces exactly that scenario: the floor search
 * returns whatever `contact`/`ledge_id` the test supplies, and
 * `mpCheckMultiple` always reports no obstruction. Under that forced
 * scenario the pinned function's output depends only on the box-construction
 * and edge/contact-threshold arithmetic that `fighter::ledge::snap_catch`
 * ports -- exactly the "query's arithmetic" this differential exists to
 * pin, not the full spatial floor search this codebase does not model
 * (see docs/ledges.md's unported-scope paragraph). */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef uint8_t u8;

typedef struct {
    float x, y;
} Vec2;
typedef struct {
    float x, y, z;
} Vec3;
typedef struct {
    Vec2 top, bottom, left, right;
} ftECB;
typedef struct {
    ftECB ecb;
    Vec3 cur_pos, prev_pos;
    Vec3 contact;
    int floor_skip, joint_id_skip, joint_id_only;
    float ledge_snap_x, ledge_snap_y, ledge_snap_height;
} CollData;

static _Thread_local int stub_ledge_id;
static _Thread_local Vec3 stub_contact;
static _Thread_local Vec3 stub_edge;
/* left, bottom, right, top, exactly as passed to the floor search. */
static _Thread_local float stub_box[4];

static bool mpCheckedBounding(void) { return false; }
static void mpBoundingCheck(float left, float bottom, float right, float top) {
    (void) left;
    (void) bottom;
    (void) right;
    (void) top;
}
static void mpUncheckBounding(void) {}
static int mpLib_80051BA8_Floor(Vec3* contact, int floor_skip, int joint_skip,
                                int joint_only, int direction, float left,
                                float bottom, float right, float top) {
    (void) floor_skip;
    (void) joint_skip;
    (void) joint_only;
    (void) direction;
    stub_box[0] = left;
    stub_box[1] = bottom;
    stub_box[2] = right;
    stub_box[3] = top;
    *contact = stub_contact;
    return stub_ledge_id;
}
static void mpFloorGetLeft(int id, Vec3* edge) {
    (void) id;
    *edge = stub_edge;
}
static void mpFloorGetRight(int id, Vec3* edge) {
    (void) id;
    *edge = stub_edge;
}
static bool mpCheckMultiple(float x, float y, float cx, float cy, void* a,
                            int* line_id, void* b, void* c, int mask, int skip,
                            int only) {
    (void) x;
    (void) y;
    (void) cx;
    (void) cy;
    (void) a;
    (void) b;
    (void) c;
    (void) mask;
    (void) skip;
    (void) only;
    if (line_id != NULL) {
        *line_id = 0;
    }
    return false;
}
static int mpJointFromLine(int line_id) { return line_id; }

#include "ledge_snap_original.inc"

/* side: 0 selects mpColl_80044164 (left ledge), 1 selects mpColl_800443C4
 * (right ledge). Packs the pinned function's [bool result | int ledge_id]
 * into a u64 and writes the box it computed (left, bottom, right, top) to
 * box_out, so the Rust differential can check both the box arithmetic and
 * the final catch decision against one call. */
uint64_t oracle_ledge_snap(int side, float cur_x, float cur_y, float prev_x,
                           float prev_y, float ecb_right_x, float ecb_left_x,
                           float ecb_bottom_x, float ecb_bottom_y,
                           float ecb_top_x, float ecb_top_y, float snap_x,
                           float snap_y, float snap_height, int ledge_id,
                           float contact_x, float contact_y, float edge_x,
                           float edge_y, float* box_out) {
    CollData cd = { 0 };
    cd.cur_pos = (Vec3) { cur_x, cur_y, 0.0F };
    cd.prev_pos = (Vec3) { prev_x, prev_y, 0.0F };
    cd.ecb.right.x = ecb_right_x;
    cd.ecb.left.x = ecb_left_x;
    cd.ecb.bottom.x = ecb_bottom_x;
    cd.ecb.bottom.y = ecb_bottom_y;
    cd.ecb.top.x = ecb_top_x;
    cd.ecb.top.y = ecb_top_y;
    cd.ledge_snap_x = snap_x;
    cd.ledge_snap_y = snap_y;
    cd.ledge_snap_height = snap_height;
    cd.floor_skip = -1;
    cd.joint_id_skip = -1;
    cd.joint_id_only = -1;

    stub_ledge_id = ledge_id;
    stub_contact = (Vec3) { contact_x, contact_y, 0.0F };
    stub_edge = (Vec3) { edge_x, edge_y, 0.0F };
    stub_box[0] = stub_box[1] = stub_box[2] = stub_box[3] = 0.0F;

    int out_id = -1;
    bool grabbed = side == 0 ? mpColl_80044164(&cd, &out_id)
                             : mpColl_800443C4(&cd, &out_id);

    box_out[0] = stub_box[0];
    box_out[1] = stub_box[1];
    box_out[2] = stub_box[2];
    box_out[3] = stub_box[3];
    return ((uint64_t) (uint32_t) out_id << 32) | (uint32_t) grabbed;
}
