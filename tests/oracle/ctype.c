/* Byte-for-byte upstream header and source. Rename standard function names to
 * avoid accidentally calling host libc or triggering compiler builtins.
 */
#define tolower oracle_tolower
#define toupper oracle_toupper
#define isalpha original_isalpha
#define isdigit original_isdigit
#define isspace original_isspace
#define isupper original_isupper
#define isxdigit original_isxdigit
#include "original/ctype.h"
#include "ctype_original.inc"

unsigned char oracle_ctype_flags(int c) { return __ctype_map[(unsigned char)c]; }

int oracle_isalpha(int c) { return original_isalpha(c); }
int oracle_isdigit(int c) { return original_isdigit(c); }
int oracle_isspace(int c) { return original_isspace(c); }
int oracle_isupper(int c) { return original_isupper(c); }
int oracle_isxdigit(int c) { return original_isxdigit(c); }
