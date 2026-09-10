use skirmish::game::{
    data::{Attack, AttackFrame, Hitbox, MatchData},
    ledge::{Attachment, AttackMotion, Frame, Jump, Motion, Parameters, Rules},
};

pub fn profile(mut data: MatchData) -> MatchData {
    data.rules.ledge = Some(Rules {
        catch_horizontal_range: 1.0,
        catch_vertical_below: 20.0,
        catch_vertical_above: 1.5,
        catch_down_threshold: 0.7,
        option_stick_threshold: 0.3,
        option_angle_radians: 0.7,
        wait_frames: 30,
        slow: None,
        regrab_cooldown: 6,
        intangibility_frames: 12,
    });
    for fighter in &mut data.fighters {
        let bones = fighter.bones.clone();
        let frame = |anchor_offset| Frame {
            bones: bones.clone(),
            anchor_offset,
        };
        let motion = |offsets: &[[f32; 3]]| Motion {
            frames: offsets.iter().copied().map(frame).collect(),
        };
        let attack_offsets = [
            [-0.35, -0.7, 0.0],
            [-0.1, -0.4, 0.0],
            [0.35, -0.1, 0.0],
            [0.7, 0.0, 0.0],
            [0.8, 0.0, 0.0],
        ];
        let attack = Attack {
            move_id: None,
            frames: attack_offsets
                .iter()
                .enumerate()
                .map(|(index, _)| AttackFrame {
                    bones: bones.clone(),
                    hitboxes: if index == 1 {
                        vec![Hitbox {
                            clank: false,
                            rebound: false,
                            group: 0,
                            bone: 0,
                            center: [1.0, 0.8, 0.0],
                            radius: 1.0,
                            damage: 7,
                            shield_damage: 0,
                            angle_degrees: 45.0,
                            growth: 50,
                            fixed: 0,
                            base: 30,
                        }]
                    } else {
                        vec![]
                    },
                    hurtbox_states: vec![],
                })
                .collect(),
        };
        fighter.ledge = Some(Parameters {
            attachment: Attachment {
                bone: 0,
                point: [0.0; 3],
            },
            catch: motion(&[[-0.25, -0.5, 0.0], [-0.35, -0.7, 0.0]]),
            wait: frame([-0.35, -0.7, 0.0]),
            climb: motion(&[
                [-0.35, -0.7, 0.0],
                [-0.1, -0.3, 0.0],
                [0.35, -0.05, 0.0],
                [0.8, 0.0, 0.0],
            ]),
            jump: Jump {
                motion: motion(&[
                    [-0.35, -0.7, 0.0],
                    [-0.1, -0.4, 0.0],
                    [0.2, -0.1, 0.0],
                    [0.6, 0.2, 0.0],
                ]),
                release_frame: 2,
                launch_velocity: [0.8, 2.0],
            },
            attack: AttackMotion {
                attack,
                anchor_offsets: attack_offsets.to_vec(),
            },
            escape: motion(&[
                [-0.35, -0.7, 0.0],
                [0.0, -0.3, 0.0],
                [0.8, 0.0, 0.0],
                [1.6, 0.0, 0.0],
            ]),
            slow: None,
        });
    }
    data
}
