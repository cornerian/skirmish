/* Host adapter for the fired laser item (`itfoxlaser.c`,
 * `itfoxlaser.functions.json`): spawn (`it_8029C504`/`it_8029C6A4`),
 * per-frame motion (`itFoxlaser_UnkMotion1_Anim` -> `Item_UpdateRayAnimation`),
 * the shield bounce (`itFoxLaser_Logic94_ShieldBounced` ->
 * `Item_BounceRayOffShield` -> the real `lbVector_Mirror`, pinned separately
 * in `lbvector.c` and linked directly), and the Reflector hand-off's own
 * item-side callback (`itFoxLaser_Logic94_Reflected` ->
 * `Item_ResetRayAfterReflection`). Also extracts `Item_80269F14`
 * (`item.c`, `item.functions.json`) for the Reflector hand-off's owner-swap
 * and damage-scaling arithmetic (`ftColl_80077464`'s own `max_damage`
 * eligibility gate is a separate verbatim-excerpt check against the
 * already-pinned `combat_knockback.c`, in
 * `tests/reflect_gate_differential.rs`, since a full extraction would drag
 * in `ReflectAttr`/hit-direction bookkeeping this port has no equivalent
 * for).
 *
 * Dependency treatment:
 *  - FAITHFUL, hand-duplicated verbatim against the pinned
 *    `it_kinds_inlines.h` snapshot (all `static inline`, no `.c` file of
 *    their own to extract from -- checked by
 *    `tests/fox_laser_differential.rs`'s own
 *    `adapter_statements_are_verbatim_in_the_pinned_sources` test):
 *    `Item_InitRaySpawnPosition`, `Item_InitRaySpawnFields`,
 *    `Item_UpdateRayAnimation`, `Item_BounceRayOffShield`,
 *    `Item_ResetRayAfterReflection`.
 *  - REAL, extracted/linked directly: `it_8026BB68` (`it_26b1.c`),
 *    `ftLib_80086990` (`ftlib.c`), `lbVector_Mirror` (`lbvector.c`,
 *    already pinned whole-file by an existing adapter alias).
 *  - CAPTURED (logged, not implemented): `it_80275158` (records the
 *    lifetime argument), `Item_80268B18` (the generic item allocator --
 *    returns a fixed thread-local `Item`, with `item->pos` seeded from
 *    `spawn->pos` matching this port's own confirmed understanding of the
 *    generic allocator's behavior, see `docs/fox-neutral-special.md`'s
 *    "Spawn position" section; nothing in the pinned decomp exposes this
 *    allocator's own source).
 *  - Ghost/GFX no-ops: `HSD_JObjSetRotationX/Y`, `HSD_JObjSetScaleZ`,
 *    `it_80273130`, `Item_80268E5C`, `db_80225DD8`.
 *
 * Thread-local state isolates independent concurrent test calls, matching
 * every other adapter in this directory. */
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif
#ifndef M_TAU
#define M_TAU (M_PI * 2.0)
#endif
#ifndef M_PI_2
#define M_PI_2 (M_PI / 2.0)
#endif
#define ABS(x) ((x) < 0 ? -(x) : (x))
#define PAD_STACK(n)

typedef uint8_t u8;
typedef uint16_t u16;
typedef uint32_t u32;
typedef int16_t s16;
typedef int32_t s32;
typedef float f32;
typedef double f64;
typedef s32 enum_t;
typedef int ItemKind;

typedef struct {
    float x, y, z;
} Vec3;
typedef struct {
    float x, y;
} Vec2;
typedef struct {
    s32 x, y;
} S32Vec2;

typedef struct HSD_JObj_s HSD_JObj;
struct HSD_JObj_s {
    int unused;
};

/* A single generic game-object type, matching the real engine's own
 * architecture (`HSD_GObj`/`Fighter_GObj`/`Item_GObj` are all the same
 * underlying object with a generic `user_data`, cast per domain by
 * `GET_FIGHTER`/`GET_ITEM`/`HSD_GObjGetUserData` -- `Item_80269F14`'s own
 * pinned signature is `Item_80269F14(HSD_GObj* gobj)` even though it always
 * receives an item's own object, exactly this genericity). */
typedef struct GObj_s {
    void* user_data;
} GObj;
typedef GObj HSD_GObj;
typedef GObj Item_GObj;
#define HSD_GObjGetUserData(gobj) (((GObj*) (gobj))->user_data)

/* ---- Fighter side (the laser's own "parent"): only the two fields
 * `ftLib_80086990` itself reads/writes. ---- */
typedef struct {
    struct {
        struct {
            float y;
        } top, bottom;
    } ecb;
} CollData;
typedef struct Fighter {
    CollData coll_data;
    Vec3 cur_pos;
} Fighter;
#define GET_FIGHTER(gobj) ((Fighter*) HSD_GObjGetUserData(gobj))

/* ---- Item side. ---- */
typedef struct { float scale, angle, speed; Vec3 pos; } RayVars;
typedef union { RayVars foxlaser; RayVars ray; } ItemVar;

typedef struct { float lifetime, scale; } FoxLaserAttr;
typedef struct { void* x0_common_attr; FoxLaserAttr* x4_specialAttributes; } Article;

typedef enum { HitCapsule_Disabled = 0, HitCapsule_Active = 1 } HitCapsuleState;
typedef struct { HitCapsuleState state; float damage; } HitCapsule;
typedef struct { HitCapsule hit; } ItemHitbox;
typedef struct { bool (*reflected)(HSD_GObj*); } ItemLogicTable;
typedef union { int x2070_int; } Struct2070;

typedef struct Item {
    Vec3 pos;
    float facing_dir;
    ItemKind kind;
    Article* xC4_article_data;
    Vec3 x40_vel;
    float xC54;
    Vec3 xC58;
    HSD_GObj* xC64_reflectGObj;
    float xC68;
    float xC6C;
    s32 xC74;
    Vec2 xC78;
    S32Vec2 xC80;
    s32 xC88;
    u16 xC8C;
    HSD_GObj* owner;
    u8 x20_team_id;
    struct {
        u8 b0 : 1;
        u8 b1 : 1;
        u8 b2 : 1;
        u8 b3 : 1;
        u8 b4567 : 4;
    } xDCC_flag;
    Struct2070 xD90;
    Vec2 xD94;
    S32Vec2 xD9C;
    u32 xDA4_word;
    u16 xDA8_short;
    ItemLogicTable* xB8_itemLogicTable;
    ItemHitbox x5D4_hitboxes[4];
    s32 destroy_type;
    ItemVar xDD4_itemVar;
} Item;
#define GET_ITEM(gobj) ((Item*) HSD_GObjGetUserData(gobj))

/* `melee/it/types.h:689-704`, only the fields this adapter's own extracted
 * functions read/write. */
typedef struct {
    HSD_GObj* x0_parent_gobj;
    HSD_GObj* x4_parent_gobj2;
    ItemKind kind;
    Vec3 pos;
    Vec3 prev_pos;
    Vec3 vel;
    float facing_dir;
    s16 x3C_damage;
    s32 x40;
    struct {
        u8 b0 : 1;
    } x44_flag;
} SpawnItem;

#define ITEM_ANIM_UPDATE 0

/* ---- CAPTURED: logged, not reimplemented. ---- */
static _Thread_local Item laser_item_storage;
static _Thread_local Item_GObj laser_item_gobj_storage;
static _Thread_local Article laser_article_storage;
static _Thread_local FoxLaserAttr laser_attr_storage;
static _Thread_local float captured_lifetime;
static _Thread_local int it_80275158_calls;

static void reset_laser_item(float lifetime_attr) {
    memset(&laser_item_storage, 0, sizeof(laser_item_storage));
    laser_attr_storage.lifetime = lifetime_attr;
    laser_attr_storage.scale = 3.0f;
    laser_article_storage.x4_specialAttributes = &laser_attr_storage;
    laser_item_storage.xC4_article_data = &laser_article_storage;
    laser_item_gobj_storage.user_data = &laser_item_storage;
    captured_lifetime = 0.0f;
    it_80275158_calls = 0;
}

/* The generic item allocator: not itself in the pinned decomp (`item.c`'s
 * own `Item_80268B18` is the real, much larger, general-purpose entry point
 * every item kind shares -- allocation, GObj wiring, default state, none of
 * it Blaster-specific). `item->pos = spawn->pos` matches this port's own
 * confirmed understanding of that allocator's behavior (see this file's
 * header note and `docs/fox-neutral-special.md`'s "Spawn position"
 * section), not a guess. */
static Item_GObj* Item_80268B18(SpawnItem* spawn) {
    laser_item_storage.pos = spawn->pos;
    laser_item_storage.facing_dir = spawn->facing_dir;
    laser_item_storage.kind = spawn->kind;
    return &laser_item_gobj_storage;
}
static void Item_80268E5C(Item_GObj* gobj, enum_t msid, int flag) {
    (void) gobj;
    (void) msid;
    (void) flag;
}
static void it_80275158(Item_GObj* gobj, float lifetime) {
    (void) gobj;
    it_80275158_calls++;
    captured_lifetime = lifetime;
}
static void db_80225DD8(Item_GObj* gobj, HSD_GObj* owner) {
    (void) gobj;
    (void) owner;
}

/* ---- Ghost/GFX no-ops. ---- */
static void HSD_JObjSetRotationY(HSD_JObj* jobj, float radians) {
    (void) jobj;
    (void) radians;
}
static void HSD_JObjSetRotationX(HSD_JObj* jobj, float radians) {
    (void) jobj;
    (void) radians;
}
static void HSD_JObjSetScaleZ(HSD_JObj* jobj, float scale) {
    (void) jobj;
    (void) scale;
}
static bool it_80273130(Item_GObj* gobj) {
    (void) gobj;
    return true;
}
static _Thread_local HSD_JObj laser_jobj_storage;
#define GET_JOBJ(gobj) ((void) (gobj), &laser_jobj_storage)

/* ---- FAITHFUL, hand-duplicated verbatim: `melee/it/kinds/inlines.h`
 * (`static inline`, no `.c` file of their own to extract from). Checked
 * against the pinned `tests/oracle/original/it_kinds_inlines.h` snapshot. */
/* BEGIN VERBATIM INIT RAY SPAWN FIELDS */
static inline void Item_InitRaySpawnFields(SpawnItem* spawn, HSD_GObj* parent,
                                           f32 facing_dir)
{
    spawn->facing_dir = facing_dir;
    spawn->x3C_damage = 0;
    spawn->vel.x = spawn->vel.y = spawn->vel.z = 0.0F;
    spawn->x0_parent_gobj = parent;
    spawn->x4_parent_gobj2 = spawn->x0_parent_gobj;
    spawn->x44_flag.b0 = true;
    spawn->x40 = 0;
}
/* END VERBATIM INIT RAY SPAWN FIELDS */

/* The real `ftLib_80086990` (`ftlib.c`, extracted via `ftlib.functions.json`
 * = ["vector_add", "ftLib_80086990"]) and `it_8026BB68` (`it_26b1.c`,
 * `it_26b1.functions.json`), linked directly rather than stubbed: the
 * spawn position comparison below depends on this exact arithmetic. */
#include "ftlib_original.inc"
#include "it_26b1_original.inc"

/* BEGIN VERBATIM INIT RAY SPAWN POSITION */
static inline void Item_InitRaySpawnPosition(SpawnItem* spawn,
                                             HSD_GObj* parent, Vec3* pos)
{
    spawn->prev_pos = *pos;
    spawn->prev_pos.z = 0.0F;
    it_8026BB68(parent, &spawn->pos);
}
/* END VERBATIM INIT RAY SPAWN POSITION */

/* BEGIN VERBATIM UPDATE RAY ANIMATION */
static inline bool Item_UpdateRayAnimation(Item_GObj* gobj, Item* ip,
                                           HSD_JObj* jobj,
                                           const f32* max_scale,
                                           f32 scale_divisor)
{
    f32 dir;
    f32 vel_x;

    ip->x40_vel.x =
        ip->xDD4_itemVar.ray.speed * cosf(ip->xDD4_itemVar.ray.angle);
    ip->x40_vel.y =
        ip->xDD4_itemVar.ray.speed * sinf(ip->xDD4_itemVar.ray.angle);
    ip->x40_vel.z = 0.0F;
    if (ip->x40_vel.x > 0.0F) {
        dir = +1.0F;
    } else {
        dir = -1.0F;
    }
    ip->facing_dir = dir;
    HSD_JObjSetRotationY(jobj, M_PI_2 * ip->facing_dir);
    if (ip->facing_dir == 1.0F) {
        vel_x = -ip->x40_vel.x;
    } else {
        vel_x = +ip->x40_vel.x;
    }
    HSD_JObjSetRotationX(jobj, M_PI + atan2f(ip->x40_vel.y, vel_x));
    ip->xDD4_itemVar.ray.scale +=
        ABS(ip->xDD4_itemVar.ray.speed) / scale_divisor;
    if (ip->xDD4_itemVar.ray.scale > *max_scale) {
        ip->xDD4_itemVar.ray.scale = *max_scale;
    }
    if (ip->xDD4_itemVar.ray.scale < 1e-5F) {
        ip->xDD4_itemVar.ray.scale = 1e-3F;
    }
    HSD_JObjSetScaleZ(jobj, ip->xDD4_itemVar.ray.scale);
    return it_80273130(gobj);
}
/* END VERBATIM UPDATE RAY ANIMATION */

void lbVector_Mirror(Vec3* a, Vec3* unit_mirror_axis);

/* BEGIN VERBATIM BOUNCE RAY OFF SHIELD */
static inline bool Item_BounceRayOffShield(Item_GObj* gobj)
{
    Item* ip = GET_ITEM(gobj);

    lbVector_Mirror(&ip->x40_vel, &ip->xC58);
    ip->xDD4_itemVar.ray.scale = 1e-3F;
    ip->xDD4_itemVar.ray.angle = atan2f(ip->x40_vel.y, ip->x40_vel.x);
    while (ip->xDD4_itemVar.ray.angle < 0.0F) {
        ip->xDD4_itemVar.ray.angle += M_TAU;
    }
    while (ip->xDD4_itemVar.ray.angle > M_TAU) {
        ip->xDD4_itemVar.ray.angle -= M_TAU;
    }
    return false;
}
/* END VERBATIM BOUNCE RAY OFF SHIELD */

/* BEGIN VERBATIM RESET RAY AFTER REFLECTION */
static inline void Item_ResetRayAfterReflection(Item* ip, HSD_JObj* jobj)
{
    HSD_JObjSetScaleZ(jobj, ip->xDD4_itemVar.ray.scale = 1e-3F);
    ip->xDD4_itemVar.ray.angle += M_PI;
    while (ip->xDD4_itemVar.ray.angle < 0.0F) {
        ip->xDD4_itemVar.ray.angle += M_TAU;
    }
    while (ip->xDD4_itemVar.ray.angle > M_TAU) {
        ip->xDD4_itemVar.ray.angle -= M_TAU;
    }
}
/* END VERBATIM RESET RAY AFTER REFLECTION */

/* Forward declarations for every extracted `itfoxlaser.c` function. */
static inline void normalizeAngle(f32* angle);
void it_8029C504(HSD_GObj* parent, Vec3* pos, enum_t msid, int kind, f32 angle, f32 speed);
void it_8029C6A4(f32 angle, f32 vel, HSD_GObj* parent, Vec3* vec, int kind);
bool itFoxlaser_UnkMotion1_Anim(Item_GObj* item_gobj);
void itFoxlaser_UnkMotion1_Phys(Item_GObj* item_gobj);
bool itFoxLaser_Logic94_Reflected(Item_GObj* item_gobj);
bool itFoxLaser_Logic94_ShieldBounced(Item_GObj* item_gobj);

#include "itfoxlaser_original.inc"

/* `Item_80269F14` (`item.c`): the Reflector hand-off's own owner-swap and
 * damage-scaling excerpt. Extracted for real (not a verbatim excerpt like
 * the `max_damage` gate) since its own struct dependencies are small enough
 * to model directly. */
static bool Item_80269F14(HSD_GObj* gobj);
static _Thread_local int it_80272460_calls;
static _Thread_local u32 captured_scaled_damage;
static void it_80272460(HitCapsule* hitbox, u32 damage, Item_GObj* item_gobj) {
    (void) hitbox;
    (void) item_gobj;
    it_80272460_calls++;
    captured_scaled_damage = damage;
}
static s32 ftLib_80086EB4(HSD_GObj* gobj) {
    (void) gobj;
    return 0;
}
static void Item_8026A8EC(HSD_GObj* gobj) {
    (void) gobj;
}
enum { It_Kind_M_Ball = -1 };

/* `item.c`'s own runtime item-common-data global (`ItemCommonData*
 * it_804D6D28`): this pinned decomp snapshot never initializes it (no
 * reader for its real runtime value exists in source), so `xD8` -- the
 * scaled-reflect-damage cap `Item_80269F14` reads -- is test-controlled
 * rather than a claimed real value (see `oracle_reflect_owner_and_damage`
 * below and this file's header note). */
typedef struct {
    u32 xD8;
} ItemCommonData;
static _Thread_local ItemCommonData itemCommonData_;
#define it_804D6D28 (&itemCommonData_)

#include "item_original.inc"

/* ------------------------------------------------------------------- */
/* Oracle entry points. */

static void reset_fighter(Fighter* fp, f32 pos_x, f32 pos_y, f32 ecb_top, f32 ecb_bottom) {
    memset(fp, 0, sizeof(*fp));
    fp->cur_pos.x = pos_x;
    fp->cur_pos.y = pos_y;
    fp->coll_data.ecb.top.y = ecb_top;
    fp->coll_data.ecb.bottom.y = ecb_bottom;
}

/* `it_8029C6A4`: spawn position (`ftLib_80086990`'s own ECB-midpoint
 * formula, through `Item_InitRaySpawnPosition`), angle normalization, and
 * the item's own initial `ray` fields. */
void oracle_laser_spawn(f32 owner_x, f32 owner_y, f32 ecb_top, f32 ecb_bottom, f32 angle_in,
                         f32 speed_in, s32 kind_in, f32 lifetime_attr, f32* out_pos_x,
                         f32* out_pos_y, f32* out_angle, f32* out_speed, f32* out_facing_dir,
                         f32* out_lifetime) {
    Fighter parent_fp;
    reset_fighter(&parent_fp, owner_x, owner_y, ecb_top, ecb_bottom);
    HSD_GObj parent_gobj = { &parent_fp };
    reset_laser_item(lifetime_attr);
    Vec3 hold_joint_pos = { 0.0f, 0.0f, 0.0f };
    it_8029C6A4(angle_in, speed_in, &parent_gobj, &hold_joint_pos, kind_in);
    *out_pos_x = laser_item_storage.pos.x;
    *out_pos_y = laser_item_storage.pos.y;
    *out_angle = laser_item_storage.xDD4_itemVar.ray.angle;
    *out_speed = laser_item_storage.xDD4_itemVar.ray.speed;
    *out_facing_dir = laser_item_storage.facing_dir;
    *out_lifetime = captured_lifetime;
}

/* `itFoxlaser_UnkMotion1_Anim` -> `Item_UpdateRayAnimation`: per-frame
 * velocity recompute and facing. */
void oracle_laser_motion(f32 speed_in, f32 angle_in, f32* out_vel_x, f32* out_vel_y,
                          f32* out_facing_dir) {
    reset_laser_item(35.0f);
    laser_item_storage.xDD4_itemVar.ray.speed = speed_in;
    laser_item_storage.xDD4_itemVar.ray.angle = angle_in;
    itFoxlaser_UnkMotion1_Anim(&laser_item_gobj_storage);
    *out_vel_x = laser_item_storage.x40_vel.x;
    *out_vel_y = laser_item_storage.x40_vel.y;
    *out_facing_dir = laser_item_storage.facing_dir;
}

/* `itFoxLaser_Logic94_ShieldBounced` -> `Item_BounceRayOffShield` -> the
 * real `lbVector_Mirror`. */
void oracle_laser_shield_bounce(f32 vel_x_in, f32 vel_y_in, f32 normal_x_in, f32 normal_y_in,
                                 f32* out_vel_x, f32* out_vel_y, f32* out_angle) {
    reset_laser_item(35.0f);
    laser_item_storage.x40_vel.x = vel_x_in;
    laser_item_storage.x40_vel.y = vel_y_in;
    laser_item_storage.xC58.x = normal_x_in;
    laser_item_storage.xC58.y = normal_y_in;
    itFoxLaser_Logic94_ShieldBounced(&laser_item_gobj_storage);
    *out_vel_x = laser_item_storage.x40_vel.x;
    *out_vel_y = laser_item_storage.x40_vel.y;
    *out_angle = laser_item_storage.xDD4_itemVar.ray.angle;
}

/* `itFoxLaser_Logic94_Reflected`: facing snap to the reflector's own
 * `xC68` direction, plus `Item_ResetRayAfterReflection`'s `angle += pi`. */
void oracle_laser_reflected(f32 facing_dir_in, f32 xc68_in, f32 angle_in, f32* out_facing_dir,
                             f32* out_angle) {
    reset_laser_item(35.0f);
    laser_item_storage.facing_dir = facing_dir_in;
    laser_item_storage.xC68 = xc68_in;
    laser_item_storage.xDD4_itemVar.ray.angle = angle_in;
    itFoxLaser_Logic94_Reflected(&laser_item_gobj_storage);
    *out_facing_dir = laser_item_storage.facing_dir;
    *out_angle = laser_item_storage.xDD4_itemVar.ray.angle;
}

/* `Item_80269F14`: the owner swap and the damage-scaling loop (`hit.damage
 * * xC6C + 0.99f`, truncated, capped at `it_804D6D28->xD8`). A single
 * hitbox (index 0) is populated; the other three stay `HitCapsule_Disabled`
 * and are skipped, matching the loop's own per-hitbox gate. */
void oracle_reflect_owner_and_damage(f32 hitbox_damage_in, f32 damage_mul_in, u32 damage_cap_in,
                                      int* out_owner_swapped, int* out_reflect_called,
                                      u32* out_scaled_damage) {
    Item item;
    memset(&item, 0, sizeof(item));
    HSD_GObj reflector_sentinel;
    reflector_sentinel.user_data = NULL;
    item.xC64_reflectGObj = &reflector_sentinel;
    item.owner = NULL;
    item.kind = 54; /* It_Kind_Fox_Laser, distinct from It_Kind_M_Ball (-1) */
    item.xC6C = damage_mul_in;
    ItemLogicTable table;
    table.reflected = itFoxLaser_Logic94_Reflected;
    item.xB8_itemLogicTable = &table;
    item.x5D4_hitboxes[0].hit.state = HitCapsule_Active;
    item.x5D4_hitboxes[0].hit.damage = hitbox_damage_in;
    item.x5D4_hitboxes[1].hit.state = HitCapsule_Disabled;
    item.x5D4_hitboxes[2].hit.state = HitCapsule_Disabled;
    item.x5D4_hitboxes[3].hit.state = HitCapsule_Disabled;
    Item_GObj gobj = { &item };
    it_80272460_calls = 0;
    captured_scaled_damage = 0;
    /* Test-controlled, not the real game's own runtime value (see this
     * file's header note): exercised both far above (effectively
     * disabled) and below the scaled value to prove the excerpted
     * truncation/cap formula. */
    itemCommonData_.xD8 = damage_cap_in;
    Item_80269F14(&gobj);
    *out_owner_swapped = item.owner == &reflector_sentinel;
    *out_reflect_called = it_80272460_calls;
    *out_scaled_damage = captured_scaled_damage;
}
