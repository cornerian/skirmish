/* Host adapter for the unmodified source in original/bytecode.c.
 *
 * build.rs removes includes and mechanically replaces pointer-encoded stack
 * words with the u/i/f fields below. Argument and float-store pointer casts
 * become memcpy bit copies. The stack therefore remains exactly 32 bits on a
 * 64-bit host, without aliasing violations or reads past a float's storage.
 * C union type-punning is defined by the C standard. No VM operation is replaced.
 * Allocation/reporting are host shims; only valid terminating programs are run.
 * -fwrapv models wrapping integer operations. Undefined division/cast cases are
 * excluded, and host libm is not a PowerPC transcendental oracle.
 */
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef uint8_t u8;
typedef uint32_t u32;
typedef int32_t s32;
typedef float f32;
typedef double f64;

_Static_assert(sizeof(float) == 4 && sizeof(int) == 4, "32-bit VM scalars required");

typedef struct HSD_SList {
    struct HSD_SList *next;
    union { uint32_t u; int32_t i; float f; } data;
} HSD_SList;

static uint32_t float_bits(float value) {
    uint32_t result;
    memcpy(&result, &value, sizeof(result));
    return result;
}

static HSD_SList *HSD_SListAllocAndPrepend(HSD_SList *next, uint32_t data) {
    HSD_SList *node = malloc(sizeof(*node));
    assert(node);
    node->next = next;
    node->data.u = data;
    return node;
}

static HSD_SList *HSD_SListRemove(HSD_SList *node) {
    if (!node) return NULL;
    HSD_SList *next = node->next;
    free(node);
    return next;
}

#define HSD_ASSERT(line, condition) assert(condition)
#define HSD_Panic(file, line, message) abort()
#define OSReport(...) fprintf(stderr, __VA_ARGS__)
#define DEG_TO_RAD 0.017453292519943295
#define RAD_TO_DEG 57.29577951308232
#define fabsf_bitwise(value) fabsf(value)

extern float HSD_Randf(void);
extern int32_t HSD_Randi(int32_t max_val);

#include "bytecode_original.inc"
