/* Original action-instance bodies with the referenced fighter layout. The
 * adapter fixes unrelated metadata inputs to the ordinary non-Luigi branch. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef uint8_t u8;
typedef uint16_t u16;
typedef uint32_t u32;
typedef int32_t s32;
typedef float f32;
typedef struct { f32 x, y; } Vec2;
typedef struct { s32 x, y; } S32Vec2;
union Struct2070 {
    struct { u8 x2070, x2071, x2072, x2073; };
    int x2070_int;
};
struct Struct2074 {
    Vec2 x2074_vec;
    S32Vec2 x207C;
    int x2084_b0, x2084_b1, x2084_b2, x2084_b3, x2084_b4, x2084_b5;
    int x2084_b6, x2084_b7, x2085_b0, x2085_b1, x2085_b2, x2085_b3;
    int x2085_b4;
    u16 x2088;
};
typedef struct Fighter {
    int kind;
    void* item_gobj;
    union Struct2070 x2070;
    struct Struct2074 x2074;
    struct { f32 x1830_percent; } dmg;
    int is_metal;
    void* x197C;
    int x221D_b6, x2226_b4, x2220_b5, x2220_b6;
} Fighter;
typedef struct Fighter_GObj { Fighter* user_data; } Fighter_GObj;
#define GET_FIGHTER(gobj) ((gobj)->user_data)
#define FTKIND_LUIGI 17

_Thread_local u16 unk_804D6480;
static void ft_80089768(Vec2* value) { (void) value; }
static void ft_80089460(Fighter* fighter) { (void) fighter; }
static int it_8026B6C8(void* item) { (void) item; return 0; }
static void pl_80037C60(Fighter_GObj* gobj, int value) {
    (void) gobj;
    (void) value;
}
#include "attack_counter_original.inc"
#include "action_instance_original.inc"

void oracle_action_transition(uint16_t state[3], uint8_t identity) {
    Fighter fighter = {.kind=0, .x2074={.x2088=state[1]}};
    fighter.x2070.x2073 = state[0];
    union Struct2070 flags = {.x2073=identity};
    unk_804D6480 = state[2];
    ft_800895E0(&fighter, flags.x2070_int);
    state[0] = fighter.x2070.x2073;
    state[1] = fighter.x2074.x2088;
    state[2] = unk_804D6480;
}

void oracle_action_restart(uint16_t state[2]) {
    Fighter fighter = {.x2074={.x2088=state[0]}};
    Fighter_GObj gobj = {&fighter};
    unk_804D6480 = state[1];
    ft_80089824(&gobj);
    state[0] = fighter.x2074.x2088;
    state[1] = unk_804D6480;
}
