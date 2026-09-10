use skirmish::game::{
    data::{Attack, AttackFrame, Hitbox, MatchData},
    tilt::{ForwardTilts, GroundAttack, GroundFrameFlags, Parameters, Rules},
};

/// Invented tilt rules; thresholds are radians for the folded stick angle.
pub const RULES: Rules = Rules {
    forward_stick_threshold: 0.5,
    angle_limit: 0.5,
    forward_high: 0.4,
    forward_high_slight: 0.2,
    forward_low_slight: -0.2,
    forward_low: -0.4,
    up_stick_threshold: 0.5,
    down_stick_threshold: -0.5,
};

fn hitbox() -> Hitbox {
    Hitbox {
        clank: false,
        rebound: false,
        element: Default::default(),
        group: 0,
        bone: 1,
        center: [1.5, 0.0, 0.0],
        radius: 1.25,
        damage: 9,
        shield_damage: 0,
        angle_degrees: 45.0,
        growth: 60,
        fixed: 0,
        base: 20,
    }
}

/// `frames` poses; frames 1 and 2 carry a forward hitbox; interrupts open at
/// `interrupt_from`; the repeat flag opens at `repeat_from` when given.
pub fn attack(
    bones: &[skirmish::game::data::Bone],
    move_id: u16,
    frames: usize,
    interrupt_from: usize,
    repeat_from: Option<usize>,
) -> GroundAttack {
    GroundAttack {
        attack: Attack {
            move_id: Some(move_id),
            frames: (0..frames)
                .map(|frame| AttackFrame {
                    bones: bones.to_vec(),
                    hitboxes: if (1..=2).contains(&frame) {
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
                repeat_ready: repeat_from.is_some_and(|from| frame >= from),
            })
            .collect(),
    }
}

/// Install every forward variant (distinct lengths identify them), an up tilt
/// and a repeatable down tilt on both fighters.
pub fn profile(mut data: MatchData) -> MatchData {
    data.rules.tilt = Some(RULES);
    for fighter in &mut data.fighters {
        let bones = fighter.bones.clone();
        fighter.tilts = Some(Parameters {
            forward: ForwardTilts {
                high: Some(attack(&bones, 6, 9, 6, None)),
                high_slight: Some(attack(&bones, 6, 8, 6, None)),
                straight: attack(&bones, 6, 7, 4, None),
                low_slight: Some(attack(&bones, 6, 10, 6, None)),
                low: Some(attack(&bones, 6, 11, 6, None)),
            },
            up: attack(&bones, 7, 6, 4, None),
            down: attack(&bones, 8, 6, 4, Some(3)),
        });
    }
    data
}
