/* Complete mnmain.c functions run unmodified against scalar host declarations.
 * Menu enums are the exact pinned forward.h. PAD masks are from Dolphin pad.h;
 * MenuInput and the timer helper are from mn/inlines.h. A provenance test checks
 * the helper verbatim. MenuFlow contains only accessed fields, retaining widths;
 * it is not intended to reproduce the original pointer-bearing memory layout.
 * Rendering, audio, allocation and scene scheduling are no-ops. External submenu
 * initializers record their identity without executing unported leaf behavior.
 * Thus callback comparisons stop at delegation, before any callee side effects.
 */
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include "original/mn_forward.h"

typedef uint8_t u8;
typedef uint16_t u16;
typedef uint32_t u32;
typedef uint64_t u64;
typedef int32_t s32;
typedef struct { u16 cooldown, x2; s32 x4; } MenuInputState;
typedef struct {
    u8 cur_menu, prev_menu;
    u16 hovered_selection;
    u64 buttons;
    u8 entering_menu;
} MenuFlow;
typedef struct HSD_GObj { int unused; } HSD_GObj;
typedef struct { u8 flags_3; } HSD_GObjProc;
typedef struct { int pending_mode; } MenuExitData;
struct MenuKindData {
    u8 selection_count;
    void (*think)(HSD_GObj*);
};

#define PAD_STACK(n)
#define PAD_BUTTON_A (1 << 8)
#define PAD_BUTTON_START (1 << 12)
#define PAD_TRIGGER_L (1 << 6)
#define PAD_TRIGGER_R (1 << 5)
#define PAD_BUTTON_X (1 << 10)
#define PAD_BUTTON_Y (1 << 11)
#define PAD_CONFIRM (1ULL << 32)
#define PAD_CANCEL (1ULL << 33)
#define PAD_ANY_UP (1ULL << 36)
#define PAD_ANY_DOWN (1ULL << 37)
#define PAD_ANY_LEFT (1ULL << 38)
#define PAD_ANY_RIGHT (1ULL << 39)

typedef enum _MenuInput {
    MenuInput_Up = 1 << 0,          ///< 0x0001
    MenuInput_Down = 1 << 1,        ///< 0x0002
    MenuInput_Left = 1 << 2,        ///< 0x0004
    MenuInput_Right = 1 << 3,       ///< 0x0008
    MenuInput_Confirm = 1 << 4,     ///< 0x0010
    MenuInput_Back = 1 << 5,        ///< 0x0020
    MenuInput_LTrigger = 1 << 6,    ///< 0x0040
    MenuInput_RTrigger = 1 << 7,    ///< 0x0080
    MenuInput_StartButton = 1 << 8, ///< 0x0100
    MenuInput_AButton = 1 << 9,     ///< 0x0200
    MenuInput_XButton = 1 << 10,    ///< 0x0400
    MenuInput_YButton = 1 << 11,    ///< 0x0800
} MenuInput;

/* Scene IDs from src/melee/gm/forward.h at the pinned revision. */
enum {
    GM_TITLE = 0, GM_VS = 2, GM_CLASSIC = 3, GM_ADVENTURE = 4,
    GM_ALLSTAR = 5, GM_CAMERA_MODE = 10, GM_TOY_GALLERY = 11,
    GM_TOY_LOTTERY = 12, GM_TOY_COLLECTION = 13, GM_TARGET_TEST = 15,
    GM_SUPER_SUDDEN_DEATH_VS = 16, GM_INVISIBLE_VS = 17, GM_SLOMO_VS = 18,
    GM_LIGHTNING_VS = 19, GM_TOURNAMENT = 27, GM_TRAINING = 28,
    GM_TINY_VS = 29, GM_GIANT_VS = 30, GM_STAMINA_VS = 31,
    GM_HOME_RUN_CONTEST = 32, GM_CAMERA_VS = 42, GM_SINGLE_BUTTON_VS = 44
};

/* Thread-local, the same fix as `fox_specials.c`/`special_air.c`/
 * `special_s.c`/`id.c` (see those files' own notes): every one of these is
 * written per call by this adapter's own host functions below (menu-input
 * state, the scene-kind table, the mocked GObj/proc pair, edge-trigger
 * masks, the all-star/sound-test flags, the pending-exit struct, and the
 * leaf request), and read back by the included decomp source
 * (`mnmain_original.inc`) within the same call. `tests/menu_differential.rs`
 * runs its several `#[test]`s -- including a `proptest!` block -- under the
 * default parallel test runner, so a plain (non-thread-local) global here
 * would let one concurrently-running test's menu state bleed into another's,
 * exactly like the race `3b56a66` fixed for the shared `ftCommonData`.
 * `HSD_GObj_CurrentInvokedProcGObj` is a macro rather than a plain pointer
 * for the same reason `p_ftCommonData` is: a pointer initialized once at
 * declaration would only ever resolve to one thread's `dummy_gobj`. */
static _Thread_local MenuInputState mn_804D6BC8;
static _Thread_local MenuFlow mn_804A04F0;
static _Thread_local MenuKindData mn_803EB6B0[34];
static _Thread_local HSD_GObj dummy_gobj;
static _Thread_local HSD_GObjProc dummy_proc;
#define HSD_GObj_CurrentInvokedProcGObj (&dummy_gobj)
static _Thread_local u8 HSD_GObj_804D783C;
static _Thread_local u64 triggered[5], repeated[5];
static _Thread_local bool all_star, sound_test;
static _Thread_local MenuExitData exit_data;
static _Thread_local u32 request_kind, request_value, controller_port;

static inline void Menu_DecrementAnimTimer(void)
{
    mn_804D6BC8.cooldown--;
    mn_804D6BC8.x2 = 0;
    mn_804D6BC8.x4 = 0;
}

static u64 gm_GetButtonsTriggered(u32 port) { return triggered[port]; }
static u64 gm_801A36C0(u32 port) { return repeated[port]; }
static bool gmMainLib_8015EDD4(void) { return all_star; }
static bool gmMainLib_8015EE90(void) { return sound_test; }
static MenuExitData* gm_GetCurrentSceneExitData(void) { return &exit_data; }
static void gm_801A4B60(void) { request_kind = 1; request_value = exit_data.pending_mode; }
static void gm_801677E8(u8 port) { controller_port = port; }
static void sfxForward(void) {}
static void sfxBack(void) {}
static void sfxMove(void) {}
static void lbAudioAx_80023694(void) {}
static void lbAudioAx_800236DC(void) {}
static HSD_GObj* mn_8022B3A0(int animation) { return &dummy_gobj; }
static void HSD_GObj_80390CD4(HSD_GObj* object) {}
static void HSD_GObjFree(HSD_GObj* object) {}
static HSD_GObj* GObj_Create(int a, int b, int c) { return &dummy_gobj; }
static HSD_GObjProc* HSD_GObj_SetupProc(HSD_GObj* object, void (*callback)(HSD_GObj*), int priority)
{ return &dummy_proc; }

/* Leaf IDs are adapter protocol values, deliberately separate from game IDs. */
static void leaf(u32 value) { request_kind = 2; request_value = value; }
static void mnHyaku_8024CD64(int a) { leaf(1); }
static void mnDiagram_Init(int a, int b) { leaf(2); }
static void mnInfoBonus_80252F8C(void) { leaf(3); }
static void mnCount_Create(void) { leaf(4); }
static void mnSnap_80257F24(void) { leaf(5); }
static void mnGallery_80259868(void) { leaf(6); }
static void mnSoundTest_8024BEE0(int a) { leaf(7); }
static void mnInfo_80252758(void) { leaf(8); }
static void mnVibration_Init(int a) { leaf(9); }
static void mnSound_8024A09C(int a) { leaf(10); }
static void mnDeflicker_8024A6C4(int a) { leaf(11); }
static void mnLanguage_8024C5C0(HSD_GObj* a) { leaf(12); }
static void mnDataDel_80250170(void) { leaf(13); }
static void mn_80231714(void) { leaf(14); }
static void mnName_8023AC40(void) { leaf(15); }
static void mnEvent_8024E838(int a, int b) { leaf(16); }

#include "mnmain_original.inc"

u32 oracle_menu_translate(u16* cooldown, u16* x2, s32* x4, u64 trigger, u64 repeat)
{
    mn_804D6BC8 = (MenuInputState) { *cooldown, *x2, *x4 };
    triggered[4] = trigger;
    repeated[4] = repeat;
    u32 result = mn_80229624(4);
    *cooldown = mn_804D6BC8.cooldown;
    *x2 = mn_804D6BC8.x2;
    *x4 = mn_804D6BC8.x4;
    return result;
}

u8 oracle_menu_confirming_port(const u64 ports[4])
{
    memcpy(triggered, ports, 4 * sizeof(u64));
    return mn_802295AC();
}

u32 oracle_menu_available(u8 menu, u16 selection, u8 star, u8 sound)
{
    all_star = star;
    sound_test = sound;
    return mn_80229938(menu, selection);
}

u32 oracle_menu_available_before(u8 menu, u16 selection, u8 star, u8 sound)
{
    all_star = star;
    sound_test = sound;
    return mn_80229A04(menu, selection);
}

/* Explicit array protocol avoids depending on either language's struct ABI:
 * menu, previous menu, selection, buttons, entering, cooldown, controller port,
 * request kind (0 none, 1 scene, 2 leaf), request value. Each call is one live
 * branch callback; callers stop stepping once it delegates to another system.
 */
void oracle_menu_step(u32 state[9], u32 buttons, u8 star, u8 sound, u8 port)
{
    mn_804A04F0 = (MenuFlow) {
        .cur_menu = state[0], .prev_menu = state[1],
        .hovered_selection = state[2], .buttons = state[3],
        .entering_menu = state[4]
    };
    mn_804D6BC8 = (MenuInputState) { state[5], 0, 0 };
    controller_port = state[6];
    request_kind = state[7];
    request_value = state[8];
    all_star = star;
    sound_test = sound;
    memset(triggered, 0, sizeof(triggered));
    memset(repeated, 0, sizeof(repeated));
    const u64 masks[12] = {
        PAD_ANY_UP, PAD_ANY_DOWN, PAD_ANY_LEFT, PAD_ANY_RIGHT,
        PAD_CONFIRM, PAD_CANCEL, PAD_TRIGGER_L, PAD_TRIGGER_R,
        PAD_BUTTON_START, PAD_BUTTON_A, PAD_BUTTON_X, PAD_BUTTON_Y
    };
    for (u32 i = 0; i < 12; i++) {
        if (buttons & (1u << i)) {
            if (i < 4) repeated[4] |= masks[i];
            else triggered[4] |= masks[i];
        }
    }
    if (buttons & MenuInput_Confirm) triggered[port] = PAD_CONFIRM;
    /* Selection counts are the scalar fields of mn_803EB6B0 in mnmain.c. */
    mn_803EB6B0[0] = (MenuKindData) { 5, mn_8022DB10 };
    mn_803EB6B0[1] = (MenuKindData) { 5, mn_8022D7F4 };
    mn_803EB6B0[2] = (MenuKindData) { 5, mn_8022D594 };
    mn_803EB6B0[3] = (MenuKindData) { 4, mn_8022D34C };
    mn_803EB6B0[4] = (MenuKindData) { 6, mn_8022D104 };
    mn_803EB6B0[5] = (MenuKindData) { 5, mn_8022CE6C };
    mn_803EB6B0[6] = (MenuKindData) { 3, mn_8022CC28 };
    mn_803EB6B0[9] = (MenuKindData) { 3, mn_8022C7CC };
    mn_803EB6B0[12] = (MenuKindData) { 10, mn_8022C4F4 };
    mn_803EB6B0[28] = (MenuKindData) { 3, mn_8022CA54 };
    mn_803EB6B0[mn_804A04F0.cur_menu].think(&dummy_gobj);
    state[0] = mn_804A04F0.cur_menu;
    state[1] = mn_804A04F0.prev_menu;
    state[2] = mn_804A04F0.hovered_selection;
    state[3] = mn_804A04F0.buttons;
    state[4] = mn_804A04F0.entering_menu;
    state[5] = mn_804D6BC8.cooldown;
    state[6] = controller_port;
    state[7] = request_kind;
    state[8] = request_value;
}
