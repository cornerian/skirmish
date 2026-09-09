use skirmish::game::{data::MatchData, rebirth::Rules};

pub fn profile(mut data: MatchData) -> MatchData {
    data.rules.rebirth = Some(Rules {
        entry_positions: [[-2.0, 10.0], [2.0, 10.0]],
        platform_positions: [[-2.0, 6.0], [2.0, 6.0]],
        travel_frames: 3,
        wait_frames: 5,
        release_stick_threshold: 0.3,
    });
    data
}
