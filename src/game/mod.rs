//! A native experimental match slice, not a complete or verified Melee simulator.
//! Physics bones, contacts and translated arithmetic run without presentation.
//! See `docs/match.md` for the explicit scheduler and unsupported gameplay rules.
#![forbid(unsafe_code)]

pub mod aerial;
pub mod clank;
pub mod collision;
mod combat_history;
pub mod damage;
pub mod dash;
pub mod data;
pub mod death;
pub mod edge;
pub mod entry;
pub mod escape;
pub mod escape_air;
pub mod grab;
pub mod hitboxes;
pub mod idle;
pub mod jab;
pub mod landing;
pub mod ledge;
pub mod locomotion;
pub mod movement;
pub mod nudge;
pub mod projectile;
pub mod rebirth;
pub mod shield;
pub mod simulation;
pub mod smash;
pub mod stage_motion;
pub mod staling;
pub mod taunt;
pub mod tilt;
pub mod validation;
pub mod wall_jump;

use crate::collision::ecb;
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
/// `sysdolphin/baselib/controller.h:13`: `HSD_PAD_DPADLEFT = 1 << 0`.
pub const BUTTON_DPAD_LEFT: u16 = 0x1;
/// `controller.h:14`: `HSD_PAD_DPADRIGHT = 1 << 1`.
pub const BUTTON_DPAD_RIGHT: u16 = 0x2;
/// `controller.h:15`: `HSD_PAD_DPADDOWN = 1 << 2`.
pub const BUTTON_DPAD_DOWN: u16 = 0x4;
/// `controller.h:16`: `HSD_PAD_DPADUP = 1 << 3`. The only D-pad bit with an
/// observable effect (`ftCo_800DE9B8`'s taunt press); left/right/down are
/// accepted as inert input everywhere they are read.
pub const BUTTON_DPAD_UP: u16 = 0x8;

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
    RunTurn,
    RunBrake,
    Turn,
    Squat,
    SquatWait,
    SquatRv,
    /// Slippi 245, animation 210. `ftCo_Ottotto.c`.
    Ottotto,
    /// Slippi 246, animation 211. `ftCo_Ottotto.c`.
    OttottoWait,
    /// Slippi 264, animation 239. `ftCo_AppealS.c`.
    AppealSR,
    /// Slippi 265, animation 240. `ftCo_AppealS.c`.
    AppealSL,
    JumpSquat,
    Jump,
    JumpAerial,
    GuardOn,
    Guard,
    GuardOff,
    GuardSetOff,
    GuardReflect,
    ShieldBreakFly,
    ShieldBreakFall,
    ShieldBreakDown,
    ShieldBreakStand,
    Furafura,
    EscapeF,
    EscapeB,
    EscapeN,
    EscapeAir,
    Pass,
    Fall,
    FallSpecial,
    LandingFallSpecial,
    Jab,
    Attack12,
    Attack13,
    Attack100Start,
    Attack100Loop,
    Attack100End,
    AttackDash,
    AttackS3Hi,
    AttackS3HiS,
    AttackS3S,
    AttackS3LwS,
    AttackS3Lw,
    AttackHi3,
    AttackLw3,
    AttackS4Hi,
    AttackS4HiS,
    AttackS4S,
    AttackS4LwS,
    AttackS4Lw,
    AttackHi4,
    AttackLw4,
    Catch,
    CatchDash,
    CatchPull,
    CatchDashPull,
    CatchWait,
    CatchAttack,
    CatchCut,
    ThrowF,
    ThrowB,
    ThrowHi,
    ThrowLw,
    CapturePulledHi,
    CaptureWaitHi,
    CaptureDamageHi,
    CapturePulledLw,
    CaptureWaitLw,
    CaptureDamageLw,
    CaptureCut,
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
    /// Slippi 322, state age -1. `ftCo_Entry_Anim`. `docs/match-start.md`.
    Entry,
    /// Slippi 323, state age counts from 0. `ftCo_EntryStart_Anim`/`_Phys`.
    EntryStart,
    /// Slippi 324, state age -1. `ftCo_EntryEnd_Anim`/`_Phys`.
    EntryEnd,
    /// Slippi 341 (`ftFx_MS_SpecialNStart == ftCo_MS_Count`, confirmed
    /// against `ftFox/forward.h`'s own declaration order: Neutral precedes
    /// Side, whose own `SpecialSStart` is confirmed `ftCo_MS_Count + 6`).
    /// `ftFx_SpecialN_Enter`. `docs/fox-neutral-special.md`.
    SpecialNStart,
    /// Slippi 342. `ftFox_SpecialN_BeginLoopTransition`'s ground call site.
    SpecialNLoop,
    /// Slippi 343. Entered from Loop's own Anim callback when the repeat
    /// flag was not armed this cycle.
    SpecialNEnd,
    /// Slippi 344. `ftFx_SpecialAirN_Enter`.
    SpecialAirNStart,
    /// Slippi 345. `ftFox_SpecialN_BeginLoopTransition`'s air call site.
    SpecialAirNLoop,
    /// Slippi 346. Air counterpart of `SpecialNEnd`.
    SpecialAirNEnd,
    /// Slippi 347 (`ftFx_MS_SpecialSStart = ftCo_MS_Count + 6`, confirmed
    /// against `ftFox/forward.h`). Animation 301 is an unverified
    /// extrapolation; see `observation::animation_index`.
    /// `ftFx_SpecialSStart_Enter`.
    SpecialSStart,
    /// Slippi 348. `ftFx_SpecialS_Enter`.
    SpecialS,
    /// Slippi 349. `ftFx_SpecialSEnd_Enter`.
    SpecialSEnd,
    /// Slippi 350. `ftFx_SpecialAirSStart_Enter`.
    SpecialAirSStart,
    /// Slippi 351. `ftFx_SpecialAirS_Enter`.
    SpecialAirS,
    /// Slippi 352. `ftFx_SpecialAirSEnd_Enter`.
    SpecialAirSEnd,
    /// Slippi 353. `ftFx_SpecialHi_Enter`. Grounded Firefox/Firebird charge.
    SpecialHiHold,
    /// Slippi 354. `ftFx_SpecialAirHiStart_Enter`. Aerial charge.
    SpecialHiHoldAir,
    /// Slippi 355. `ftFx_SpecialAirHi_AirToGround`. Grounded travel.
    SpecialHi,
    /// Slippi 356. `ftFx_SpecialAirHi_Enter`. Aerial travel/launch.
    SpecialAirHi,
    /// Slippi 357. `ftFx_SpecialHiFall_AirToGround` /
    /// `ftFx_SpecialHiLanding_GroundToAir` (despite the "GroundToAir" name,
    /// this enters the *grounded* landing). Entered at frame 13 from Fall's
    /// own landing, or at frame 0 from Travel's duration end.
    SpecialHiLanding,
    /// Slippi 358. `ftFx_SpecialHiLanding_GroundToAir` (despite the name,
    /// this enters the *aerial* fall). Entered at frame 0 from Travel's
    /// duration end while airborne.
    SpecialHiFall,
    /// Slippi 359. `ftFx_SpecialHiBound_Enter`. Shared grounded/aerial
    /// rebound off a steep Travel impact.
    SpecialHiBound,
    /// Slippi 360 (`ftFx_MS_SpecialLwStart`, `ftFox/forward.h:71-80`).
    /// `ftFx_SpecialLw_Enter`.
    SpecialLwStart,
    /// Slippi 361. `ftFx_SpecialLwLoop_Enter`.
    SpecialLw,
    /// Slippi 362. Reachable only through `ftFx_SpecialLwHit_Enter`'s
    /// projectile-reflect callback, unmodeled since Skirmish has no
    /// projectiles; kept reachable through tests only (`docs/fox-down-
    /// special.md`).
    SpecialLwHit,
    /// Slippi 363. `ftFx_SpecialLwEnd_Enter`.
    SpecialLwEnd,
    /// Slippi 364. `ftFx_SpecialLwTurn_Check`.
    SpecialLwTurn,
    /// Slippi 365. `ftFx_SpecialAirLw_Enter`.
    SpecialAirLwStart,
    /// Slippi 366. `ftFx_SpecialAirLwLoop_Enter`.
    SpecialAirLw,
    /// Slippi 367. See `SpecialLwHit`: test-only.
    SpecialAirLwHit,
    /// Slippi 368. `ftFx_SpecialAirLwEnd_Enter`.
    SpecialAirLwEnd,
    /// Slippi 369. `ftFx_SpecialLwTurn_Check`'s aerial branch.
    SpecialAirLwTurn,
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
    /// Source xF0 scalar used to decay and reproject grounded knockback.
    pub ground_knockback: f32,
    pub ground_velocity: f32,
    pub facing: f32,
    pub grounded: bool,
    pub ground_line: Option<usize>,
    /// Retained collision-line identity serialized by Slippi even while airborne.
    pub last_ground_line: Option<usize>,
    /// Pass skips its supporting line until the next action transition.
    pub skip_floor: Option<usize>,
    pub floor_normal: [f32; 3],
    /// Last collision pass's floor, ceiling, left/right-facing wall IDs.
    pub contacts: [Option<usize>; 4],
    /// Set for one frame when a mode-2/mode-1 floor-end clamp (`Collide_Left
    /// Edge`/`Collide_RightEdge`/`Collide_Edge`) held this frame's position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_contact: Option<edge::EdgeSide>,
    /// All interpolation history is included in checkpoints and trace output.
    pub ecb: ecb::State,
    pub ecb_lock: u8,
    pub locomotion: locomotion::State,
    pub shield: shield::ShieldState,
    pub aerial: aerial::State,
    pub side_special: crate::characters::fox::side::State,
    pub up_special: crate::characters::fox::up::State,
    pub down_special: crate::characters::fox::down::State,
    pub neutral_special: crate::characters::fox::neutral::State,
    pub tilt: tilt::State,
    pub smash: smash::State,
    pub dash: dash::State,
    pub jab: jab::State,
    /// Read only while `action == Action::Wait`; reset on every Wait entry.
    pub idle: idle::State,
    pub clank: clank::State,
    /// Paired capture ownership is privileged deterministic physics state.
    pub grab: grab::State,
    pub ledge: ledge::State,
    pub death: death::State,
    /// `Fighter::mv.co.entry`, shared by Entry/EntryStart/EntryEnd. Read
    /// only while `entry::owns_action(action)`, but not reset by every
    /// other transition (unlike `dash`/`smash`/`idle`) since it must
    /// persist across the Entry -> EntryStart -> EntryEnd sequence itself.
    pub entry: entry::State,
    pub action: Action,
    pub action_frame: u32,
    pub percent: f32,
    pub stocks: u8,
    pub hitlag: f32,
    pub hitstun: u32,
    /// Current motion-family instance recorded by Slippi 3.16+.
    pub action_instance: crate::fighter::action_instance::State,
    /// Retained hit attribution and raw combo-counter state recorded by Slippi.
    pub combo: crate::fighter::combo::State,
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
    /// Selected ordinary Damage motion; its sampled bones are physics state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_motion: Option<crate::fighter::damage::DamageMotion>,
    /// Last wall/ceiling reflected during the current damage lifecycle.
    pub last_damage_surface: Option<crate::collision::stage::Surface>,
    /// Frames before another configured damage-surface reflection is eligible.
    pub reflect_lockout: u8,
    pub surface_tech: damage::SurfaceTechState,
    pub wall_jump: wall_jump::State,
    pub invincibility: u32,
    /// Intangibility has priority over ordinary invincibility in Melee's
    /// fighter-wide hurtbox collision state.
    pub intangibility: u32,
    /// Script-driven `Fighter::x1988` collision state sampled from the current
    /// motion; Slippi reports it ahead of the timed counters above.
    pub body_state: data::BodyState,
    /// Slippi's per-frame landing result: none, successful, unsuccessful.
    pub l_cancel_status: u8,
    /// `mv.co.landing.allow_interrupt`: set by the ordinary Landing entry
    /// (`ftCo_Landing_Enter_Basic` passes `true`); every other transition,
    /// including `LandingFallSpecial`, resets it to `false` in `enter`.
    pub landing_allow_interrupt: bool,
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
    GrabEscaped {
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
    WallJumped {
        player: usize,
        line: usize,
    },
    ProjectileSpawned {
        owner: usize,
        projectile_kind: projectile::ProjectileKind,
    },
    ProjectileHit {
        owner: usize,
        victim: usize,
    },
    ProjectileReflected {
        owner: usize,
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
    /// In-flight fired projectiles (`game::projectile`), included in
    /// checkpoints like every other match-state field.
    pub projectiles: Vec<projectile::Projectile>,
    pub rng_seed: u32,
    pub attack_instances: crate::fighter::stale::InstanceCounter,
    /// Independent `plAttack_80037B08` sequence for fighter/item actions.
    pub action_instances: crate::fighter::instance::Counter,
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
        Self::new_with_slots(data, seed, [0, 1])
    }

    /// `slots` is each player's 0-indexed port (P1=0..P4=3), used only when
    /// `data.rules.entry` is `Some` (the per-port entry delay,
    /// `crate::fighter::entry::entry_delay`); every other resource profile
    /// ignores it. Callers that know a replay's real ports (`make-
    /// initialization`) should supply them; every other caller keeps using
    /// `new`, which defaults to `[0, 1]` (today's two-player convention).
    pub fn new_with_slots(data: MatchData, seed: u32, slots: [u32; 2]) -> Result<Self, Error> {
        validation::validate(&data)?;
        let resource_id =
            Sha256::digest(serde_json::to_vec(&data).map_err(|e| Error::Data(e.to_string()))?)
                .into();
        let state = simulation::initial_state(&data, seed, slots)?;
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
