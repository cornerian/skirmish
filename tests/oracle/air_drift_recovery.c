/* Host adapter for the complete `ftCommon_8007CF58` (`ftcommon.c:283-306`,
 * reusing the "ftcommon" alias like `combat_hitlag`/`nudge`/`grab_mash`):
 * the ordinary aerial drift-or-friction call every Reflector air phase
 * uses. Extracted whole (both the under- and over-`air_drift_max`
 * branches) from the pinned snapshot, with a minimal host `Fighter`/
 * `ftCo_DatAttrs` exposing only the four fields the function itself reads
 * or writes. `characters::fox::down`'s own move-specific adapter
 * (`ftfoxspeciallw.c`) cannot link against this translation unit's own
 * struct layout directly (see that file's own header note on why every
 * cross-file common function is duplicated instead), so it reproduces the
 * identical body against its own `Fighter` struct; this adapter is what
 * independently proves that hand copy faithful to the pinned source. */
#include <stdbool.h>

typedef struct {
    float x, y, z;
} Vec3;

typedef struct {
    float aerial_friction;
    float air_drift_max;
} ftCo_DatAttrs;

typedef struct {
    Vec3 self_vel;
    Vec3 x74_anim_vel;
    ftCo_DatAttrs co_attrs;
} Fighter;

typedef struct {
    float x1FC;
} ftCommonData;
/* Thread-local: `oracle_air_drift_recovery` writes `x1FC` on every call,
 * and libtest runs `arbitrary_air_drift_recovery` and
 * `arbitrary_air_drift_recovery_full_range` (this file's two callers)
 * concurrently by default. A plain (non-thread-local) global here let one
 * property test's `x1fc` bleed into the other's concurrent
 * `ftCommon_8007CF58` call -- the same cross-thread aliasing
 * `ftfoxspeciallw.c`'s own copy of this state had (see that file's note).
 * `p_ftCommonData` is a macro rather than a plain pointer so it resolves
 * per-thread instead of freezing to whichever thread ran static init. */
static _Thread_local ftCommonData ftCommonData_;
#define p_ftCommonData (&ftCommonData_)

#define ABS(x) ((x) < 0 ? -(x) : (x))

bool ftCommon_8007CF58(Fighter* fp);

#include "air_drift_recovery_original.inc"

/* Runs the real extracted function once; returns its own bool result
 * (true: over the drift maximum) and the resulting `x74_anim_vel.x`. */
bool oracle_air_drift_recovery(float self_vel_x, float aerial_friction, float air_drift_max,
                                float x1fc, float* out_anim_vel_x) {
    Fighter fp = { 0 };
    fp.self_vel.x = self_vel_x;
    fp.co_attrs.aerial_friction = aerial_friction;
    fp.co_attrs.air_drift_max = air_drift_max;
    p_ftCommonData->x1FC = x1fc;
    bool result = ftCommon_8007CF58(&fp);
    *out_anim_vel_x = fp.x74_anim_vel.x;
    return result;
}
