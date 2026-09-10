use skirmish::game::{
    data::{Capsule, MatchData},
    grab::{
        Attachment, Catch, CatchFrame, Escape, EscapeRules, Parameters, Pummel, Rules, Throw,
        ThrowHit, Throws,
    },
};

pub fn profile(mut data: MatchData) -> MatchData {
    data.rules.grab = Some(Rules {
        horizontal_threshold: 0.7,
        up_threshold: 0.6,
        down_threshold: -0.6,
        escape: EscapeRules {
            timer_base: 100.0,
            timer_percent_scale: 0.5,
            timer_decrement: 1.0,
            mash_penalty: 10.0,
            stick_threshold: 0.7,
            release_speed: 0.4,
        },
    });
    for fighter in &mut data.fighters {
        let bones = fighter.bones.clone();
        let catch = Catch {
            frames: (0..4)
                .map(|frame| CatchFrame {
                    bones: bones.clone(),
                    grabboxes: if frame < 3 {
                        vec![Capsule {
                            bone: 1,
                            start: [1.0, 0.0, 0.0],
                            end: [1.0, 0.0, 0.0],
                            radius: 0.8,
                        }]
                    } else {
                        vec![]
                    },
                })
                .collect(),
            pull_frames: 2,
            grounded_targets_only: true,
        };
        let throw = |angle_degrees| Throw {
            poses: vec![bones.clone(); 6],
            release_frame: 2,
            hit: ThrowHit {
                damage: 8,
                angle_degrees,
                growth: 50,
                fixed: 0,
                base: 30,
            },
        };
        fighter.grab = Some(Parameters {
            catch,
            attachment: Attachment {
                holder_bone: 1,
                holder_point: [0.6, 0.0, 0.0],
                victim_bone: 1,
                victim_point: [0.0; 3],
            },
            pummel: Pummel {
                poses: vec![bones.clone(); 4],
                hit_frame: 1,
                damage: 3,
            },
            capture_damage_poses: vec![bones.clone(); 3],
            escape: Escape {
                catch_cut_poses: vec![bones.clone(); 3],
                capture_cut_poses: vec![bones.clone(); 3],
            },
            throws: Throws {
                forward: throw(30.0),
                backward: throw(150.0),
                up: throw(90.0),
                down: throw(270.0),
            },
        });
    }
    data
}
