//! File-backed comparison against the actual native match stepper. Replay
//! observations are never used to correct the evolving simulator state.
use crate::{observation, slippi};
use anyhow::{Result, ensure};
use replay_validation::{Checkpoint, FrameStepper, Transition, ValidationError};
use serde::{Deserialize, Serialize};
use skirmish::game;
use slippi::{Port, Replay, Timeline};

/// Reproducible native initialization, supplied independently of replay state.
/// `next_frame` labels the input to apply after all explicit warmup steps.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Initialization {
    pub data: game::data::MatchData,
    pub seed: u32,
    pub ports: [Port; 2],
    pub next_frame: i32,
    pub warmup: Vec<[game::Controller; 2]>,
}

pub fn initialize(initialization: &Initialization) -> Result<game::Match> {
    ensure!(
        initialization.ports[0] != initialization.ports[1],
        "duplicate player ports"
    );
    let mut game = game::Match::new(initialization.data.clone(), initialization.seed)?;
    for input in &initialization.warmup {
        game.step(*input)?;
    }
    Ok(game)
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub replay: slippi::Summary,
    pub policy: &'static str,
    pub input_policy: &'static str,
    pub fields: Vec<&'static str>,
    pub resources_sha256: String,
    pub ports: [Port; 2],
    pub checkpoint_next_frame: i32,
    pub outcome: Outcome,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    Matched {
        first_frame: i32,
        last_frame: i32,
        checked_frames: u64,
    },
    Mismatch {
        frame: i32,
        checked_frames: u64,
        difference: observation::Difference,
    },
    Error {
        frame: Option<i32>,
        checked_frames: u64,
        message: String,
    },
}

impl Report {
    pub fn is_match(&self) -> bool {
        matches!(self.outcome, Outcome::Matched { .. })
    }
}

struct Stepper<'a> {
    game: &'a mut game::Match,
    ports: [Port; 2],
    characters: [u8; 2],
}

impl FrameStepper for Stepper<'_> {
    type Checkpoint = game::Checkpoint;
    type Input = [game::Controller; 2];
    type Observation = observation::Observation;
    type Error = game::Error;

    fn restore(&mut self, checkpoint: &Self::Checkpoint) -> Result<(), Self::Error> {
        self.game.restore_checkpoint(checkpoint)
    }
    fn advance(&mut self, input: &Self::Input) -> Result<Self::Observation, Self::Error> {
        self.game.step(*input)?;
        Ok(observation::observe(self.game, self.ports, self.characters))
    }
}

/// Compare every selected frame from the checkpoint label to the timeline end.
/// Float fields use raw bits; the report names the limited observation policy.
pub fn validate(
    replay: &Replay,
    game: &mut game::Match,
    checkpoint: &Checkpoint<game::Checkpoint>,
    ports: [Port; 2],
    timeline: Timeline,
) -> Result<Report> {
    let settings = &replay.game().start;
    ensure!(ports[0] != ports[1], "duplicate player ports");
    ensure!(
        settings.players.len() == 2 && settings.players.iter().all(|p| ports.contains(&p.port)),
        "port mapping must cover exactly the two recorded players"
    );
    ensure!(
        settings
            .players
            .iter()
            .all(|p| p.r#type == slippi::peppi::game::PlayerType::Human),
        "physical-button replay validation requires human players"
    );
    ensure!(!settings.is_teams, "team matches are not implemented");
    ensure!(
        replay
            .game()
            .frames
            .ports
            .iter()
            .all(|p| p.follower.is_none()),
        "follower simulation is not implemented"
    );
    let indices = replay.frame_indices(timeline)?;
    let characters = ports.map(|port| {
        settings
            .players
            .iter()
            .find(|player| player.port == port)
            .expect("port coverage checked above")
            .character
    });
    ensure!(
        characters
            .into_iter()
            .all(|character| observation::internal_character(character).is_some()),
        "unsupported external character ID"
    );
    let ids = replay.game().frames.id.values();
    let start = indices.partition_point(|&i| ids[i] < checkpoint.next_frame);
    ensure!(
        indices
            .get(start)
            .is_some_and(|&i| ids[i] == checkpoint.next_frame),
        "checkpoint frame {} is absent from the selected timeline",
        checkpoint.next_frame
    );
    let transitions = indices[start..].iter().map(|&index| {
        let frame = replay.frame(index).map_err(std::io::Error::other)?;
        if frame.items.as_ref().is_some_and(|v| !v.is_empty())
            || frame.fod_platforms.as_ref().is_some_and(|v| !v.is_empty())
            || frame
                .dreamland_whispys
                .as_ref()
                .is_some_and(|v| !v.is_empty())
            || frame
                .stadium_transformations
                .as_ref()
                .is_some_and(|v| !v.is_empty())
        {
            return Err(std::io::Error::other(
                "replay item and dynamic stage simulation are not implemented",
            ));
        }
        Ok(Transition {
            frame: frame.id,
            input: observation::controllers(&frame, ports).map_err(std::io::Error::other)?,
            expected: observation::expected(&frame, ports).map_err(std::io::Error::other)?,
        })
    });
    let resources_sha256 = game
        .resource_id()
        .map(|byte| format!("{byte:02x}"))
        .concat();
    let mut stepper = Stepper {
        game,
        ports,
        characters,
    };
    let result = replay_validation::validate_fallible(
        &mut stepper,
        checkpoint,
        transitions,
        observation::compare,
    );
    let outcome = match result {
        Ok(report) => Outcome::Matched {
            first_frame: report.first_frame,
            last_frame: report.last_frame,
            checked_frames: report.checked_frames,
        },
        Err(ValidationError::Mismatch {
            frame,
            checked_frames,
            difference,
        }) => Outcome::Mismatch {
            frame,
            checked_frames,
            difference,
        },
        Err(error) => {
            let (frame, checked_frames) = match &error {
                ValidationError::Advance {
                    frame,
                    checked_frames,
                    ..
                } => (Some(*frame), *checked_frames),
                ValidationError::Read { checked_frames, .. } => (
                    i64::from(checkpoint.next_frame)
                        .checked_add_unsigned(*checked_frames)
                        .and_then(|f| i32::try_from(f).ok()),
                    *checked_frames,
                ),
                ValidationError::Sequence {
                    actual,
                    checked_frames,
                    ..
                } => (Some(*actual), *checked_frames),
                ValidationError::FrameOverflow {
                    after,
                    checked_frames,
                } => (Some(*after), *checked_frames),
                _ => (None, 0),
            };
            Outcome::Error {
                frame,
                checked_frames,
                message: error.to_string(),
            }
        }
    };
    Ok(Report {
        replay: replay.summary(timeline)?,
        policy: "fighter-post-v11",
        input_policy: observation::INPUT_POLICY,
        fields: observation::fields(settings.slippi.version),
        resources_sha256,
        ports,
        checkpoint_next_frame: checkpoint.next_frame,
        outcome,
    })
}
