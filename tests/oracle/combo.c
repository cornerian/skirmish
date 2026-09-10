/* Original ftcoll function bodies with only their referenced fighter/common
 * layout. Pointer identities replace GameCube GObjs without changing branches. */
#include <stddef.h>
#include <stdint.h>
typedef uint16_t u16;
typedef uint32_t u32;
typedef int32_t enum_t;
typedef float f32;
typedef struct { f32 x, y, z; } Vec3;
typedef struct Fighter Fighter;
typedef struct Fighter_GObj { Fighter* user_data; } Fighter_GObj;
struct Fighter {
    enum_t x208C;
    u16 x2090;
    u16 x2092;
    Fighter_GObj* x2094;
    u16 x2098;
    int x221C_b6;
    Fighter_GObj* victim_gobj;
    int ground_or_air;
    struct { struct { Vec3 normal; } floor; } coll_data;
    f32 facing_dir;
    Vec3 cur_pos;
};
struct CommonData {
    int x4C4;
    int x4C8;
    int x4CC;
    f32 x4D0;
    f32 x4D4;
    u32 x4D8;
};
static _Thread_local struct CommonData common_data;
#define p_ftCommonData (&common_data)
#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define GA_Ground 0
#include "combo_original.inc"

void oracle_combo_record(uint16_t state[4], uint16_t attack_id,
                         int32_t push_count, uint32_t push_frames) {
    Fighter source = {.x208C=state[0], .x2090=state[1], .x2092=state[2]};
    Fighter target = {0}, other = {0};
    Fighter_GObj source_gobj = {&source}, target_gobj = {&target}, other_gobj = {&other};
    source.x2094 = state[3] == 1 ? &target_gobj : state[3] == 2 ? &other_gobj : NULL;
    common_data.x4C4 = push_count;
    common_data.x4D8 = push_frames;
    ftColl_800763C0(&source_gobj, &target_gobj, attack_id);
    state[0] = source.x208C;
    state[1] = source.x2090;
    state[2] = source.x2092;
    state[3] = source.x2094 == NULL ? 0 : source.x2094 == &target_gobj ? 1 : 2;
}

void oracle_combo_update(uint16_t state[2], int victim_hitstun,
                         uint16_t victim_escape) {
    Fighter source = {.x2098=state[0]}, target = {
        .x2098=victim_escape, .x221C_b6=victim_hitstun,
    };
    Fighter_GObj source_gobj = {&source}, target_gobj = {&target};
    source.x2094 = state[1] ? &target_gobj : NULL;
    ftColl_800764DC(&source_gobj);
    state[0] = source.x2098;
    state[1] = source.x2094 != NULL;
}

void oracle_combo_push(uint16_t state[2], float position[2], float facing,
                       const float normal[2], int grounded, int holding,
                       int32_t strong_count, const float distances[2]) {
    Fighter source = {
        .x2090=state[0], .x2092=state[1],
        .ground_or_air=grounded ? GA_Ground : 1,
        .facing_dir=facing,
        .cur_pos={position[0], position[1], 0},
    }, target = {0};
    Fighter_GObj source_gobj = {&source}, target_gobj = {&target};
    source.victim_gobj = holding ? &target_gobj : NULL;
    source.coll_data.floor.normal = (Vec3){normal[0], normal[1], 0};
    common_data.x4C8 = strong_count;
    common_data.x4D0 = distances[0];
    common_data.x4D4 = distances[1];
    ftColl_80076528(&source_gobj);
    state[0] = source.x2090;
    state[1] = source.x2092;
    position[0] = source.cur_pos.x;
    position[1] = source.cur_pos.y;
}
