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

/* Exact horizontal input branch. The same scalar helper is used with the
 * independent vertical threshold. Ancillary source counters are retained in
 * this adapter so the original block remains byte-for-byte unchanged. */
#include <stdint.h>
uint8_t oracle_damage_tilt_timer(uint8_t timer, float current, float previous,
                                float threshold) {
    struct {
        struct { Vec2 lstick[2]; } input;
        uint8_t x670_timer_lstick_tilt_x, x673, x679_x, x676_x;
        int x2228_b7;
    } fighter = { .input.lstick={{current,0},{previous,0}},
        .x670_timer_lstick_tilt_x=timer }, *fp = &fighter;
    struct { float horizontal_stick_smash_deadzone; }
        common = {threshold}, *p_ftCommonData = &common;
/* BEGIN VERBATIM TILT TIMER */
            if (fp->input.lstick[0].x >=
                p_ftCommonData->horizontal_stick_smash_deadzone)
            {
                if (fp->input.lstick[1].x >=
                    p_ftCommonData->horizontal_stick_smash_deadzone)
                {
                    // Fighter_ClampThreeValues
                    fp->x670_timer_lstick_tilt_x++;
                    if (fp->x670_timer_lstick_tilt_x > 0xFE) {
                        fp->x670_timer_lstick_tilt_x = 0xFE;
                    }
                    fp->x673++;
                    if (fp->x673 > 0xFE) {
                        fp->x673 = 0xFE;
                    }
                    fp->x679_x++;
                    if (fp->x679_x > 0xFE) {
                        fp->x679_x = 0xFE;
                    }
                } else {
                    fp->x676_x = 0;
                    fp->x673 = 0;
                    fp->x670_timer_lstick_tilt_x = 0;
                    fp->x2228_b7 = 1;
                }
            } else if (fp->input.lstick[0].x <=
                       -p_ftCommonData->horizontal_stick_smash_deadzone)
            {
                if (fp->input.lstick[1].x <=
                    -p_ftCommonData->horizontal_stick_smash_deadzone)
                {
                    // Fighter_ClampThreeValues
                    fp->x670_timer_lstick_tilt_x++;
                    if (fp->x670_timer_lstick_tilt_x > 0xFE) {
                        fp->x670_timer_lstick_tilt_x = 0xFE;
                    }
                    fp->x673++;
                    if (fp->x673 > 0xFE) {
                        fp->x673 = 0xFE;
                    }
                    fp->x679_x++;
                    if (fp->x679_x > 0xFE) {
                        fp->x679_x = 0xFE;
                    }
                } else {
                    fp->x676_x = 0;
                    fp->x673 = 0;
                    fp->x670_timer_lstick_tilt_x = 0;
                    fp->x2228_b7 = 0;
                }
            } else {
                fp->x679_x = 0xFEU;
                fp->x673 = 0xFEU;
                fp->x670_timer_lstick_tilt_x = 0xFEU;
            }
/* END VERBATIM TILT TIMER */
    return fp->x670_timer_lstick_tilt_x;
}
