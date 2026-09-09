/* Whole original function bodies; minimal host layout and thread-local player
 * routing replace GameCube globals. No copied Rust algorithm is used here. */
#include <stdint.h>
typedef uint16_t u16;
typedef uint8_t u8;
typedef uint32_t u32;
typedef int32_t s32;
typedef float f32;
typedef struct { u16 move_id, attack_instance; } Entry;
typedef struct { int current_index; Entry StaleMoves[10]; } StaleMoveTable;
typedef struct { int player_id; s32 x2068_attackID; u16 x206C_attack_instance; } Fighter;
typedef struct { Fighter* user_data; } HSD_GObj;
typedef HSD_GObj Fighter_GObj;
#define GET_FIGHTER(gobj) ((gobj)->user_data)
static _Thread_local StaleMoveTable table;
static _Thread_local u16 staleAttackInstance;
static _Thread_local float weights[9];
static _Thread_local int DbLevel;
#define DbLKind_DebugRom 1
#define Fighter_804D6548 weights
static StaleMoveTable* Player_GetStaleMoveTableIndexPtr(s32 slot) { (void)slot; return &table; }
#include "stale_queue_original.inc"
#include "stale_damage_original.inc"

static void load_table(uint8_t next, const uint16_t* entries) {
    table.current_index=next;
    for (int i=0;i<10;i++) table.StaleMoves[i]=(Entry){entries[2*i],entries[2*i+1]};
}
uint8_t oracle_stale_record(uint8_t next, uint16_t* entries, uint16_t move,
                            uint16_t instance, int self_hit, int reset) {
    load_table(next,entries);
    Fighter attacker={0,move,instance}, victim={0};
    HSD_GObj a={&attacker}, b={&victim};
    if (reset) plStale_ResetStaleMoveTableForPlayer(0);
    else plStale_UpdateStaleMovesFromFighter(&a,self_hit?&a:&b);
    for (int i=0;i<10;i++) { entries[2*i]=table.StaleMoves[i].move_id;
        entries[2*i+1]=table.StaleMoves[i].attack_instance; }
    return (uint8_t)table.current_index;
}
float oracle_stale_damage(uint8_t next,const uint16_t* entries,int32_t move,
                          uint16_t instance,float base,const float* penalties,int bypass) {
    load_table(next,entries);
    for(int i=0;i<9;i++) weights[i]=penalties[i];
    DbLevel=bypass;
    Fighter fighter={0};
    return ft_80089228(&fighter,move,instance,base);
}
void oracle_stale_identity(uint16_t* identity,uint16_t* next,uint16_t move,int operation) {
    Fighter fighter={0,identity[0],identity[1]};
    HSD_GObj object={&fighter};
    staleAttackInstance=*next;
    if(operation==0) ft_800890BC(&fighter);
    else if(operation==1) ft_800890D0(&fighter,move);
    else ft_800892A0(&object);
    identity[0]=(uint16_t)fighter.x2068_attackID;
    identity[1]=fighter.x206C_attack_instance;
    *next=staleAttackInstance;
}
