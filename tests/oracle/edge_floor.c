/* Host adapter for the pinned floor-end callbacks `mpColl_8004A45C_Floor`
 * (mode 2, always clamp) and `mpColl_8004A678_Floor` (mode 1, teeter),
 * extracted from the same `mpcoll.c` snapshot the ordinary ECB adapter
 * (`tests/oracle/mpcoll.c`) already pins, under the `edge_floor` alias
 * (`tests/oracle/adapters.json`). Every callee is fully scripted: line
 * validity (`mpLib_80054ED8`/`mpLineGetKind`) always affirms a floor line,
 * `mpFloorGetLeft`/`mpFloorGetRight` answer the supplied edge point
 * directly, `mpLib_8004DD90_Floor` always reports no continuation (floor_id
 * -1, no Y adjustment) -- the only configuration Skirmish's own
 * project_floor-failure precondition reaches, since a real continuation
 * would have already been walked and succeeded before this code runs -- and
 * `mpCheckLeftWall`/`mpCheckRightWall` answer a single scripted bit for
 * whichever side is actually consulted. */
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef uint32_t u32;
typedef struct {
    float x, y;
} Vec2;
typedef struct {
    float x, y, z;
} Vec3;

enum { CollLine_Floor = 2 };
enum {
    Collide_LeftEdge = 1 << 0,
    Collide_RightEdge = 1 << 1,
    Collide_Edge = 1 << 2,
    /* Pre-existing wall-contact flags mpColl_8004A678_Floor also checks.
     * Always unset here: Skirmish's own wall check (`wall_blocked`, folded
     * into the scripted `mpCheckLeftWall`/`mpCheckRightWall` answer) is the
     * only wall gate this oracle and its Rust mirror model. */
    Collide_LeftWallMask = 1 << 3,
    Collide_RightWallMask = 1 << 4,
};

typedef struct {
    Vec2 top, bottom, left, right;
} CollEcb;
typedef struct {
    int index;
    u32 flags;
    Vec3 normal;
} CollFloor;

typedef struct {
    Vec3 cur_pos;
    CollEcb ecb;
    u32 env_flags;
    CollFloor floor;
    int joint_id_skip, joint_id_only;
    int facing_dir;
    float lstick_x;
} CollData;

static _Thread_local Vec3 scripted_edge_left;
static _Thread_local Vec3 scripted_edge_right;
static _Thread_local bool scripted_wall_blocked;

static bool mpLib_80054ED8(int line_id)
{
    (void) line_id;
    return true;
}
static int mpLineGetKind(int line_id)
{
    (void) line_id;
    return CollLine_Floor;
}
static void mpFloorGetLeft(int line_id, Vec3 *out)
{
    (void) line_id;
    *out = scripted_edge_left;
}
static void mpFloorGetRight(int line_id, Vec3 *out)
{
    (void) line_id;
    *out = scripted_edge_right;
}
static int mpLib_8004DD90_Floor(int line_id, Vec3 *edge, float *y, u32 *flags, Vec3 *normal)
{
    (void) line_id;
    (void) edge;
    *y = 0.0F;
    *flags = 0;
    *normal = (Vec3) { 0.0F, 1.0F, 0.0F };
    return -1;
}
static bool mpCheckLeftWall(float edge_x, float edge_y, float right_x, float right_y,
                            void *a, void *b, void *c, void *d, int joint_id_skip,
                            int joint_id_only)
{
    (void) edge_x;
    (void) edge_y;
    (void) right_x;
    (void) right_y;
    (void) a;
    (void) b;
    (void) c;
    (void) d;
    (void) joint_id_skip;
    (void) joint_id_only;
    return scripted_wall_blocked;
}
static bool mpCheckRightWall(float edge_x, float edge_y, float left_x, float left_y,
                             void *a, void *b, void *c, void *d, int joint_id_skip,
                             int joint_id_only)
{
    (void) edge_x;
    (void) edge_y;
    (void) left_x;
    (void) left_y;
    (void) a;
    (void) b;
    (void) c;
    (void) d;
    (void) joint_id_skip;
    (void) joint_id_only;
    return scripted_wall_blocked;
}

#include "edge_floor_original.inc"

/* mode: 1 selects `mpColl_8004A678_Floor` (teeter), 2 selects
 * `mpColl_8004A45C_Floor` (clamp). `ecb_*` are the fighter's ECB
 * bottom/left/right offsets from `cur_pos` (matching Skirmish's
 * `f.ecb.current.{bottom,left,right}`). Returns whether the callback
 * reports on_edge; writes the resulting `cur_pos.{x,y}` and `env_flags`. */
int oracle_edge_floor(int mode, float cur_x, float cur_y, float cur_z, float ecb_bottom_x,
                      float ecb_bottom_y, float ecb_left_x, float ecb_left_y, float ecb_right_x,
                      float ecb_right_y, float edge_left_x, float edge_left_y, float edge_right_x,
                      float edge_right_y, int facing_dir, float lstick_x, int wall_blocked,
                      float *out_pos, u32 *out_flags)
{
    scripted_edge_left = (Vec3) { edge_left_x, edge_left_y, 0.0F };
    scripted_edge_right = (Vec3) { edge_right_x, edge_right_y, 0.0F };
    scripted_wall_blocked = wall_blocked != 0;
    CollData cd = { 0 };
    cd.cur_pos = (Vec3) { cur_x, cur_y, cur_z };
    cd.ecb.bottom = (Vec2) { ecb_bottom_x, ecb_bottom_y };
    cd.ecb.left = (Vec2) { ecb_left_x, ecb_left_y };
    cd.ecb.right = (Vec2) { ecb_right_x, ecb_right_y };
    cd.facing_dir = facing_dir;
    cd.lstick_x = lstick_x;
    bool on_edge;
    if (mode == 1) {
        on_edge = mpColl_8004A678_Floor(&cd, 0);
    } else if (mode == 2) {
        on_edge = mpColl_8004A45C_Floor(&cd, 0);
    } else {
        abort();
    }
    out_pos[0] = cd.cur_pos.x;
    out_pos[1] = cd.cur_pos.y;
    *out_flags = cd.env_flags;
    return on_edge;
}
