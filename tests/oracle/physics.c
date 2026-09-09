/* Host adapter for verbatim ftcommon.c functions listed in physics.functions.json.
 * The compact Fighter/HSD_GObj structs contain only fields those functions use;
 * there are no replacements for external gameplay functions. They do not claim
 * GameCube ABI compatibility. Source function bodies are extracted unchanged.
 */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct { float x, y; } Vec2;
typedef struct { float x, y, z; } Vec3;
typedef struct {
    float ground_max_horizontal_velocity;
    float air_max_horizontal_velocity;
    float air_drift_stick_mul;
    float aerial_drift_base;
    float air_drift_max;
    float aerial_friction;
    float gravity;
    float terminal_velocity;
    float fast_fall_velocity;
} ftCo_DatAttrs;
typedef struct {
    Vec3 self_vel;
    Vec3 x74_anim_vel;
    float gr_vel;
    float xE4_ground_accel_1;
    float xF0_ground_kb_vel;
    float xF4_ground_attacker_shield_kb_vel;
    struct { Vec2 lstick[1]; } input;
    struct { struct { Vec3 normal; } floor; } coll_data;
    ftCo_DatAttrs co_attrs;
} Fighter;
typedef struct { Fighter* user_data; } HSD_GObj;

#define ABS(x) ((x) < 0 ? -(x) : (x))
#define SOLUTION 0
void ftCommon_8007D174(Fighter*, float, float, float, float);
void ftCommon_8007D28C(Fighter*, float);
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-variable"
#include "ftcommon_original.inc"
#pragma GCC diagnostic pop

_Static_assert(sizeof(Vec3) == 3 * sizeof(float), "packed scalar vector");
_Static_assert(sizeof(ftCo_DatAttrs) == 9 * sizeof(float), "packed scalar attributes");

/* Raw state indices are documented in tests/physics_differential.rs. */
void oracle_physics_step(float state[23], uint32_t operation, const float args[4])
{
    Fighter fp = {0};
    HSD_GObj object = {&fp};
    memcpy(&fp.self_vel, state, sizeof(Vec3));
    memcpy(&fp.x74_anim_vel, state + 3, sizeof(Vec3));
    fp.gr_vel = state[6];
    fp.xE4_ground_accel_1 = state[7];
    fp.xF0_ground_kb_vel = state[8];
    fp.xF4_ground_attacker_shield_kb_vel = state[9];
    fp.input.lstick[0].x = state[10];
    memcpy(&fp.coll_data.floor.normal, state + 11, sizeof(Vec3));
    memcpy(&fp.co_attrs, state + 14, sizeof(ftCo_DatAttrs));

    switch (operation) {
    case 0: ftCommon_ApplyFrictionGround(&fp, args[0]); break;
    case 1: ftCommon_8007C98C(&fp, args[0], args[1], args[2]); break;
    case 2: ftCommon_8007CA80(&fp, args[0], args[1], args[2]); break;
    case 3: ftCommon_8007CADC(&fp, args[0], args[1], args[2]); break;
    case 4: {
        HSD_GObj* result = ftCommon_ApplyGroundMovementNoSlide(&object);
        if (result != &object) abort();
        break;
    }
    case 5: ftCommon_ClampGrVel(&fp, args[0]); break;
    case 6: ftCommon_8007CCA0(&fp, args[0]); break;
    case 7: ftCommon_8007CE4C(&fp, args[0]); break;
    case 8: ftCommon_ApplyFrictionAir(&fp, args[0]); break;
    case 9: ftCommon_8007CEF4(&fp); break;
    case 10: ftCommon_8007D140(&fp, args[0], args[1], args[2]); break;
    case 11: ftCommon_8007D174(&fp, args[0], args[1], args[2], args[3]); break;
    case 12: ftCommon_8007D268(&fp); break;
    case 13: ftCommon_8007D28C(&fp, args[0]); break;
    case 14: ftCommon_8007D2E8(&fp, args[0], args[1], args[2]); break;
    case 15: ftCommon_8007D344(&fp, args[0], args[1], args[2]); break;
    case 16: ftCommon_8007D3A8(&fp, args[0], args[1], args[2]); break;
    case 17: ftCommon_ClampSelfVelX(&fp, args[0]); break;
    case 18: ftCommon_ClampAirDrift(&fp); break;
    case 19: ftCommon_Fall(&fp, args[0], args[1]); break;
    case 20: ftCommon_FallBasic(&fp); break;
    case 21: ftCommon_FallFast(&fp); break;
    case 22: ftCommon_ClampFallSpeed(&fp, args[0]); break;
    case 23: ftCommon_Ascend(&fp, args[0], args[1]); break;
    default: abort();
    }

    memcpy(state, &fp.self_vel, sizeof(Vec3));
    memcpy(state + 3, &fp.x74_anim_vel, sizeof(Vec3));
    state[6] = fp.gr_vel;
    state[7] = fp.xE4_ground_accel_1;
    state[8] = fp.xF0_ground_kb_vel;
    state[9] = fp.xF4_ground_attacker_shield_kb_vel;
    state[10] = fp.input.lstick[0].x;
    memcpy(state + 11, &fp.coll_data.floor.normal, sizeof(Vec3));
    memcpy(state + 14, &fp.co_attrs, sizeof(ftCo_DatAttrs));
}
