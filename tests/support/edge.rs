use skirmish::game::{
    data::{Bone, MatchData},
    edge::{Rules, Teeter, TeeterFrame},
};

/// Invented edge/teeter rules: `teeter_stick_limit` is mpcoll's own 0.75
/// literal; the rest are invented (`+474`/`+478`/`+47C`).
pub const RULES: Rules = Rules {
    teeter_stick_limit: 0.75,
    teeter_walk_threshold: 0.6,
    teeter_exit_distance: 2.0,
    teeter_exit_tolerance: 0.5,
};

/// Ottotto's own supplied pose count (`animation 210`).
pub const START_FRAMES: usize = 3;

fn frame(bones: &[Bone]) -> TeeterFrame {
    TeeterFrame {
        bones: bones.to_vec(),
        hurtbox_states: vec![],
    }
}

pub fn teeter(bones: &[Bone]) -> Teeter {
    Teeter {
        start: (0..START_FRAMES).map(|_| frame(bones)).collect(),
        wait: frame(bones),
    }
}

/// Install `rules.edge` and a teeter pose set (matching the synthetic
/// integration skeleton) on both fighters.
pub fn profile(mut data: MatchData) -> MatchData {
    data.rules.edge = Some(RULES);
    for fighter in &mut data.fighters {
        let bones = fighter.bones.clone();
        fighter.teeter = Some(teeter(&bones));
    }
    data
}
