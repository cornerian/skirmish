/* Exact upstream lookup implementation with host struct/allocator shims.
 * Pointer widths and allocator metadata are not compared; payload pointers
 * refer to live u32 handles for the entire trace. Track allocations separately
 * because upstream setup/forget drops bucket heads without freeing entries.
 */
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef uint32_t u32;
typedef int32_t s32;
typedef struct IDEntry { struct IDEntry* next; u32 id; void* data; } IDEntry;
typedef struct { IDEntry* table[101]; } HSD_IDTable;
typedef struct { size_t size; } HSD_ObjAllocData;
/* Thread-local: `oracle_id_trace` resets and repopulates these on every
 * call, and `tests/id_differential.rs`'s tests (including a `proptest!`
 * block) run under the default parallel test runner; a plain global here
 * would let one thread's allocation tracking corrupt another's concurrent
 * call, the same class of race `3b56a66` fixed for the shared
 * `ftCommonData` struct elsewhere in this directory. */
static _Thread_local void* oracle_allocations[1024];
static _Thread_local size_t oracle_allocation_count;

static void HSD_ObjAllocInit(HSD_ObjAllocData* data, size_t size, int alignment)
{
    (void)alignment;
    data->size = size;
}

static void* HSD_ObjAlloc(HSD_ObjAllocData* data)
{
    assert(oracle_allocation_count < 1024);
    void* allocation = malloc(data->size);
    oracle_allocations[oracle_allocation_count++] = allocation;
    return allocation;
}

static void HSD_ObjFree(HSD_ObjAllocData* data, void* allocation)
{
    (void)data;
    for (size_t i = 0; i < oracle_allocation_count; i++) {
        if (oracle_allocations[i] == allocation) {
            oracle_allocations[i] = NULL;
            free(allocation);
            return;
        }
    }
    assert(0);
}

#define HSD_ASSERT(line, condition) assert(condition)
#include "id_original.inc"

/* Each input is [kind, explicit_table, id, data]. Each output is [data, found]. */
void oracle_id_trace(const uint32_t* ops, uint32_t count, uint32_t* output)
{
    assert(count <= 512);
    uint32_t payloads[512];
    HSD_IDTable explicit_table = {0};
    oracle_allocation_count = 0;
    HSD_IDInitAllocData();
    HSD_IDSetup();
    for (uint32_t i = 0; i < count; i++) {
        const uint32_t* op = &ops[i * 4];
        HSD_IDTable* table = op[1] ? &explicit_table : NULL;
        payloads[i] = op[3];
        switch (op[0]) {
        case 0:
            HSD_IDInsertToTable(table, op[2], op[3] ? &payloads[i] : NULL);
            break;
        case 1: HSD_IDRemoveByIDFromTable(table, op[2]); break;
        case 2: break;
        case 3: HSD_IDSetup(); break;
        case 4: _HSD_IDForgetMemory(NULL, NULL); break;
        default: assert(0);
        }
        int32_t found = -1;
        void* data = HSD_IDGetDataFromTable(table, op[2], &found);
        output[i * 2] = data ? *(uint32_t*)data : 0;
        output[i * 2 + 1] = found;
        /* The optional success pointer must not affect the returned payload. */
        assert(HSD_IDGetDataFromTable(table, op[2], NULL) == data);
    }
    for (size_t i = 0; i < oracle_allocation_count; i++) {
        free(oracle_allocations[i]);
    }
    HSD_IDSetup();
}
