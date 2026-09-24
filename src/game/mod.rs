//! A native experimental match slice, not a complete or verified Melee simulator.
//! Physics bones, contacts and translated arithmetic run without presentation.
//! This module owns match state, inter-fighter interactions, and deeper game
//! logic such as clank, grab, nudge, hit resolution, death, entry, and rebirth;
//! fighter-local mechanics and state live in [`crate::fighter`].
//! See `docs/match.md` for the explicit scheduler and unsupported gameplay rules.
#![forbid(unsafe_code)]

pub mod clank;
pub mod collision;
pub(crate) mod combat_history;
pub mod data;
pub mod flow;
pub mod grab;
pub(crate) mod hit_resolution;
pub mod hitboxes;
pub mod nudge;
pub mod projectile;
pub mod script;
pub mod simulation;
pub(crate) mod special_capture;
pub mod staling;
pub mod validation;
pub mod wall_jump;

#[cfg(all(test, feature = "experimental-continuations"))]
mod move_exhaustion_tests;

use self::flow::{death, entry, stage_motion};
use crate::collision::ecb;
use crate::fighter::{
    aerial, damage, dash, edge, escape, escape_air, idle, jab, ledge, locomotion, movement, shield,
    smash, taunt, tilt,
};
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

/// Stable identity for an action declared by a native fighter namespace.
///
/// The value is deliberately not an index into a definition table.  Custom
/// actions can therefore travel through snapshots and rollback without
/// depending on registration order.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct CustomActionId(pub u64);

impl CustomActionId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, serde::Deserialize,
)]
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
    /// Slippi 275, `ftCo_MS_CaptureCaptain`. This is Captain Falcon's
    /// dedicated Falcon Dive victim state and is intentionally separate from
    /// the ordinary grab/capture family above.
    CaptureCaptain,
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
    /// Captain Falcon's Falcon Dive catch phase (Slippi 355). Entered by
    /// the special's attacker-side catch callback after it contacts a
    /// fighter. Victim capture/attachment remains owned by the host grab
    /// system and is not synthesized by this action alone.
    SpecialHiCatch,
    /// Captain Falcon's Falcon Dive throw phase (Slippi 356). The native
    /// catch animation transitions here before the attacker falls.
    SpecialHiThrow,
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
    /// Captain Falcon's grounded Falcon Kick motion end (Slippi 358).
    /// This is distinct from Fox's `SpecialLwEnd` action, which retains its
    /// existing canonical identity and discriminant.
    SpecialLwGroundEnd,
    /// Captain Falcon's aerial Falcon Kick landing end (Slippi 360).
    SpecialAirLwLandingEnd,
    /// Captain Falcon's aerial Falcon Kick airborne end (Slippi 361).
    SpecialAirLwEndAir,
    /// Captain Falcon's grounded Falcon Kick airborne end (Slippi 362).
    SpecialLwEndAir,
    Eliminated,
    /// Source-defined action state. The identifier is a stable hash of the
    /// qualified fighter namespace and canonical action key.
    Custom(CustomActionId),
}

impl Action {
    /// Number of dense built-in action slots. Keep this independent of the
    /// custom variant so adding a source action cannot shift wire/index IDs.
    pub const BUILTIN_COUNT: usize = 168;

    /// Return the dense built-in slot, or `None` for a source-defined action.
    /// This explicit match preserves the existing O(1) action tables on the
    /// native hot path while custom actions use their immutable maps.
    pub const fn builtin_index(self) -> Option<usize> {
        match self {
            Self::Wait => Some(0),
            Self::Walk => Some(1),
            Self::Dash => Some(2),
            Self::Run => Some(3),
            Self::RunTurn => Some(4),
            Self::RunBrake => Some(5),
            Self::Turn => Some(6),
            Self::Squat => Some(7),
            Self::SquatWait => Some(8),
            Self::SquatRv => Some(9),
            Self::Ottotto => Some(10),
            Self::OttottoWait => Some(11),
            Self::AppealSR => Some(12),
            Self::AppealSL => Some(13),
            Self::JumpSquat => Some(14),
            Self::Jump => Some(15),
            Self::JumpAerial => Some(16),
            Self::GuardOn => Some(17),
            Self::Guard => Some(18),
            Self::GuardOff => Some(19),
            Self::GuardSetOff => Some(20),
            Self::GuardReflect => Some(21),
            Self::ShieldBreakFly => Some(22),
            Self::ShieldBreakFall => Some(23),
            Self::ShieldBreakDown => Some(24),
            Self::ShieldBreakStand => Some(25),
            Self::Furafura => Some(26),
            Self::EscapeF => Some(27),
            Self::EscapeB => Some(28),
            Self::EscapeN => Some(29),
            Self::EscapeAir => Some(30),
            Self::Pass => Some(31),
            Self::Fall => Some(32),
            Self::FallSpecial => Some(33),
            Self::LandingFallSpecial => Some(34),
            Self::Jab => Some(35),
            Self::Attack12 => Some(36),
            Self::Attack13 => Some(37),
            Self::Attack100Start => Some(38),
            Self::Attack100Loop => Some(39),
            Self::Attack100End => Some(40),
            Self::AttackDash => Some(41),
            Self::AttackS3Hi => Some(42),
            Self::AttackS3HiS => Some(43),
            Self::AttackS3S => Some(44),
            Self::AttackS3LwS => Some(45),
            Self::AttackS3Lw => Some(46),
            Self::AttackHi3 => Some(47),
            Self::AttackLw3 => Some(48),
            Self::AttackS4Hi => Some(49),
            Self::AttackS4HiS => Some(50),
            Self::AttackS4S => Some(51),
            Self::AttackS4LwS => Some(52),
            Self::AttackS4Lw => Some(53),
            Self::AttackHi4 => Some(54),
            Self::AttackLw4 => Some(55),
            Self::Catch => Some(56),
            Self::CatchDash => Some(57),
            Self::CatchPull => Some(58),
            Self::CatchDashPull => Some(59),
            Self::CatchWait => Some(60),
            Self::CatchAttack => Some(61),
            Self::CatchCut => Some(62),
            Self::ThrowF => Some(63),
            Self::ThrowB => Some(64),
            Self::ThrowHi => Some(65),
            Self::ThrowLw => Some(66),
            Self::CapturePulledHi => Some(67),
            Self::CaptureWaitHi => Some(68),
            Self::CaptureDamageHi => Some(69),
            Self::CapturePulledLw => Some(70),
            Self::CaptureWaitLw => Some(71),
            Self::CaptureDamageLw => Some(72),
            Self::CaptureCut => Some(73),
            Self::CaptureCaptain => Some(74),
            Self::ThrownF => Some(75),
            Self::ThrownB => Some(76),
            Self::ThrownHi => Some(77),
            Self::ThrownLw => Some(78),
            Self::CliffCatch => Some(79),
            Self::CliffWait => Some(80),
            Self::CliffClimb => Some(81),
            Self::CliffJump => Some(82),
            Self::CliffAttack => Some(83),
            Self::CliffEscape => Some(84),
            Self::Rebirth => Some(85),
            Self::RebirthWait => Some(86),
            Self::Entry => Some(87),
            Self::EntryStart => Some(88),
            Self::EntryEnd => Some(89),
            Self::SpecialNStart => Some(90),
            Self::SpecialNLoop => Some(91),
            Self::SpecialNEnd => Some(92),
            Self::SpecialAirNStart => Some(93),
            Self::SpecialAirNLoop => Some(94),
            Self::SpecialAirNEnd => Some(95),
            Self::SpecialSStart => Some(96),
            Self::SpecialS => Some(97),
            Self::SpecialSEnd => Some(98),
            Self::SpecialAirSStart => Some(99),
            Self::SpecialAirS => Some(100),
            Self::SpecialAirSEnd => Some(101),
            Self::SpecialHiHold => Some(102),
            Self::SpecialHiHoldAir => Some(103),
            Self::SpecialHi => Some(104),
            Self::SpecialAirHi => Some(105),
            Self::SpecialHiCatch => Some(106),
            Self::SpecialHiThrow => Some(107),
            Self::SpecialHiLanding => Some(108),
            Self::SpecialHiFall => Some(109),
            Self::SpecialHiBound => Some(110),
            Self::SpecialLwStart => Some(111),
            Self::SpecialLw => Some(112),
            Self::SpecialLwHit => Some(113),
            Self::SpecialLwEnd => Some(114),
            Self::SpecialLwTurn => Some(115),
            Self::SpecialAirLwStart => Some(116),
            Self::SpecialAirLw => Some(117),
            Self::SpecialAirLwHit => Some(118),
            Self::SpecialAirLwEnd => Some(119),
            Self::SpecialAirLwTurn => Some(120),
            Self::AttackAirN => Some(121),
            Self::AttackAirF => Some(122),
            Self::AttackAirB => Some(123),
            Self::AttackAirHi => Some(124),
            Self::AttackAirLw => Some(125),
            Self::LandingAirN => Some(126),
            Self::LandingAirF => Some(127),
            Self::LandingAirB => Some(128),
            Self::LandingAirHi => Some(129),
            Self::LandingAirLw => Some(130),
            Self::Damage => Some(131),
            Self::DamageFall => Some(132),
            Self::FlyReflectWall => Some(133),
            Self::FlyReflectCeiling => Some(134),
            Self::PassiveWall => Some(135),
            Self::PassiveWallJump => Some(136),
            Self::PassiveCeiling => Some(137),
            Self::Passive => Some(138),
            Self::PassiveStandF => Some(139),
            Self::PassiveStandB => Some(140),
            Self::DownBound => Some(141),
            Self::DownWait => Some(142),
            Self::DownDamage => Some(143),
            Self::DownForward => Some(144),
            Self::DownBack => Some(145),
            Self::DownAttack => Some(146),
            Self::DownStand => Some(147),
            Self::ReboundStop => Some(148),
            Self::Rebound => Some(149),
            Self::Landing => Some(150),
            Self::DeadDown => Some(151),
            Self::DeadLeft => Some(152),
            Self::DeadRight => Some(153),
            Self::DeadUp => Some(154),
            Self::DeadUpStar => Some(155),
            Self::DeadUpStarIce => Some(156),
            Self::DeadUpFall => Some(157),
            Self::DeadUpFallHitCamera => Some(158),
            Self::DeadUpFallHitCameraFlat => Some(159),
            Self::DeadUpFallIce => Some(160),
            Self::DeadUpFallHitCameraIce => Some(161),
            Self::Respawn => Some(162),
            Self::SpecialLwGroundEnd => Some(163),
            Self::SpecialAirLwLandingEnd => Some(164),
            Self::SpecialAirLwEndAir => Some(165),
            Self::SpecialLwEndAir => Some(166),
            Self::Eliminated => Some(167),
            Self::Custom(_) => None,
        }
    }

    pub const fn custom_id(self) -> Option<CustomActionId> {
        match self {
            Self::Custom(id) => Some(id),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fighter {
    /// Deterministic Luau locals owned by this fighter. The state is part of
    /// every match snapshot so rollback/checkpoint replay restores behavior
    /// exactly along with the native physics state.
    #[serde(skip_serializing_if = "script::LocalState::is_empty")]
    pub script_state: script::LocalState,
    /// Generic fighter action variables owned by the active script. These
    /// replace the former per-character native special state slots; native
    /// mechanisms may retain transient compatibility state while migration
    /// of the bundled definitions is in progress.
    #[serde(skip_serializing_if = "script::LocalState::is_empty")]
    pub action_state: script::LocalState,
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
    pub tilt: tilt::State,
    pub smash: smash::State,
    pub dash: dash::State,
    pub jab: jab::State,
    /// Read only while `action == Action::Wait`; reset on every Wait entry.
    pub idle: idle::State,
    pub clank: clank::State,
    /// Paired capture ownership is privileged deterministic physics state.
    pub grab: grab::State,
    /// Captain Falcon's Falcon Dive capture is not an ordinary grab pair.
    /// Keeping its relation in the checkpointed fighter state prevents the
    /// generic grab scanner, escape clock, and throw scheduler from claiming
    /// this source-specific interaction.
    pub special_capture: special_capture::State,
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
    pub action_instance: crate::fighter::state::action_instance::State,
    /// Retained hit attribution and raw combo-counter state recorded by Slippi.
    pub combo: crate::fighter::state::combo::State,
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
    /// Small rollback-tracked native event state. The event dispatcher owns
    /// action generations and availability gates; it never contains VM data.
    #[serde(default)]
    pub script_events: script::events::NativeEventState,
    /// Fully validated native projectile commands staged by callbacks.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) pending_projectiles: Vec<projectile::PendingProjectile>,
    /// Compact numeric article commands emitted by typed fighter scripts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) pending_article_spawns: Vec<projectile::PendingArticleSpawn>,
    /// Effects parented to this fighter, included in checkpoints and cleared
    /// together according to the native fighter ownership boundary.
    pub effects: crate::game::flow::effects::EffectState,
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
    pub attack_instances: crate::fighter::state::stale::InstanceCounter,
    /// Independent `plAttack_80037B08` sequence for fighter/item actions.
    pub action_instances: crate::fighter::state::instance::Counter,
    pub events: Vec<Event>,
}

/// Borrowed state at the native post-physics, pre-contact-resolution boundary.
///
/// The projection is intentionally ephemeral: it is never stored in the
/// authoritative match or checkpoint. `action_age_offsets` accounts for the
/// animation tail increment that native skips when a body contact immediately
/// transitions the victim into damage.
pub struct ObservationBoundary<'a> {
    pub state: &'a State,
    pub data: &'a MatchData,
    pub action_age_offsets: [f32; 2],
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
    /// `crate::game::flow::entry::entry_delay`); every other resource profile
    /// ignores it. Callers that know a replay's real ports (`make-
    /// initialization`) should supply them; every other caller keeps using
    /// `new`, which defaults to `[0, 1]` (today's two-player convention).
    pub fn new_with_slots(mut data: MatchData, seed: u32, slots: [u32; 2]) -> Result<Self, Error> {
        // Project class-defined native attributes once before validation,
        // resource caching, and resource identity calculation.
        script::attributes::apply(&mut data)?;
        for fighter in &mut data.fighters {
            // The cache is a derived, match-owned artifact. Clear a cache
            // carried by a cloned FighterData before validating and rebuilding
            // it, so edits to serialized script/resources cannot reuse a
            // linked program from an earlier match registration.
            fighter.script_resources = Default::default();
            if let Some(specials) = &mut fighter.specials {
                specials
                    .resources
                    .index_attacks()
                    .map_err(|error| Error::Data(format!("invalid specials resource: {error}")))?;
            }
        }
        let rules = &data.rules;
        for fighter in &mut data.fighters {
            let resources = script::lifecycle::resource_cache(fighter, rules)
                .map_err(|error| Error::Data(error.to_string()))?;
            fighter.script_resources.replace(resources);
            #[cfg(feature = "experimental-continuations")]
            if let Some(program) = fighter
                .script_resources
                .get()
                .and_then(|resources| resources.program())
            {
                program
                    .prepare_for_current_thread()
                    .map_err(|error| Error::Data(error.to_string()))?;
            }
        }
        validation::validate(&data)?;
        let mut resource_bytes =
            serde_json::to_vec(&data).map_err(|e| Error::Data(e.to_string()))?;
        // Script source and all registered import dependencies are part of
        // the native resource identity, including when FighterData.script is
        // absent and a bundled source is selected from the resource profile.
        script::identity::append(&mut resource_bytes, &data.fighters);
        let resource_id = Sha256::digest(resource_bytes).into();
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

    /// Prepare all linked Pon programs for the calling gameplay thread.
    ///
    /// Match construction prepares its resource programs on the loading
    /// thread. A match moved to another thread must cross this explicit
    /// session boundary before [`Self::step`]; stepping never compiles or
    /// discovers script source.
    pub fn prepare_for_current_thread(&self) -> Result<(), Error> {
        for fighter in &self.data.fighters {
            let Some(resources) = fighter.script_resources.get() else {
                continue;
            };
            let Some(program) = resources.program() else {
                continue;
            };
            program
                .prepare_for_current_thread()
                .map_err(|error| Error::Data(format!("prepare fighter script: {error}")))?;
        }
        Ok(())
    }

    /// Errors leave the match untouched. Clones share only immutable resources.
    pub fn step(&mut self, input: [Controller; 2]) -> Result<&State, Error> {
        self.step_with_capture(input, |_| ())?;
        Ok(&self.state)
    }

    /// Advance one frame and expose the native observation boundary without
    /// cloning fighters or retaining presentation state in the match.
    pub fn step_with_capture<T, F>(
        &mut self,
        input: [Controller; 2],
        capture: F,
    ) -> Result<T, Error>
    where
        F: FnOnce(ObservationBoundary<'_>) -> T,
    {
        if matches!(self.state.phase, Phase::Finished { .. }) {
            return Err(Error::Finished);
        }
        validation::inputs(&input)?;
        let mut next = self.state.clone();
        next.events.clear();
        next.next_frame = next.next_frame.checked_add(1).ok_or(Error::FrameOverflow)?;
        let mut capture = Some(capture);
        let captured = simulation::advance(&self.data, &mut next, input, &mut capture)?;
        validation::state(&next)?;
        self.state = next;
        if let Some(capture) = capture {
            Ok(capture(ObservationBoundary {
                state: &self.state,
                data: &self.data,
                action_age_offsets: [0.0; 2],
            }))
        } else {
            Ok(captured.expect("body contact capture must be invoked"))
        }
    }

    pub fn restore_checkpoint(&mut self, checkpoint: &Checkpoint) -> Result<(), Error> {
        if self.resource_id != checkpoint.resource_id {
            return Err(Error::Resources);
        }
        self.state = checkpoint.state.clone();
        Ok(())
    }
}
