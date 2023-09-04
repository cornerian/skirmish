# Description of all tournament legal states

Note: only the states described by Peppi are in the following tables.

States that are not possible in a tournament setting are crossed out.

## General (all characters)

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 000  | DEAD_DOWN                  | Standard downward death |
| 001  | DEAD_LEFT                  | Standard leftward death |
| 002  | DEAD_RIGHT                 | Standard rightward death |
| 003  | ~~DEAD_UP~~                | Upward death used in 1P "Team Kirby", etc. |
| 004  | DEAD_UP_STAR               | Standard Star KO |
| 005  | ~~DEAD_UP_STAR_ICE~~       | Star KO while encased in ice (Freezie) |
| 006  | ~~DEAD_UP_FALL~~           | 64-esque front fall, unused(?) |
| 007  | DEAD_UP_FALL_HIT_CAMERA    | Standard hit camera death |
| 008  | DEAD_UP_FALL_HIT_CAMERA_FLAT | Hit camera death, stuck for a moment |
| 009  | ~~DEAD_UP_FALL_ICE~~       | Hit camera death while encased in ice |
| 00A  | ~~DEAD_UP_FALL_HIT_CAMERA_ICE~~ | Hit camera death while encased in ice (different?) |
| 00B  | ~~SLEEP~~                  | "Nothing" state, probably - it is the state Shiek/Zelda is in when their counterpart is the one currently playing. Slippi doesn't record this state, it just records different characters, hence we don't care |
| 00C  | REBIRTH                    | Entering on respawn halo |
| 00D  | REBIRTH_WAIT               | Waiting on respawn halo |
| 00E  | WAIT                       | Standing idle state |
| 00F  | WALK_SLOW                  | Slow walk |
| 010  | WALK_MIDDLE                | Medium walk |
| 011  | WALK_FAST                  | Fast walk |
| 012  | TURN                       | Turn while walking |
| 013  | TURN_RUN                   | Turn while running |
| 014  | DASH                       | Beginning of run? |
| 015  | RUN                        | Running after dash? |
| 016  | ~~RUN_DIRECT~~             | ? |
| 017  | RUN_BRAKE                  | Braking from running |
| 018  | KNEE_BEND                  | Jumpsquat |
| 019  | JUMP_F                     | Forward jump from ground |
| 01A  | JUMP_B                     | Backward jump from ground |
| 01B  | JUMP_AERIAL_F              | Forward jump in air |
| 01C  | JUMP_AERIAL_B              | Backward jump in air |
| 01D  | FALL                       | Falling straight down, player has control |
| 01E  | FALL_F                     | Falling with forward DI, player has control |
| 01F  | FALL_B                     | Falling with backward DI, player has control |
| 020  | FALL_AERIAL                | Falling straight down after double jump, player has control |
| 021  | FALL_AERIAL_F              | Falling with forward DI after double jump, player has control |
| 022  | FALL_AERIAL_B              | Falling with backward DI after double jump, player has control |
| 023  | FALL_SPECIAL               | Freefalling straight down, player does not have control |
| 024  | FALL_SPECIAL_F             | Freefalling with forward DI, player does not have control |
| 025  | FALL_SPECIAL_B             | Freefalling with backward DI, player does not have control |
| 026  | DAMAGE_FALL                | Tumbling, hitstun |
| 027  | SQUAT                      | Going from stand to crouch |
| 028  | SQUAT_WAIT                 | Crouching |
| 029  | SQUAT_RV                   | Going from crouch to stand |
| 02A  | LANDING                    | Landing state, can be cancelled |
| 02B  | LANDING_FALL_SPECIAL       | Landing from freefall |
| 02C  | ATTACK_11                  | First jab |
| 02D  | ATTACK_12                  | Second jab |
| 02E  | ATTACK_13                  | Third jab |
| 02F  | ATTACK_100_START           | Start of a multi-jab loop |
| 030  | ATTACK_100_LOOP            | Middle of a multi-jab loop |
| 031  | ATTACK_100_END             | End of a multi-jab loop |
| 032  | ATTACK_DASH                | Dash attack |
| 033  | ATTACK_S_3_HI              | High Ftilt |
| 034  | ATTACK_S_3_HI_S            | High-mid Ftilt |
| 035  | ATTACK_S_3_S               | Mid Ftilt |
| 036  | ATTACK_S_3_LW_S            | Low-mid Ftilt |
| 037  | ATTACK_S_3_LW              | Low Ftilt |
| 038  | ATTACK_HI_3                | Uptilt |
| 039  | ATTACK_LW_3                | Downtilt |
| 03A  | ATTACK_S_4_HI              | High Fsmash |
| 03B  | ATTACK_S_4_HI_S            | High-mid Fsmash |
| 03C  | ATTACK_S_4_S               | Mid Fsmash |
| 03D  | ATTACK_S_4_LW_S            | Low-mid Fsmash |
| 03E  | ATTACK_S_4_LW              | Low Fsmash |
| 03F  | ATTACK_HI_4                | Upsmash |
| 040  | ATTACK_LW_4                | Downsmash |
| 041  | ATTACK_AIR_N               | Nair |
| 042  | ATTACK_AIR_F               | Fair |
| 043  | ATTACK_AIR_B               | Bair |
| 044  | ATTACK_AIR_HI              | Uair |
| 045  | ATTACK_AIR_LW              | Dair |
| 046  | LANDING_AIR_N              | Landing during Nair |
| 047  | LANDING_AIR_F              | Landing during Fair |
| 048  | LANDING_AIR_B              | Landing during Bair |
| 049  | LANDING_AIR_HI             | Landing during Uair |
| 04A  | LANDING_AIR_LW             | Landing during Dair |
| 04B  | DAMAGE_HI_1                | Hitlag states? |
| 04C  | DAMAGE_HI_2                | ? |
| 04D  | DAMAGE_HI_3                | ? |
| 04E  | DAMAGE_N_1                 | ? |
| 04F  | DAMAGE_N_2                 | ? |
| 050  | DAMAGE_N_3                 | ? |
| 051  | DAMAGE_LW_1                | ? |
| 052  | DAMAGE_LW_2                | ? |
| 053  | DAMAGE_LW_3                | ? |
| 054  | DAMAGE_AIR_1               | ? |
| 055  | DAMAGE_AIR_2               | ? |
| 056  | DAMAGE_AIR_3               | ? |
| 057  | DAMAGE_FLY_HI              | ? |
| 058  | DAMAGE_FLY_N               | ? |
| 059  | DAMAGE_FLY_LW              | ? |
| 05A  | DAMAGE_FLY_TOP             | ? |
| 05B  | DAMAGE_FLY_ROLL            | ? |
| 05C  | LIGHT_GET                  | Picking up an item |
| 05D  | ~~HEAVY_GET~~              | Picking up a heavy item (barrel) |
| 05E  | LIGHT_THROW_F              | Forward throw item at standard speed |
| 05F  | LIGHT_THROW_B              | Backward throw item at standard speed |
| 060  | LIGHT_THROW_HI             | Up throw item at standard speed |
| 061  | LIGHT_THROW_LW             | Down throw item at standard speed |
| 062  | LIGHT_THROW_DASH           | Dash throw item at standard speed |
| 063  | LIGHT_THROW_DROP           | Drop item at standard speed |
| 064  | LIGHT_THROW_AIR_F          | Forward throw item in air at standard speed |
| 065  | LIGHT_THROW_AIR_B          | Backward throw item in air at standard speed |
| 066  | LIGHT_THROW_AIR_HI         | Up throw item in air at standard speed |
| 067  | LIGHT_THROW_AIR_LW         | Down throw item in air at standard speed |
| 068  | ~~HEAVY_THROW_F~~          | Forward throw item at slow speed |
| 069  | ~~HEAVY_THROW_B~~          | Backward throw item at slow speed |
| 06A  | ~~HEAVY_THROW_HI~~         | Up throw item at slow speed |
| 06B  | ~~HEAVY_THROW_LW~~         | Down throw item at slow speed |
| 06C  | LIGHT_THROW_F_4            | Smash forward throw item |
| 06D  | LIGHT_THROW_B_4            | Smash back throw item |
| 06E  | LIGHT_THROW_HI_4           | Smash upthrow item |
| 06F  | LIGHT_THROW_LW_4           | Smash downthrow item |
| 070  | LIGHT_THROW_AIR_F_4        | Smash forward throw item in air |
| 071  | LIGHT_THROW_AIR_B_4        | Smash back throw item in air |
| 072  | LIGHT_THROW_AIR_HI_4       | Smash upthrow item in air |
| 073  | LIGHT_THROW_AIR_LW_4       | Smash downthrow item in air |
| 074  | ~~HEAVY_THROW_F_4~~        | Smash forward throw heavy item |
| 075  | ~~HEAVY_THROW_B_4~~        | Smash backthrow heavy item |
| 076  | ~~HEAVY_THROW_HI_4~~       | Smash upthrow heavy item |
| 077  | ~~HEAVY_THROW_LW_4~~       | Smash downthrow heavy item |
| 078  | SWORD_SWING_1              | Beam sword item swing 1 |
| 079  | SWORD_SWING_3              | Beam sword item swing 2 |
| 07A  | SWORD_SWING_4              | Beam sword item swing 3 |
| 07B  | SWORD_SWING_DASH           | Beam sword item dash swing |
| 07C  | ~~BAT_SWING_1~~            | Home Run Bat item swing 1 |
| 07D  | ~~BAT_SWING_3~~            | Home Run Bat item swing 2 |
| 07E  | ~~BAT_SWING_4~~            | Home Run Bat item swing 3 |
| 07F  | ~~BAT_SWING_DASH~~         | Home Run Bat item dash swing |
| 080  | ~~PARASOL_SWING_1~~        | Parasol item swing 1 |
| 081  | ~~PARASOL_SWING_3~~        | Parasol item swing 2 |
| 082  | ~~PARASOL_SWING_4~~        | Parasol item swing 3 |
| 083  | ~~PARASOL_SWING_DASH~~     | Parasol item dash swing |
| 084  | ~~HARISEN_SWING_1~~        | Fan item swing 1 |
| 085  | ~~HARISEN_SWING_3~~        | Fan item swing 2 |
| 086  | ~~HARISEN_SWING_4~~        | Fan item swing 3 |
| 087  | ~~HARISEN_SWING_DASH~~     | Fan item dash swing |
| 088  | ~~STAR_ROD_SWING_1~~       | Star Rod item swing 1 |
| 089  | ~~STAR_ROD_SWING_3~~       | Star Rod item swing 2 |
| 08A  | ~~STAR_ROD_SWING_4~~       | Star Rod item swing 3 |
| 08B  | ~~STAR_ROD_SWING_DASH~~    | Star Rod item dash swing |
| 08C  | ~~LIP_STICK_SWING_1~~      | Lip's Stick item swing 1 |
| 08D  | ~~LIP_STICK_SWING_3~~      | Lip's Stick item swing 2 |
| 08E  | ~~LIP_STICK_SWING_4~~      | Lip's Stick item swing 3 |
| 08F  | ~~LIP_STICK_SWING_DASH~~   | Lip's Stick item dash swing |
| 090  | ~~ITEM_PARASOL_OPEN~~      | Parasol item open |
| 091  | ~~ITEM_PARASOL_FALL~~      | Parasol item fall |
| 092  | ~~ITEM_PARASOL_FALL_SPECIAL~~ | Parasol item freefall after close |
| 093  | ~~ITEM_PARASOL_DAMAGE_FALL~~ | Parasol item tumble |
| 094  | ~~L_GUN_SHOOT~~            | Raygun item shoot |
| 095  | ~~L_GUN_SHOOT_AIR~~        | Raygun item shoot in air |
| 096  | ~~L_GUN_SHOOT_EMPTY~~      | Raygun item shoot while empty |
| 097  | ~~L_GUN_SHOOT_AIR_EMPTY~~  | Raygun item shoot while empty in air |
| 098  | ~~FIRE_FLOWER_SHOOT~~      | Fire flower item shoot |
| 099  | ~~FIRE_FLOWER_SHOOT_AIR~~  | Fire flower item shoot in air |
| 09A  | ~~ITEM_SCREW~~             | Screw item use? |
| 09B  | ~~ITEM_SCREW_AIR~~         | Screw item use in air? |
| 09C  | ~~DAMAGE_SCREW~~           | Damaged by screw item? |
| 09D  | ~~DAMAGE_SCREW_AIR~~       | Damaged by screw item in air? |
| 09E  | ~~ITEM_SCOPE_START~~       | Item scope start |
| 09F  | ~~ITEM_SCOPE_RAPID~~       | Item scope rapid |
| 0A0  | ~~ITEM_SCOPE_FIRE~~        | Item scope fire |
| 0A1  | ~~ITEM_SCOPE_END~~         | Item scope end |
| 0A2  | ~~ITEM_SCOPE_AIR_START~~   | Item scope start in air |
| 0A3  | ~~ITEM_SCOPE_AIR_RAPID~~   | Item scope rapid in air |
| 0A4  | ~~ITEM_SCOPE_AIR_FIRE~~    | Item scope fire in air |
| 0A5  | ~~ITEM_SCOPE_AIR_END~~     | Item scope end in air |
| 0A6  | ~~ITEM_SCOPE_START_EMPTY~~ | Item scope start empty |
| 0A7  | ~~ITEM_SCOPE_RAPID_EMPTY~~ | Item scope rapid empty |
| 0A8  | ~~ITEM_SCOPE_FIRE_EMPTY~~  | Item scope fire empty |
| 0A9  | ~~ITEM_SCOPE_END_EMPTY~~   | Item scope end empty |
| 0AA  | ~~ITEM_SCOPE_AIR_START_EMPTY~~ | Item scope start empty in air |
| 0AB  | ~~ITEM_SCOPE_AIR_RAPID_EMPTY~~ | Item scope rapid empty in air |
| 0AC  | ~~ITEM_SCOPE_AIR_FIRE_EMPTY~~ | Item scope fire empty in air |
| 0AD  | ~~ITEM_SCOPE_AIR_END_EMPTY~~ | Item scope end empty in air |
| 0AE  | ~~LIFT_WAIT                | ? |
| 0AF  | ~~LIFT_WALK_1              | ? |
| 0B0  | ~~LIFT_WALK_2              | ? |
| 0B1  | ~~LIFT_TURN                | ? |
| 0B2  | GUARD_ON                   | Shield startup |
| 0B3  | GUARD                      | Holding shield |
| 0B4  | GUARD_OFF                  | Dropping shield |
| 0B5  | GUARD_SET_OFF              | Shield stun |
| 0B6  | GUARD_REFLECT              | Powershield |
| 0B7  | DOWN_BOUND_U               | Missed tech bounce, facing up |
| 0B8  | DOWN_WAIT_U                | Laying face up on ground after missed tech |
| 0B9  | DOWN_DAMAGE_U              | Getting hit while laying face up on ground |
| 0BA  | DOWN_STAND_U               | Neutral get up from laying face up on ground |
| 0BB  | DOWN_ATTACK_U              | Get up attack from laying face up on ground |
| 0BC  | DOWN_FOWARD_U              | Roll forward from laying face up on ground |
| 0BD  | DOWN_BACK_U                | Roll backward from laying face up on ground |
| 0BE  | ~~DOWN_SPOT_U~~            | Downed in stamina mode? |
| 0BF  | DOWN_BOUND_D               | Missed tech bounce, facing down |
| 0C0  | DOWN_WAIT_D                | Laying face down on ground after missed tech |
| 0C1  | DOWN_DAMAGE_D              | Getting hit while laying face down on ground |
| 0C2  | DOWN_STAND_D               | Neutral get up from laying face down on ground |
| 0C3  | DOWN_ATTACK_D              | Get up attack from laying face down on ground |
| 0C4  | DOWN_FOWARD_D              | Roll forward from laying face down on ground |
| 0C5  | DOWN_BACK_D                | Roll backward from laying face down on ground |
| 0C6  | ~~DOWN_SPOT_D~~            | Downed in stamina mode? |
| 0C7  | PASSIVE                    | Neutral tech |
| 0C8  | PASSIVE_STAND_F            | Forward roll tech |
| 0C9  | PASSIVE_STAND_B            | Backward roll tech |
| 0CA  | PASSIVE_WALL               | Wall tech |
| 0CB  | PASSIVE_WALL_JUMP          | Walljump tech |
| 0CC  | PASSIVE_CEIL               | Ceiling tech (I would strike but I remembered the dreamland Tech De Jesus. Inordinately rare but still possible) |
| 0CD  | SHIELD_BREAK_FLY           | Beginning of shield break, when going up into the air |
| 0CE  | SHIELD_BREAK_FALL          | Beginning of shield break, when falling from the air |
| 0CF  | SHIELD_BREAK_DOWN_U        | Falling on ground face up |
| 0D0  | SHIELD_BREAK_DOWN_D        | Falling on ground face down |
| 0D1  | SHIELD_BREAK_STAND_U       | Standing up to enter dazed when face up |
| 0D2  | SHIELD_BREAK_STAND_D       | Standing up to enter dazed when face down |
| 0D3  | FURA_FURA                  | Shield break, dazed |
| 0D4  | CATCH                      | Grab |
| 0D5  | CATCH_PULL                 | Grab success, pulling them in |
| 0D6  | CATCH_DASH                 | Dash grab |
| 0D7  | CATCH_DASH_PULL            | Dash grab success |
| 0D8  | CATCH_WAIT                 | Grabbing and holding, grab idling |
| 0D9  | CATCH_ATTACK               | Grab pummel |
| 0DA  | CATCH_CUT                  | Grab broken by oppenent mash |
| 0DB  | THROW_F                    | Grab forward throw |
| 0DC  | THROW_B                    | Grab back throw |
| 0DD  | THROW_HI                   | Grab upthrow |
| 0DE  | THROW_LW                   | Grab downthrow |
| 0DF  | CAPTURE_PULLED_HI          | Getting grabbed by a taller opponent or off an edge |
| 0E0  | CAPTURE_WAIT_HI            | Taller opponent (or if you're off an edge) who has grabbed you is idling |
| 0E1  | CAPTURE_DAMAGE_HI          | Getting grab pummeled by a taller opponent (or if you're off an edge) |
| 0E2  | CAPTURE_PULLED_LW          | Getting grabbed by a shorter/same height opponent |
| 0E3  | CAPTURE_WAIT_LW            | Shorter/same height oppenent who has grabbed you is idling |
| 0E4  | CAPTURE_DAMAGE_LW          | Getting grab pummeled by a shorter/same height opponent |
| 0E5  | CAPTURE_CUT                | Mashing out of grab and being released |
| 0E6  | CAPTURE_JUMP               | Mashing out of grab while holding jump? |
| 0E7  | CAPTURE_NECK               | Hit in the neck by another player while grabbed? |
| 0E8  | CAPTURE_FOOT               | Hit in the foot by another player while grabbed? |
| 0E9  | ESCAPE_F                   | Forward roll |
| 0EA  | ESCAPE_B                   | Backward roll |
| 0EB  | ESCAPE                     | Spotdodge |
| 0EC  | ESCAPE_AIR                 | Airdodge |
| 0ED  | REBOUND_STOP               | Clanks? |
| 0EE  | REBOUND                    | Clanks? |
| 0EF  | THROWN_F                   | Getting forward thrown from grab |
| 0F0  | THROWN_B                   | Getting back thrown from grab |
| 0F1  | THROWN_HI                  | Getting upthrown from grab |
| 0F2  | THROWN_LW                  | Getting downthrown from grab |
| 0F3  | THROWN_LW_WOMEN            | ??? |
| 0F4  | PASS                       | Drop through platform |
| 0F5  | OTTOTTO                    | Beginning of ledge teeter |
| 0F6  | OTTOTTO_WAIT               | Ledge teeter |
| 0F7  | FLY_REFLECT_WALL           | Bouncing off of wall while in hitstun |
| 0F8  | FLY_REFLECT_CEIL           | Bouncing off ceiling while in hitstun |
| 0F9  | STOP_WALL                  | Bump into wall |
| 0FA  | ~~STOP_CEIL~~              | Bump into ceiling |
| 0FB  | MISS_FOOT                  | Bumped off edge while shielding, before teetering? |
| 0FC  | CLIFF_CATCH                | Ledge grab |
| 0FD  | CLIFF_WAIT                 | Hanging on ledge |
| 0FE  | CLIFF_CLIMB_SLOW           | Neutral get up on the ledge, >100% |
| 0FF  | CLIFF_CLIMB_QUICK          | Neutral get up on the ledge, <100% |
| 100  | CLIFF_ATTACK_SLOW          | Ledge attack, >100% |
| 101  | CLIFF_ATTACK_QUICK         | Ledge attack, <100% |
| 102  | CLIFF_ESCAPE_SLOW          | Ledge roll, >100% |
| 103  | CLIFF_ESCAPE_QUICK         | Ledge roll, <100% |
| 104  | CLIFF_JUMP_SLOW_1          | Ledge jump 1, >100% |
| 105  | CLIFF_JUMP_SLOW_2          | Ledge jump 2, >100% (why are there 2?) |
| 106  | CLIFF_JUMP_QUICK_1         | Ledge jump 1, <100% |
| 107  | CLIFF_JUMP_QUICK_2         | Ledge jump 2, <100% (why are there 2?) |
| 108  | APPEAL_R                   | Taunt right |
| 109  | APPEAL_L                   | Taunt left |
| 10A  | SHOULDERED_WAIT            | Cargo carried by DK |
| 10B  | SHOULDERED_WALK_SLOW       | Cargo carried by DK while he's walking slow |
| 10C  | SHOULDERED_WALK_MIDDLE     | Cargo carried by DK while he's walking medium |
| 10D  | SHOULDERED_WALK_FAST       | Cargo carried by DK while he's walking fast |
| 10E  | SHOULDERED_TURN            | Cargo carried by DK while he's turning |
| 10F  | THROWN_F_F                 | Forward thrown out of DK's cargo throw |
| 110  | THROWN_F_B                 | Back thrown out of DK's cargo throw |
| 111  | THROWN_F_HI                | Upthrown out of DK's cargo throw |
| 112  | THROWN_F_LW                | Downthrown out of DK's cargo throw |
| 113  | CAPTURE_CAPTAIN            | Caught in Captain Falcon's Raptor Boost |
| 114  | CAPTURE_YOSHI              | Caught in Yoshi's Egg Lay |
| 115  | YOSHI_EGG                  | Inside Yoshi egg |
| 116  | CAPTURE_KOOPA              | Caught in Bowser's Koopa Klaw |
| 117  | CAPTURE_DAMAGE_KOOPA       | Pummeled in Koopa Klaw |
| 118  | CAPTURE_WAIT_KOOPA         | Idled in Koopa Klaw |
| 119  | THROWN_KOOPA_F             | Forward thrown out of Koopa Klaw |
| 11A  | THROWN_KOOPA_B             | Back thrown out of Koopa Klaw |
| 11B  | CAPTURE_KOOPA_AIR          | Caught by Koopa Klaw in air |
| 11C  | CAPTURE_DAMAGE_KOOPA_AIR   | Pummeled in Koopa Klaw in air |
| 11D  | CAPTURE_WAIT_KOOPA_AIR     | Idled in Koopa Klaw in air |
| 11E  | THROWN_KOOPA_AIR_F         | Forward thrown out of Koopa Klaw in air |
| 11F  | THROWN_KOOPA_AIR_B         | Back thrown out of Koopa Klaw in air |
| 120  | CAPTURE_KIRBY              | Caught in Kirby's Swallow |
| 121  | CAPTURE_WAIT_KIRBY         | Idled in Swallow |
| 122  | THROWN_KIRBY_STAR          | In star after being spit out by Kirby (without copy) |
| 123  | THROWN_COPY_STAR           | In star after being spit out by Kirby (with copy) |
| 124  | THROWN_KIRBY               | After being in the star? |
| 125  | ~~BARREL_WAIT~~            | Idling while holding barrel? |
| 126  | BURY                       | Beginning of being buried? |
| 127  | BURY_WAIT                  | Buried? |
| 128  | BURY_JUMP                  | Jumping out of being buried? |
| 129  | DAMAGE_SONG                | Caught in Jigglypuff's Sing |
| 12A  | DAMAGE_SONG_WAIT           | Loop of being caught by Sing |
| 12B  | DAMAGE_SONG_RV             | Get up after Sing wears off |
| 12C  | DAMAGE_BIND                | Dazed after Mewtwo's Disable? |
| 12D  | CAPTURE_MEWTWO             | Caught in Disable? |
| 12E  | CAPTURE_MEWTWO_AIR         | Caught in Disable in air? |
| 12F  | THROWN_MEWTWO              | Thrown by Mewtwo? |
| 130  | THROWN_MEWTWO_AIR          | Thrown in the air (?) by Mewtwo? |
| 131  | ~~WARP_STAR_JUMP~~         | Using warp star item |
| 132  | ~~WARP_STAR_FALL~~         | Falling after using warp star item |
| 133  | ~~HAMMER_WAIT~~            | Idling with hammer item |
| 134  | ~~HAMMER_WALK~~            | Walking with hammer item |
| 135  | ~~HAMMER_TURN~~            | Turning with hammer item |
| 136  | ~~HAMMER_KNEE_BEND~~       | Jumpsquat with hammer item |
| 137  | ~~HAMMER_FALL~~            | Falling with hammer item |
| 138  | ~~HAMMER_JUMP~~            | Jumping with hammer item |
| 139  | ~~HAMMER_LANDING~~         | Landing with hammer item |
| 13A  | ~~KINOKO_GIANT_START~~     | Super (giant) mushroom start |
| 13B  | ~~KINOKO_GIANT_START_AIR~~ | Super (giant) mushroom start in air |
| 13C  | ~~KINOKO_GIANT_END~~       | Super (giant) mushroom end |
| 13D  | ~~KINOKO_GIANT_END_AIR~~   | Super (giant) mushroom end in air |
| 13E  | ~~KINOKO_SMALL_START~~     | Poison (small) mushroom start |
| 13F  | ~~KINOKO_SMALL_START_AIR~~ | Poison (small) mushroom start in air |
| 140  | ~~KINOKO_SMALL_END~~       | Poison (small) mushroom end |
| 141  | ~~KINOKO_SMALL_END_AIR~~   | Poison (small) mushroom end in air |
| 142  | ENTRY                      | Warping in at the beginning of a match |
| 143  | ENTRY_START                | Starting to warp in at the beginning of a match |
| 144  | ENTRY_END                  | Ending warp in at the beginning of a match |
| 145  | DAMAGE_ICE                 | Caught in ice? (IC freeze?) |
| 146  | DAMAGE_ICE_JUMP            | Jumping out of caught in ice? |
| 147  | ~~CAPTURE_MASTER_HAND~~    | Grabbed by Master Hand |
| 148  | ~~CAPTURE_DAMAGE_MASTER_HAND~~ | Pummeled by Master Hand |
| 149  | ~~CAPTURE_WAIT_MASTER_HAND~~ | Grab idled by Master Hand |
| 14A  | ~~THROWN_MASTER_HAND~~     | Thrown by Master Hand |
| 14B  | CAPTURE_KIRBY_YOSHI        | Caught by Kirby's copied ability Egg Lay |
| 14C  | KIRBY_YOSHI_EGG            | Within Kirby's copied ability Egg? |
| 14D  | ~~CAPTURE_REDEAD~~         | Grabbed while dead in stamina mode? |
| 14E  | ~~CAPTURE_LIKE_LIKE~~      | ? |
| 14F  | DOWN_REFLECT               | Reflection state of Shine? |
| 150  | ~~CAPTURE_CRAZY_HAND~~     | Grabbed by Crazy Hand |
| 151  | ~~CAPTURE_DAMAGE_CRAZY_HAND~~ | Pummeled by Crazy Hand |
| 152  | ~~CAPTURE_WAIT_CRAZY_HAND~~ | Grab idled by Crazy Hand |
| 153  | ~~THROWN_CRAZY_HAND~~      | Thrown by Crazy Hand |
| 154  | ~~BARREL_CANNON_WAIT~~     | Waiting inside barrel cannon |

## Character-specific

### Bowser

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | FIRE_BREATH_GROUND_STARTUP | Beginning of Fire Breath on the ground (neutral-b) |
| 156  | FIRE_BREATH_GROUND_LOOP    | Looping Fire Breath on the ground |
| 157  | FIRE_BREATH_GROUND_END     | Ending of Fire Breath on the ground |
| 158  | FIRE_BREATH_AIR_STARTUP    | Beginning of Fire Breath while in the air |
| 159  | FIRE_BREATH_AIR_LOOP       | Looping Fire Breath while in the air |
| 15A  | FIRE_BREATH_AIR_END        | Ending of Fire Breath while in the air |
| 15B  | KOOPA_KLAW_GROUND          | Koopa Klaw startup on the ground (side-b) |
| 15C  | KOOPA_KLAW_GROUND_GRAB     | Grabbing opponent with Koopa Klaw on the ground |
| 15D  | KOOPA_KLAW_GROUND_PUMMEL   | Pummeling with Koopa Klaw on the ground |
| 15E  | KOOPA_KLAW_GROUND_WAIT     | Idling with Koopa Klaw on the ground |
| 15F  | KOOPA_KLAW_GROUND_THROW_F  | Forward throw with Koopa Klaw on the ground |
| 160  | KOOPA_KLAW_GROUND_THROW_B  | Backward throw with Koopa Klaw on the ground |
| 161  | KOOPA_KLAW_AIR             | Koopa Klaw startup while in the air |
| 162  | KOOPA_KLAW_AIR_GRAB        | Grabbing opponent with Koopa Klaw while in the air |
| 163  | KOOPA_KLAW_AIR_PUMMEL      | Pummeling with Koopa Klaw while in the air |
| 164  | KOOPA_KLAW_AIR_WAIT        | Idling with Koopa Klaw while in the air |
| 165  | KOOPA_KLAW_AIR_THROW_F     | Forward throw with Koopa Klaw while in the air |
| 166  | KOOPA_KLAW_AIR_THROW_B     | Backward throw with Koopa Klaw while in the air |
| 167  | WHIRLING_FORTRESS_GROUND   | Whirling Fortress on the ground (up-b) |
| 168  | WHIRLING_FORTRESS_AIR      | Whirling Fortress while in the air |
| 169  | BOMB_GROUND_BEGIN          | Beginning of Bowser Bomb on the ground (down-b) |
| 16A  | BOMB_AIR                   | Bowser Bomb while in the air |
| 16B  | BOMB_LAND                  | Landing after Bowser Bomb |

### Captain Falcon

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 15B  | FALCON_PUNCH_GROUND        | Falcon Punch startup on the ground (neutral-b) |
| 15C  | FALCON_PUNCH_AIR           | Falcon Punch startup while in the air |
| 15D  | RAPTOR_BOOST_GROUND        | Raptor Boost startup on the ground (side-b) |
| 15E  | RAPTOR_BOOST_GROUND_HIT    | Connecting with Raptor Boost on the ground |
| 15F  | RAPTOR_BOOST_AIR           | Raptor Boost startup while in the air |
| 160  | RAPTOR_BOOST_AIR_HIT       | Connecting with Raptor Boost while in the air |
| 161  | FALCON_DIVE_GROUND         | Falcon Dive startup on the ground (up-b) |
| 162  | FALCON_DIVE_AIR            | Falcon Dive startup while in the air |
| 163  | FALCON_DIVE_CATCH          | Grabbing opponent with Falcon Dive |
| 164  | FALCON_DIVE_ENDING         | Ending of Falcon Dive |
| 165  | FALCON_KICK_GROUND         | Falcon Kick startup on the ground (down-b) |
| 166  | FALCON_KICK_GROUND_ENDING_ON_GROUND | Ending of Falcon Kick on the ground |
| 167  | FALCON_KICK_AIR            | Falcon Kick startup while in the air |
| 168  | FALCON_KICK_AIR_ENDING_ON_GROUND | Falcon Kick landing on the ground after being used in the air |
| 169  | FALCON_KICK_AIR_ENDING_IN_AIR | Ending of Falcon Kick while in the air |
| 16A  | FALCON_KICK_GROUND_ENDING_IN_AIR | Falcon Kick ending in the air after being used on the ground |
| 16B  | FALCON_KICK_HIT_WALL       | Hitting a wall with Falcon Kick |

### Donkey Kong (DK)

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 15F  | KONG_KARRY_WAIT            | Cargo Carry idling |
| 160  | KONG_KARRY_WALK_SLOW       | Cargo Carry slow walking |
| 161  | KONG_KARRY_WALK_MIDDLE     | Cargo Carry medium speed walking |
| 162  | KONG_KARRY_WALK_FAST       | Cargo Carry fast walking |
| 163  | KONG_KARRY_TURN            | Turning while in Cargo Carry |
| 164  | KONG_KARRY_JUMP_SQUAT      | Jump squat while in Cargo Carry |
| 165  | KONG_KARRY_FALL            | Falling while in Cargo Carry |
| 166  | KONG_KARRY_JUMP            | Jumping while in Cargo Carry |
| 167  | KONG_KARRY_LANDING         | Landing while in Cargo Carry |
| 169  | KONG_KARRY_GROUND_THROW_FORWARD | Cargo Carry forward throw on the ground |
| 16A  | KONG_KARRY_GROUND_THROW_BACKWARD | Cargo Carry backward throw on the ground |
| 16B  | KONG_KARRY_GROUND_THROW_UP | Cargo Carry upward throw on the ground |
| 16C  | KONG_KARRY_GROUND_THROW_DOWN | Cargo Carry downward throw on the ground |
| 16D  | KONG_KARRY_AIR_THROW_FORWARD | Cargo Carry forward throw while in the air |
| 16E  | KONG_KARRY_AIR_THROW_BACKWARD | Cargo Carry backward throw while in the air |
| 16F  | KONG_KARRY_AIR_THROW_UP    | Cargo Carry upward throw while in the air |
| 170  | KONG_KARRY_AIR_THROW_DOWN  | Cargo Carry downward throw while in the air |
| 171  | GIANT_PUNCH_GROUND_CHARGE_STARTUP | Giant Punch charge startup on the ground (neutral-b) |
| 172  | GIANT_PUNCH_GROUND_CHARGE_LOOP | Giant Punch charge looping on the ground |
| 173  | GIANT_PUNCH_GROUND_CHARGE_STOP | Ending Giant Punch charge on the ground |
| 174  | GIANT_PUNCH_GROUND_EARLY_PUNCH | Releasing Giant Punch early on the ground |
| 175  | GIANT_PUNCH_GROUND_FULL_CHARGE_PUNCH | Fully charged Giant Punch on the ground |
| 176  | GIANT_PUNCH_AIR_CHARGE_STARTUP | Giant Punch charge startup while in the air |
| 177  | GIANT_PUNCH_AIR_CHARGE_LOOP | Giant Punch charge looping while in the air |
| 178  | GIANT_PUNCH_AIR_CHARGE_STOP | Ending Giant Punch charge while in the air |
| 179  | GIANT_PUNCH_AIR_EARLY_PUNCH | Releasing Giant Punch early while in the air |
| 17A  | GIANT_PUNCH_AIR_FULL_CHARGE_PUNCH | Fully charged Giant Punch while in the air |
| 17B  | HEADBUTT_GROUND            | Headbutt on the ground (side-b) |
| 17C  | HEADBUTT_AIR               | Headbutt while in the air |
| 17D  | SPINNING_KONG_GROUND       | Spinning Kong on the ground (up-b) |
| 17E  | SPINNING_KONG_AIR          | Spinning Kong while in the air |
| 17F  | HAND_SLAP_STARTUP          | Hand Slap startup on the ground (down-b) |
| 180  | HAND_SLAP_LOOP             | Hand Slap looping on the ground |
| 181  | HAND_SLAP_END              | Ending Hand Slap on the ground |

### Dr. Mario

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | TAUNT_R                    | Dr. Mario's right-side taunt |
| 157  | MEGAVITAMIN_GROUND         | Firing a Megavitamin on the ground (neutral-b) |
| 158  | MEGAVITAMIN_AIR            | Firing a Megavitamin while in the air |
| 159  | SUPER_SHEET_GROUND         | Using Super Sheet on the ground (side-b) |
| 15A  | SUPER_SHEET_AIR            | Using Super Sheet while in the air |
| 15B  | SUPER_JUMP_PUNCH_GROUND    | Super Jump Punch on the ground (up-b) |
| 15C  | SUPER_JUMP_PUNCH_AIR       | Super Jump Punch while in the air |
| 15D  | TORNADO_GROUND             | Performing the Tornado attack on the ground (down-b) |
| 15E  | TORNADO_AIR                | Performing the Tornado attack while in the air |

### Falco

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | BLASTER_GROUND_STARTUP     | Beginning animation of using Laser on the ground (neutral-n) |
| 156  | BLASTER_GROUND_LOOP        | Looping animation of firing Laser on the ground |
| 157  | BLASTER_GROUND_END         | Ending animation of using Laser on the ground |
| 158  | BLASTER_AIR_STARTUP        | Beginning animation of using Laser while in the air |
| 159  | BLASTER_AIR_LOOP           | Looping animation of firing Laser while in the air |
| 15A  | BLASTER_AIR_END            | Ending animation of using Laser while in the air |
| 15B  | PHANTASM_GROUND_STARTUP    | Beginning animation of Phantasm on the ground (side-b) |
| 15C  | PHANTASM_GROUND            | Performing Phantasm on the ground |
| 15D  | PHANTASM_GROUND_END        | Ending animation of Phantasm on the ground |
| 15E  | PHANTASM_STARTUP_AIR       | Beginning animation of Phantasm while in the air |
| 15F  | PHANTASM_AIR               | Performing Phantasm while in the air |
| 160  | PHANTASM_AIR_END           | Ending animation of Phantasm while in the air |
| 161  | FIRE_BIRD_GROUND_STARTUP   | Beginning animation of Fire Bird on the ground (up-b) |
| 162  | FIRE_BIRD_AIR_STARTUP      | Beginning animation of Fire Bird while in the air |
| 163  | FIRE_BIRD_GROUND           | Performing Fire Bird on the ground |
| 164  | FIRE_BIRD_AIR              | Performing Fire Bird while in the air |
| 165  | FIRE_BIRD_GROUND_END       | Ending animation of Fire Bird on the ground |
| 166  | FIRE_BIRD_AIR_END          | Ending animation of Fire Bird while in the air |
| 167  | FIRE_BIRD_BOUNCE_END       | Animation for Fire Bird when it bounces off an obstacle |
| 168  | REFLECTOR_GROUND_STARTUP   | Beginning animation of Reflector on the ground (down-b) |
| 169  | REFLECTOR_GROUND_LOOP      | Looping animation of Reflector on the ground |
| 16A  | REFLECTOR_GROUND_REFLECT   | Reflecting a projectile on the ground with Reflector |
| 16B  | REFLECTOR_GROUND_END       | Ending animation of Reflector on the ground |
| 16C  | REFLECTOR_GROUND_CHANGE_DIRECTION | Changing direction while using Reflector on the ground |
| 16D  | REFLECTOR_AIR_STARTUP      | Beginning animation of Reflector while in the air |
| 16E  | REFLECTOR_AIR_LOOP         | Looping animation of Reflector while in the air |
| 16F  | REFLECTOR_AIR_REFLECT      | Reflecting a projectile while in the air with Reflector |
| 170  | REFLECTOR_AIR_END          | Ending animation of Reflector while in the air |
| 171  | REFLECTOR_AIR_CHANGE_DIRECTION | Changing direction while using Reflector in the air |
| 172  | ~~SMASH_TAUNT_RIGHT_STARTUP~~ | Starting animation for the right-side Smash Taunt (Corneria easter egg taunt) |
| 173  | ~~SMASH_TAUNT_LEFT_STARTUP~~ | Starting animation for the left-side Smash Taunt |
| 174  | ~~SMASH_TAUNT_RIGHT_RISE~~ | Rising animation for the right-side Smash Taunt |
| 175  | ~~SMASH_TAUNT_LEFT_RISE~~  | Rising animation for the left-side Smash Taunt |
| 176  | ~~SMASH_TAUNT_RIGHT_FINISH~~ | Finishing animation for the right-side Smash Taunt |
| 177  | ~~SMASH_TAUNT_LEFT_FINISH~~ | Finishing animation for the left-side Smash Taunt |

### Fox

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | BLASTER_GROUND_STARTUP     | Beginning animation of using Laser on the ground (neutral-n) |
| 156  | BLASTER_GROUND_LOOP        | Looping animation of firing Laser on the ground |
| 157  | BLASTER_GROUND_END         | Ending animation of using Laser on the ground |
| 158  | BLASTER_AIR_STARTUP        | Beginning animation of using Laser while in the air |
| 159  | BLASTER_AIR_LOOP           | Looping animation of firing Laser while in the air |
| 15A  | BLASTER_AIR_END            | Ending animation of using Laser while in the air |
| 15B  | ILLUSION_GROUND_STARTUP    | Beginning animation of Illusion on the ground (side-b) |
| 15C  | ILLUSION_GROUND            | Performing Illusion on the ground |
| 15D  | ILLUSION_GROUND_END        | Ending animation of Illusion on the ground |
| 15E  | ILLUSION_STARTUP_AIR       | Beginning animation of Illusion while in the air |
| 15F  | ILLUSION_AIR               | Performing Illusion while in the air |
| 160  | ILLUSION_AIR_END           | Ending animation of Illusion while in the air |
| 161  | FIRE_FOX_GROUND_STARTUP    | Beginning animation of Fire Fox on the ground (up-b) |
| 162  | FIRE_FOX_AIR_STARTUP       | Beginning animation of Fire Fox while in the air |
| 163  | FIRE_FOX_GROUND            | Performing Fire Fox on the ground |
| 164  | FIRE_FOX_AIR               | Performing Fire Fox while in the air |
| 165  | FIRE_FOX_GROUND_END        | Ending animation of Fire Fox on the ground |
| 166  | FIRE_FOX_AIR_END           | Ending animation of Fire Fox while in the air |
| 167  | FIRE_FOX_BOUNCE_END        | Animation for Fire Fox when it bounces off an obstacle |
| 168  | REFLECTOR_GROUND_STARTUP   | Beginning animation of Reflector on the ground (down-b) |
| 169  | REFLECTOR_GROUND_LOOP      | Looping animation of Reflector on the ground |
| 16A  | REFLECTOR_GROUND_REFLECT   | Reflecting a projectile on the ground with Reflector |
| 16B  | REFLECTOR_GROUND_END       | Ending animation of Reflector on the ground |
| 16C  | REFLECTOR_GROUND_CHANGE_DIRECTION | Changing direction while using Reflector on the ground |
| 16D  | REFLECTOR_AIR_STARTUP      | Beginning animation of Reflector while in the air |
| 16E  | REFLECTOR_AIR_LOOP         | Looping animation of Reflector while in the air |
| 16F  | REFLECTOR_AIR_REFLECT      | Reflecting a projectile while in the air with Reflector |
| 170  | REFLECTOR_AIR_END          | Ending animation of Reflector while in the air |
| 171  | REFLECTOR_AIR_CHANGE_DIRECTION | Changing direction while using Reflector in the air |
| 172  | ~~SMASH_TAUNT_RIGHT_STARTUP~~ | Starting animation for the right-side Smash Taunt (Corneria easter egg taunt) |
| 173  | ~~SMASH_TAUNT_LEFT_STARTUP~~ | Starting animation for the left-side Smash Taunt |
| 174  | ~~SMASH_TAUNT_RIGHT_RISE~~ | Rising animation for the right-side Smash Taunt |
| 175  | ~~SMASH_TAUNT_LEFT_RISE~~  | Rising animation for the left-side Smash Taunt |
| 176  | ~~SMASH_TAUNT_RIGHT_FINISH~~ | Finishing animation for the right-side Smash Taunt |
| 177  | ~~SMASH_TAUNT_LEFT_FINISH~~ | Finishing animation for the left-side Smash Taunt |

### Game and Watch

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | JAB                        | Basic jab attack |
| 156  | RAPID_JABS_START           | Start of rapid jabs |
| 157  | RAPID_JABS_LOOP            | Loop of rapid jabs |
| 158  | RAPID_JABS_END             | End of rapid jabs |
| 159  | DOWN_TILT                  | Down tilt |
| 15A  | SIDE_SMASH                 | Side smash |
| 15B  | NAIR                       | Special Nair (side note: if you've ever wondered why G&W can't L-cancel his aerials, this is why. They're counted as special moves because they have special states and hence aren't covered by the L-cancel code, which only targets the general aerial landings as covered in 046-04A) |
| 15C  | BAIR                       | Special Bair |
| 15D  | UAIR                       | Special Uair |
| 15E  | NAIR_LANDING               | Landing during special Nair |
| 15F  | BAIR_LANDING               | Landing during special Bair |
| 160  | UAIR_LANDING               | Landing during special Uair |
| 161  | CHEF_GROUND                | Chef special move on the ground (neutral-b) |
| 162  | CHEF_AIR                   | Chef special move while in the air |
| 163  | JUDGMENT_1_GROUND          | Using Judgment with a result of 1 on the ground (side-b) |
| 164  | JUDGMENT_2_GROUND          | Using Judgment with a result of 2 on the ground |
| 165  | JUDGMENT_3_GROUND          | Using Judgment with a result of 3 on the ground |
| 166  | JUDGMENT_4_GROUND          | Using Judgment with a result of 4 on the ground |
| 167  | JUDGMENT_5_GROUND          | Using Judgment with a result of 5 on the ground |
| 168  | JUDGMENT_6_GROUND          | Using Judgment with a result of 6 on the ground |
| 169  | JUDGMENT_7_GROUND          | Using Judgment with a result of 7 on the ground |
| 16A  | JUDGMENT_8_GROUND          | Using Judgment with a result of 8 on the ground |
| 16B  | JUDGMENT_9_GROUND          | Using Judgment with a result of 9 on the ground |
| 16C  | JUDGMENT_1_AIR             | Using Judgment with a result of 1 while in the air |
| 16D  | JUDGMENT_2_AIR             | Using Judgment with a result of 2 while in the air |
| 16E  | JUDGMENT_3_AIR             | Using Judgment with a result of 3 while in the air |
| 16F  | JUDGMENT_4_AIR             | Using Judgment with a result of 4 while in the air |
| 170  | JUDGMENT_5_AIR             | Using Judgment with a result of 5 while in the air |
| 171  | JUDGMENT_6_AIR             | Using Judgment with a result of 6 while in the air |
| 172  | JUDGMENT_7_AIR             | Using Judgment with a result of 7 while in the air |
| 173  | JUDGMENT_8_AIR             | Using Judgment with a result of 8 while in the air |
| 174  | JUDGMENT_9_AIR             | Using Judgment with a result of 9 while in the air |
| 175  | FIRE_GROUND                | Fire special move on the ground (up-b) |
| 176  | FIRE_AIR                   | Fire special move while in the air |
| 177  | OIL_PANIC_GROUND           | Using Oil Panic on the ground (down-b) |
| 178  | OIL_PANIC_GROUND_ABSORB    | Absorbing a projectile with Oil Panic on the ground |
| 179  | OIL_PANIC_GROUND_SPILL     | Spilling absorbed projectiles with Oil Panic on the ground |
| 17A  | OIL_PANIC_AIR              | Using Oil Panic while in the air |
| 17B  | OIL_PANIC_AIR_ABSORB       | Absorbing a projectile with Oil Panic while in the air |
| 17C  | OIL_PANIC_AIR_SPILL        | Spilling absorbed projectiles with Oil Panic while in the air |

### Ganondorf

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 15B  | WARLOCK_PUNCH_GROUND       | Warlock Punch on ground (neutral-b) |
| 15C  | WARLOCK_PUNCH_AIR          | Warlock Punch in air |
| 15D  | GERUDO_DRAGON_GROUND       | Gerudo Dragon startup on ground (side-b) |
| 15E  | GERUDO_DRAGON_GROUND_HIT   | Gerudo Dragon connects on ground |
| 15F  | GERUDO_DRAGON_AIR          | Gerudo Dragon startup in air |
| 160  | GERUDO_DRAGON_AIR_HIT      | Gerudo Dragon connects in air |
| 161  | DARK_DIVE_GROUND           | Dark Dive startup on ground (up-b) |
| 162  | DARK_DIVE_AIR              | Dark Dive startup in air |
| 163  | DARK_DIVE_CATCH            | Dark Dive catches opponent |
| 164  | DARK_DIVE_ENDING           | Dark Dive ends |
| 165  | WIZARDS_FOOT_GROUND        | Wizard's Foot startup on ground (down-b) |
| 166  | WIZARDS_FOOT_GROUND_ENDING_ON_GROUND | Wizard's Foot ends on ground |
| 167  | WIZARDS_FOOT_AIR           | Wizard's Foot startup in air |
| 168  | WIZARDS_FOOT_AIR_ENDING_ON_GROUND | Wizard's Foot ends on ground from air |
| 169  | WIZARDS_FOOT_AIR_ENDING_IN_AIR | Wizard's Foot ends in air |
| 16A  | WIZARDS_FOOT_GROUND_ENDING_IN_AIR | Wizard's Foot ends in air from ground |
| 16B  | WIZARDS_FOOT_HIT_WALL      | Wizard's Foot collides with wall |

### Jigglypuff

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | JUMP_2                     | Second jump in the air |
| 156  | JUMP_3                     | Third jump in the air |
| 157  | JUMP_4                     | Fourth jump in the air |
| 158  | JUMP_5                     | Fifth jump in the air |
| 159  | JUMP_6                     | Sixth jump in the air |
| 15A  | ROLLOUT_GROUND_START_CHARGE_RIGHT | Rollout start charge on ground facing right (side-b) |
| 15B  | ROLLOUT_GROUND_START_CHARGE_LEFT | Rollout start charge on ground facing left |
| 15C  | ROLLOUT_GROUND_CHARGE_LOOP | Rollout charge loop on ground |
| 15D  | ROLLOUT_GROUND_FULLY_CHARGED | Rollout fully charged on ground |
| 15E  | ROLLOUT_GROUND_CHARGE_RELEASE | Rollout charge release on ground |
| 15F  | ROLLOUT_GROUND_START_TURN  | Rollout start turning on ground |
| 160  | ROLLOUT_GROUND_END_RIGHT   | Rollout end on ground facing right |
| 161  | ROLLOUT_GROUND_END_LEFT    | Rollout end on ground facing left |
| 162  | ROLLOUT_AIR_START_CHARGE_RIGHT | Rollout start charge in air facing right |
| 163  | ROLLOUT_AIR_START_CHARGE_LEFT | Rollout start charge in air facing left |
| 164  | ROLLOUT_AIR_CHARGE_LOOP    | Rollout charge loop in air |
| 165  | ROLLOUT_AIR_FULLY_CHARGED  | Rollout fully charged in air |
| 166  | ROLLOUT_AIR_CHARGE_RELEASE | Rollout charge release in air |
| 168  | ROLLOUT_AIR_END_RIGHT      | Rollout end in air facing right |
| 169  | ROLLOUT_AIR_END_LEFT       | Rollout end in air facing left |
| 16A  | ROLLOUT_HIT                | Rollout connects with opponent |
| 16B  | POUND_GROUND               | Pound on ground (side-b) |
| 16C  | POUND_AIR                  | Pound in air |
| 16D  | SING_GROUND_LEFT           | Sing on ground facing left (up-b) |
| 16E  | SING_AIR_LEFT              | Sing in air facing left |
| 16F  | SING_GROUND_RIGHT          | Sing on ground facing right |
| 170  | SING_AIR_RIGHT             | Sing in air facing right |
| 171  | REST_GROUND_LEFT           | Rest on ground facing left (down-b) |
| 172  | REST_AIR_LEFT              | Rest in air facing left |
| 173  | REST_GROUND_RIGHT          | Rest on ground facing right |
| 174  | REST_AIR_RIGHT             | Rest in air facing right |

### Kirby

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | JUMP_2                     | Second jump while in the air |
| 156  | JUMP_3                     | Third jump while in the air |
| 157  | JUMP_4                     | Fourth jump while in the air |
| 158  | JUMP_5                     | Fifth jump while in the air |
| 159  | JUMP_6                     | Sixth jump while in the air |
| 15A  | JUMP_2_WITH_HAT            | Second jump with copied ability |
| 15B  | JUMP_3_WITH_HAT            | Third jump with copied ability |
| 15C  | JUMP_4_WITH_HAT            | Fourth jump with copied ability |
| 15D  | JUMP_5_WITH_HAT            | Fifth jump with copied ability |
| 15E  | JUMP_6_WITH_HAT            | Sixth jump with copied ability |
| 15F  | DASH_ATTACK_GROUND         | Dash attack startup on ground |
| 160  | DASH_ATTACK_AIR            | Dash attack while in the air |
| 161  | SWALLOW_GROUND_STARTUP     | Inhale startup (neutral-b) |
| 162  | SWALLOW_GROUND_LOOP        | Looping inhale on ground |
| 163  | SWALLOW_GROUND_END         | End of inhale on ground |
| 164  | SWALLOW_GROUND_CAPTURE     | Capturing opponent in inhale |
| 166  | SWALLOW_GROUND_CAPTURED    | Holding opponent in inhale |
| 167  | SWALLOW_GROUND_CAPTURE_WAIT | Waiting to act after inhale |
| 168  | SWALLOW_CAPTURE_WALK_SLOW  | Slow walk with captured opponent |
| 169  | SWALLOW_CAPTURE_WALK_MIDDLE | Moderate walk with captured opponent |
| 16A  | SWALLOW_CAPTURE_WALK_FAST  | Fast walk with captured opponent |
| 16B  | SWALLOW_GROUND_CAPTURE_TURN | Turning with captured opponent |
| 16C  | SWALLOW_CAPTURE_JUMP_SQUAT | Jump squat with captured opponent |
| 16D  | SWALLOW_CAPTURE_JUMP       | Jumping with captured opponent |
| 16E  | SWALLOW_CAPTURE_LANDING    | Landing with captured opponent |
| 16F  | SWALLOW_GROUND_DIGEST      | Digesting opponent |
| 171  | SWALLOW_GROUND_SPIT        | Spitting out opponent |
| 173  | SWALLOW_AIR_STARTUP        | Inhale startup while in the air |
| 174  | SWALLOW_AIR_LOOP           | Looping inhale in the air |
| 175  | SWALLOW_AIR_END            | End of inhale while in the air |
| 176  | SWALLOW_AIR_CAPTURE        | Capturing opponent in inhale while in the air |
| 178  | SWALLOW_AIR_CAPTURED       | Holding opponent in inhale while in the air |
| 179  | SWALLOW_AIR_CAPTURE_WAIT   | Waiting to act after inhale while in the air |
| 17A  | SWALLOW_AIR_DIGEST         | Digesting opponent while in the air |
| 17C  | SWALLOW_AIR_SPIT           | Spitting out opponent while in the air |
| 17E  | SWALLOW_AIR_CAPTURE_TURN   | Turning with captured opponent while in the air |
| 17F  | HAMMER_GROUND              | Using hammer attack (side-b) on ground |
| 180  | HAMMER_AIR                 | Using hammer attack (side-b) while in the air |
| 181  | FINAL_CUTTER_GROUND_STARTUP | Final Cutter startup (up-b) on ground |
| 184  | FINAL_CUTTER_GROUND_END    | End of Final Cutter on ground |
| 185  | FINAL_CUTTER_AIR_STARTUP   | Final Cutter startup (up-b) while in the air |
| 186  | FINAL_CUTTER_AIR_APEX      | Apex of Final Cutter in the air |
| 187  | FINAL_CUTTER_SWORD_DESCENT | Sword descent during Final Cutter |
| 188  | FINAL_CUTTER_AIR_END       | End of Final Cutter while in the air |
| 189  | STONE_GROUND_STARTUP       | Stone transformation startup (down-b) on ground |
| 18A  | STONE_GROUND               | Transformed into stone on ground |
| 18B  | STONE_GROUND_END           | End of stone transformation on ground |
| 18C  | STONE_AIR_STARTUP          | Stone transformation startup (down-b) while in the air |
| 18D  | STONE_AIR                  | Transformed into stone while in the air |
| 18E  | STONE_AIR_END              | End of stone transformation while in the air |
| 1C7  | DK_GIANT_PUNCH_GROUND_CHARGE_STARTUP | Kirby starts charging Donkey Kong's Giant Punch on the ground (all special moves from this point are naturally neutral-b) |
| 1C8  | DK_GIANT_PUNCH_GROUND_CHARGE_LOOP | Looping charge of Donkey Kong's Giant Punch on the ground |
| 1C9  | DK_GIANT_PUNCH_GROUND_CHARGE_STOP | Kirby stops charging Giant Punch on the ground |
| 1CA  | DK_GIANT_PUNCH_GROUND_EARLY_PUNCH | Kirby releases a partially charged Giant Punch on the ground |
| 1CB  | DK_GIANT_PUNCH_GROUND_FULL_CHARGE_PUNCH | Kirby releases a fully charged Giant Punch on the ground |
| 1CC  | DK_GIANT_PUNCH_AIR_CHARGE_STARTUP | Kirby starts charging Giant Punch while in the air |
| 1CD  | DK_GIANT_PUNCH_AIR_CHARGE_LOOP | Looping charge of Giant Punch while in the air |
| 1CE  | DK_GIANT_PUNCH_AIR_CHARGE_STOP | Kirby stops charging Giant Punch while in the air |
| 1CF  | DK_GIANT_PUNCH_AIR_EARLY_PUNCH | Kirby releases a partially charged Giant Punch while in the air |
| 1D0  | DK_GIANT_PUNCH_AIR_FULL_CHARGE_PUNCH | Kirby releases a fully charged Giant Punch while in the air |
| 1D1  | ZELDA_NAYRUS_LOVE_GROUND   | Kirby uses Zelda's Nayru's Love move on the ground |
| 1D2  | ZELDA_NAYRUS_LOVE_AIR      | Kirby uses Zelda's Nayru's Love move while in the air |
| 1D3  | SHEIK_NEEDLE_STORM_GROUND_START_CHARGE | Kirby starts charging Sheik's Needle Storm on the ground |
| 1D4  | SHEIK_NEEDLE_STORM_GROUND_CHARGE_LOOP | Looping charge of Sheik's Needle Storm on the ground |
| 1D5  | SHEIK_NEEDLE_STORM_GROUND_END_CHARGE | Kirby ends charging Needle Storm on the ground |
| 1D6  | SHEIK_NEEDLE_STORM_GROUND_FIRE | Kirby fires Sheik's Needle Storm on the ground |
| 1D7  | SHEIK_NEEDLE_STORM_AIR_START_CHARGE | Kirby starts charging Needle Storm while in the air |
| 1D8  | SHEIK_NEEDLE_STORM_AIR_CHARGE_LOOP | Looping charge of Needle Storm while in the air |
| 1D9  | SHEIK_NEEDLE_STORM_AIR_END_CHARGE | Kirby ends charging Needle Storm while in the air |
| 1DA  | SHEIK_NEEDLE_STORM_AIR_FIRE | Kirby fires Sheik's Needle Storm while in the air |
| 1DB  | JIGGLYPUFF_ROLLOUT_GROUND_START_CHARGE_RIGHT | Kirby starts charging Jigglypuff's Rollout move facing right |
| 1DC  | JIGGLYPUFF_ROLLOUT_GROUND_START_CHARGE_LEFT | Kirby starts charging Jigglypuff's Rollout move facing left |
| 1DD  | JIGGLYPUFF_ROLLOUT_GROUND_CHARGE_LOOP | Looping charge of Jigglypuff's Rollout on the ground |
| 1DE  | JIGGLYPUFF_ROLLOUT_GROUND_FULLY_CHARGED | Kirby is fully charged for Jigglypuff's Rollout on the ground |
| 1DF  | JIGGLYPUFF_ROLLOUT_GROUND_CHARGE_RELEASE | Kirby releases a charged Rollout on the ground |
| 1E0  | JIGGLYPUFF_ROLLOUT_GROUND_START_TURN | Kirby starts turning while using Rollout on the ground |
| 1E1  | JIGGLYPUFF_ROLLOUT_GROUND_END_RIGHT | Kirby ends Rollout move facing right |
| 1E2  | JIGGLYPUFF_ROLLOUT_GROUND_END_LEFT | Kirby ends Rollout move facing left |
| 1E3  | JIGGLYPUFF_ROLLOUT_AIR_START_CHARGE_RIGHT | Kirby starts charging Rollout facing right while in the air |
| 1E4  | JIGGLYPUFF_ROLLOUT_AIR_START_CHARGE_LEFT | Kirby starts charging Rollout facing left while in the air |
| 1E5  | JIGGLYPUFF_ROLLOUT_AIR_CHARGE_LOOP | Looping charge of Rollout while in the air |
| 1E6  | JIGGLYPUFF_ROLLOUT_AIR_FULLY_CHARGED | Kirby is fully charged for Rollout while in the air |
| 1E7  | JIGGLYPUFF_ROLLOUT_AIR_CHARGE_RELEASE | Kirby releases a charged Rollout while in the air |
| 1E9  | JIGGLYPUFF_ROLLOUT_AIR_END_RIGHT | Kirby ends Rollout move facing right while in the air |
| 1EA  | JIGGLYPUFF_ROLLOUT_AIR_END_LEFT | Kirby ends Rollout move facing left while in the air |
| 1EB  | JIGGLYPUFF_ROLLOUT_HIT     | Kirby makes contact while using Jigglypuff's Rollout |
| 1EC  | MARTH_SHIELD_BREAKER_GROUND_START_CHARGE | Kirby starts charging Marth's Shield Breaker on the ground |
| 1ED  | MARTH_SHIELD_BREAKER_GROUND_CHARGE_LOOP | Looping charge of Marth's Shield Breaker on the ground |
| 1EE  | MARTH_SHIELD_BREAKER_GROUND_EARLY_RELEASE | Kirby releases an early Shield Breaker on the ground |
| 1EF  | MARTH_SHIELD_BREAKER_GROUND_FULLY_CHARGED | Kirby releases a fully charged Shield Breaker on the ground |
| 1F0  | MARTH_SHIELD_BREAKER_AIR_START_CHARGE | Kirby starts charging Shield Breaker while in the air |
| 1F1  | MARTH_SHIELD_BREAKER_AIR_CHARGE_LOOP | Looping charge of Shield Breaker while in the air |
| 1F2  | MARTH_SHIELD_BREAKER_AIR_EARLY_RELEASE | Kirby releases an early Shield Breaker while in the air |
| 1F3  | MARTH_SHIELD_BREAKER_AIR_FULLY_CHARGED | Kirby releases a fully charged Shield Breaker while in the air |
| 1F4  | MEWTWO_SHADOW_BALL_GROUND_START_CHARGE | Kirby starts charging Mewtwo's Shadow Ball on the ground |
| 1F5  | MEWTWO_SHADOW_BALL_GROUND_CHARGE_LOOP | Looping charge of Mewtwo's Shadow Ball on the ground |
| 1F6  | MEWTWO_SHADOW_BALL_GROUND_FULLY_CHARGED | Kirby is fully charged for Mewtwo's Shadow Ball on the ground
| 1F7  | MEWTWO_SHADOW_BALL_GROUND_END_CHARGE | Kirby ends charging Shadow Ball on the ground |
| 1F8  | MEWTWO_SHADOW_BALL_GROUND_FIRE | Kirby fires Mewtwo's Shadow Ball on the ground |
| 1F9  | MEWTWO_SHADOW_BALL_AIR_START_CHARGE | Kirby starts charging Shadow Ball while in the air |
| 1FA  | MEWTWO_SHADOW_BALL_AIR_CHARGE_LOOP | Looping charge of Shadow Ball while in the air |
| 1FB  | MEWTWO_SHADOW_BALL_AIR_FULLY_CHARGED | Kirby fully charges Shadow Ball while in the air |
| 1FC  | MEWTWO_SHADOW_BALL_AIR_END_CHARGE | Kirby ends charging Shadow Ball while in the air |
| 1FD  | MEWTWO_SHADOW_BALL_AIR_FIRE | Kirby fires Mewtwo's Shadow Ball while in the air |
| 1FE  | GAMEAND_WATCH_OIL_PANIC_GROUND | Kirby performs Game and Watch's Oil Panic on the ground |
| 1FF  | GAMEAND_WATCH_OIL_PANIC_AIR | Kirby performs Oil Panic while in the air |
| 200  | DOC_MEGAVITAMIN_GROUND     | Kirby throws Dr. Mario's Megavitamin on the ground |
| 201  | DOC_MEGAVITAMIN_AIR        | Kirby throws Megavitamin while in the air |
| 202  | YOUNG_LINK_FIRE_BOW_GROUND_CHARGE | Kirby starts charging Young Link's Fire Bow on the ground |
| 203  | YOUNG_LINK_FIRE_BOW_GROUND_FULLY_CHARGED | Kirby fully charges Young Link's Fire Bow on the ground |
| 204  | YOUNG_LINK_FIRE_BOW_GROUND_FIRE | Kirby fires Young Link's Fire Bow on the ground |
| 205  | YOUNG_LINK_FIRE_BOW_AIR_CHARGE | Kirby starts charging Fire Bow while in the air |
| 206  | YOUNG_LINK_FIRE_BOW_AIR_FULLY_CHARGED | Kirby fully charges Fire Bow while in the air |
| 207  | YOUNG_LINK_FIRE_BOW_AIR_FIRE | Kirby fires Young Link's Fire Bow while in the air |
| 208  | FALCO_BLASTER_GROUND_STARTUP | Kirby starts Falco's Laser on the ground |
| 209  | FALCO_BLASTER_GROUND_LOOP  | Looping shots of Falco's Laser on the ground |
| 20A  | FALCO_BLASTER_GROUND_END   | Kirby ends Falco's Laser on the ground |
| 20B  | FALCO_BLASTER_AIR_STARTUP  | Kirby starts Falco's Laser while in the air |
| 20C  | FALCO_BLASTER_AIR_LOOP     | Looping shots of Falco's Laser while in the air |
| 20D  | FALCO_BLASTER_AIR_END      | Kirby ends Falco's Laser while in the air |
| 20E  | PICHU_THUNDER_JOLT_GROUND  | Kirby fires Pichu's Thunder Jolt on the ground |
| 20F  | PICHU_THUNDER_JOLT_AIR     | Kirby fires Thunder Jolt while in the air |
| 210  | GANON_WARLOCK_PUNCH_GROUND | Kirby performs Ganon's Warlock Punch on the ground |
| 211  | GANON_WARLOCK_PUNCH_AIR    | Kirby performs Warlock Punch while in the air |
| 212  | ROY_FLARE_BLADE_GROUND_START_CHARGE | Kirby starts charging Roy's Flare Blade on the ground |
| 213  | ROY_FLARE_BLADE_GROUND_CHARGE_LOOP | Looping charge of Roy's Flare Blade on the ground |
| 214  | ROY_FLARE_BLADE_GROUND_EARLY_RELEASE | Kirby early releases Roy's Flare Blade on the ground |
| 215  | ROY_FLARE_BLADE_GROUND_FULLY_CHARGED | Kirby fully charges Roy's Flare Blade on the ground |
| 216  | ROY_FLARE_BLADE_AIR_START_CHARGE | Kirby starts charging Flare Blade while in the air |
| 217  | ROY_FLARE_BLADE_AIR_CHARGE_LOOP | Looping charge of Flare Blade while in the air |
| 218  | ROY_FLARE_BLADE_AIR_EARLY_RELEASE | Kirby early releases Flare Blade while in the air |
| 219  | ROY_FLARE_BLADE_AIR_FULLY_CHARGED | Kirby fully charges Flare Blade while in the air |

### Link

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | SIDE_SMASH_2               | Second hit of side smash attack |
| 158  | BOW_GROUND_CHARGE          | Charging bow attack on the ground (neutral-b) |
| 159  | BOW_GROUND_FULLY_CHARGED   | Fully charged bow attack on the ground |
| 15A  | BOW_GROUND_FIRE            | Firing bow attack on the ground |
| 15B  | BOW_AIR_CHARGE             | Charging bow attack while in the air |
| 15C  | BOW_AIR_FULLY_CHARGED      | Fully charged bow attack while in the air |
| 15D  | BOW_AIR_FIRE               | Firing bow attack while in the air |
| 15E  | BOOMERANG_GROUND_THROW     | Throwing boomerang on the ground (side-b) |
| 15F  | BOOMERANG_GROUND_CATCH     | Catching boomerang on the ground |
| 160  | BOOMERANG_GROUND_THROW_EMPTY | Throwing boomerang on the ground, empty hand (no boomerang to catch) |
| 161  | BOOMERANG_AIR_THROW        | Throwing boomerang while in the air |
| 162  | BOOMERANG_AIR_CATCH        | Catching boomerang while in the air |
| 163  | BOOMERANG_AIR_THROW_EMPTY  | Throwing boomerang in the air, empty hand (no boomerang to catch) |
| 164  | SPIN_ATTACK_GROUND         | Spin attack on the ground (up-b) |
| 165  | SPIN_ATTACK_AIR            | Spin attack while in the air |
| 166  | BOMB_GROUND                | Pulling out a bomb on the ground (down-b) |
| 167  | BOMB_AIR                   | Pulling out a bomb while in the air |
| 168  | ZAIR                       | Using tether grab in the air (z) |
| 169  | ZAIR_CATCH                 | Catching the ledge with tether grab |

### Luigi

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | FIREBALL_GROUND            | Firing Fireball on the ground (neutral-b) |
| 156  | FIREBALL_AIR               | Firing Fireball while in the air |
| 157  | GREEN_MISSILE_GROUND_STARTUP | Beginning of Green Missile on the ground (side-b) |
| 158  | GREEN_MISSILE_GROUND_CHARGE | Charging Green Missile on the ground |
| 15A  | GREEN_MISSILE_GROUND_LANDING | Landing after using Green Missile on the ground |
| 15B  | GREEN_MISSILE_GROUND_TAKEOFF | Launching Green Missile on the ground |
| 15C  | GREEN_MISSILE_GROUND_TAKEOFF_MISFIRE | Misfiring Green Missile on the ground |
| 15D  | GREEN_MISSILE_AIR_STARTUP  | Beginning of Green Missile while in the air |
| 15E  | GREEN_MISSILE_AIR_CHARGE   | Charging Green Missile while in the air |
| 15F  | GREEN_MISSILE_AIR          | Using Green Missile while in the air |
| 160  | GREEN_MISSILE_AIR_END      | Ending Green Missile while in the air |
| 161  | GREEN_MISSILE_AIR_TAKEOFF  | Launching Green Missile while in the air |
| 162  | GREEN_MISSILE_AIR_TAKEOFF_MISFIRE | Misfiring Green Missile in the air |
| 163  | SUPER_JUMP_PUNCH_GROUND    | Using Super Jump Punch on the ground (up-b) |
| 164  | SUPER_JUMP_PUNCH_AIR       | Using Super Jump Punch while in the air |
| 165  | CYCLONE_GROUND             | Using Cyclone on the ground (down-b) |
| 166  | CYCLONE_AIR                | Using Cyclone while in the air |

### Mario

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 157  | FIREBALL_GROUND            | Firing Fireball on the ground (neutral-b) |
| 158  | FIREBALL_AIR               | Firing Fireball while in the air |
| 159  | CAPE_GROUND                | Using Cape on the ground (side-b) |
| 15A  | CAPE_AIR                   | Using Cape while in the air |
| 15B  | SUPER_JUMP_PUNCH_GROUND    | Using Super Jump Punch on the ground (up-b) |
| 15C  | SUPER_JUMP_PUNCH_AIR       | Using Super Jump Punch while in the air |
| 15D  | TORNADO_GROUND             | Using Tornado on the ground (down-b) |
| 15E  | TORNADO_AIR                | Using Tornado while in the air |

### Marth

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | SHIELD_BREAKER_GROUND_START_CHARGE | Starting charge for Shield Breaker on the ground (neutral-b) |
| 156  | SHIELD_BREAKER_GROUND_CHARGE_LOOP | Looping Shield Breaker charging on the ground |
| 157  | SHIELD_BREAKER_GROUND_EARLY_RELEASE | Early release of Shield Breaker on the ground |
| 158  | SHIELD_BREAKER_GROUND_FULLY_CHARGED | Fully charged Shield Breaker on the ground |
| 159  | SHIELD_BREAKER_AIR_START_CHARGE | Starting charge for Shield Breaker in the air |
| 15A  | SHIELD_BREAKER_AIR_CHARGE_LOOP | Looping Shield Breaker charging in the air |
| 15B  | SHIELD_BREAKER_AIR_EARLY_RELEASE | Early release of Shield Breaker in the air |
| 15C  | SHIELD_BREAKER_AIR_FULLY_CHARGED | Fully charged Shield Breaker in the air |
| 15D  | DANCING_BLADE_1_GROUND     | First hit of Dancing Blade on the ground (side-b, then any direction) |
| 15E  | DANCING_BLADE_2_UP_GROUND  | Second upward hit of Dancing Blade on the ground (side-b, then up) |
| 15F  | DANCING_BLADE_2_SIDE_GROUND | Second sideways hit of Dancing Blade on the ground (side-b, then side) |
| 160  | DANCING_BLADE_3_UP_GROUND  | Third upward hit of Dancing Blade on the ground (side-b, then up, up) |
| 161  | DANCING_BLADE_3_SIDE_GROUND | Third sideways hit of Dancing Blade on the ground (side-b, then side, side) |
| 162  | DANCING_BLADE_3_DOWN_GROUND | Third downward hit of Dancing Blade on the ground (side-b, then down) |
| 163  | DANCING_BLADE_4_UP_GROUND  | Fourth upward hit of Dancing Blade on the ground (side-b, then up, up, up) |
| 164  | DANCING_BLADE_4_SIDE_GROUND | Fourth sideways hit of Dancing Blade on the ground (side-b, then side, side, side) |
| 165  | DANCING_BLADE_4_DOWN_GROUND | Fourth downward hit of Dancing Blade on the ground (side-b, then down, down) |
| 166  | DANCING_BLADE_1_AIR        | First hit of Dancing Blade in the air |
| 167  | DANCING_BLADE_2_UP_AIR     | Second upward hit of Dancing Blade in the air |
| 168  | DANCING_BLADE_2_SIDE_AIR   | Second sideways hit of Dancing Blade in the air |
| 169  | DANCING_BLADE_3_UP_AIR     | Third upward hit of Dancing Blade in the air |
| 16A  | DANCING_BLADE_3_SIDE_AIR   | Third sideways hit of Dancing Blade in the air |
| 16B  | DANCING_BLADE_3_DOWN_AIR   | Third downward hit of Dancing Blade in the air |
| 16C  | DANCING_BLADE_4_UP_AIR     | Fourth upward hit of Dancing Blade in the air |
| 16D  | DANCING_BLADE_4_SIDE_AIR   | Fourth sideways hit of Dancing Blade in the air |
| 16E  | DANCING_BLADE_4_DOWN_AIR   | Fourth downward hit of Dancing Blade in the air |
| 16F  | DOLPHIN_SLASH_GROUND       | Using Dolphin Slash on the ground (up-b) |
| 170  | DOLPHIN_SLASH_AIR          | Using Dolphin Slash while in the air |
| 171  | COUNTER_GROUND             | Using Counter on the ground (down-b) |
| 172  | COUNTER_GROUND_HIT         | Successful Counter hit on the ground |
| 173  | COUNTER_AIR                | Using Counter while in the air |
| 174  | COUNTER_AIR_HIT            | Successful Counter hit in the air |

### Mewtwo

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | SHADOW_BALL_GROUND_START_CHARGE | Starting the charge for Shadow Ball on the ground (neutral-b) |
| 156  | SHADOW_BALL_GROUND_CHARGE_LOOP | Looping animation for charging Shadow Ball on the ground |
| 157  | SHADOW_BALL_GROUND_FULLY_CHARGED | Fully charged Shadow Ball on the ground |
| 158  | SHADOW_BALL_GROUND_END_CHARGE | Ending the charge animation for Shadow Ball on the ground |
| 159  | SHADOW_BALL_GROUND_FIRE    | Firing the Shadow Ball on the ground |
| 15A  | SHADOW_BALL_AIR_START_CHARGE | Starting the charge for Shadow Ball in the air |
| 15B  | SHADOW_BALL_AIR_CHARGE_LOOP | Looping animation for charging Shadow Ball in the air |
| 15C  | SHADOW_BALL_AIR_FULLY_CHARGED | Fully charged Shadow Ball in the air |
| 15D  | SHADOW_BALL_AIR_END_CHARGE | Ending the charge animation for Shadow Ball in the air |
| 15E  | SHADOW_BALL_AIR_FIRE       | Firing the Shadow Ball in the air |
| 15F  | CONFUSION_GROUND           | Using Confusion on the ground (side-b) |
| 160  | CONFUSION_AIR              | Using Confusion while in the air |
| 161  | TELEPORT_GROUND_STARTUP    | Startup animation for Teleport on the ground (up-b) |
| 162  | TELEPORT_GROUND_DISAPPEAR  | Disappearing animation for Teleport on the ground |
| 163  | TELEPORT_GROUND_REAPPEAR   | Reappearing animation for Teleport on the ground |
| 164  | TELEPORT_AIR_STARTUP       | Startup animation for Teleport in the air |
| 165  | TELEPORT_AIR_DISAPPEAR     | Disappearing animation for Teleport in the air |
| 166  | TELEPORT_AIR_REAPPEAR      | Reappearing animation for Teleport in the air |
| 167  | DISABLE_GROUND             | Using Disable on the ground (down-b) |
| 168  | DISABLE_AIR                | Using Disable while in the air |

### Nana

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | ICE_SHOT_GROUND            | Firing an ice block while on the ground (side-b) |
| 156  | ICE_SHOT_AIR               | Firing an ice block while in the air |
| 165  | BLIZZARD_GROUND            | Using Blizzard on the ground (down-b) |
| 166  | BLIZZARD_AIR               | Using Blizzard while in the air |
| 167  | SQUALL_HAMMER_GROUND_TOGETHER | Using Squall Hammer on the ground while both Nana and Popo are together (side-b) |
| 168  | SQUALL_HAMMER_AIR_TOGETHER | Using Squall Hammer in the air while both Nana and Popo are together |
| 169  | BELAY_CATAPULT_STARTUP     | Startup for Belay catapult (up-b) |
| 16A  | BELAY_GROUND_CATAPULT_END  | Ending for Belay catapult on the ground |
| 16D  | BELAY_CATAPULTING          | Catapulting for Belay (up-b) |

### Ness

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | SIDE_SMASH                 | Charging the bat swing (forward smash) |
| 156  | UP_SMASH                   | Charging yo-yo upward (up smash) |
| 157  | UP_SMASH_CHARGE            | Charging up smash yo-yo |
| 158  | UP_SMASH_CHARGED           | Fully charged up smash yo-yo |
| 159  | DOWN_SMASH                 | Charging yo-yo downward (down smash) |
| 15A  | DOWN_SMASH_CHARGE          | Charging down smash yo-yo |
| 15B  | DOWN_SMASH_CHARGED         | Fully charged down smash yo-yo |
| 15C  | PK_FLASH_GROUND_STARTUP    | Beginning of PK Flash on the ground (neutral-b) |
| 15D  | PK_FLASH_GROUND_CHARGE     | Charging PK Flash on the ground |
| 15E  | PK_FLASH_GROUND_EXPLODE    | PK Flash explosion on the ground |
| 15F  | PK_FLASH_GROUND_END        | End of PK Flash on the ground |
| 160  | PK_FLASH_AIR_STARTUP       | Beginning of PK Flash in the air |
| 161  | PK_FLASH_AIR_CHARGE        | Charging PK Flash while in the air |
| 162  | PK_FLASH_AIR_EXPLODE       | PK Flash explosion while in the air |
| 163  | PK_FLASH_AIR_END           | End of PK Flash while in the air |
| 164  | PK_FIRE_GROUND             | PK Fire on the ground (side-b) |
| 165  | PK_FIRE_AIR                | PK Fire while in the air |
| 166  | PK_THUNDER_GROUND_STARTUP  | Beginning of PK Thunder on the ground (up-b) |
| 167  | PK_THUNDER_GROUND          | Controlling PK Thunder on the ground |
| 168  | PK_THUNDER_GROUND_END      | End of PK Thunder on the ground |
| 169  | PK_THUNDER_GROUND_HIT      | PK Thunder self-hit on the ground |
| 16A  | PK_THUNDER_AIR_STARTUP     | Beginning of PK Thunder while in the air |
| 16B  | PK_THUNDER_AIR             | Controlling PK Thunder while in the air |
| 16C  | PK_THUNDER_AIR_END         | End of PK Thunder while in the air |
| 16D  | PK_THUNDER_AIR_HIT         | PK Thunder self-hit while in the air |
| 16E  | PK_THUNDER_AIR_HIT_WALL    | PK Thunder hitting a wall while in the air |
| 16F  | PSI_MAGNET_GROUND_STARTUP  | Beginning of PSI Magnet on the ground (down-b) |
| 170  | PSI_MAGNET_GROUND_LOOP     | Looping PSI Magnet on the ground |
| 171  | PSI_MAGNET_GROUND_ABSORB   | Absorbing energy on the ground with PSI Magnet |
| 172  | PSI_MAGNET_GROUND_END      | End of PSI Magnet on the ground |
| 174  | PSI_MAGNET_AIR_STARTUP     | Beginning of PSI Magnet while in the air |
| 175  | PSI_MAGNET_AIR_LOOP        | Looping PSI Magnet while in the air |
| 176  | PSI_MAGNET_AIR_ABSORB      | Absorbing energy while in the air with PSI Magnet |
| 177  | PSI_MAGNET_AIR_END         | End of PSI Magnet while in the air |

### Peach

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | FLOAT                      | Floating in the air (hold jump button) |
| 156  | FLOAT_END_FORWARD          | Ending the float while moving forward |
| 157  | FLOAT_END_BACKWARD         | Ending the float while moving backward |
| 158  | FLOAT_NAIR                 | Neutral air attack while floating |
| 159  | FLOAT_FAIR                 | Forward air attack while floating |
| 15A  | FLOAT_BAIR                 | Backward air attack while floating |
| 15B  | FLOAT_UAIR                 | Upward air attack while floating |
| 15C  | FLOAT_DAIR                 | Downward air attack while floating |
| 15D  | SIDE_SMASH_GOLF_CLUB       | Forward smash with golf club |
| 15E  | SIDE_SMASH_FRYING_PAN      | Forward smash with frying pan |
| 15F  | SIDE_SMASH_TENNIS_RACKET   | Forward smash with tennis racket |
| 160  | VEGETABLE_GROUND           | Pulling vegetable out of ground (down-b) |
| 161  | VEGETABLE_AIR              | Pulling vegetable while in the air |
| 162  | BOMBER_GROUND_STARTUP      | Beginning of Peach Bomber on the ground (side-b) |
| 163  | BOMBER_GROUND_END          | End of Peach Bomber on the ground |
| 165  | BOMBER_AIR_STARTUP         | Beginning of Peach Bomber in the air |
| 166  | BOMBER_AIR_END             | End of Peach Bomber while in the air |
| 167  | BOMBER_AIR_HIT             | Peach Bomber connecting while in the air |
| 168  | BOMBER_AIR                 | Peach Bomber while in the air |
| 169  | PARASOL_GROUND_START       | Beginning of Parasol on the ground (up-b) |
| 16B  | PARASOL_AIR_START          | Beginning of Parasol in the air |
| 16D  | TOAD_GROUND                | Beginning of Toad Counter on the ground (down-b) |
| 16E  | TOAD_GROUND_ATTACK         | Toad counterattack on the ground |
| 16F  | TOAD_AIR                   | Beginning of Toad Counter in the air |
| 170  | TOAD_AIR_ATTACK            | Toad counterattack in the air |
| 171  | PARASOL_OPENING            | Opening parasol (up-b) |
| 172  | PARASOL_OPEN               | Parasol fully open (gliding) |

### Pichu

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | THUNDER_JOLT_GROUND        | Firing Thunder Jolt on ground (neutral-b) |
| 156  | THUNDER_JOLT_AIR           | Firing Thunder Jolt in the air |
| 157  | SKULL_BASH_GROUND_STARTUP  | Beginning of Skull Bash on the ground (side-b) |
| 158  | SKULL_BASH_GROUND_CHARGE   | Charging Skull Bash on the ground |
| 15A  | SKULL_BASH_GROUND_LANDING  | Landing from a Skull Bash on the ground |
| 15B  | SKULL_BASH_GROUND_TAKEOFF  | Takeoff for Skull Bash on the ground |
| 15C  | SKULL_BASH_AIR_STARTUP     | Beginning of Skull Bash in the air |
| 15D  | SKULL_BASH_AIR_CHARGE      | Charging Skull Bash in the air |
| 15E  | SKULL_BASH_AIR             | Performing Skull Bash in the air |
| 15F  | SKULL_BASH_AIR_END         | End of Skull Bash in the air |
| 160  | SKULL_BASH_AIR_TAKEOFF     | Takeoff for Skull Bash in the air |
| 161  | AGILITY_GROUND_STARTUP     | Beginning of Agility on the ground (up-b) |
| 162  | AGILITY_GROUND             | Performing Agility on the ground |
| 163  | AGILITY_GROUND_END         | End of Agility on the ground |
| 164  | AGILITY_AIR_STARTUP        | Beginning of Agility in the air |
| 165  | AGILITY_AIR                | Performing Agility in the air |
| 166  | AGILITY_AIR_END            | End of Agility in the air |
| 167  | THUNDER_GROUND_STARTUP     | Beginning of Thunder on the ground (down-b) |
| 168  | THUNDER_GROUND             | Thunder cloud appearing on the ground |
| 169  | THUNDER_GROUND_HIT         | Thunder connecting while on the ground |
| 16A  | THUNDER_GROUND_END         | End of Thunder on the ground |
| 16B  | THUNDER_AIR_STARTUP        | Beginning of Thunder in the air |
| 16C  | THUNDER_AIR                | Thunder cloud appearing in the air |
| 16D  | THUNDER_AIR_HIT            | Thunder connecting while in the air |
| 16E  | THUNDER_AIR_END            | End of Thunder in the air |

### Pikachu

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | THUNDER_JOLT_GROUND        | Firing Thunder Jolt on ground (neutral-b) |
| 156  | THUNDER_JOLT_AIR           | Firing Thunder Jolt in the air |
| 157  | SKULL_BASH_GROUND_STARTUP  | Beginning of Skull Bash on the ground (side-b) |
| 158  | SKULL_BASH_GROUND_CHARGE   | Charging Skull Bash on the ground |
| 15A  | SKULL_BASH_GROUND_LANDING  | Landing from a Skull Bash on the ground |
| 15B  | SKULL_BASH_GROUND_TAKEOFF  | Takeoff for Skull Bash on the ground |
| 15C  | SKULL_BASH_AIR_STARTUP     | Beginning of Skull Bash in the air |
| 15D  | SKULL_BASH_AIR_CHARGE      | Charging Skull Bash in the air |
| 15E  | SKULL_BASH_AIR             | Performing Skull Bash in the air |
| 15F  | SKULL_BASH_AIR_END         | End of Skull Bash in the air |
| 160  | SKULL_BASH_AIR_TAKEOFF     | Takeoff for Skull Bash in the air |
| 161  | QUICK_ATTACK_GROUND_STARTUP | Beginning of Quick Attack on the ground (up-b) |
| 162  | QUICK_ATTACK_GROUND        | Performing Quick Attack on the ground |
| 163  | QUICK_ATTACK_GROUND_END    | End of Quick Attack on the ground |
| 164  | QUICK_ATTACK_AIR_STARTUP   | Beginning of Quick Attack in the air |
| 165  | QUICK_ATTACK_AIR           | Performing Quick Attack in the air |
| 166  | QUICK_ATTACK_AIR_END       | End of Quick Attack in the air |
| 167  | THUNDER_GROUND_STARTUP     | Beginning of Thunder on the ground (down-b) |
| 168  | THUNDER_GROUND             | Thunder cloud appearing on the ground |
| 169  | THUNDER_GROUND_HIT         | Thunder connecting while on the ground |
| 16A  | THUNDER_GROUND_END         | End of Thunder on the ground |
| 16B  | THUNDER_AIR_STARTUP        | Beginning of Thunder in the air |
| 16C  | THUNDER_AIR                | Thunder cloud appearing in the air |
| 16D  | THUNDER_AIR_HIT            | Thunder connecting while in the air |
| 16E  | THUNDER_AIR_END            | End of Thunder in the air |

### Popo

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | ICE_SHOT_GROUND            | Firing Ice Shot on the ground (neutral-b) |
| 156  | ICE_SHOT_AIR               | Firing Ice Shot in the air |
| 157  | SQUALL_HAMMER_GROUND_SOLO  | Solo Squall Hammer on the ground (side-b) |
| 158  | SQUALL_HAMMER_GROUND_TOGETHER | Squall Hammer together with Nana on the ground |
| 159  | SQUALL_HAMMER_AIR_SOLO     | Solo Squall Hammer in the air |
| 15A  | SQUALL_HAMMER_AIR_TOGETHER | Squall Hammer together with Nana in the air |
| 15B  | BELAY_GROUND_STARTUP       | Beginning of Belay on the ground (up-b) |
| 15C  | BELAY_GROUND_CATAPULTING_NANA | Catapulting with Nana using Belay on the ground |
| 15E  | BELAY_GROUND_FAILED_CATAPULTING | Failed catapulting using Belay on the ground |
| 15F  | BELAY_GROUND_FAILED_CATAPULTING_END | End of failed catapulting using Belay on the ground |
| 160  | BELAY_AIR_STARTUP          | Beginning of Belay in the air |
| 161  | BELAY_AIR_CATAPULTING_NANA | Catapulting with Nana using Belay in the air |
| 162  | BELAY_CATAPULTING          | Catapulting using Belay in the air? |
| 163  | BELAY_AIR_FAILED_CATAPULTING | Failed catapulting using Belay in the air |
| 164  | BELAY_AIR_FAILED_CATAPULTING_END | End failed catapulting using Belay in the air |
| 165  | BLIZZARD_GROUND            | Blizzard on the ground (down-b) |
| 166  | BLIZZARD_AIR               | Blizzard in the air |

### Roy

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | FLARE_BLADE_GROUND_START_CHARGE | Start of Flare Blade charge on the ground (neutral-b) |
| 156  | FLARE_BLADE_GROUND_CHARGE_LOOP | Looping charge for Flare Blade on the ground |
| 157  | FLARE_BLADE_GROUND_EARLY_RELEASE | Releasing Flare Blade early on the ground |
| 158  | FLARE_BLADE_GROUND_FULLY_CHARGED | Fully charging and releasing Flare Blade on the ground |
| 159  | FLARE_BLADE_AIR_START_CHARGE | Starting to charge Flare Blade in the air |
| 15A  | FLARE_BLADE_AIR_CHARGE_LOOP | Looping charge for Flare Blade in the air |
| 15B  | FLARE_BLADE_AIR_EARLY_RELEASE | Releasing Flare Blade early in the air |
| 15C  | FLARE_BLADE_AIR_FULLY_CHARGED | Fully charging and releasing Flare Blade in the air |
| 15D  | DOUBLE_EDGE_DANCE_1_GROUND | First hit of Double Edge Dance on the ground |
| 15E  | DOUBLE_EDGE_DANCE_2_UP_GROUND | Second hit (upward) of Double Edge Dance on the ground |
| 15F  | DOUBLE_EDGE_DANCE_2_SIDE_GROUND | Second hit (side) of Double Edge Dance on the ground |
| 160  | DOUBLE_EDGE_DANCE_3_UP_GROUND | Third hit (upward) of Double Edge Dance on the ground |
| 161  | DOUBLE_EDGE_DANCE_3_SIDE_GROUND | Third hit (side) of Double Edge Dance on the ground |
| 162  | DOUBLE_EDGE_DANCE_3_DOWN_GROUND | Third hit (downward) of Double Edge Dance on the ground |
| 163  | DOUBLE_EDGE_DANCE_4_UP_GROUND | Fourth hit (upward) of Double Edge Dance on the ground |
| 164  | DOUBLE_EDGE_DANCE_4_SIDE_GROUND | Fourth hit (side) of Double Edge Dance on the ground |
| 165  | DOUBLE_EDGE_DANCE_4_DOWN_GROUND | Fourth hit (downward) of Double Edge Dance on the ground |
| 166  | DOUBLE_EDGE_DANCE_1_AIR    | First hit of Double Edge Dance in the air |
| 167  | DOUBLE_EDGE_DANCE_2_UP_AIR | Second hit (upward) of Double Edge Dance in the air |
| 168  | DOUBLE_EDGE_DANCE_2_SIDE_AIR | Second hit (side) of Double Edge Dance in the air |
| 169  | DOUBLE_EDGE_DANCE_3_UP_AIR | Third hit (upward) of Double Edge Dance in the air |
| 16A  | DOUBLE_EDGE_DANCE_3_SIDE_AIR | Third hit (side) of Double Edge Dance in the air |
| 16B  | DOUBLE_EDGE_DANCE_3_DOWN_AIR | Third hit (downward) of Double Edge Dance in the air |
| 16C  | DOUBLE_EDGE_DANCE_4_UP_AIR | Fourth hit (upward) of Double Edge Dance in the air |
| 16D  | DOUBLE_EDGE_DANCE_4_SIDE_AIR | Fourth hit (side) of Double Edge Dance in the air |
| 16E  | DOUBLE_EDGE_DANCE_4_DOWN_AIR | Fourth hit (downward) of Double Edge Dance in the air |
| 16F  | BLAZER_GROUND              | Blazer on the ground (up-b) |
| 170  | BLAZER_AIR                 | Blazer in the air |
| 171  | COUNTER_GROUND             | Counter on the ground (down-b) |
| 172  | COUNTER_GROUND_HIT         | Getting hit while performing Counter on the ground |
| 173  | COUNTER_AIR                | Counter in the air |
| 174  | COUNTER_AIR_HIT            | Getting hit while performing Counter in the air |

### Samus

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | BOMB_JUMP_GROUND           | Bomb jump on the ground (down-b) |
| 156  | BOMB_JUMP_AIR              | Bomb jump in the air |
| 157  | CHARGE_SHOT_GROUND_START   | Starting to charge for charge shot on the ground (neutral-b) |
| 158  | CHARGE_SHOT_GROUND_LOOP    | Looping while charging for charge shot on the ground |
| 159  | CHARGE_SHOT_GROUND_END     | End of charging for charge shot on the ground |
| 15A  | CHARGE_SHOT_GROUND_FIRE    | Firing charged charge shot on the ground |
| 15B  | CHARGE_SHOT_AIR_START      | Start of charge for charge shot in the air |
| 15C  | CHARGE_SHOT_AIR_FIRE       | Firing charged charge shot in the air |
| 15D  | MISSILE_GROUND             | Firing a regular missile on the ground (side-b tilt) |
| 15E  | MISSILE_SMASH_GROUND       | Firing a super missile on the ground (side-b tap) |
| 15F  | MISSILE_AIR                | Firing a regular missile in the air |
| 160  | MISSILE_SMASH_AIR          | Firing a super missile in the air |
| 161  | SCREW_ATTACK_GROUND        | Screw attack on the ground (up-b) |
| 162  | SCREW_ATTACK_AIR           | Screw attack in the air |
| 163  | BOMB_END_GROUND            | End of using bomb on the ground |
| 164  | BOMB_AIR                   | Using bomb in the air |
| 165  | ZAIR                       | Using tether grab in the air (z) |
| 166  | ZAIR_CATCH                 | Catching the ledge with tether grab |

### Shiek

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | NEEDLE_STORM_GROUND_START_CHARGE | Beginning of charging the Needle Storm move on the ground (neutral-b) |
| 156  | NEEDLE_STORM_GROUND_CHARGE_LOOP | Looping sequence of charging the Needle Storm move on the ground |
| 157  | NEEDLE_STORM_GROUND_END_CHARGE | End of charging the Needle Storm move on the ground |
| 158  | NEEDLE_STORM_GROUND_FIRE   | Firing the charged Needle Storm move on the ground |
| 159  | NEEDLE_STORM_AIR_START_CHARGE | Beginning of charging the Needle Storm move in the air |
| 15A  | NEEDLE_STORM_AIR_CHARGE_LOOP | Looping sequence of charging the Needle Storm move in the air |
| 15B  | NEEDLE_STORM_AIR_END_CHARGE | End of charging the Needle Storm move in the air |
| 15C  | NEEDLE_STORM_AIR_FIRE      | Firing the charged Needle Storm move in the air |
| 15D  | CHAIN_GROUND_STARTUP       | Beginning of using the Chain move on the ground (side-b) |
| 15E  | CHAIN_GROUND_LOOP          | Looping sequence when Chain move on the ground is ongoing |
| 15F  | CHAIN_GROUND_END           | End of the Chain move on the ground |
| 160  | CHAIN_AIR_STARTUP          | Beginning of using the Chain move in the air |
| 161  | CHAIN_AIR_LOOP             | Looping sequence when Chain move in the air is on-going |
| 162  | CHAIN_AIR_END              | End of the Chain move in the air |
| 163  | VANISH_GROUND_STARTUP      | Startup of the Vanish move on the ground (up-b) |
| 164  | VANISH_GROUND_DISAPPEAR    | The moment of disappearing while using Vanish on the ground (what state is the actual invisibility?) |
| 165  | VANISH_GROUND_REAPPEAR     | The moment of reappearing after using Vanish on the ground |
| 166  | VANISH_AIR_STARTUP         | Startup of the Vanish move in the air |
| 167  | VANISH_AIR_DISAPPEAR       | The moment of disappearing while using Vanish in the air |
| 168  | VANISH_AIR_REAPPEAR        | The moment of reappearing after using Vanish in the air |
| 169  | TRANSFORM_GROUND           | Beginning of transformation to Zelda on the ground (down-b) |
| 16A  | TRANSFORM_GROUND_ENDING    | Transformation to Zelda on the ground |
| 16B  | TRANSFORM_AIR              | Beginning of transformation to Zelda while in the air |
| 16C  | TRANSFORM_AIR_ENDING       | Transforming to Zelda while in the air |

### Yoshi

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 156  | SHIELD_HOLD                | Holding shield, deflecting damage |
| 157  | SHIELD_RELEASE             | Releasing shield |
| 158  | SHIELD_DAMAGE              | Shield being damaged |
| 159  | SHIELD_STARTUP             | Beginning of shield block (this is when parries can happen?) |
| 15A  | EGG_LAY_GROUND             | Egg lay on ground (neutral-b) |
| 15B  | EGG_LAY_GROUND_CAPTURE_START | Start of ground egg lay capture animation |
| 15D  | EGG_LAY_GROUND_CAPTURE     | Capturing opponent in egg on ground |
| 15F  | EGG_LAY_AIR                | Egg lay in the air |
| 160  | EGG_LAY_AIR_CAPTURE_START  | Start of air egg lay capture animation |
| 162  | EGG_LAY_AIR_CAPTURE        | Capturing opponent in egg while in the air |
| 164  | EGG_ROLL_GROUND_STARTUP    | Beginning of ground egg roll (side-B) |
| 165  | EGG_ROLL_GROUND            | Rolling in egg on the ground |
| 166  | EGG_ROLL_GROUND_CHANGE_DIRECTION | Changing direction while rolling in egg on ground |
| 167  | EGG_ROLL_GROUND_END        | End of ground egg roll |
| 168  | EGG_ROLL_AIR_START         | Beginning of egg roll in the air (side-b) |
| 169  | EGG_ROLL_AIR               | Rolling in egg while in the air |
| 16A  | EGG_ROLL_BOUNCE            | Bouncing off a wall (or floor?) while egg rolling in the air |
| 16B  | EGG_ROLL_AIR_END           | End of air egg roll |
| 16C  | EGG_THROW_GROUND           | Throwing egg on the ground (up-B) |
| 16D  | EGG_THROW_AIR              | Throwing egg while in the air (up-B) |
| 16E  | BOMB_GROUND                | Yoshi bomb on ground (down-b) |
| 16F  | BOMB_LAND                  | Yoshi bomb landing |
| 170  | BOMB_AIR                   | Yoshi bomb in air |

### Young Link

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | SIDE_SMASH_2               | Second hit of side smash attack |
| 156  | TAUNT_R                    | Taunt right |
| 157  | TAUNT_L                    | Taunt left |
| 158  | FIRE_BOW_GROUND_CHARGE     | Charging bow attack on the ground (neutral-b) |
| 159  | FIRE_BOW_GROUND_FULLY_CHARGED | Fully charged bow attack on the ground |
| 15A  | FIRE_BOW_GROUND_FIRE       | Firing bow attack on the ground |
| 15B  | FIRE_BOW_AIR_CHARGE        | Charging bow attack while in the air |
| 15C  | FIRE_BOW_AIR_FULLY_CHARGED | Fully charged bow attack while in the air |
| 15D  | FIRE_BOW_AIR_FIRE          | Firing bow attack while in the air |
| 15E  | BOOMERANG_GROUND_THROW     | Throwing boomerang on the ground (side-b) |
| 15F  | BOOMERANG_GROUND_CATCH     | Catching boomerang on the ground |
| 160  | BOOMERANG_GROUND_THROW_EMPTY | Throwing boomerang on the ground, empty hand (no boomerang to catch) |
| 161  | BOOMERANG_AIR_THROW        | Throwing boomerang while in the air |
| 162  | BOOMERANG_AIR_CATCH        | Catching boomerang while in the air |
| 163  | BOOMERANG_AIR_THROW_EMPTY  | Throwing boomerang in the air, empty hand (no boomerang to catch) |
| 164  | SPIN_ATTACK_GROUND         | Spin attack on the ground (up-b) |
| 165  | SPIN_ATTACK_AIR            | Spin attack while in the air |
| 166  | BOMB_GROUND                | Pulling out a bomb on the ground (down-b) |
| 167  | BOMB_AIR                   | Pulling out a bomb while in the air |
| 168  | ZAIR                       | Using tether grab in the air (z) |
| 169  | ZAIR_CATCH                 | Catching the ledge with tether grab |

### Zelda

| ID   | Action State               | Description |
| ---- | -------------------------- | ----------- |
| 155  | NAYRUS_LOVE_GROUND         | Using Nayru's Love on the ground (neutral-b) |
| 156  | NAYRUS_LOVE_AIR            | Using Nayru's Love while in the air |
| 157  | DINS_FIRE_GROUND_STARTUP   | Beginning of Din's Fire on the ground (side-b) |
| 158  | DINS_FIRE_GROUND_TRAVEL    | Din's Fire traveling after being fired on the ground |
| 159  | DINS_FIRE_GROUND_EXPLODE   | Din's Fire exploding after travel on the ground |
| 15A  | DINS_FIRE_AIR_STARTUP      | Beginning of Din's Fire while in the air |
| 15B  | DINS_FIRE_AIR_TRAVEL       | Din's Fire traveling after being fired in the air |
| 15C  | DINS_FIRE_AIR_EXPLODE      | Din's Fire exploding after travel in the air |
| 15D  | FARORES_WIND_GROUND        | Using Farore's Wind on the ground (up-b) |
| 15E  | FARORES_WIND_GROUND_DISAPPEAR | Disappearing during Farore's Wind on the ground |
| 15F  | FARORES_WIND_GROUND_REAPPEAR | Reappearing during Farore's Wind on the ground |
| 160  | FARORES_WIND_AIR           | Using Farore's Wind while in the air |
| 161  | FARORES_WIND_AIR_DISAPPEAR | Disappearing during Farore's Wind while in the air |
| 162  | FARORES_WIND_AIR_REAPPEAR  | Reappearing during Farore's Wind while in the air |
| 163  | TRANSFORM_GROUND           | Beginning of transformation on the ground (down-b) |
| 164  | TRANSFORM_GROUND_ENDING    | End of transformation on the ground |
| 165  | TRANSFORM_AIR              | Beginning of transformation while in the air |
| 166  | TRANSFORM_AIR_ENDING       | End of transformation while in the air |