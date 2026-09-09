//! Explicit invented aerial resources for native orchestration tests. These
//! samples exercise the source callbacks; they are not extracted fighter data.
#![allow(dead_code)]
#[path = "conformance.rs"]
pub mod conformance;

use skirmish::{
    fighter::aerial::SelectionRules,
    game::{
        Action, Controller, Match,
        aerial::{FrameFlags, Move, Parameters},
        data::{Attack, AttackFrame, MatchData},
    },
};

pub const ATTACKS: [Action; 5] = [
    Action::AttackAirN,
    Action::AttackAirF,
    Action::AttackAirB,
    Action::AttackAirHi,
    Action::AttackAirLw,
];
pub const LANDINGS: [Action; 5] = [
    Action::LandingAirN,
    Action::LandingAirF,
    Action::LandingAirB,
    Action::LandingAirHi,
    Action::LandingAirLw,
];
pub const STICKS: [[f32; 2]; 5] = [[0.0; 2], [1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]];

pub fn data() -> MatchData {
    let mut data = conformance::data();
    data.provenance.push_str(" Aerial resources are explicit invented eight-frame attacks with distinct landing lags; no authentic fighter fidelity is asserted.");
    data.stage.spawns = [[-10.0, 100.0], [10.0, 100.0]];
    data.stage.blast = [-1000.0, 1000.0, -1000.0, 1000.0];
    for fighter in &mut data.fighters {
        fighter.aerials = Some(Parameters {
            selection: SelectionRules {
                thresholds: [0.3; 2],
                vertical_angle: core::f32::consts::FRAC_PI_4,
            },
            l_cancel_window: 3,
            l_cancel_divisor: 2.0,
            moves: core::array::from_fn(|index| Move {
                attack: Attack {
                    move_id: Some(20 + index as u16),
                    frames: vec![
                        AttackFrame {
                            bones: fighter.bones.clone(),
                            hitboxes: vec![]
                        };
                        8
                    ],
                },
                flags: vec![
                    FrameFlags {
                        landing_lag: true,
                        ..Default::default()
                    };
                    8
                ],
                landing_lag: 8.0 + 2.0 * index as f32,
                landing_animation_end: 9.9,
                landing_poses: vec![fighter.bones.clone(); 11],
            }),
        });
    }
    data
}

pub fn game() -> Match {
    Match::new(data(), 0).unwrap()
}

pub fn input(player: usize, controller: Controller) -> [Controller; 2] {
    let mut inputs = [Controller::default(); 2];
    inputs[player] = controller;
    inputs
}

pub fn step(game: &mut Match, controller: Controller) -> &skirmish::game::State {
    game.step(input(0, controller))
        .expect("explicit aerial scenario must execute")
}
