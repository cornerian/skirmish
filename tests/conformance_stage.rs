//! Required integration behavior still absent from the experimental scheduler.
//! Run explicitly with `cargo test --test conformance_stage -- --ignored`.
#[path = "support/reference_scenario.rs"]
mod reference;
#[path = "support/conformance.rs"]
mod support;

use skirmish::{
    collision::stage,
    fighter::nudge::Rules as NudgeRules,
    game::{BUTTON_X, Event, Match, data::StageGeometry, nudge::Attributes as NudgeAttributes},
};
use support::*;

// ftCo_Pass.c / ftCo_Squat.c: downward stick enters Pass on a passable floor.
#[test]
fn downward_input_drops_through_a_passable_platform() {
    let mut data = data();
    data.stage.spawns = [[0.0, 6.0], [20.0, 6.0]];
    data.stage.geometry = Some(StageGeometry {
        lines: vec![stage::Line {
            start: [-50.0, 6.0],
            end: [50.0, 6.0],
            flags: stage::FLOOR | stage::ENABLED,
            material_flags: stage::PLATFORM as u16,
            ..Default::default()
        }],
        joints: vec![stage::Joint {
            id: 0,
            flags: stage::ENABLED,
            bounds_min: [-100.0, -100.0],
            bounds_max: [100.0, 100.0],
            floor: 0..1,
            ..Default::default()
        }],
    });
    let mut game = Match::new(data, 0).unwrap();
    assert!(game.state().fighters[0].grounded);
    step(&mut game, idle());
    let mut input = idle();
    input[0].stick[1] = -1.0;
    for _ in 0..8 {
        step(&mut game, input);
    }
    let fighter = &game.state().fighters[0];
    assert!(
        !fighter.grounded && fighter.position[1] < 6.0,
        "Pass must remove support and move below the platform: {fighter:?}"
    );
}

// Fighter_procUpdate uses fixed push velocities. It does not solve penetration
// or guarantee that arbitrary walk speeds cannot cross, so this contract checks
// the exact bounded displacement and strict-touching cutoff supplied by E0E4.
#[test]
fn grounded_opponents_receive_source_fixed_push_before_movement() {
    let mut data = data();
    data.stage.spawns = [[0.0, 0.0], [1.75, 0.0]];
    data.rules.nudge = Some(NudgeRules {
        horizontal_step: 0.25,
        depth_step: 0.125,
        depth_limit: 0.5,
        follower_depth_step: 0.375,
        follower_depth_limit: 1.0,
    });
    for fighter in &mut data.fighters {
        fighter.nudge = Some(NudgeAttributes {
            center_offset: 0.0,
            half_width: 1.0,
            nudge_disabled: false,
            overlap_disabled: false,
        });
    }
    let mut game = Match::new(data, 0).unwrap();
    let state = step(&mut game, idle());
    assert_eq!(state.fighters.each_ref().map(|f| f.nudge[0]), [-0.25, 0.25]);
    assert_eq!(
        state.fighters.each_ref().map(|f| f.position[0]),
        [-0.25, 2.0]
    );
}

// src/melee/ft/ft_0D31.c: crossing the upper boundary in an ordinary jump does not
// satisfy the damaged-upward-launch death condition.
#[test]
fn ordinary_jump_above_the_upper_blast_line_preserves_the_stock() {
    let mut data = data();
    data.stage.blast[3] = 5.0;
    let mut game = Match::new(data, 0).unwrap();
    let stocks = game.state().fighters[0].stocks;
    let mut input = idle();
    input[0].buttons = BUTTON_X;
    let mut crossed = false;
    for _ in 0..20 {
        let state = step(&mut game, input);
        crossed |= state.fighters[0].position[1] > 5.0;
        assert_eq!(
            state.fighters[0].stocks, stocks,
            "ordinary jump was incorrectly treated as a top KO"
        );
    }
    assert!(crossed, "scenario must actually cross the upper blast line");
}

// src/melee/ft/ft_0D4D.c: rebirth waits above the stage on its platform, with an
// invulnerable fighter; it does not immediately respawn grounded at the spawn.
#[test]
#[ignore = "unimplemented: rebirth platform lifecycle"]
fn lost_stock_returns_on_an_airborne_rebirth_platform() {
    let mut data = data();
    data.stage.floor.left = -5.0;
    data.stage.floor.right = 5.0;
    data.stage.spawns = [[4.0, 0.0], [-4.0, 0.0]];
    data.stage.blast[1] = 6.0;
    let mut game = Match::new(data, 0).unwrap();
    let mut input = idle();
    input[0].stick[0] = 0.5;
    let mut lost = false;
    for _ in 0..40 {
        let state = step(&mut game, if lost { idle() } else { input });
        lost |= state.fighters[0].stocks < 4;
        if state.events.contains(&Event::Respawned { player: 0 }) {
            let fighter = &state.fighters[0];
            assert!(
                lost && !fighter.grounded && fighter.position[1] > 3.0 && fighter.invincibility > 0,
                "rebirth platform state missing: {fighter:?}"
            );
            return;
        }
    }
    panic!("scenario never reached rebirth");
}

#[test]
#[ignore = "blocked: independent moving-platform carry/remapping reference required"]
fn moving_platform_carries_a_supported_fighter() {
    reference::assert_reference("moving_platform_carry");
}

#[test]
#[ignore = "blocked: independent multi-surface ECB corner reference required"]
fn simultaneous_corner_contacts_resolve_in_source_order() {
    reference::assert_reference("ecb_corner_resolution");
}

#[test]
#[ignore = "blocked: independent moving-surface squeeze reference required"]
fn squeezing_surfaces_apply_the_full_ecb_response() {
    reference::assert_reference("ecb_squeeze_response");
}

#[test]
#[ignore = "blocked: independent star/screen death lifecycle reference required"]
fn upward_damage_ko_runs_the_selected_star_or_screen_death_lifecycle() {
    reference::assert_reference("star_screen_death");
}
