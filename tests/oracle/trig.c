/* Unambiguous, uniquely named entry points for the pinned trigonometry
 * bodies in `trigf_body.c`/`math_data_body.c`/`lbtrigf_body.c`, which are
 * themselves compiled under process-unique names (`skirmish_msl_sinf`,
 * `skirmish_lb_atan2f`, ...) rather than libm's own (`sinf`, `atan2f`, ...):
 * see `trigf_body.c`'s header comment for why reusing libm's names here
 * would silently replace Rust std's own trig methods throughout this test
 * binary. `oracle_*` matches this crate's other C-oracle adapters' naming.
 */
float skirmish_msl_sinf(float x);
float skirmish_msl_cosf(float x);
float skirmish_msl_tanf(float x);
float skirmish_lb_atan2f(float y, float x);
float skirmish_lb_atanf(float x);
float skirmish_lb_asinf(float x);
float skirmish_lb_acosf(float x);

float oracle_sinf(float x) { return skirmish_msl_sinf(x); }
float oracle_cosf(float x) { return skirmish_msl_cosf(x); }
float oracle_tanf(float x) { return skirmish_msl_tanf(x); }
float oracle_atan2f(float y, float x) { return skirmish_lb_atan2f(y, x); }
float oracle_atanf(float x) { return skirmish_lb_atanf(x); }
float oracle_asinf(float x) { return skirmish_lb_asinf(x); }
float oracle_acosf(float x) { return skirmish_lb_acosf(x); }
