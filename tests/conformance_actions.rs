//! Executable gaps in native action dispatch. The shared world is synthetic;
//! these assert action/lifecycle semantics, not authentic character numerics.
//! Every ignored test must become a normal passing test when its gap is ported.

#[path = "support/aerial.rs"]
mod aerial_resources;
#[path = "support/grab.rs"]
mod grab_resources;
#[path = "support/ledge.rs"]
mod ledge_resources;
#[path = "support/special.rs"]
mod special_resources;
use aerial_resources::conformance as support;

use skirmish::game::{BUTTON_A, BUTTON_X, Controller, Match, State};
use support::{action, data, game, idle, step};

// Original physical PAD masks: src/sysdolphin/baselib/controller.h.
const BUTTON_B: u16 = 0x0200;
const BUTTON_Z: u16 = 0x0010;
const BUTTON_L: u16 = 0x0040;

fn input(buttons: u16, stick: [f32; 2]) -> [Controller; 2] {
    [
        Controller {
            cstick: [0.0; 2],
            trigger: 0.0,
            buttons,
            stick,
        },
        Controller::default(),
    ]
}

fn airborne_game() -> Match {
    let mut game = game();
    let startup = game.data().fighters[0].movement.jump_startup_frames;
    for _ in 0..startup + 3 {
        let state = step(&mut game, input(BUTTON_X, [0.0; 2]));
        if !state.fighters[0].grounded {
            return game;
        }
    }
    assert!(
        !game.state().fighters[0].grounded,
        "ground jump must launch before aerial action checks"
    );
    game
}

// src/melee/ft/kinds/ftCommon/ftCo_Dash.c: ftCo_Dash_CheckInput/Enter.
#[test]
fn forward_flick_enters_dash_and_moves_forward() {
    let mut game = game();
    let x = game.state().fighters[0].position[0];
    let state = step(&mut game, input(0, [1.0, 0.0]));
    assert_eq!(action(&state, 0), "dash");
    assert!(state.fighters[0].ground_velocity > 0.0);
    // Dash Enter changes gr_accel2, integrated after this frame's ground
    // projection; the new speed displaces the fighter on the next callback.
    let state = step(&mut game, input(0, [1.0, 0.0]));
    assert!(state.fighters[0].position[0] > x);
}

// src/melee/ft/kinds/ftCommon/ftCo_Turn.c: ftCo_Turn_Enter_Basic/Anim_Inner.
#[test]
fn backward_tilt_enters_turn_before_reversing_facing() {
    let mut game = game();
    let facing = game.state().fighters[0].facing;
    let state = step(&mut game, input(0, [-0.5, 0.0]));
    assert_eq!(action(&state, 0), "turn");
    assert_eq!(state.fighters[0].facing, facing);
    for _ in 0..60 {
        step(&mut game, idle());
    }
    assert_eq!(game.state().fighters[0].facing, -facing);
}

// src/melee/ft/kinds/ftCommon/{ftCo_Squat,ftCo_SquatWait,ftCo_SquatRv}.c.
#[test]
fn down_stick_crouches_and_release_returns_to_wait() {
    let mut game = game();
    let state = step(&mut game, input(0, [0.0, -1.0]));
    assert_eq!(action(&state, 0), "squat");
    assert!(state.fighters[0].grounded);
    for _ in 0..60 {
        step(&mut game, idle());
    }
    assert_eq!(action(game.state(), 0), "wait");
}

// src/melee/ft/kinds/ftCommon/ftCo_JumpAerial.c: ft_did_jump and jump entry.
#[test]
fn fresh_jump_press_in_air_restores_upward_velocity() {
    let mut game = airborne_game();
    // Let gravity lower the first jump beneath the aerial jump's launch speed;
    // JumpAerial itself applies gravity on its entry callback.
    step(&mut game, idle());
    let before = step(&mut game, idle());
    let after = step(&mut game, input(BUTTON_X, [0.0; 2]));
    assert_eq!(action(&after, 0), "jump_aerial");
    assert!(!after.fighters[0].grounded);
    assert!(after.fighters[0].velocity[1] > before.fighters[0].velocity[1]);
}

// src/melee/ft/kinds/ftCommon/ftCo_AttackAir.c: neutral selection and entry.
#[test]
fn neutral_attack_in_air_selects_neutral_aerial() {
    let mut game = aerial_resources::game();
    step(&mut game, idle());
    let state = step(&mut game, input(BUTTON_A, [0.0; 2]));
    assert_eq!(action(&state, 0), "attack_air_n");
    assert!(!state.fighters[0].grounded);
}

// src/melee/ft/kinds/ftCommon/ftCo_AttackAir.c: GetMsidFromCStick uses the
// main stick when the C-stick did not initiate the attack.
#[test]
fn directional_aerials_select_forward_back_up_and_down_states() {
    for (stick, expected) in [
        ([1.0, 0.0], "attack_air_f"),
        ([-1.0, 0.0], "attack_air_b"),
        ([0.0, 1.0], "attack_air_hi"),
        ([0.0, -1.0], "attack_air_lw"),
    ] {
        let mut game = aerial_resources::game();
        step(&mut game, idle());
        let state = step(&mut game, input(BUTTON_A, stick));
        assert_eq!(action(&state, 0), expected);
    }
}

// src/melee/ft/kinds/ftCommon/ftCo_Attack100.c routes neutral B to callbacks;
// e.g. src/melee/ft/kinds/ftCaptain/ftcaptainspecialn.c provides SpecialN.
#[test]
fn grounded_neutral_b_enters_a_character_special() {
    let mut game = Match::new(special_resources::profile(data()), 0).unwrap();
    let state = step(&mut game, input(BUTTON_B, [0.0; 2]));
    assert!(action(&state, 0).starts_with("special"));
}

// src/melee/ft/kinds/ftCommon/ftCo_SpecialAir.c dispatches aerial specials.
#[test]
fn airborne_neutral_b_enters_an_aerial_special() {
    let mut game = special_resources::airborne_game(data());
    let state = step(&mut game, input(BUTTON_B, [0.0; 2]));
    assert!(action(&state, 0).starts_with("special_air"));
    assert!(!state.fighters[0].grounded);
}

// src/melee/ft/kinds/ftCommon/ftCo_Guard.c: GuardOn/Guard/GuardOff dispatch.
#[test]
fn digital_shoulder_raises_shield_and_release_lowers_it() {
    let mut game = game();
    let state = step(&mut game, input(BUTTON_L, [0.0; 2]));
    assert!(action(&state, 0).starts_with("guard"));
    for _ in 0..60 {
        step(&mut game, idle());
    }
    assert_eq!(action(game.state(), 0), "wait");
}

fn grabbed_game() -> Match {
    let mut data = grab_resources::profile(data());
    data.stage.spawns = [[0.0, 0.0], [1.0, 0.0]];
    let mut game = Match::new(data, 0).unwrap();
    step(&mut game, input(BUTTON_Z, [0.0; 2]));
    for _ in 0..60 {
        if action(game.state(), 0) == "catch_wait" {
            break;
        }
        step(&mut game, idle());
    }
    assert_eq!(
        action(game.state(), 0),
        "catch_wait",
        "successful grab must reach the hold state"
    );
    assert!(action(game.state(), 1).starts_with("capture"));
    game
}

// src/melee/ft/kinds/ftCommon/{ftCo_Catch,ftCo_CatchPull,ftCo_CapturePulled}.c;
// Fighter_Spaghetti_8006AD10 expands physical Z to A plus HSD_PAD_LR.
#[test]
fn grab_contact_places_attacker_and_victim_in_paired_hold_states() {
    let game = grabbed_game();
    assert_eq!(game.state().fighters[1].percent, 0.0);
    assert_eq!(game.state().fighters[0].stocks, 4);
    assert_eq!(game.state().fighters[1].stocks, 4);
}

// src/melee/ft/kinds/ftCommon/{ftCo_Throw,ftCo_Thrown}.c: direction selection,
// throw animation event, and release apply damage/knockback to the held fighter.
#[test]
fn forward_throw_releases_the_victim_with_damage_and_knockback() {
    let mut game = grabbed_game();
    let state = step(&mut game, input(0, [1.0, 0.0]));
    assert_eq!(action(&state, 0), "throw_f");
    let mut released = false;
    for _ in 0..120 {
        let state = step(&mut game, idle());
        if state.fighters[1].percent > 0.0 && state.fighters[1].knockback[0] > 0.0 {
            released = true;
            assert!(!action(&state, 1).starts_with("capture"));
            break;
        }
    }
    assert!(
        released,
        "the throw animation must release and launch its captured target"
    );
}

fn ledge_game() -> Match {
    let mut data = ledge_resources::profile(data());
    data.stage.floor.left = -2.0;
    data.stage.floor.right = 2.0;
    data.stage.spawns = [[-1.9, 1.0], [1.0, 0.0]];
    let mut game = Match::new(data, 0).unwrap();
    // Start inside the legal spawn bounds, then drift across the left edge.
    step(&mut game, input(0, [-1.0, 0.0]));
    for _ in 0..60 {
        if action(game.state(), 0) == "cliff_wait" {
            break;
        }
        step(&mut game, idle());
    }
    assert_eq!(
        action(game.state(), 0),
        "cliff_wait",
        "descending beside a free ledge must enter the ledge hang state"
    );
    // CliffWait requires a neutral stick sample before climb/drop requests.
    step(&mut game, idle());
    game
}

// src/melee/ft/ftcliffcommon.c and kinds/ftCommon/ftCo_CliffWait.c.
#[test]
fn descending_fighter_catches_a_free_ledge_and_stops_falling() {
    let mut game = ledge_game();
    let state = step(&mut game, idle());
    assert_eq!(action(&state, 0), "cliff_wait");
    assert_eq!(state.fighters[0].velocity[1], 0.0);
}

fn ledge_option(buttons: u16, stick: [f32; 2]) -> State {
    let mut game = ledge_game();
    step(&mut game, input(buttons, stick))
}

// src/melee/ft/kinds/ftCommon/ftCo_CliffClimb.c.
#[test]
fn stick_toward_stage_selects_ledge_climb() {
    assert!(action(&ledge_option(0, [1.0, 0.0]), 0).starts_with("cliff_climb"));
}

// src/melee/ft/kinds/ftCommon/ftCo_CliffJump.c.
#[test]
fn jump_button_selects_ledge_jump() {
    assert!(action(&ledge_option(BUTTON_X, [0.0; 2]), 0).starts_with("cliff_jump"));
}

// src/melee/ft/kinds/ftCommon/ftCo_CliffAttack.c.
#[test]
fn attack_button_selects_ledge_attack() {
    assert!(action(&ledge_option(BUTTON_A, [0.0; 2]), 0).starts_with("cliff_attack"));
}

// src/melee/ft/kinds/ftCommon/ftCo_CliffEscape.c.
#[test]
fn shoulder_button_selects_ledge_roll() {
    assert!(action(&ledge_option(BUTTON_L, [0.0; 2]), 0).starts_with("cliff_escape"));
}

// src/melee/ft/kinds/ftCommon/ftCo_CliffClimb.c: ftCo_8009AA0C lets go
// when stick displacement exceeds the threshold away from the ledge.
#[test]
fn down_stick_releases_the_ledge_and_starts_falling() {
    let state = ledge_option(0, [0.0, -1.0]);
    assert!(matches!(action(&state, 0).as_str(), "fall" | "damage_fall"));
    assert!(!state.fighters[0].grounded);
}

// src/melee/ft/kinds/ftCommon/{ftCo_Jump,ftCo_KneeBend}.c: tap-jump input
// uses a stick threshold/window and remembers its distinct short-hop source.
#[test]
fn upward_stick_flick_without_buttons_starts_jump_squat_and_launches() {
    let mut game = game();
    let startup = game.data().fighters[0].movement.jump_startup_frames;
    let state = step(&mut game, input(0, [0.0, 1.0]));
    assert_eq!(action(&state, 0), "jump_squat");
    for _ in 0..startup + 2 {
        step(&mut game, input(0, [0.0, 1.0]));
    }
    assert!(!game.state().fighters[0].grounded);
    assert!(game.state().fighters[0].velocity[1] > 0.0);
}

// src/melee/ft/kinds/ftCommon/ftCo_Dash.c: timer_lstick_tilt_x prevents a
// gradual tilt from being treated as a fresh smash, even at the same final XY.
#[test]
fn gradual_tilt_walks_but_a_fresh_flick_dashes_at_the_same_stick_value() {
    let mut gradual = game();
    for amount in 1..=20 {
        step(&mut gradual, input(0, [amount as f32 / 20.0, 0.0]));
    }
    let mut flick = game();
    let fast = step(&mut flick, input(0, [1.0, 0.0]));
    assert_eq!(action(gradual.state(), 0), "walk");
    assert_eq!(action(&fast, 0), "dash");
}
