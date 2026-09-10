//! A native experimental match slice, not a complete or verified Melee simulator.
//! Physics bones, contacts and translated arithmetic run without presentation.
//! See `docs/match.md` for the explicit scheduler and unsupported gameplay rules.
#![forbid(unsafe_code)]

pub mod aerial;
pub mod clank;
mod collision;
pub mod damage;
pub mod data;
pub mod death;
pub mod grab;
pub mod hitboxes;
pub mod ledge;
pub mod locomotion;
pub mod nudge;
pub mod rebirth;
pub mod shield;
mod simulation;
pub mod special;
pub mod stage_motion;
pub mod staling;
mod validation;

use crate::{collision::ecb, replay::FrameStepper};
use data::MatchData;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub const BUTTON_A: u16 = 0x100;
pub const BUTTON_B: u16 = 0x200;
pub const BUTTON_Z: u16 = 0x10;
pub const BUTTON_L: u16 = 0x40;
pub const BUTTON_R: u16 = 0x20;
pub const BUTTON_X: u16 = 0x400;
pub const BUTTON_Y: u16 = 0x800;

/// Normalized fighter inputs. Raw PAD calibration remains in `input`.
/// Sticks and trigger are processed game inputs; PAD calibration is separate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Controller {
    pub buttons: u16,
    pub stick: [f32; 2],
    #[serde(default)]
    pub cstick: [f32; 2],
    /// Processed analog trigger value, before the digital L/R override.
    #[serde(default)]
    pub trigger: f32,
}

impl Controller {
    /// Fighter_Spaghetti_8006AD10: either digital shoulder forces full pressure.
    pub fn shield_pressure(self) -> f32 {
        if self.buttons & (BUTTON_L | BUTTON_R) != 0 {
            1.0
        } else {
            self.trigger
        }
    }

    pub fn shield_held(self) -> bool {
        self.shield_pressure() != 0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Wait,
    Walk,
    Dash,
    Run,
    RunBrake,
    Turn,
    Squat,
    SquatWait,
    SquatRv,
    JumpSquat,
    Jump,
    JumpAerial,
    GuardOn,
    Guard,
    GuardOff,
    GuardSetOff,
    ShieldBreakFly,
    ShieldBreakFall,
    ShieldBreakDown,
    ShieldBreakStand,
    Furafura,
    Pass,
    Fall,
    Jab,
    Catch,
    CatchPull,
    CatchWait,
    ThrowF,
    ThrowB,
    ThrowHi,
    ThrowLw,
    CapturePulled,
    CaptureWait,
    ThrownF,
    ThrownB,
    ThrownHi,
    ThrownLw,
    CliffCatch,
    CliffWait,
    CliffClimb,
    CliffJump,
    CliffAttack,
    CliffEscape,
    Rebirth,
    RebirthWait,
    SpecialN,
    SpecialAirN,
    AttackAirN,
    AttackAirF,
    AttackAirB,
    AttackAirHi,
    AttackAirLw,
    LandingAirN,
    LandingAirF,
    LandingAirB,
    LandingAirHi,
    LandingAirLw,
    Damage,
    DamageFall,
    FlyReflectWall,
    FlyReflectCeiling,
    PassiveWall,
    PassiveWallJump,
    PassiveCeiling,
    Passive,
    PassiveStandF,
    PassiveStandB,
    DownBound,
    DownWait,
    DownDamage,
    DownForward,
    DownBack,
    DownAttack,
    DownStand,
    ReboundStop,
    Rebound,
    Landing,
    DeadDown,
    DeadLeft,
    DeadRight,
    DeadUp,
    DeadUpStar,
    DeadUpStarIce,
    DeadUpFall,
    DeadUpFallHitCamera,
    DeadUpFallHitCameraFlat,
    DeadUpFallIce,
    DeadUpFallHitCameraIce,
    Respawn,
    Eliminated,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fighter {
    pub position: [f32; 2],
    /// Persistent gameplay depth. Bone, hitbox and hurtbox transforms include it.
    pub depth: f32,
    /// Original xD4 position offset used by push overlap calculations.
    pub deferred_position: [f32; 3],
    /// Per-frame X/Z push velocity, sampled before any physics integration.
    pub nudge: [f32; 2],
    pub velocity: [f32; 2],
    pub knockback: [f32; 2],
    pub ground_velocity: f32,
    pub facing: f32,
    pub grounded: bool,
    pub ground_line: Option<usize>,
    /// Pass skips its supporting line until the next action transition.
    pub skip_floor: Option<usize>,
    pub floor_normal: [f32; 3],
    /// Last collision pass's floor, ceiling, left/right-facing wall IDs.
    pub contacts: [Option<usize>; 4],
    /// All interpolation history is included in checkpoints and trace output.
    pub ecb: ecb::State,
    pub ecb_lock: u8,
    pub locomotion: locomotion::State,
    pub shield: shield::ShieldState,
    pub aerial: aerial::State,
    pub clank: clank::State,
    /// Paired capture ownership is privileged deterministic physics state.
    pub grab: grab::State,
    pub ledge: ledge::State,
    pub death: death::State,
    pub action: Action,
    pub action_frame: u32,
    pub percent: f32,
    pub stocks: u8,
    pub hitlag: f32,
    pub hitstun: u32,
    /// Time since the previous damage transition; freezes during hitlag.
    pub damage_elapsed: i32,
    pub damage_angle_flag: u8,
    pub damage_angle_timer: u8,
    pub di_pending: bool,
    /// Whether the current damage launch uses the tumble floor-response graph.
    pub tumbling: bool,
    /// Evaluated hip orientation for the current missed-tech recovery suffix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prone: Option<damage::ProneOrientation>,
    /// Shared DownWait/DownDamage countdown from the original state union.
    pub down_timer: u32,
    /// Last wall/ceiling reflected during the current damage lifecycle.
    pub last_damage_surface: Option<crate::collision::stage::Surface>,
    /// Frames before another configured damage-surface reflection is eligible.
    pub reflect_lockout: u8,
    pub surface_tech: damage::SurfaceTechState,
    pub invincibility: u32,
    pub short_hop: bool,
    pub fast_fall: bool,
    /// Attack hit-group history is checkpointed, not inferred from observations.
    pub hit_groups: u16,
    pub hitboxes: [hitboxes::Track; 4],
    pub staling: staling::State,
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
    ShieldHit {
        attacker: usize,
        victim: usize,
        damage: f32,
        broken: bool,
    },
    Clank {
        /// Slot indices and suppression flags in native player order.
        slots: [usize; 2],
        suppressed: [bool; 2],
    },
    Grabbed {
        holder: usize,
        victim: usize,
    },
    LedgeCaught {
        player: usize,
        line: usize,
        side: ledge::Side,
    },
    Landed {
        player: usize,
    },
    SurfaceReflected {
        player: usize,
        surface: crate::collision::stage::Surface,
        line: usize,
    },
    SurfaceTeched {
        player: usize,
        surface: crate::collision::stage::Surface,
        line: usize,
        jump: bool,
    },
    Knockout {
        player: usize,
        stocks: u8,
    },
    DeathStarted {
        player: usize,
        death: death::Kind,
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
    pub stage: stage_motion::State,
    pub fighters: [Fighter; 2],
    pub rng_seed: u32,
    pub attack_instances: crate::fighter::stale::InstanceCounter,
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
    initial: State,
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
        let state = simulation::initial_state(&data, seed)?;
        validation::state(&state)?;
        Ok(Self {
            data: Arc::new(data),
            resource_id,
            initial: state.clone(),
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
    /// Current collision geometry, reconstructed from resources and stage time.
    pub fn stage_geometry(&self) -> std::borrow::Cow<'_, data::StageGeometry> {
        stage_motion::geometry(&self.data.stage, self.state.stage.frame)
    }
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            resource_id: self.resource_id,
            state: self.state.clone(),
        }
    }

    pub fn reset(&mut self, seed: u32) -> &State {
        self.state = self.initial.clone();
        self.state.rng_seed = seed;
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
