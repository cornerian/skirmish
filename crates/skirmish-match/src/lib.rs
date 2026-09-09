//! A native experimental match slice, not a complete or verified Melee simulator.
//! Physics bones, contacts and translated arithmetic run without presentation.
//! See `docs/match.md` for the explicit scheduler and unsupported gameplay rules.
#![forbid(unsafe_code)]

pub mod data;
mod simulation;
mod validation;

use data::MatchData;
use serde::Serialize;
use sha2::{Digest, Sha256};
use skirmish_replay::FrameStepper;
use std::sync::Arc;

pub const BUTTON_A: u16 = 0x100;
pub const BUTTON_X: u16 = 0x400;
pub const BUTTON_Y: u16 = 0x800;

/// Normalized fighter inputs. Raw PAD calibration remains in `melee-input`.
/// This slice accepts A/X/Y only; unsupported buttons are errors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Controller {
    pub buttons: u16,
    pub stick: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Wait,
    Walk,
    JumpSquat,
    Jump,
    Fall,
    Jab,
    Damage,
    Landing,
    Respawn,
    Eliminated,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fighter {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub knockback: [f32; 2],
    pub ground_velocity: f32,
    pub facing: f32,
    pub grounded: bool,
    pub action: Action,
    pub action_frame: u32,
    pub percent: f32,
    pub stocks: u8,
    pub hitlag: f32,
    pub hitstun: u32,
    pub invincibility: u32,
    pub short_hop: bool,
    pub fast_fall: bool,
    /// Attack hit-group history is checkpointed, not inferred from observations.
    pub hit_groups: u16,
    pub previous_input: Controller,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Phase {
    Countdown {
        remaining: u32,
    },
    Playing,
    Finished {
        winner: Option<usize>,
        reason: FinishReason,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stocks,
    Time,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Started,
    Hit {
        attacker: usize,
        victim: usize,
        damage: f32,
        knockback: f32,
    },
    Landed {
        player: usize,
    },
    Knockout {
        player: usize,
        stocks: u8,
    },
    Respawned {
        player: usize,
    },
    Finished {
        winner: Option<usize>,
        reason: FinishReason,
    },
}

/// Privileged simulator state, not an observation model for a human player.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct State {
    /// Number of steps already completed; a fresh match has next_frame == 0.
    pub next_frame: u32,
    pub remaining_frames: u32,
    pub phase: Phase,
    pub fighters: [Fighter; 2],
    pub rng_seed: u32,
    pub events: Vec<Event>,
}

/// Opaque in-memory checkpoint includes the resource identity and all state.
#[derive(Clone, Debug)]
pub struct Checkpoint {
    resource_id: [u8; 32],
    state: State,
}

#[derive(Clone, Debug)]
pub struct Match {
    data: Arc<MatchData>,
    resource_id: [u8; 32],
    state: State,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid native match data: {0}")]
    Data(String),
    #[error("invalid or unsupported input for player {0}")]
    Input(usize),
    #[error("match has finished; reset or restore before stepping")]
    Finished,
    #[error("checkpoint belongs to different native resources or rules")]
    Resources,
    #[error("frame counter exhausted")]
    FrameOverflow,
    #[error("physics failed: {0}")]
    Physics(String),
    #[error("simulation produced a non-finite value")]
    NonFinite,
}

impl Match {
    pub fn new(data: MatchData, seed: u32) -> Result<Self, Error> {
        validation::validate(&data)?;
        let resource_id =
            Sha256::digest(serde_json::to_vec(&data).map_err(|e| Error::Data(e.to_string()))?)
                .into();
        let state = simulation::initial_state(&data, seed);
        Ok(Self {
            data: Arc::new(data),
            resource_id,
            state,
        })
    }

    pub fn data(&self) -> &MatchData {
        &self.data
    }
    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn resource_id(&self) -> [u8; 32] {
        self.resource_id
    }
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            resource_id: self.resource_id,
            state: self.state.clone(),
        }
    }

    pub fn reset(&mut self, seed: u32) -> &State {
        self.state = simulation::initial_state(&self.data, seed);
        &self.state
    }

    /// Errors leave the match untouched. Clones share only immutable resources.
    pub fn step(&mut self, input: [Controller; 2]) -> Result<&State, Error> {
        if matches!(self.state.phase, Phase::Finished { .. }) {
            return Err(Error::Finished);
        }
        validation::inputs(&input)?;
        let mut next = self.state.clone();
        next.events.clear();
        next.next_frame = next.next_frame.checked_add(1).ok_or(Error::FrameOverflow)?;
        simulation::advance(&self.data, &mut next, input)?;
        validation::state(&next)?;
        self.state = next;
        Ok(&self.state)
    }

    pub fn restore_checkpoint(&mut self, checkpoint: &Checkpoint) -> Result<(), Error> {
        if self.resource_id != checkpoint.resource_id {
            return Err(Error::Resources);
        }
        self.state = checkpoint.state.clone();
        Ok(())
    }
}

impl FrameStepper for Match {
    type Checkpoint = Checkpoint;
    type Input = [Controller; 2];
    type Observation = State;
    type Error = Error;

    fn restore(&mut self, checkpoint: &Checkpoint) -> Result<(), Error> {
        self.restore_checkpoint(checkpoint)
    }

    fn advance(&mut self, input: &Self::Input) -> Result<State, Error> {
        self.step(*input).cloned()
    }
}
