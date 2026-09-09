/* Complete functions from gm_1A36.c run unchanged against native digital input.
 * Controller-map declarations and PAD masks below are verbatim snapshot excerpts,
 * verified by Rust tests. Native pointer size can change the enclosing struct's
 * offsets, but selected functions access fields by name and retain scalar widths.
 * The adapter supplies HSD's already-digital button/edge words and sets its unused
 * ordinary repeat word to zero. This does not test analog conversion, PAD error
 * handling, HSD's separate repeat algorithm, or hardware polling.
 */
#include <stdint.h>
#include <stddef.h>
#include <string.h>

typedef uint8_t u8;
typedef uint16_t u16;
typedef uint32_t u32;
typedef uint64_t u64;
typedef int32_t s32;
#define PAD_STACK(n)

/* BEGIN PAD SNAPSHOT */
#define PAD_MAX_CONTROLLERS 4

#define PAD_BUTTON_LEFT (1 << 0)     // 0x0001
#define PAD_BUTTON_RIGHT (1 << 1)    // 0x0002
#define PAD_BUTTON_DOWN (1 << 2)     // 0x0004
#define PAD_BUTTON_UP (1 << 3)       // 0x0008
#define PAD_TRIGGER_Z (1 << 4)       // 0x0010
#define PAD_TRIGGER_R (1 << 5)       // 0x0020
#define PAD_TRIGGER_L (1 << 6)       // 0x0040
#define PAD_BUTTON_A (1 << 8)        // 0x0100
#define PAD_BUTTON_B (1 << 9)        // 0x0200
#define PAD_BUTTON_X (1 << 10)       // 0x0400
#define PAD_BUTTON_Y (1 << 11)       // 0x0800
#define PAD_BUTTON_MENU (1 << 12)    // 0x1000
#define PAD_BUTTON_START (1 << 12)   // 0x1000
#define PAD_STICK_UP (1 << 16)       // 0x10000
#define PAD_STICK_DOWN (1 << 17)     // 0x20000
#define PAD_STICK_LEFT (1 << 18)     // 0x40000
#define PAD_STICK_RIGHT (1 << 19)    // 0x80000
#define PAD_SUBSTICK_UP (1 << 20)    // 0x100000
#define PAD_SUBSTICK_DOWN (1 << 21)  // 0x200000
#define PAD_SUBSTICK_LEFT (1 << 22)  // 0x400000
#define PAD_SUBSTICK_RIGHT (1 << 23) // 0x800000
#define PAD_TRIGGER_LR (1 << 31)     // 0x80000000
#define PAD_CONFIRM (1ULL << 32)        // 0x100000000
#define PAD_CANCEL (1ULL << 33)         // 0x200000000
#define PAD_LR_START (1ULL << 34)       // 0x400000000
#define PAD_LRA_START (1ULL << 35)      // 0x800000000
#define PAD_ANY_UP (1ULL << 36)         // 0x1000000000
#define PAD_ANY_DOWN (1ULL << 37)       // 0x2000000000
#define PAD_ANY_LEFT (1ULL << 38)       // 0x4000000000
#define PAD_ANY_RIGHT (1ULL << 39)      // 0x8000000000
/* END PAD SNAPSHOT */

/* BEGIN STRUCT SNAPSHOT */
struct gm_controller_map {
    /* 00 */ u64 button;
    /* 08 */ u64 trigger; ///< buttons pressed this frame, maybe rename?
    /* 10 */ u64 repeat;
    /* 18 */ u64 release;
    /* 20 */ u64 repeat2;
    /* 28 */ s32 repeat_timer;
    /* 2C */ s32 x2C;
};

static struct controller_map {
    struct gm_controller_map x0[PAD_MAX_CONTROLLERS + 1];
    /* F0 */ void (*xF0)(int);
    /* F4 */ u16 xF4;
    /* F6 */ u8 xF6;
    /* F8 */ u16 xF8;
    /* FA */ u8 xFA;
    /* FC */ u16 xFC;
    /* FE */ u8 xFE;
} controller_map;
/* END STRUCT SNAPSHOT */

/* Only these four fields of HSD_PadStatus are read by the selected functions. */
static struct {
    u32 button, trigger, repeat, release;
} HSD_PadCopyStatus[4];

#include "gm_controller_original.inc"

typedef struct {
    u64 triggered, repeated;
} OracleMenuPadFrame;

void oracle_menu_controllers_reset(void)
{
    memset(HSD_PadCopyStatus, 0, sizeof(HSD_PadCopyStatus));
    gm_801A3E88();
}

void oracle_menu_controllers_poll(const u32 buttons[4], OracleMenuPadFrame out[5])
{
    for (int i = 0; i < 4; i++) {
        u32 last_button = HSD_PadCopyStatus[i].button;
        HSD_PadCopyStatus[i].button = buttons[i];
        HSD_PadCopyStatus[i].trigger = buttons[i] & (last_button ^ buttons[i]);
        HSD_PadCopyStatus[i].release = last_button & (last_button ^ buttons[i]);
    }
    gm_EvaluateAllControllerInputs();
    for (int i = 0; i < 5; i++) {
        out[i].triggered = controller_map.x0[i].trigger;
        out[i].repeated = controller_map.x0[i].repeat2;
    }
}
