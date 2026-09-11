use skirmish::game::{
    data::{Attack, AttackFrame, Bone, Hitbox, MatchData},
    jab::{Attributes, FrameFlags, JabAttack, Parameters, RapidJab, Script},
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

/// `frames` poses; samples `hit_from..=hit_to` carry a forward hitbox (an
/// empty range gives none); native move identity `move_id`.
fn build(bones: &[Bone], move_id: u16, frames: usize, hit_from: usize, hit_to: usize) -> Attack {
    Attack {
        move_id: Some(move_id),
        frames: (0..frames)
            .map(|frame| AttackFrame {
                bones: bones.to_vec(),
                hitboxes: if (hit_from..=hit_to).contains(&frame) {
                    vec![hitbox()]
                } else {
                    vec![]
                },
                hurtbox_states: vec![],
            })
            .collect(),
    }
}

/// Second jab: six poses, a hitbox on 2..=3, interruptible from 4, the
/// follow-up flag from 2, root motion on every pose. Its own entry pose (0)
/// turns the rapid flag back off.
pub fn second(bones: &[Bone]) -> JabAttack {
    JabAttack {
        attack: build(bones, 2, 6, 2, 3),
        script: Script {
            flags: (0..6)
                .map(|frame| FrameFlags {
                    allow_interrupt: frame >= 4,
                    follow_up_ready: frame >= 2,
                    rapid: if frame == 0 { Some(false) } else { None },
                    loop_check: false,
                    clear_hits: false,
                })
                .collect(),
            root_translations: Some(vec![0.0, 0.5, 1.0, 0.5, 0.0, 0.0]),
        },
    }
}

/// Third jab: seven poses, a hitbox on 2..=3, interruptible from 5. It has no
/// follow-up (nothing chains after it); pose 3 clears its own hit group so
/// the pair on 2..=3 can hit the same victim twice within one jab.
pub fn third(bones: &[Bone]) -> JabAttack {
    JabAttack {
        attack: build(bones, 3, 7, 2, 3),
        script: Script {
            flags: (0..7)
                .map(|frame| FrameFlags {
                    allow_interrupt: frame >= 5,
                    follow_up_ready: false,
                    rapid: None,
                    loop_check: false,
                    clear_hits: frame == 3,
                })
                .collect(),
            root_translations: None,
        },
    }
}

/// The rapid jab: a three-pose start with no hitbox, a four-pose cycle with
/// a hitbox on pose 1 and the loop's continuation check on pose 3, and a
/// three-pose end with no hitbox. All three share one native move identity.
pub fn rapid(bones: &[Bone]) -> RapidJab {
    let inert = |frames| JabAttack {
        attack: build(bones, 4, frames, 1, 0),
        script: Script {
            flags: vec![FrameFlags::default(); frames],
            root_translations: None,
        },
    };
    RapidJab {
        start: inert(3),
        cycle: JabAttack {
            attack: build(bones, 4, 4, 1, 1),
            script: Script {
                flags: (0..4)
                    .map(|frame| FrameFlags {
                        allow_interrupt: false,
                        follow_up_ready: false,
                        rapid: None,
                        loop_check: frame == 3,
                        clear_hits: false,
                    })
                    .collect(),
                root_translations: None,
            },
        },
        end: inert(3),
    }
}

/// Install a jab combo on both fighters: the fixture's existing `jab` attack
/// becomes the first jab (allow_interrupt and the follow-up flag both from
/// pose 3, the rapid flag raised on pose 0, applied at entry), plus the
/// second, third and rapid jabs above.
pub fn profile(mut data: MatchData) -> MatchData {
    for fighter in &mut data.fighters {
        let bones = fighter.bones.clone();
        let first_frames = fighter.jab.frames.len();
        fighter.jab_combo = Some(Parameters {
            attributes: Attributes {
                second_window: 6.0,
                third_window: 6.0,
                rapid_window: 3,
            },
            first: Script {
                flags: (0..first_frames)
                    .map(|frame| FrameFlags {
                        allow_interrupt: frame >= 3,
                        follow_up_ready: frame >= 3,
                        rapid: if frame == 0 { Some(true) } else { None },
                        loop_check: false,
                        clear_hits: false,
                    })
                    .collect(),
                root_translations: None,
            },
            second: Some(second(&bones)),
            third: Some(third(&bones)),
            rapid: Some(rapid(&bones)),
        });
    }
    data
}
