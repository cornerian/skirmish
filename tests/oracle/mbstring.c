/* GameCube wchar_t and size_t are unsigned 16-bit and 32-bit respectively.
 * Substitute only those type names and the exported symbol; retain the exact
 * upstream conversion loop. Its #include is stripped by build.rs.
 */
#include <stdint.h>
#define wchar_t uint16_t
#define size_t uint32_t
#define wcstombs oracle_wcstombs
#include "mbstring_original.inc"
