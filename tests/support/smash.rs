use skirmish::game::{
    data::{Attack, AttackFrame, Hitbox, MatchData},
    smash::{ChargeCommand, ForwardSmashes, Parameters, Rules, SmashAttack},
    tilt::GroundFrameFlags,
};

/// Invented smash rules; forward thresholds are radians for the folded stick
/// angle, the forward magnitude and window come from locomotion.
pub const RULES: Rules = Rules {
    forward_high: 0.4,
    forward_high_slight: 0.2,
    forward_low_slight: -0.2,
    forward_low: -0.4,
    up_stick_threshold: 0.7,
    up_window: 4.0,
    down_stick_threshold: -0.7,
    down_window: 4.0,
    charging_knockback_multiplier: 0.5,
};

/// Every attack charges on pose 2 for ten frames at up to 1.5x damage.
pub const CHARGE: ChargeCommand = ChargeCommand {
    frame: 2,
    hold_frames: 10.0,
    damage_multiplier: 1.5,
};

/// Hitbox on samples 4 and 5.
pub const HIT_FROM: usize = 4;
pub const HIT_TO: usize = 5;

fn hitbox() -> Hitbox {
    Hitbox {
        clank: false,
        rebound: false,
        element: Default::default(),
        group: 0,
        bone: 1,
        center: [1.5, 0.0, 0.0],
        radius: 1.25,
        damage: 10,
        shield_damage: 0,
        angle_degrees: 45.0,
        growth: 60,
        fixed: 0,
        base: 20,
    }
}

/// `frames` poses; samples 4 and 5 carry a forward hitbox; interrupts open
/// at `interrupt_from`; forward smashes carry the given TransN deltas.
pub fn attack(
    bones: &[skirmish::game::data::Bone],
    move_id: u16,
    frames: usize,
    interrupt_from: usize,
    root_translations: Option<Vec<f32>>,
) -> SmashAttack {
    SmashAttack {
        attack: Attack {
            move_id: Some(move_id),
            frames: (0..frames)
                .map(|frame| AttackFrame {
                    bones: bones.to_vec(),
                    hitboxes: if (HIT_FROM..=HIT_TO).contains(&frame) {
                        vec![hitbox()]
                    } else {
                        vec![]
                    },
                    hurtbox_states: vec![],
                })
                .collect(),
        },
        flags: (0..frames)
            .map(|frame| GroundFrameFlags {
                allow_interrupt: frame >= interrupt_from,
                repeat_ready: false,
            })
            .collect(),
        charge: Some(CHARGE),
        root_translations,
    }
}

/// The straight forward smash's TransN deltas: it steps forward on samples
/// 3..=5.
pub fn straight_roots() -> Vec<f32> {
    vec![0.0, 0.0, 0.0, 0.5, 1.0, 0.5, 0.0, 0.0, 0.0, 0.0]
}

/// Install every forward variant (distinct lengths identify them), an up
/// smash and a down smash on both fighters.
pub fn profile(mut data: MatchData) -> MatchData {
    data.rules.smash = Some(RULES);
    for fighter in &mut data.fighters {
        let bones = fighter.bones.clone();
        fighter.smashes = Some(Parameters {
            forward: ForwardSmashes {
                high: Some(attack(&bones, 9, 12, 9, None)),
                high_slight: Some(attack(&bones, 9, 11, 9, None)),
                straight: attack(&bones, 9, 10, 7, Some(straight_roots())),
                low_slight: Some(attack(&bones, 9, 13, 9, None)),
                low: Some(attack(&bones, 9, 14, 9, None)),
            },
            up: attack(&bones, 10, 9, 7, None),
            down: attack(&bones, 11, 8, 6, None),
        });
    }
    data
}
