use skirmish::game::{
    dash::{DashAttack, Rules},
    data::{Attack, AttackFrame, Bone, Hitbox, MatchData},
    grab::ShieldGrabRules,
    tilt::GroundFrameFlags,
};

/// Invented dash rules: `early_frames` (x44), `roll_frames` (x48),
/// `transition_friction` (x54) and `attack_friction_multiplier` (x50).
pub const RULES: Rules = Rules {
    early_frames: 3.0,
    roll_frames: 2.0,
    transition_friction: 0.25,
    attack_friction_multiplier: 2.0,
};

/// `rules.grab.shield_grab` supplies the middle-phase limit (x4C) and the
/// AttackDash catch buffer (x68); the base grab profile leaves it `None`, so
/// the dash profile installs its own value here. `tests/fixtures/game/
/// locomotion.json`'s `dash_run_frame` is 8: a limit of 8.0 would make the
/// Dash-to-Run transition fire on the very frame the late phase begins
/// whenever the entry stick is still held, confounding the late-phase
/// re-dash and shield tests below. 6.0 keeps a clean late-phase window
/// (frames 7) before that transition can apply.
pub const BUFFER: ShieldGrabRules = ShieldGrabRules {
    dash_buffer_frames: 5.0,
    dash_buffer_frame_limit: 6.0,
};

pub const FRAMES: usize = 8;
pub const HIT_FROM: usize = 3;
pub const HIT_TO: usize = 4;
pub const INTERRUPT_FROM: usize = 6;

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
        growth: 50,
        fixed: 0,
        base: 15,
    }
}

/// `FRAMES` poses; samples `HIT_FROM..=HIT_TO` carry a forward hitbox;
/// interrupts open at `INTERRUPT_FROM`.
pub fn attack(bones: &[Bone], root_translations: Option<Vec<f32>>) -> DashAttack {
    DashAttack {
        attack: Attack {
            move_id: Some(5),
            frames: (0..FRAMES)
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
        flags: (0..FRAMES)
            .map(|frame| GroundFrameFlags {
                allow_interrupt: frame >= INTERRUPT_FROM,
                repeat_ready: false,
            })
            .collect(),
        root_translations,
    }
}

/// Install the dash rules, the shield-grab buffer/limit and a dash attack on
/// both fighters. Callers must already have installed grab and locomotion.
pub fn profile(mut data: MatchData) -> MatchData {
    data.rules.dash = Some(RULES);
    data.rules
        .grab
        .as_mut()
        .expect("the dash profile requires grab rules")
        .shield_grab = Some(BUFFER);
    for fighter in &mut data.fighters {
        let bones = fighter.bones.clone();
        fighter.dash_attack = Some(attack(&bones, None));
    }
    data
}
