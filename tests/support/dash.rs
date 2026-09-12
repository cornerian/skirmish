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
/// locomotion.json`'s `dash_run_frame` is 8, checked against `action_frame +
/// 1` (`game::dash::update_dash_or_run`'s real-replay-verified Dash-to-Run
/// timing fix), so it is already satisfied on the very frame the late phase
/// begins (`action_frame` 7) whenever the entry stick is still held. The
/// late-phase re-dash/Turn and shield tests below reach their own checks
/// first (`try_dash`/the shield-entry buffer, both ahead of the run
/// transition in `Phase::Late`), so they are unaffected; a scenario that
/// reaches Phase::Late with a fresh forward stick and *no* competing check
/// ahead of it (the `rules.dash = None` fallback below, where `try_dash` is
/// unreachable) must stay one frame short of `action_frame` 7 instead, or it
/// enters Run rather than exercising the scenario it's testing.
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
