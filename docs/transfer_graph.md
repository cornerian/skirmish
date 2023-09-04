# All possible state transfers

This document covers which states can be accessed given the last frame's state.

Only tournament legal states are covered - BONC doesn't support non-tournament games yet. From now on "all states" will refer to "all states that can be achieved in a tournament legal game".

Virtually all states can be interrupted (immediate transfer) by death states, with the exception of the respawn platform states (REBIRTH/REBIRTH_WAIT). Many, many states can immediately transfer to hitlag states, with the exception of states that give the player invulnerability or invincibility (roll (?), spotdodge (?), airdodge (?), but NOT shield (GUARD) due to poking). Because of this, the states that *cannot* be immediately transferred into a hitlag state are notated instead of the opposite.

States that lead to themselves (almost all of them) are not notated as such. Assume that all states lead to themselves until their counter is over (or indefinitely in the case of looping states).

## General

```mermaid
%%{init: {"flowchart": {"defaultRenderer": "elk"}} }%%
flowchart LR
    subgraph dead
        DEAD_DOWN
        DEAD_LEFT
        DEAD_RIGHT
        DEAD_UP_STAR
        DEAD_UP_FALL_HIT_CAMERA
        DEAD_UP_FALL_HIT_CAMERA_FLAT
    end

    subgraph respawn
        REBIRTH
        REBIRTH_WAIT
    end

    subgraph ambulations
        WAIT
        subgraph walking
            WALK_SLOW
            WALK_MIDDLE
            WALK_FAST
            subgraph walk turn
                TURN
            end
        end
        subgraph running
            DASH
            RUN
            RUN_BRAKE
            subgraph run turn
                TURN_RUN
            end
        end
        KNEE_BEND
        subgraph crouching
            SQUAT
            SQUAT_WAIT
            SQUAT_RV
        end

        subgraph platform
            PASS
        end

        subgraph ledge teeter
            OTTOTTO
            OTTOTTO_WAIT
        end

        subgraph bumping
            STOP_WALL
        end
    end

    subgraph jumping
        subgraph from ground
            JUMP_F
            JUMP_B
        end
        subgraph from air
            JUMP_AERIAL_F
            JUMP_AERIAL_B
        end
    end

    subgraph falling with control
        FALL
        subgraph control with di
            FALL_F
            FALL_B
        end
        subgraph after double jump
            FALL_AERIAL
            subgraph with di
                FALL_AERIAL_F
                FALL_AERIAL_B
            end
        end
    end

    subgraph freefalling
        FALL_SPECIAL
        subgraph freefalling with di
            FALL_SPECIAL_F
            FALL_SPECIAL_B
        end
    end

    subgraph tumbling
        DAMAGE_FALL
    end

    subgraph landing
        subgraph autocancellable
            LANDING
        end
        subgraph not cancellable
            LANDING_FALL_SPECIAL
        end
        subgraph l cancellable
            LANDING_AIR_N
            LANDING_AIR_F
            LANDING_AIR_B
            LANDING_AIR_HI
            LANDING_AIR_LW
        end
    end

    subgraph attacking
        subgraph jabs
            ATTACK_11
            ATTACK_12
            ATTACK_13
            subgraph multijabs
                ATTACK_100_START
                ATTACK_100_LOOP
                ATTACK_100_END
            end
        end
        ATTACK_DASH
        subgraph tilts
            subgraph forward tilts
                %% high, high-mid, mid, low-mid, low
                ATTACK_S_3_HI
                ATTACK_S_3_HI_S
                ATTACK_S_3_S
                ATTACK_S_3_LW_S
                ATTACK_S_3_LW
            end
            subgraph up tilt
                ATTACK_HI_3
            end
            subgraph down tilt
                ATTACK_LW_3
            end
        end

        subgraph smashes
            subgraph forward smashes
                %% high, high-mid, mid, low-mid, low
                ATTACK_S_4_HI
                ATTACK_S_4_HI_S
                ATTACK_S_4_S
                ATTACK_S_4_LW_S
                ATTACK_S_4_LW
            end
            subgraph up smash
                ATTACK_HI_4
            end
            subgraph down smash
                ATTACK_LW_4
            end
        end
        subgraph aerials
            ATTACK_AIR_N
            ATTACK_AIR_F
            ATTACK_AIR_B
            ATTACK_AIR_HI
            ATTACK_AIR_LW
        end
    end

    subgraph item interaction
        subgraph picking up item
            LIGHT_GET
        end
        subgraph releasing item
            subgraph releasing item
                subgraph throwing item grounded
                    LIGHT_THROW_F
                    LIGHT_THROW_B
                    LIGHT_THROW_HI
                    LIGHT_THROW_LW
                    
                    LIGHT_THROW_DASH
                    subgraph smashthrowing item grounded
                        LIGHT_THROW_F_4
                        LIGHT_THROW_B_4
                        LIGHT_THROW_HI_4
                        LIGHT_THROW_LW_4
                    end
                end
                subgraph throwing item airborne
                    LIGHT_THROW_AIR_F
                    LIGHT_THROW_AIR_B
                    LIGHT_THROW_AIR_HI
                    LIGHT_THROW_AIR_LW
                    
                    subgraph smashthrowing item airborne
                        LIGHT_THROW_AIR_F_4
                        LIGHT_THROW_AIR_B_4
                        LIGHT_THROW_AIR_HI_4
                        LIGHT_THROW_AIR_LW_4
                    end
                end
                LIGHT_THROW_DROP
            end
        end
        subgraph using item
            subgraph swinging beam sword
                SWORD_SWING_1
                SWORD_SWING_3
                SWORD_SWING_4

                SWORD_SWING_DASH
            end
        end
    end

    subgraph shield
        GUARD_ON
        GUARD
        GUARD_OFF
        GUARD_SET_OFF
        GUARD_REFLECT

        subgraph shield breaks
            SHIELD_BREAK_FLY
            SHIELD_BREAK_FALL

            SHIELD_BREAK_DOWN_U
            SHIELD_BREAK_DOWN_D

            SHIELD_BREAK_STAND_U
            SHIELD_BREAK_STAND_D
        
            FURA_FURA
        end

        MISS_FOOT
    end

    subgraph missed tech
        subgraph face up missed tech
            DOWN_BOUND_U

            DOWN_WAIT_U

            DOWN_DAMAGE_U

            subgraph getting up from face up missed tech
                DOWN_STAND_U
                DOWN_ATTACK_U
                DOWN_FOWARD_U
                DOWN_BACK_U
            end
        end
        subgraph face down missed tech
            DOWN_BOUND_D

            DOWN_WAIT_D

            DOWN_DAMAGE_D

            subgraph getting up from face down missed tech
                DOWN_STAND_D
                DOWN_ATTACK_D
                DOWN_FOWARD_D
                DOWN_BACK_D
            end
        end

        FLY_REFLECT_WALL
        FLY_REFLECT_CEIL
    end

    subgraph tech
        PASSIVE

        PASSIVE_STAND_F
        PASSIVE_STAND_B

        PASSIVE_WALL
        PASSIVE_WALL_JUMP

        PASSIVE_CEIL
    end

    subgraph grab
        CATCH
        CATCH_PULL

        CATCH_DASH
        CATCH_DASH_PULL

        CATCH_WAIT
        CATCH_ATTACK
        CATCH_CUT

        subgraph grab throws
            THROW_F
            THROW_B
            THROW_HI
            THROW_LW
        end
    end

    subgraph grabbed
        subgraph grabbed by taller opponent or off edge
            CAPTURE_PULLED_HI
            CAPTURE_WAIT_HI
            CAPTURE_DAMAGE_HI
        end
        subgraph grabbed by shorter/same height opponent
            CAPTURE_PULLED_LW
            CAPTURE_WAIT_LW
            CAPTURE_DAMAGE_LW
        end
        subgraph grabbed by the same character
            CAPTURE_LIKE_LIKE
        end

        CAPTURE_CUT
        CAPTURE_JUMP

        CAPTURE_NECK
        CAPTURE_FOOT

        THROWN_F
        THROWN_B
        THROWN_HI
        THROWN_LW
        %% THROWN_LW_WOMEN ?

        subgraph cargo carried
            SHOULDERED_WAIT

            SHOULDERED_WALK_SLOW
            SHOULDERED_WALK_MIDDLE
            SHOULDERED_WALK_FAST

            SHOULDERED_TURN

            subgraph thrown out of cargo carry
                THROWN_F_F
                THROWN_F_B
                THROWN_F_HI
                THROWN_F_LW
            end
        end
    end

    subgraph escapes
        subgraph rolls
            ESCAPE_F
            ESCAPE_B
        end
        ESCAPE
        ESCAPE_AIR
    end

    subgraph ledge
        CLIFF_CATCH

        CLIFF_WAIT

        subgraph ledge getups
            subgraph ledge neutral getups
                CLIFF_CLIMB_SLOW
                CLIFF_CLIMB_QUICK
            end
            subgraph ledge attack
                CLIFF_ATTACK_SLOW
                CLIFF_ATTACK_QUICK
            end
            subgraph ledge rolls
                CLIFF_ESCAPE_SLOW
                CLIFF_ESCAPE_QUICK
            end
            subgraph ledge jumps
                CLIFF_JUMP_SLOW_1
                CLIFF_JUMP_SLOW_2
                
                CLIFF_JUMP_QUICK_1
                CLIFF_JUMP_QUICK_2
            end
        end
    end

    subgraph taunts
        APPEAL_R
        APPEAL_L
    end

    %% "caught" is for states in which you're grappled/immobilized but are not grabs
    subgraph caught
        subgraph caught by captain falcon's raptor boost
            CAPTURE_CAPTAIN
        end
        subgraph caught by yoshi's egg lay
            CAPTURE_YOSHI
            YOSHI_EGG
        end
        subgraph caught by bowser's koopa klaw
            subgraph caught by koopa klaw while grounded
                CAPTURE_KOOPA
                CAPTURE_DAMAGE_KOOPA
                CAPTURE_WAIT_KOOPA
                subgraph thrown from koopa klaw while grounded
                    THROWN_KOOPA_F
                    THROWN_KOOPA_B
                end
            end
            subgraph caught by koopa klaw while airborne
                CAPTURE_KOOPA_AIR
                CAPTURE_DAMAGE_KOOPA_AIR
                CAPTURE_WAIT_KOOPA_AIR
                subgraph thrown from koopa klaw while airborne
                    THROWN_KOOPA_AIR_F
                    THROWN_KOOPA_AIR_B
                end
            end
        end
        subgraph caught in kirby's swallow
            CAPTURE_KIRBY
            CAPTURE_WAIT_KIRBY
            subgraph spit out of kirby's swallow
                THROWN_KIRBY_STAR
                THROWN_COPY_STAR
                THROWN_KIRBY
            end
        end
        subgraph buried
            BURY
            BURY_WAIT
            BURY_JUMP
        end
        subgraph caught by jigglypuff's sing
            DAMAGE_SONG
            DAMAGE_SONG_WAIT
            DAMAGE_SONG_RV
            DAMAGE_BIND
        end
        subgraph caught by mewtwo's disable
            CAPTURE_MEWTWO
            subgraph thrown from mewtwo's disable
                THROWN_MEWTWO
            end
            subgraph caught by mewtwo's disable while airborne
                CAPTURE_MEWTWO_AIR
                subgraph thrown from mewtwo's disable while airborne
                    THROWN_MEWTWO_AIR
                end
            end
        end
        subgraph caught in ice
            DAMAGE_ICE
            DAMAGE_ICE_JUMP
        end
        subgraph caught by kirby's copied ability egg lay
            CAPTURE_KIRBY_YOSHI
            KIRBY_YOSHI_EGG
        end
    end

    subgraph warping in
        ENTRY
        ENTRY_START
        ENTRY_END
    end

    %% ////// HUBS //////
        HUB::GROUNDED_ACTIONABLE{"Grounded actionable"}

        %% --- Grounded actionable transfers ---
            %% --- to ambulations ---
                HUB::GROUNDED_ACTIONABLE --> WAIT

                HUB::GROUNDED_ACTIONABLE --> WALK_SLOW
                HUB::GROUNDED_ACTIONABLE --> WALK_MIDDLE
                HUB::GROUNDED_ACTIONABLE --> WALK_FAST

                HUB::GROUNDED_ACTIONABLE --> TURN

                HUB::GROUNDED_ACTIONABLE --> DASH

                HUB::GROUNDED_ACTIONABLE --> KNEE_BEND

                HUB::GROUNDED_ACTIONABLE --> SQUAT

                HUB::GROUNDED_ACTIONABLE -->|ONLY if on platform| PASS

                HUB::GROUNDED_ACTIONABLE --> OTTOTTO
            %% ------
            %% --- to attacks ---
                %% --- to jabs ---
                    HUB::GROUNDED_ACTIONABLE --> ATTACK_11
                %% ------
                %% --- to tilts ---
                    HUB::GROUNDED_ACTIONABLE --> ATTACK_S_3_HI
                    HUB::GROUNDED_ACTIONABLE --> ATTACK_S_3_HI_S
                    HUB::GROUNDED_ACTIONABLE --> ATTACK_S_3_S
                    HUB::GROUNDED_ACTIONABLE --> ATTACK_S_3_LW_S
                    HUB::GROUNDED_ACTIONABLE --> ATTACK_S_3_LW

                    HUB::GROUNDED_ACTIONABLE --> ATTACK_HI_3
                %% ------
                %% --- to smashes ---
                    HUB::GROUNDED_ACTIONABLE --> ATTACK_S_4_HI
                    HUB::GROUNDED_ACTIONABLE --> ATTACK_S_4_HI_S
                    HUB::GROUNDED_ACTIONABLE --> ATTACK_S_4_S
                    HUB::GROUNDED_ACTIONABLE --> ATTACK_S_4_LW_S
                    HUB::GROUNDED_ACTIONABLE --> ATTACK_S_4_LW

                    HUB::GROUNDED_ACTIONABLE --> ATTACK_HI_4

                    HUB::GROUNDED_ACTIONABLE --> ATTACK_LW_4
                %% ------
            %% ------
            %% --- to item interactions ---
                HUB::GROUNDED_ACTIONABLE --> LIGHT_GET

                HUB::GROUNDED_ACTIONABLE --> LIGHT_THROW_F
                HUB::GROUNDED_ACTIONABLE --> LIGHT_THROW_B
                HUB::GROUNDED_ACTIONABLE --> LIGHT_THROW_HI
                HUB::GROUNDED_ACTIONABLE --> LIGHT_THROW_LW

                HUB::GROUNDED_ACTIONABLE --> LIGHT_THROW_F_4
                HUB::GROUNDED_ACTIONABLE --> LIGHT_THROW_B_4
                HUB::GROUNDED_ACTIONABLE --> LIGHT_THROW_HI_4
                HUB::GROUNDED_ACTIONABLE --> LIGHT_THROW_LW_4

                HUB::GROUNDED_ACTIONABLE --> LIGHT_THROW_DROP

                HUB::GROUNDED_ACTIONABLE --> SWORD_SWING_1
                HUB::GROUNDED_ACTIONABLE --> SWORD_SWING_3
                HUB::GROUNDED_ACTIONABLE --> SWORD_SWING_4
            %% ------
            %% --- to shielding ---
                HUB::GROUNDED_ACTIONABLE --> GUARD_ON
            %% ------
            %% --- to grabbing ---
                HUB::GROUNDED_ACTIONABLE --> CATCH
            %% ------
            %% --- to escapes ---
                %% --- to rolls ---
                    HUB::GROUNDED_ACTIONABLE --> ESCAPE_F
                    HUB::GROUNDED_ACTIONABLE --> ESCAPE_B
                %% ------
                HUB::GROUNDED_ACTIONABLE --> ESCAPE
            %% ------
            %% --- to taunts ---
                HUB::GROUNDED_ACTIONABLE --> APPEAL_R
                HUB::GROUNDED_ACTIONABLE --> APPEAL_L
            %% ------
        %% ------

        HUB::AERIAL_ACTIONABLE{"Aerial actionable"}

        %% --- Aerial actionable transfers ---
            %% --- to ambulations ---
                HUB::AERIAL_ACTIONABLE --> STOP_CEIL
            %% ------
            %% --- ONLY with remaining jump ---
                %% --- to jumping ---
                    HUB::AERIAL_ACTIONABLE -->|ONLY WITH remaining jump| JUMP_AERIAL_F
                    HUB::AERIAL_ACTIONABLE -->|ONLY WITH remaining jump| JUMP_AERIAL_B
                %% ------
                %% --- to falling ---
                    HUB::AERIAL_ACTIONABLE -->|ONLY WITH remaining jump| FALL

                    HUB::AERIAL_ACTIONABLE -->|ONLY WITH remaining jump| FALL_F
                    HUB::AERIAL_ACTIONABLE -->|ONLY WITH remaining jump| FALL_B
                %% ------
            %% ------
            %% --- ONLY without remaining jump ---
                %% --- to falling ---
                    HUB::AERIAL_ACTIONABLE -->|ONLY WITHOUT remaining jump| FALL_AERIAL

                    HUB::AERIAL_ACTIONABLE -->|ONLY WITHOUT remaining jump| FALL_AERIAL_F
                    HUB::AERIAL_ACTIONABLE -->|ONLY WITHOUT remaining jump| FALL_AERIAL_B
                %% ------
            %% ------
            %% --- to landing ---
                HUB::AERIAL_ACTIONABLE --> LANDING
            %% ------
            %% --- to attacking ---
                HUB::AERIAL_ACTIONABLE --> ATTACK_AIR_N
                HUB::AERIAL_ACTIONABLE --> ATTACK_AIR_F
                HUB::AERIAL_ACTIONABLE --> ATTACK_AIR_B
                HUB::AERIAL_ACTIONABLE --> ATTACK_AIR_HI
                HUB::AERIAL_ACTIONABLE --> ATTACK_AIR_LW
            %% ------
            %% --- to item interactions ---
                HUB::AERIAL_ACTIONABLE --> LIGHT_GET

                HUB::AERIAL_ACTIONABLE -->|ONLY if has item| LIGHT_THROW_F
                HUB::AERIAL_ACTIONABLE -->|ONLY if has item| LIGHT_THROW_B
                HUB::AERIAL_ACTIONABLE -->|ONLY if has item| LIGHT_THROW_HI
                HUB::AERIAL_ACTIONABLE -->|ONLY if has item| LIGHT_THROW_LW

                HUB::AERIAL_ACTIONABLE -->|ONLY if has item| LIGHT_THROW_F_4
                HUB::AERIAL_ACTIONABLE -->|ONLY if has item| LIGHT_THROW_B_4
                HUB::AERIAL_ACTIONABLE -->|ONLY if has item| LIGHT_THROW_HI_4
                HUB::AERIAL_ACTIONABLE -->|ONLY if has item| LIGHT_THROW_LW_4

                HUB::AERIAL_ACTIONABLE -->|ONLY if has item| LIGHT_THROW_DROP

                HUB::AERIAL_ACTIONABLE -->|ONLY if has beam sword item| SWORD_SWING_1
                HUB::AERIAL_ACTIONABLE -->|ONLY if has beam sword item| SWORD_SWING_3
                HUB::AERIAL_ACTIONABLE -->|ONLY if has beam sword item| SWORD_SWING_4
            %% ------
            %% --- to escapes ---
                HUB::AERIAL_ACTIONABLE --> ESCAPE_AIR
            %% ------
            %% --- to ledge ---
                HUB::AERIAL_ACTIONABLE --> CLIFF_CATCH
            %% ------
        %% ------

        HUB::DASH::ATTACKS(["Dash attacks"])

        %% --- Dash attack transfers ---
            %% --- Standard grounded moves ---
                HUB::DASH::ATTACKS --> ATTACK_DASH
                HUB::DASH::ATTACKS --> CATCH_DASH
            %% ------
            %% --- Item moves ---
                HUB::DASH::ATTACKS -->|ONLY if has item| LIGHT_THROW_DASH

                HUB::DASH::ATTACKS -->|ONLY if has beam sword item| SWORD_SWING_DASH
            %% ------
            %% side bs can be performed from dash
        %% ------

        HUB::GRAB::THROWS(["Grab throws"])

        %% --- Grab throw transfers ---
            HUB::GRAB::THROWS --> THROW_F
            HUB::GRAB::THROWS --> THROW_B
            HUB::GRAB::THROWS --> THROW_HI
            HUB::GRAB::THROWS --> THROW_LW
        %% ------

        %% !! NOTE: in the future we're gonna consider all of these as a hit event, as it is in the code
        HUB::GRABBED::THROWNS(["Thrown by opponent"])

        %% --- Grabbed throwns transfers ---
            HUB::GRABBED::THROWNS --> THROWN_F
            HUB::GRABBED::THROWNS --> THROWN_B
            HUB::GRABBED::THROWNS --> THROWN_HI
            HUB::GRABBED::THROWNS --> THROWN_LW
            %% --- Out of cargo carry ---
                HUB::GRABBED::CARGO[["In cargo carry"]]

                HUB::GRABBED::CARGO --> SHOULDERED_WAIT
                HUB::GRABBED::CARGO --> SHOULDERED_WALK_SLOW
                HUB::GRABBED::CARGO --> SHOULDERED_WALK_MIDDLE
                HUB::GRABBED::CARGO --> SHOULDERED_WALK_FAST
                HUB::GRABBED::CARGO --> SHOULDERED_TURN

                HUB::GRABBED::CARGO --> HUB::GRABBED::THROWNS::CARGO::THROWN
            %% ------
            %% --- Out of cargo carry ---
                HUB::GRABBED::THROWNS::CARGO::THROWN[["Thrown out of cargo carry"]]

                HUB::GRABBED::THROWNS::CARGO::THROWN --> THROWN_F_F
                HUB::GRABBED::THROWNS::CARGO::THROWN --> THROWN_F_B
                HUB::GRABBED::THROWNS::CARGO::THROWN --> THROWN_F_HI
                HUB::GRABBED::THROWNS::CARGO::THROWN --> THROWN_F_LW
            %% ------
        
        HUB::LEDGE::GETUPS(["Get up from ledge"])

        %% --- Getup transfers ---
            HUB::LEDGE::GETUPS --> CLIFF_CLIMB_QUICK
            HUB::LEDGE::GETUPS --> CLIFF_ATTACK_QUICK
            HUB::LEDGE::GETUPS --> CLIFF_ESCAPE_QUICK
            HUB::LEDGE::GETUPS --> CLIFF_JUMP_QUICK_1
            HUB::LEDGE::GETUPS --> CLIFF_JUMP_QUICK_2

            %% --- Above 100% ---
                HUB::LEDGE::GETUPS -->|ONLY if above 100%| CLIFF_CLIMB_SLOW
                HUB::LEDGE::GETUPS -->|ONLY if above 100%| CLIFF_ATTACK_SLOW
                HUB::LEDGE::GETUPS -->|ONLY if above 100%| CLIFF_ESCAPE_SLOW
                HUB::LEDGE::GETUPS -->|ONLY if above 100%| CLIFF_JUMP_SLOW_1
                HUB::LEDGE::GETUPS -->|ONLY if above 100%| CLIFF_JUMP_SLOW_2
            %% ------
        %% ------
    %% ////// HUBS //////

    %% --- Death transfers ---
        DEAD_DOWN --> REBIRTH
        DEAD_LEFT --> REBIRTH
        DEAD_RIGHT --> REBIRTH
        DEAD_UP_STAR --> REBIRTH
        DEAD_UP_FALL_HIT_CAMERA --> REBIRTH
        DEAD_UP_FALL_HIT_CAMERA_FLAT --> REBIRTH
    %% ------

    %% --- Respawn transfers ---
        REBIRTH --> REBIRTH_WAIT

        REBIRTH_WAIT --> FALL
        REBIRTH_WAIT --> FALL_F
        REBIRTH_WAIT --> FALL_B
    %% ------

    %% --- Ambulation transfers ---
        WAIT --> HUB::GROUNDED_ACTIONABLE

        %% --- Walking ---
            WALK_SLOW --> HUB::GROUNDED_ACTIONABLE
            WALK_MIDDLE --> HUB::GROUNDED_ACTIONABLE
            WALK_FAST --> HUB::GROUNDED_ACTIONABLE

            TURN --> HUB::GROUNDED_ACTIONABLE
        %% ------

        %% --- Run cycle ---
            DASH --> RUN
            RUN --> RUN_BRAKE
            RUN --> TURN_RUN

            DASH -->|counter over and not holding dash direction| HUB::GROUNDED_ACTIONABLE
            RUN_BRAKE -->|counter over| HUB::GROUNDED_ACTIONABLE
            TURN_RUN -->|counter over and not holding opposite direction| HUB::GROUNDED_ACTIONABLE

            DASH --> KNEE_BEND
            RUN --> KNEE_BEND
            RUN_BRAKE --> KNEE_BEND
            TURN_RUN --> KNEE_BEND

            %% --- Bumping ---
                DASH --> STOP_WALL
                RUN --> STOP_WALL
            %% ------

            %% --- to attacking ---
                DASH --> HUB::DASH::ATTACKS
                RUN --> HUB::DASH::ATTACKS
            %% ------
        %% ------

        KNEE_BEND --> JUMP_F
        KNEE_BEND --> JUMP_B

        %% --- Squat cycle ---
            SQUAT --> HUB::GROUNDED_ACTIONABLE
            SQUAT --> SQUAT_WAIT

            SQUAT_WAIT --> HUB::GROUNDED_ACTIONABLE
            SQUAT_WAIT --> SQUAT_RV

            SQUAT_RV --> HUB::GROUNDED_ACTIONABLE

            %% --- to attacking ---
                SQUAT --> ATTACK_LW_3
                SQUAT_WAIT --> ATTACK_LW_3
            %% ------
        %% ------

        PASS --> HUB::AERIAL_ACTIONABLE

        %% --- Ledge teeter cycle ---
            OTTOTTO --> HUB::GROUNDED_ACTIONABLE
            OTTOTTO --> OTTOTTO_WAIT

            OTTOTTO_WAIT --> HUB::GROUNDED_ACTIONABLE
        %% ------

        %% --- Bumping ---
            STOP_WALL --> HUB::GROUNDED_ACTIONABLE
        %% ------
    %% ------

    %% --- Jump transfers ---
        %% --- Grounded ---
            JUMP_F --> HUB::AERIAL_ACTIONABLE
            JUMP_B --> HUB::AERIAL_ACTIONABLE
        %% ------

        %% --- Airborne ---
            JUMP_AERIAL_F --> HUB::AERIAL_ACTIONABLE
            JUMP_AERIAL_B --> HUB::AERIAL_ACTIONABLE
        %% ------
    %% ------

    %% --- Falling transfers ---
        %% --- Before jumping ---
            FALL --> HUB::AERIAL_ACTIONABLE

            FALL_F --> HUB::AERIAL_ACTIONABLE
            FALL_B --> HUB::AERIAL_ACTIONABLE
        %% ------

        %% --- After jumping ---
            FALL_AERIAL --> HUB::AERIAL_ACTIONABLE

            FALL_AERIAL_F --> HUB::AERIAL_ACTIONABLE
            FALL_AERIAL_B --> HUB::AERIAL_ACTIONABLE
        %% ------

        %% --- Freefalling to freefalling (drifts) ---
            FALL_SPECIAL --> FALL_SPECIAL_F
            FALL_SPECIAL --> FALL_SPECIAL_B
SQUAT
            FALL_SPECIAL_F --> FALL_SPECIAL
            FALL_SPECIAL_F --> FALL_SPECIAL_B

            FALL_SPECIAL_B --> FALL_SPECIAL
            FALL_SPECIAL_B --> FALL_SPECIAL_F
        %% ------

        %% --- Falling to landings ---
            FALL_SPECIAL --> LANDING_FALL_SPECIAL
            FALL_SPECIAL_F --> LANDING_FALL_SPECIAL
            FALL_SPECIAL_B --> LANDING_FALL_SPECIAL
        %% ------
    %% ------

    %% --- Tumbling/freefall transfers ---
        %% if hitstun frames run out
        DAMAGE_FALL -->|NO ECB collision interrupt| HUB::AERIAL_ACTIONABLE
        %% --- Missed tech ---
            DAMAGE_FALL --> DOWN_BOUND_U
            DAMAGE_FALL --> DOWN_BOUND_D

            DAMAGE_FALL --> FLY_REFLECT_WALL
            DAMAGE_FALL --> FLY_REFLECT_CEIL
        %% ------
        %% --- Successful tech ---
            DAMAGE_FALL --> PASSIVE
            DAMAGE_FALL --> PASSIVE_STAND_F
            DAMAGE_FALL --> PASSIVE_STAND_B
            DAMAGE_FALL --> PASSIVE_WALL
            DAMAGE_FALL --> PASSIVE_WALL_JUMP
            DAMAGE_FALL --> PASSIVE_CEIL
        %% ------
    %% ------

    %% --- Attack transfers ---
        %% --- Jab transfers ---
            ATTACK_11 --> HUB::GROUNDED_ACTIONABLE
            ATTACK_11 -->|ONLY if >1 jab| ATTACK_12
            ATTACK_11 -->|ONLY if 1 jab and has multijab| ATTACK_100_START

            ATTACK_12 --> HUB::GROUNDED_ACTIONABLE
            ATTACK_12 -->|ONLY if >2 jabs| ATTACK_13
            ATTACK_12 -->|ONLY if 2 jabs and has multijab| ATTACK_100_START

            ATTACK_13 --> HUB::GROUNDED_ACTIONABLE
            ATTACK_13 -->|ONLY if has multijab| ATTACK_100_START
            %% --- Multijabs ---
                ATTACK_100_START --> ATTACK_100_LOOP
                ATTACK_100_LOOP --> ATTACK_100_END
                ATTACK_100_END --> HUB::GROUNDED_ACTIONABLE
            %% ------
        %% ------

        ATTACK_DASH --> HUB::GROUNDED_ACTIONABLE

        %% --- Grounded transfers ---
            ATTACK_S_3_HI --> HUB::GROUNDED_ACTIONABLE
            ATTACK_S_3_HI_S --> HUB::GROUNDED_ACTIONABLE
            ATTACK_S_3_S --> HUB::GROUNDED_ACTIONABLE
            ATTACK_S_3_LW_S --> HUB::GROUNDED_ACTIONABLE
            ATTACK_S_3_LW --> HUB::GROUNDED_ACTIONABLE

            ATTACK_HI_3 --> HUB::GROUNDED_ACTIONABLE

            ATTACK_LW_3 --> HUB::GROUNDED_ACTIONABLE

            ATTACK_S_4_HI --> HUB::GROUNDED_ACTIONABLE
            ATTACK_S_4_HI_S --> HUB::GROUNDED_ACTIONABLE
            ATTACK_S_4_S --> HUB::GROUNDED_ACTIONABLE
            ATTACK_S_4_LW_S --> HUB::GROUNDED_ACTIONABLE
            ATTACK_S_4_LW --> HUB::GROUNDED_ACTIONABLE

            ATTACK_HI_4 --> HUB::GROUNDED_ACTIONABLE

            ATTACK_LW_4 --> HUB::GROUNDED_ACTIONABLE
        %% ------
        %% --- Aerial transfers ---
            %% --- Aerial landings ---
                ATTACK_AIR_N --> LANDING_AIR_N
                ATTACK_AIR_F --> LANDING_AIR_F
                ATTACK_AIR_B --> LANDING_AIR_B
                ATTACK_AIR_HI --> LANDING_AIR_HI
                ATTACK_AIR_LW --> LANDING_AIR_LW
            %% ------
        %% ------
    %% ------

    %% --- Item interaction transfers ---  
        LIGHT_GET -->|ONLY while grounded| HUB::GROUNDED_ACTIONABLE
        LIGHT_GET -->|ONLY while airborne| HUB::AERIAL_ACTIONABLE

        %% --- While grounded ---
            LIGHT_THROW_F --> HUB::GROUNDED_ACTIONABLE
            LIGHT_THROW_B --> HUB::GROUNDED_ACTIONABLE
            LIGHT_THROW_HI --> HUB::GROUNDED_ACTIONABLE
            LIGHT_THROW_LW --> HUB::GROUNDED_ACTIONABLE
            
            LIGHT_THROW_DASH --> HUB::GROUNDED_ACTIONABLE

            LIGHT_THROW_F_4 --> HUB::GROUNDED_ACTIONABLE
            LIGHT_THROW_B_4 --> HUB::GROUNDED_ACTIONABLE
            LIGHT_THROW_HI_4 --> HUB::GROUNDED_ACTIONABLE
            LIGHT_THROW_LW_4 --> HUB::GROUNDED_ACTIONABLE
        %% ------
        %% --- While airborne ---
            LIGHT_THROW_AIR_F --> HUB::AERIAL_ACTIONABLE
            LIGHT_THROW_AIR_B --> HUB::AERIAL_ACTIONABLE
            LIGHT_THROW_AIR_HI --> HUB::AERIAL_ACTIONABLE
            LIGHT_THROW_AIR_LW --> HUB::AERIAL_ACTIONABLE

            LIGHT_THROW_AIR_F_4 --> HUB::AERIAL_ACTIONABLE
            LIGHT_THROW_AIR_B_4 --> HUB::AERIAL_ACTIONABLE
            LIGHT_THROW_AIR_HI_4 --> HUB::AERIAL_ACTIONABLE
            LIGHT_THROW_AIR_LW_4 --> HUB::AERIAL_ACTIONABLE
        %% ------
        
        LIGHT_THROW_DROP -->|ONLY while grounded| HUB::GROUNDED_ACTIONABLE
        LIGHT_THROW_DROP -->|ONLY while airborne| HUB::AERIAL_ACTIONABLE
                    
        %% --- Using beam sword ---
            SWORD_SWING_1 -->|ONLY while grounded| HUB::GROUNDED_ACTIONABLE
            SWORD_SWING_3 -->|ONLY while grounded| HUB::GROUNDED_ACTIONABLE
            SWORD_SWING_4 -->|ONLY while grounded| HUB::GROUNDED_ACTIONABLE

            SWORD_SWING_1 -->|ONLY while airborne| HUB::AERIAL_ACTIONABLE
            SWORD_SWING_3 -->|ONLY while airborne| HUB::AERIAL_ACTIONABLE
            SWORD_SWING_4 -->|ONLY while airborne| HUB::AERIAL_ACTIONABLE

            SWORD_SWING_DASH --> HUB::GROUNDED_ACTIONABLE
        %% ------
    %% ------

    %% --- Shield transfers ---
        GUARD_ON --> GUARD
        GUARD_ON -->|ONLY if hit| GUARD_REFLECT

        GUARD --> GUARD_OFF
        GUARD --> GUARD_SET_OFF

        GUARD --> SHIELD_BREAK_FLY

        GUARD --> MISS_FOOT

        GUARD_OFF --> HUB::GROUNDED_ACTIONABLE
        GUARD_SET_OFF --> HUB::GROUNDED_ACTIONABLE
        GUARD_SET_OFF --> MISS_FOOT
        GUARD_REFLECT --> HUB::GROUNDED_ACTIONABLE
        GUARD_REFLECT --> MISS_FOOT

        SHIELD_BREAK_FLY --> SHIELD_BREAK_FALL

        SHIELD_BREAK_FALL --> SHIELD_BREAK_DOWN_U
        SHIELD_BREAK_FALL --> SHIELD_BREAK_DOWN_D

        SHIELD_BREAK_DOWN_U --> SHIELD_BREAK_STAND_U
        
        SHIELD_BREAK_DOWN_D --> SHIELD_BREAK_STAND_D

        SHIELD_BREAK_STAND_U --> FURA_FURA
        SHIELD_BREAK_STAND_D --> FURA_FURA

        MISS_FOOT --> OTTOTTO
    %% ------

    %% --- Missed tech transfers ---
        %% --- Face up ---
            DOWN_BOUND_U --> DOWN_WAIT_U

            DOWN_BOUND_U -->|ONLY if hit| DOWN_DAMAGE_U

            DOWN_BOUND_U --> DOWN_STAND_U
            DOWN_BOUND_U --> DOWN_ATTACK_U
            DOWN_BOUND_U --> DOWN_FOWARD_U
            DOWN_BOUND_U --> DOWN_BACK_U

            DOWN_STAND_U --> HUB::GROUNDED_ACTIONABLE
            DOWN_ATTACK_U --> HUB::GROUNDED_ACTIONABLE
            DOWN_FOWARD_U --> HUB::GROUNDED_ACTIONABLE
            DOWN_BACK_U --> HUB::GROUNDED_ACTIONABLE
        %% ------
        %% --- Face down ---
            DOWN_BOUND_D --> DOWN_WAIT_D

            DOWN_BOUND_D -->|ONLY if hit| DOWN_DAMAGE_D

            DOWN_BOUND_D --> DOWN_STAND_D
            DOWN_BOUND_D --> DOWN_ATTACK_D
            DOWN_BOUND_D --> DOWN_FOWARD_D
            DOWN_BOUND_D --> DOWN_BACK_D

            DOWN_STAND_D --> HUB::GROUNDED_ACTIONABLE
            DOWN_ATTACK_D --> HUB::GROUNDED_ACTIONABLE
            DOWN_FOWARD_D --> HUB::GROUNDED_ACTIONABLE
            DOWN_BACK_D --> HUB::GROUNDED_ACTIONABLE
        %% ------
    %% ------

    %% --- Sucessful tech transfers ---
        PASSIVE --> HUB::GROUNDED_ACTIONABLE
        PASSIVE_STAND_F --> HUB::GROUNDED_ACTIONABLE
        PASSIVE_STAND_B --> HUB::GROUNDED_ACTIONABLE
        PASSIVE_WALL --> HUB::GROUNDED_ACTIONABLE
        PASSIVE_WALL_JUMP --> HUB::GROUNDED_ACTIONABLE
        PASSIVE_CEIL --> HUB::GROUNDED_ACTIONABLE
    %% ------

    %% --- Grab transfers ---
        CATCH --> HUB::GROUNDED_ACTIONABLE
        CATCH --> CATCH_PULL

        CATCH_DASH --> CATCH_DASH_PULL

        CATCH_PULL --> CATCH_WAIT
        CATCH_PULL --> CATCH_ATTACK

        CATCH_DASH_PULL --> CATCH_WAIT
        CATCH_DASH_PULL --> CATCH_ATTACK

        CATCH_WAIT --> CATCH_ATTACK
        CATCH_WAIT --> CATCH_CUT
        CATCH_WAIT --> HUB::GRAB::THROWS

        CATCH_ATTACK --> CATCH_WAIT
        CATCH_ATTACK --> CATCH_CUT
        CATCH_ATTACK --> HUB::GRAB::THROWS
        %% if the opponent has sufficiently mashed then they're released at the end of the CATCH_ATTACK state

        CATCH_CUT --> HUB::GROUNDED_ACTIONABLE

        THROW_F --> HUB::GROUNDED_ACTIONABLE
        THROW_B --> HUB::GROUNDED_ACTIONABLE
        THROW_HI --> HUB::GROUNDED_ACTIONABLE
        THROW_LW --> HUB::GROUNDED_ACTIONABLE
    %% ------

    %% --- Grabbed transfers ---
        %% --- Tall opponent or off edge ---
            CAPTURE_PULLED_HI --> CAPTURE_WAIT_HI
            CAPTURE_PULLED_HI --> CAPTURE_DAMAGE_HI

            CAPTURE_WAIT_HI --> CAPTURE_DAMAGE_HI
            CAPTURE_WAIT_HI --> CAPTURE_CUT
            CAPTURE_WAIT_HI --> CAPTURE_JUMP
            CAPTURE_WAIT_HI --> HUB::GRABBED::THROWNS

            CAPTURE_DAMAGE_HI --> CAPTURE_WAIT_HI
            CAPTURE_DAMAGE_HI --> CAPTURE_CUT
            CAPTURE_DAMAGE_HI --> CAPTURE_JUMP
            CAPTURE_DAMAGE_HI --> HUB::GRABBED::THROWNS
        %% ------
        %% --- Shorter/same height opponent ---
            CAPTURE_PULLED_LW --> CAPTURE_WAIT_LW
            CAPTURE_PULLED_LW --> CAPTURE_DAMAGE_LW

            CAPTURE_WAIT_LW --> CAPTURE_DAMAGE_LW
            CAPTURE_WAIT_LW --> CAPTURE_CUT
            CAPTURE_WAIT_LW --> CAPTURE_JUMP
            CAPTURE_WAIT_LW --> HUB::GRABBED::THROWNS

            CAPTURE_DAMAGE_LW --> CAPTURE_WAIT_LW
            CAPTURE_DAMAGE_LW --> CAPTURE_CUT
            CAPTURE_DAMAGE_LW --> CAPTURE_JUMP
            CAPTURE_DAMAGE_LW --> HUB::GRABBED::THROWNS
        %% ------

        CAPTURE_CUT --> HUB::GROUNDED_ACTIONABLE
        CAPTURE_JUMP --> HUB::AERIAL_ACTIONABLE

        CAPTURE_NECK --> CAPTURE_WAIT_LW
        CAPTURE_FOOT --> CAPTURE_WAIT_LW
        %% ??

        THROWN_F --> DAMAGE_FALL
        THROWN_B --> DAMAGE_FALL
        THROWN_HI --> DAMAGE_FALL
        THROWN_LW --> DAMAGE_FALL

        %% --- Cargo carry ---
            THROWN_F -->|ONLY if thrown by DK| SHOULDERED_WAIT

            SHOULDERED_WAIT --> HUB::GRABBED::CARGO
            SHOULDERED_WALK_SLOW --> HUB::GRABBED::CARGO
            SHOULDERED_WALK_MIDDLE --> HUB::GRABBED::CARGO
            SHOULDERED_WALK_FAST --> HUB::GRABBED::CARGO
            SHOULDERED_TURN --> HUB::GRABBED::CARGO

            %% --- Thrown out of cargo carry ---
                THROWN_F_F --> DAMAGE_FALL
                THROWN_F_B --> DAMAGE_FALL
                THROWN_F_HI --> DAMAGE_FALL
                THROWN_F_LW --> DAMAGE_FALL
            %% ------
        %% ------
    %% ------

    %% --- Escape transfers ---
        %% --- Rolls ---
            ESCAPE_F --> HUB::GROUNDED_ACTIONABLE
            ESCAPE_B --> HUB::GROUNDED_ACTIONABLE
        %% ------
        ESCAPE --> HUB::GROUNDED_ACTIONABLE

        ESCAPE_AIR --> FALL_SPECIAL
        ESCAPE_AIR --> FALL_SPECIAL_F
        ESCAPE_AIR --> FALL_SPECIAL_B
    %% ------

    %% --- Ledge transfers ---
        CLIFF_CATCH --> HUB::LEDGE::GETUPS
        CLIFF_CATCH --> CLIFF_WAIT

        CLIFF_WAIT --> HUB::LEDGE::GETUPS
    %% ------

    %% --- Taunt transfers ---
        APPEAL_R --> HUB::GROUNDED_ACTIONABLE
        APPEAL_L --> HUB::GROUNDED_ACTIONABLE
    %% ------

    %% --- Caught transfers ---

        %% ? many of these have a kind of "jump" state when you exit where you aren't actionable?

        %% --- raptor boost ---
            CAPTURE_CAPTAIN --> DAMAGE_FALL
        %% ------
        %% --- egg lay ---
            CAPTURE_YOSHI --> YOSHI_EGG
            YOSHI_EGG --> HUB::AERIAL_ACTIONABLE
        %% ------
        %% --- koopa klaw ---
            %% --- grounded ---
                CAPTURE_KOOPA --> CAPTURE_DAMAGE_KOOPA
                CAPTURE_KOOPA --> CAPTURE_WAIT_KOOPA

                CAPTURE_DAMAGE_KOOPA --> THROWN_KOOPA_F
                CAPTURE_DAMAGE_KOOPA --> THROWN_KOOPA_B
                CAPTURE_WAIT_KOOPA --> THROWN_KOOPA_F
                CAPTURE_WAIT_KOOPA --> THROWN_KOOPA_B

                THROWN_KOOPA_F --> DAMAGE_FALL
                THROWN_KOOPA_B --> DAMAGE_FALL
                %% ? are you actionable out of klaw throw?
            %% ------
            %% --- airborne ---
                CAPTURE_KOOPA_AIR --> CAPTURE_DAMAGE_KOOPA_AIR
                CAPTURE_KOOPA_AIR --> CAPTURE_DAMAGE_KOOPA
                CAPTURE_KOOPA_AIR --> CAPTURE_WAIT_KOOPA_AIR
                CAPTURE_KOOPA_AIR --> CAPTURE_WAIT_KOOPA

                CAPTURE_DAMAGE_KOOPA_AIR --> THROWN_KOOPA_AIR_F
                CAPTURE_DAMAGE_KOOPA_AIR --> THROWN_KOOPA_AIR_B
                CAPTURE_WAIT_KOOPA_AIR --> THROWN_KOOPA_AIR_F
                CAPTURE_WAIT_KOOPA_AIR --> THROWN_KOOPA_AIR_B

                THROWN_KOOPA_AIR_F --> DAMAGE_FALL
                THROWN_KOOPA_AIR_B --> DAMAGE_FALL
                %% ? are you actionable out of klaw throw?
            %% ------
        %% ------
        %% --- swallow ---
            CAPTURE_KIRBY --> CAPTURE_WAIT_KIRBY

            CAPTURE_WAIT_KIRBY --> THROWN_KIRBY_STAR
            CAPTURE_WAIT_KIRBY --> THROWN_COPY_STAR
            CAPTURE_WAIT_KIRBY --> THROWN_KIRBY

            THROWN_KIRBY_STAR --> DAMAGE_FALL
            THROWN_COPY_STAR --> DAMAGE_FALL
            THROWN_KIRBY --> DAMAGE_FALL
            %% ? are you actionable out of swallow?
        %% ------
        %% --- buried ---
            BURY --> BURY_WAIT
            BURY_WAIT --> BURY_JUMP
            BURY_JUMP --> HUB::AERIAL_ACTIONABLE
        %% ------
        %% --- sing ---
            DAMAGE_SONG --> DAMAGE_SONG_WAIT
            DAMAGE_SONG_WAIT --> DAMAGE_SONG_RV
            DAMAGE_SONG_RV --> HUB::GROUNDED_ACTIONABLE
        %% ------
        %% --- disable ---
            DAMAGE_BIND --> HUB::GROUNDED_ACTIONABLE
            DAMAGE_BIND --> HUB::GROUNDED_ACTIONABLE

			%% ? i don't really know what THROWN_MEWTWO actually is?

            %% --- grounded ---
                CAPTURE_MEWTWO --> THROWN_MEWTWO

                THROWN_MEWTWO --> DAMAGE_FALL
            %% ------
            %% --- airborne ---
                CAPTURE_MEWTWO_AIR --> THROWN_MEWTWO_AIR
                CAPTURE_MEWTWO_AIR --> THROWN_MEWTWO

                THROWN_MEWTWO_AIR --> DAMAGE_FALL
            %% ------
        %% ------
        %% --- ice ---
            DAMAGE_ICE --> DAMAGE_ICE_JUMP

            DAMAGE_ICE_JUMP --> HUB::AERIAL_ACTIONABLE
        %% ------
        %% --- copied egg lay ---
            CAPTURE_KIRBY_YOSHI --> KIRBY_YOSHI_EGG
            KIRBY_YOSHI_EGG --> HUB::AERIAL_ACTIONABLE
        %% ------
    %% ------

    %% --- Entry transfers ---
        ENTRY_START --> ENTRY
        ENTRY --> ENTRY_END
        ENTRY_END --> HUB::AERIAL_ACTIONABLE
    %% ------

    %% ////// EVENTS //////
        EVENT::START(("Game starts"))

        EVENT::START --> ENTRY_START

        EVENT::HIT(("Hit"))

        %% hitlag states go here

		EVENT::DEATH(("Death"))

		EVENT::DEATH --> DEAD_DOWN
        EVENT::DEATH --> DEAD_LEFT
        EVENT::DEATH --> DEAD_RIGHT
        EVENT::DEATH --> DEAD_UP_STAR
        EVENT::DEATH --> DEAD_UP_FALL_HIT_CAMERA
        EVENT::DEATH --> DEAD_UP_FALL_HIT_CAMERA_FLAT
    %% ////// EVENTS //////
```