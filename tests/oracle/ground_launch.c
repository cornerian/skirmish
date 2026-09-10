/* Host oracle for ftCo_8008DCE0's grounded launch branch. */
#include <math.h>
#include <stdint.h>

typedef struct { float x, y, z; } Vec3;
typedef struct { Vec3 normal; } Floor;
typedef struct { Floor floor; } Collision;
typedef struct { int x184c_damaged_hurtbox; } Damage;
typedef struct {
    Collision coll_data;
    Damage dmg;
    float xF0_ground_kb_vel;
    float facing_dir;
} Fighter;
typedef struct {
    float x1E8_radians;
    float x1EC;
} ftCommonData;

static float oracle_vector_len(Vec3* vector)
{
    return sqrtf(vector->x * vector->x + vector->y * vector->y +
                 vector->z * vector->z);
}

#define lbVector_Len oracle_vector_len
#include "ground_launch_original.inc"

#define M_PI_2_F 1.57079632679489661923F
#define ftCommon_8007D5D4(fp) (left_ground = 1)
#define ftCo_Damage_CalcVel(fp, x, y) (out_x = (x), out_y = (y))
#define inlineA0(gobj, fp, angle) ((void) (angle))

static int ftCo_803C5520[2][4][3];

void oracle_ground_launch(const float* knockback, const float* floor_normal,
                          int fly, float bounce_angle,
                          float bounce_multiplier, float* output,
                          int32_t* flags)
{
    Fighter fighter = { 0 };
    Fighter* fp = &fighter;
    ftCommonData common = { bounce_angle, bounce_multiplier };
    ftCommonData* p_ftCommonData = &common;
    Vec3* normal;
    Vec3 pos;
    float floor_angle;
    float temp_f1_3;
    float temp_f2;
    float sp40;
    float out_x = 0;
    float out_y = 0;
    int kb_level = fly ? 3 : 0;
    int msid = 0;
    int var_r27 = 1;
    int left_ground = 0;
    void* gobj = 0;
    float x = -knockback[0];
    float y = knockback[1];
    fighter.coll_data.floor.normal =
        (Vec3) { floor_normal[0], floor_normal[1], 0 };
    fighter.facing_dir = 1;

/* BEGIN VERBATIM GROUNDED LAUNCH */
        normal = &fp->coll_data.floor.normal;
        pos.x = -x * fp->facing_dir;
        pos.y = y;
        pos.z = 0;
        floor_angle = lbVector_Angle(normal, &pos);
        if (!(floor_angle < M_PI_2_F)) {
            goto block_23;
        }
        msid = ftCo_803C5520[0][kb_level][fp->dmg.x184c_damaged_hurtbox];
        ftCommon_8007D5D4(fp);
        ftCo_Damage_CalcVel(fp, pos.x, pos.y);
        fp->xF0_ground_kb_vel = 0;
        goto block_28;
    block_23:
        if (kb_level != 3) {
            goto block_27;
        }
        ftCommon_8007D5D4(fp);
        msid = ftCo_803C5520[0][kb_level][fp->dmg.x184c_damaged_hurtbox];
        if (!(floor_angle > (M_PI_2 + (double) p_ftCommonData->x1E8_radians)))
        {
            goto block_26;
        }
        ftCo_Damage_CalcVel(fp, pos.x, -pos.y * p_ftCommonData->x1EC);
        var_r27 = 0;
        fp->xF0_ground_kb_vel = 0;
        temp_f1_3 = atan2f(-normal->x, normal->y);
        sp40 = temp_f1_3;
        inlineA0(gobj, fp, &sp40);
        goto block_28;
    block_26:
        ftCo_Damage_CalcVel(fp, pos.x, pos.y);
        fp->xF0_ground_kb_vel = 0;
        goto block_28;
    block_27:
        msid = ftCo_803C5520[0][kb_level][fp->dmg.x184c_damaged_hurtbox];
        fp->xF0_ground_kb_vel = pos.x;
        temp_f2 = fp->xF0_ground_kb_vel;
        ftCo_Damage_CalcVel(fp, normal->y * temp_f2, -normal->x * temp_f2);
    block_28:
/* END VERBATIM GROUNDED LAUNCH */
    output[0] = out_x;
    output[1] = out_y;
    output[2] = fp->xF0_ground_kb_vel;
    output[3] = floor_angle;
    flags[0] = left_ground;
    flags[1] = var_r27 == 0;
    (void) msid;
}
