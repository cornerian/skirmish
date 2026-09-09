/* Original PadClamp.c, with host typedefs and PADStatus from Dolphin's header.
 * No function body adaptations. Custom calibration writes the original global;
 * Rust tests serialize these calls so concurrent cases cannot race.
 */
#include <stdint.h>
#include <string.h>

typedef int8_t s8;
typedef uint8_t u8;
typedef uint16_t u16;
typedef struct {
    u16 button;
    s8 stickX, stickY, substickX, substickY;
    u8 triggerLeft, triggerRight, analogA, analogB;
    s8 err;
} PADStatus;
#define PAD_ERR_NONE 0
#include "PadClamp_original.inc"

_Static_assert(sizeof(PADStatus) == 12, "original PADStatus scalar layout");
_Static_assert(sizeof(PADClampRegion) == 8, "original clamp region scalar layout");

void oracle_input_stick(s8 stick[2], s8 maximum, s8 corner, s8 deadzone)
{
    ClampStick(&stick[0], &stick[1], maximum, corner, deadzone);
}

u8 oracle_input_trigger(u8 trigger, u8 minimum, u8 maximum)
{
    ClampRegion.minTrigger = minimum;
    ClampRegion.maxTrigger = maximum;
    ClampTrigger(&trigger);
    return trigger;
}

void oracle_input_clamp(PADStatus status[4], const PADClampRegion* region)
{
    memcpy(&ClampRegion, region, sizeof(ClampRegion));
    PADClamp(status);
}
