/* Ordinary airborne, non-x2228_b2 knockback branch from Fighter_procUpdate.
 * Full source is preserved in original/damage_fighter.c. The marked block is
 * byte-for-byte unchanged; wrapper supplies only its local vector/common data.
 * No ground, shield recoil, wind, integration or callback behavior is modeled. */
#include <math.h>
typedef struct { float x, y; } Vec2;
typedef struct { float x204_knockbackFrameDecay; } ftCommonData;
void oracle_damage_decay(const float* velocity, float decay, float* output) {
    Vec2 state = { velocity[0], velocity[1] }, *p_kb_vel = &state;
    ftCommonData common = { decay }, *p_ftCommonData = &common;
    float kb_vel_x = state.x, kb_vel_y = state.y;
    if (kb_vel_x != 0 || kb_vel_y != 0) {
/* BEGIN VERBATIM AIR DECAY */
                    float kb_angle = atan2f(kb_vel_y, kb_vel_x);

                    if (sqrtf(kb_vel_x * kb_vel_x + kb_vel_y * kb_vel_y) <
                        p_ftCommonData->x204_knockbackFrameDecay)
                    {
                        p_kb_vel->x = p_kb_vel->y = 0;
                    } else {
                        p_kb_vel->x -=
                            p_ftCommonData->x204_knockbackFrameDecay *
                            cosf(kb_angle);
                        p_kb_vel->y -=
                            p_ftCommonData->x204_knockbackFrameDecay *
                            sinf(kb_angle);
                    }
/* END VERBATIM AIR DECAY */
    }
    output[0] = state.x;
    output[1] = state.y;
}
