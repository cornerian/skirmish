/* Host adapter for the pinned idle-animation-cycling callbacks
 * (`ftCo_8008A698`, `ftCo_8008A6D8`, `inlineA0`, `getAnimID`,
 * `ftCo_8008A7A8`, `tests/oracle/original/waitanim.c`,
 * `waitanim.functions.json`). `inlineA0`/`getAnimID` are `static inline` in
 * the original file; `build.rs`'s `extract_function` matches definitions by
 * name regardless of `static`/`inline` qualifiers (it only requires the
 * header line to start at column zero and contain no `;`/`=`/`(` before the
 * name), so both are extracted verbatim like the non-static functions --
 * they are not part of any *caller's* translation unit here, since nothing
 * else in this adapter defines `ftCo_Wait_Anim`.
 *
 * `ftAnim_IsFramesRemaining` and `HSD_Randi` are scripted; `ftData_80085CD8`,
 * `ftCo_8009E7B4`, `ftAnim_8006EBE8`, `ftAnim_8006EBA4` and `itGetHoldKind`
 * are no-ops (the last is unreachable here since `item_gobj` is always
 * NULL, but the symbol must still resolve at link time); `HSD_ASSERTREPORT`
 * is overridden to record the assert. `getAnimID` falls off its own end
 * (no `return` follows `HSD_ASSERTREPORT` in the pinned source, since the
 * real `__assert` it calls is `ATTRIBUTE_NORETURN`) -- reproducing that
 * "never returns" contract, rather than letting execution continue with an
 * undefined return value that the caller then uses as a `fp->x24`/`x28`
 * array index, is why the override `longjmp`s back to `oracle_wait_anim`
 * instead of returning normally. Thread-local state isolates independent
 * concurrent test calls, not gameplay global state. */
#include <setjmp.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef uint8_t u8;
typedef int32_t s32;
typedef uint32_t u32;
typedef float f32;
typedef s32 enum_t;

#define FTKIND_MEWTWO 18
#define FTKIND_FOX 2

typedef struct WaitStruct {
    union {
        struct {
            int* x;
            int* y;
        } p;
        struct {
            int x;
            int y;
        } i;
    } u;
} WaitStruct;

struct Fighter_WaitAnimData {
    void* x0;
    s32 x4;
    s32 x8;
    void* xC;
    s32 x10_animCurrFlags;
    u32 x14;
};

/* Sized comfortably past every sub-motion id this oracle exercises
 * (`fp->x24`/`x28` arrays sized >= 64, per the task note). */
#define WAIT_ANIM_TABLE_SIZE 64

typedef struct Fighter {
    enum_t anim_id;
    enum_t kind;
    void* item_gobj;
    struct Fighter_WaitAnimData x24[WAIT_ANIM_TABLE_SIZE];
    u8 x28[WAIT_ANIM_TABLE_SIZE][2];
    struct {
        void* u;
        s32 loop_count;
        f32 timer;
    } x3E4_fighterCmdScript;
    void* x590;
    s32 x594_s32;
} Fighter;

typedef struct {
    Fighter* user_data;
} HSD_GObj;
typedef HSD_GObj Fighter_GObj;
typedef HSD_GObj Item_GObj;

#define GET_FIGHTER(gobj) ((gobj)->user_data)

static int itGetHoldKind(Item_GObj* gobj) {
    (void) gobj;
    return 0;
}

static _Thread_local bool scripted_frames_remaining;
static bool ftAnim_IsFramesRemaining(Fighter_GObj* gobj) {
    (void) gobj;
    return scripted_frames_remaining;
}

/* Scripted HSD_Randi sequence: each call returns the next value and
 * advances the shared draw counter (`out_draws` in `oracle_wait_anim`),
 * matching the source's own "every draw, including a repeated one,
 * advances the RNG" contract (`src/random.rs`'s own `HsdRng`). Running off
 * the end of the scripted sequence is a test-setup error, not a source
 * behavior, and aborts loudly rather than silently wrapping. */
static _Thread_local const int* scripted_rng_sequence;
static _Thread_local int scripted_rng_len;
static _Thread_local int scripted_rng_used;
static int HSD_Randi(int max) {
    (void) max;
    if (scripted_rng_used >= scripted_rng_len) {
        return 0;
    }
    return scripted_rng_sequence[scripted_rng_used++];
}

static _Thread_local bool asserted;
static _Thread_local jmp_buf assert_jmp;
/* The real `__assert` this macro calls on a failed condition is
 * `ATTRIBUTE_NORETURN`; `getAnimID` relies on that (no `return` follows its
 * own `HSD_ASSERTREPORT` call). `longjmp` reproduces "never returns"
 * without executing the undefined fall-off-the-end return. */
#define HSD_ASSERTREPORT(line, cond, ...) \
    do { \
        if (!(cond)) { \
            asserted = true; \
            longjmp(assert_jmp, 1); \
        } \
    } while (0)

static void ftData_80085CD8(Fighter* fp, Fighter* fp2, enum_t anim_id) {
    (void) fp;
    (void) fp2;
    (void) anim_id;
}
static void ftCo_8009E7B4(Fighter* fp, u8 (*blend_data)[2]) {
    (void) fp;
    (void) blend_data;
}
static void ftAnim_8006EBE8(Fighter_GObj* gobj, f32 anim_start, f32 anim_rate, u8 blend) {
    (void) gobj;
    (void) anim_start;
    (void) anim_rate;
    (void) blend;
}
static void ftAnim_8006EBA4(Fighter_GObj* gobj) {
    (void) gobj;
}

#include "waitanim_original.inc"

/* Runs `ftCo_8008A7A8` once: `current_anim` seeds `fp->anim_id`,
 * `frames_remaining` scripts `ftAnim_IsFramesRemaining`, `table`/`table_len`
 * build a `WaitStruct` (NULL when `table_len == 0`, matching the source's
 * own `arg1 == NULL` no-table restart path) terminated by `{-1, 0}`, and
 * `rng_sequence`/`rng_len` script every `HSD_Randi` call `getAnimID` makes.
 * Returns the resulting `fp->anim_id`; `*out_draws` is the number of
 * `HSD_Randi` calls consumed (0 for a no-table restart or when frames are
 * still remaining); `*out_asserted` reports whether `HSD_ASSERTREPORT`
 * fired (the table's accumulated weight fell short of a draw's `max`). */
enum_t oracle_wait_anim(enum_t current_anim, bool frames_remaining, const s32* table,
                         int table_len, const int* rng_sequence, int rng_len, int* out_draws,
                         bool* out_asserted) {
    Fighter fp;
    for (int i = 0; i < WAIT_ANIM_TABLE_SIZE; i++) {
        fp.x24[i].x0 = NULL;
        fp.x24[i].x4 = 0;
        fp.x24[i].x8 = 0;
        fp.x24[i].xC = NULL;
        fp.x24[i].x10_animCurrFlags = 0;
        fp.x24[i].x14 = 0;
        fp.x28[i][0] = 0;
        fp.x28[i][1] = 0;
    }
    fp.anim_id = current_anim;
    fp.kind = 0;
    fp.item_gobj = NULL;
    fp.x3E4_fighterCmdScript.u = NULL;
    fp.x3E4_fighterCmdScript.loop_count = 0;
    fp.x3E4_fighterCmdScript.timer = 0.0f;
    /* Non-NULL so ftCo_8008A6D8's own x590-gated ftAnim_8006EBE8 branch
     * (a no-op here regardless) is exercised, matching a live Fighter. */
    fp.x590 = &fp;
    fp.x594_s32 = 0;
    Fighter_GObj gobj = {&fp};

    /* table entries are (sub_motion, weight) pairs; build the WaitStruct
     * array with the source's own -1 terminator. */
    WaitStruct wait_data[WAIT_ANIM_TABLE_SIZE + 1];
    for (int i = 0; i < table_len; i++) {
        wait_data[i].u.i.x = table[2 * i];
        wait_data[i].u.i.y = table[2 * i + 1];
    }
    wait_data[table_len].u.i.x = -1;
    wait_data[table_len].u.i.y = 0;

    scripted_frames_remaining = frames_remaining;
    scripted_rng_sequence = rng_sequence;
    scripted_rng_len = rng_len;
    scripted_rng_used = 0;
    asserted = false;

    /* getAnimID's HSD_ASSERTREPORT longjmp lands here, skipping the rest
     * of ftCo_8008A7A8 (including fp->anim_id's own assignment) exactly as
     * the source's own noreturn __assert would -- fp.anim_id is left at
     * current_anim, matching "the walk fell off the end before picking
     * anything". */
    if (setjmp(assert_jmp) == 0) {
        ftCo_8008A7A8(&gobj, table_len == 0 ? NULL : wait_data);
    }

    *out_draws = scripted_rng_used;
    *out_asserted = asserted;
    return fp.anim_id;
}
