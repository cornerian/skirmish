/* Native scalar/vector declarations only; original lbColl_80005C44 is unchanged.
 * approximatelyZero is the verbatim helper from lbcollision.h. */
#include <string.h>
typedef int bool;
#define true 1
#define false 0
#define SQ(x) ((x) * (x))
typedef struct { float x, y, z; } Vec3;

static inline bool approximatelyZero(float x)
{
    bool result;

    if ((x < 0.00001f) && (x > -0.00001f)) {
        result = true;
    } else {
        result = false;
    }

    return result;
}

#include "combat_collision_original.inc"

int oracle_combat_capsule(const float values[11], float closest[3]) {
    Vec3 start, end, center, output;
    memcpy(&start, values, sizeof(start));
    memcpy(&end, values + 3, sizeof(end));
    memcpy(&center, values + 6, sizeof(center));
    memcpy(&output, closest, sizeof(output));
    int result = lbColl_80005C44(&start, &end, &center, &output, values[9], values[10]);
    memcpy(closest, &output, sizeof(output));
    return result;
}
