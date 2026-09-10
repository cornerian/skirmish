#include <math.h>

typedef unsigned int u32;
typedef int s32;
typedef struct {
    float x, y, z;
} Vec3;
typedef struct {
    u32 kb_angle;
} HitCapsule;
typedef struct {
    Vec3 a_pos;
    Vec3 b_pos;
} HurtCapsule;
typedef struct {
    HurtCapsule capsule;
} FighterHurtCapsule;
typedef struct {
    HitCapsule* hit0;
    FighterHurtCapsule* hurt1;
    Vec3 pos;
} HitEntry;

#define MTXRadToDeg(a) ((a) * 57.29577951f)

void oracle_positional_launch(float ax, float ay, float bx, float by,
                              float contact_x, float contact_y, float* out_dir,
                              s32* out_angle)
{
    HitCapsule hit = { .kb_angle = 0x16A };
    FighterHurtCapsule hurt = {
        .capsule = {
            .a_pos = { .x = ax, .y = ay },
            .b_pos = { .x = bx, .y = by },
        },
    };
    HitEntry entry = {
        .hit0 = &hit,
        .hurt1 = &hurt,
        .pos = { .x = contact_x, .y = contact_y },
    };
    HitEntry* best_entry = &entry;
    float dir = 0.0F;
    float angle = 0.0F;
    s32 angle_int = 0;

/* BEGIN VERBATIM POSITIONAL LAUNCH */
    if ((u32) best_entry->hit0->kb_angle == 0x16A) {
        FighterHurtCapsule* hurt = best_entry->hurt1;
        float dx, dy, abs_dx;

        dx = 0.5F * (hurt->capsule.a_pos.x + hurt->capsule.b_pos.x) -
             best_entry->pos.x;
        dy = 0.5F * (hurt->capsule.a_pos.y + hurt->capsule.b_pos.y) -
             best_entry->pos.y;

        dir = (dx < 0.0F) ? 1.0F : -1.0F;

        if (dx < 0.0F) {
            abs_dx = -dx;
        } else {
            abs_dx = dx;
        }

        if (abs_dx < 1e-5F) {
            angle_int = 0;
        } else {
            angle_int = (s32) MTXRadToDeg(atanf(dy / abs_dx));
        }
        angle = (float) angle_int;
    }
/* END VERBATIM POSITIONAL LAUNCH */
    *out_dir = dir;
    *out_angle = (s32) angle;
}
