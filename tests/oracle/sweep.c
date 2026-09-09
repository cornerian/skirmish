/* Verbatim functions selected from the shared lbcollision.c snapshot. Only
 * platform types, approximatelyZero's header inline, and unused stack padding
 * are supplied here. All input and output vectors cross the ABI by memcpy. */
#include <string.h>
typedef int bool;
typedef float f32;
#define true 1
#define false 0
#define PAD_STACK(n)
typedef struct { float x, y, z; } Vec3;
static inline bool approximatelyZero(float x)
{
    bool result;
    if ((x < .00001f) && (x > -.00001f)) result = true;
    else result = false;
    return result;
}
#include "sweep_original.inc"

int oracle_sweep_capsules(const float values[14], float closest[6])
{
    Vec3 a, b, c, d, e, f;
    memcpy(&a, values, sizeof a);
    memcpy(&b, values + 3, sizeof b);
    memcpy(&c, values + 6, sizeof c);
    memcpy(&d, values + 9, sizeof d);
    memcpy(&e, closest, sizeof e);
    memcpy(&f, closest + 3, sizeof f);
    int result = lbColl_80006094(&a, &b, &c, &d, &e, &f, values[12], values[13]);
    memcpy(closest, &e, sizeof e);
    memcpy(closest + 3, &f, sizeof f);
    return result;
}

void oracle_sweep_point(const float values[9], int xy, float result[2])
{
    Vec3 start, end, point;
    memcpy(&start, values, sizeof start);
    memcpy(&end, values + 3, sizeof end);
    memcpy(&point, values + 6, sizeof point);
    result[0] = xy ? lbColl_80005FC0(&start, &end, &point, &result[1])
                   : lbColl_80005EBC(&start, &end, &point, &result[1]);
}
