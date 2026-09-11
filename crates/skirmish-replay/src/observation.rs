//! Explicit input and observation policies for the experimental native match.
//! No recorded position, action state or random seed initializes the simulator.

use crate::slippi;
use serde::Serialize;
use skirmish::game;
use slippi::Port;
use std::fmt;

pub const BASE_FIELDS: &[&str] = &[
    "action_state",
    "action_age",
    "position.x",
    "position.y",
    "direction",
    "percent",
    "shield",
    "stocks",
    "airborne",
    "jumps_remaining",
    "last_ground_id",
    "l_cancel",
];
pub const PROVENANCE_FIELDS: &[&str] = &[
    "character",
    "last_attack_landed",
    "combo_count",
    "last_hit_by",
];
pub const INSTANCE_FIELDS: &[&str] = &["last_hit_by_instance", "instance_id"];
pub const HURTBOX_FIELD: &str = "hurtbox_state";
pub const VELOCITY_FIELDS: &[&str] = &[
    "velocities.self_x_air",
    "velocities.self_y",
    "velocities.knockback_x",
    "velocities.knockback_y",
    "velocities.self_x_ground",
];
pub const HITLAG_FIELD: &str = "hitlag";
pub const ANIMATION_FIELD: &str = "animation_index";
pub const STATE_FLAG_FIELDS: &[&str] = &[
    "state_flags.reflect",
    "state_flags.protected",
    "state_flags.fast_fall",
    "state_flags.hitlag",
    "state_flags.shield",
    "state_flags.hitstun",
    "state_flags.shield_touch",
    "state_flags.powershield",
    "state_flags.dead",
    "state_flags.sleep",
];
pub const MISC_HITSTUN_FIELD: &str = "misc_as.hitstun";

pub fn fields(version: slippi::Version) -> Vec<&'static str> {
    let mut fields = BASE_FIELDS.to_vec();
    fields.extend_from_slice(PROVENANCE_FIELDS);
    fields.extend_from_slice(STATE_FLAG_FIELDS);
    fields.push(MISC_HITSTUN_FIELD);
    if version.gte(2, 1) {
        fields.push(HURTBOX_FIELD);
    }
    if version.gte(3, 5) {
        fields.extend_from_slice(VELOCITY_FIELDS);
    }
    if version.gte(3, 8) {
        fields.push(HITLAG_FIELD);
    }
    if version.gte(3, 11) {
        fields.push(ANIMATION_FIELD);
    }
    if version.gte(3, 16) {
        fields.extend_from_slice(INSTANCE_FIELDS);
    }
    fields
}

pub const INPUT_POLICY: &str = "processed main-stick, C-stick and analog trigger; physical A/B/X/Y/Z/L/R; derived stick/trigger flags allowed; no replay state or RNG overrides";

const BUTTONS: u16 = game::BUTTON_A
    | game::BUTTON_B
    | game::BUTTON_X
    | game::BUTTON_Y
    | game::BUTTON_Z
    | game::BUTTON_L
    | game::BUTTON_R;
// HSD_PadADConvert in the pinned controller.c derives these four flags from the
// main stick. C-stick directions occupy bits 20..23. Logical LR is represented
// by native digital buttons or processed analog pressure, not silently dropped.
const MAIN_STICK_FLAGS: u32 = 0x000f_0000;
const CSTICK_FLAGS: u32 = 0x00f0_0000;
const LOGICAL_TRIGGER: u32 = 0x8000_0000;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct FighterObservation {
    pub port: Port,
    /// The raw Melee motion-state ID. `None` means Skirmish's refactored action
    /// does not yet retain enough information to identify one exact state.
    pub action_state: Option<u16>,
    pub action_age: f32,
    pub position: [f32; 2],
    pub direction: f32,
    pub percent: f32,
    pub shield: f32,
    pub stocks: u8,
    pub airborne: bool,
    pub jumps_remaining: u8,
    /// Melee retains `-1` until a floor is found; Slippi narrows it to 0xffff.
    pub last_ground_id: u16,
    /// None, successful, unsuccessful.
    pub l_cancel: u8,
    pub character: u8,
    pub last_attack_landed: u8,
    pub combo_count: u8,
    pub last_hit_by: u8,
    pub last_hit_by_instance: Option<u16>,
    pub instance_id: Option<u16>,
    /// Raw Melee flag bytes. The policy selects only bits modeled by Skirmish.
    pub state_flags: Option<[u8; 5]>,
    /// Action-state union slot, compared only while the hitstun flag is set.
    pub misc_as: Option<f32>,
    pub hurtbox_state: Option<u8>,
    pub velocities: Option<[f32; 5]>,
    pub hitlag: Option<f32>,
    pub animation_index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Observation {
    pub fighters: [FighterObservation; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Difference {
    pub port: Port,
    pub field: &'static str,
    pub expected: String,
    pub actual: String,
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}: expected {}, got {}",
            self.port, self.field, self.expected, self.actual
        )
    }
}

fn actors(frame: &slippi::Frame, ports: [Port; 2]) -> Result<[&slippi::Actor; 2], String> {
    if ports[0] == ports[1] {
        return Err("native player ports must be distinct".into());
    }
    if frame.actors.len() != 2 || frame.actors.iter().any(|actor| actor.follower) {
        return Err(format!(
            "frame {} requires exactly two present leader actors without followers",
            frame.id
        ));
    }
    let find = |port| {
        frame
            .actors
            .iter()
            .find(|actor| actor.port == port)
            .ok_or_else(|| format!("frame {} is missing leader {port}", frame.id))
    };
    Ok([find(ports[0])?, find(ports[1])?])
}

pub fn controllers(
    frame: &slippi::Frame,
    ports: [Port; 2],
) -> Result<[game::Controller; 2], String> {
    let convert = |actor: &slippi::Actor| {
        let pre = &actor.pre;
        let stick = [pre.joystick.x, pre.joystick.y];
        let unsupported_physical = pre.buttons_physical & !BUTTONS;
        let unsupported_logical =
            pre.buttons & !(u32::from(BUTTONS) | MAIN_STICK_FLAGS | CSTICK_FLAGS | LOGICAL_TRIGGER);
        if unsupported_physical != 0 || unsupported_logical != 0 {
            return Err(format!(
                "{} unsupported button bits: physical {unsupported_physical:#06x}, processed {unsupported_logical:#010x}",
                actor.port
            ));
        }
        let cstick = [pre.cstick.x, pre.cstick.y];
        if stick
            .iter()
            .chain(&cstick)
            .any(|axis| !axis.is_finite() || !(-1.0..=1.0).contains(axis))
        {
            return Err(format!(
                "{} stick values must be finite and within [-1, 1]",
                actor.port
            ));
        }
        if [
            pre.triggers,
            pre.triggers_physical.l,
            pre.triggers_physical.r,
        ]
        .into_iter()
        .any(|value| !(0.0..=1.0).contains(&value))
        {
            return Err(format!(
                "{} triggers must be finite and within [0, 1]",
                actor.port
            ));
        }
        let controller = game::Controller {
            buttons: pre.buttons_physical,
            stick,
            cstick,
            trigger: pre.triggers,
        };
        if pre.buttons & LOGICAL_TRIGGER != 0 && !controller.shield_held() {
            return Err(format!(
                "{} logical trigger flag has no matching pressure",
                actor.port
            ));
        }
        Ok(controller)
    };
    let [first, second] = actors(frame, ports)?;
    Ok([convert(first)?, convert(second)?])
}

pub fn expected(frame: &slippi::Frame, ports: [Port; 2]) -> Result<Observation, String> {
    let convert = |actor: &slippi::Actor| {
        let post = &actor.post;
        let airborne = match post.airborne {
            Some(0) => false,
            Some(1) => true,
            _ => {
                return Err(format!(
                    "{} post.airborne must be present and either 0 or 1",
                    actor.port
                ));
            }
        };
        let l_cancel = post.l_cancel.filter(|status| *status <= 2).ok_or_else(|| {
            format!(
                "{} post.l_cancel must be present and within 0..=2",
                actor.port
            )
        })?;
        if post.hurtbox_state.is_some_and(|state| state > 2) {
            return Err(format!(
                "{} post.hurtbox_state must be within 0..=2",
                actor.port
            ));
        }
        let flags = post
            .state_flags
            .ok_or_else(|| format!("{} post.state_flags must be present", actor.port))?;
        Ok(FighterObservation {
            port: actor.port,
            action_state: Some(post.state),
            action_age: post
                .state_age
                .ok_or_else(|| format!("{} post.state_age must be present", actor.port))?,
            position: [post.position.x, post.position.y],
            direction: post.direction,
            percent: post.percent,
            shield: post.shield,
            stocks: post.stocks,
            airborne,
            jumps_remaining: post
                .jumps
                .ok_or_else(|| format!("{} post.jumps must be present", actor.port))?,
            last_ground_id: post
                .ground
                .ok_or_else(|| format!("{} post.ground must be present", actor.port))?,
            l_cancel,
            character: post.character,
            last_attack_landed: post.last_attack_landed,
            combo_count: post.combo_count,
            last_hit_by: post.last_hit_by,
            last_hit_by_instance: post.last_hit_by_instance,
            instance_id: post.instance_id,
            state_flags: Some([flags.0, flags.1, flags.2, flags.3, flags.4]),
            misc_as: Some(
                post.misc_as
                    .ok_or_else(|| format!("{} post.misc_as must be present", actor.port))?,
            ),
            hurtbox_state: post.hurtbox_state,
            velocities: post.velocities.map(|velocity| {
                [
                    velocity.self_x_air,
                    velocity.self_y,
                    velocity.knockback_x,
                    velocity.knockback_y,
                    velocity.self_x_ground,
                ]
            }),
            hitlag: post.hitlag,
            animation_index: post.animation_index,
        })
    };
    let [first, second] = actors(frame, ports)?;
    Ok(Observation {
        fighters: [convert(first)?, convert(second)?],
    })
}

pub fn observe(game: &game::Match, ports: [Port; 2], characters: [u8; 2]) -> Observation {
    Observation {
        fighters: std::array::from_fn(|index| {
            let fighter = &game.state().fighters[index];
            let fighter_data = &game.data().fighters[index];
            let max_jumps = fighter_data
                .locomotion
                .as_ref()
                .map_or(2, |parameters| parameters.max_jumps);
            // Slippi's state_age for Walk/Run is fp->cur_anim_frame, a float
            // animation frame (Walk's restarts on each Slow/Middle/Fast
            // retype, Run's wraps at the Run figatree's length); without
            // walk_animation/run_animation, Walk/Run keep the pre-batch
            // integer action_frame.
            let action_age = if fighter.action == game::Action::Walk
                && fighter_data.movement.walk_animation.is_some()
            {
                fighter.locomotion.walk.frame
            } else if fighter.action == game::Action::Run
                && fighter_data.movement.run_animation.is_some()
            {
                fighter.locomotion.run.frame
            } else {
                fighter.action_frame as f32
            };
            FighterObservation {
                port: ports[index],
                action_state: action_state(fighter, Some(characters[index])),
                action_age,
                position: fighter.position,
                direction: fighter.facing,
                percent: fighter.percent,
                shield: fighter.shield.health,
                stocks: fighter.stocks,
                airborne: !fighter.grounded,
                jumps_remaining: max_jumps.saturating_sub(fighter.locomotion.jumps_used),
                last_ground_id: fighter
                    .last_ground_line
                    .map_or(u16::MAX, |line| line as u16),
                l_cancel: fighter.l_cancel_status,
                character: internal_character(characters[index])
                    .expect("replay setup validates external character IDs"),
                last_attack_landed: fighter.combo.last_attack_landed as u8,
                combo_count: fighter.combo.count as u8,
                last_hit_by: fighter
                    .combo
                    .last_hit_by
                    .map_or(6, |source| ports[source] as u8),
                last_hit_by_instance: Some(fighter.combo.last_hit_by_instance),
                instance_id: Some(fighter.action_instance.id),
                state_flags: Some(state_flags(fighter)),
                misc_as: Some(fighter.hitstun as f32),
                hurtbox_state: Some(hurtbox_state(fighter)),
                velocities: Some([
                    fighter.velocity[0],
                    fighter.velocity[1],
                    fighter.knockback[0],
                    fighter.knockback[1],
                    fighter.ground_velocity,
                ]),
                hitlag: Some(fighter.hitlag),
                animation_index: animation_index(fighter, Some(characters[index])),
            }
        }),
    }
}

/// Player data uses external CSS IDs; post-frame records internal fighter IDs.
pub fn internal_character(external: u8) -> Option<u8> {
    const IDS: [u8; 26] = [
        2, 3, 1, 24, 4, 5, 6, 17, 0, 18, 16, 8, 9, 12, 10, 15, 13, 14, 19, 7, 22, 20, 21, 26, 23,
        25,
    ];
    IDS.get(usize::from(external)).copied()
}

/// Project Slippi serializes the move-induced state first, then the timed
/// game-induced state. Skirmish currently models the latter as two timers.
pub fn hurtbox_state(fighter: &game::Fighter) -> u8 {
    match fighter.body_state {
        game::data::BodyState::Invincible => 1,
        game::data::BodyState::Intangible => 2,
        game::data::BodyState::Normal if fighter.intangibility > 0 => 2,
        game::data::BodyState::Normal => u8::from(fighter.invincibility > 0),
    }
}

/// Reconstruct the Slippi flag bytes for the modeled fighter-wide bits.
pub fn state_flags(fighter: &game::Fighter) -> [u8; 5] {
    let shield = fighter.grounded
        && matches!(
            fighter.action,
            game::Action::GuardOn
                | game::Action::Guard
                | game::Action::GuardSetOff
                | game::Action::GuardReflect
        );
    [
        u8::from(fighter.shield.reflecting) << 4,
        (u8::from(hurtbox_state(fighter) != 0) << 2)
            | (u8::from(fighter.fast_fall) << 3)
            | (u8::from(fighter.hitlag > 0.0) << 5),
        u8::from(shield) << 7,
        (u8::from(fighter.hitstun > 0) << 1)
            | (u8::from(fighter.shield.touched) << 2)
            | (u8::from(fighter.shield.powershield) << 5),
        (u8::from(
            fighter.death.hidden
                || matches!(
                    fighter.action,
                    game::Action::Respawn | game::Action::Eliminated
                ),
        ) << 6)
            | (u8::from(fighter.action == game::Action::Respawn) << 4),
    ]
}

/// Map each refactored action to the exact common-state identity that remains
/// available in native state. Collapsed distinctions use their canonical first
/// state; only the current Fox neutral-special shell has a character-specific
/// mapping, and lifecycle-only states stay unmapped.
pub fn action_state(fighter: &game::Fighter, character: Option<u8>) -> Option<u16> {
    use game::Action::*;
    Some(match fighter.action {
        DeadDown => 0,
        DeadLeft => 1,
        DeadRight => 2,
        DeadUp => 3,
        DeadUpStar => 4,
        DeadUpStarIce => 5,
        DeadUpFall => 6,
        DeadUpFallHitCamera => 7,
        DeadUpFallHitCameraFlat => 8,
        DeadUpFallIce => 9,
        DeadUpFallHitCameraIce => 10,
        Respawn => 11,
        Rebirth => 12,
        RebirthWait => 13,
        Wait => 14,
        // 15/16/17 by walk kind (Slow/Middle/Fast); stays 15 whenever
        // MovementData.walk_animation is absent, since the kind is then
        // never advanced away from its default (Slow).
        Walk => 15 + fighter.locomotion.walk.kind as u16,
        Turn => 18,
        RunTurn => 19,
        Dash => 20,
        Run => 21,
        RunBrake => 23,
        JumpSquat => 24,
        // JumpB/JumpAerialB and FallAerial share JumpF/JumpAerialF/Fall's
        // callbacks and differ only in the reported motion id
        // (`ftmotionstates.c:421-430`, `443-452`); Skirmish keeps a single
        // Jump/JumpAerial/Fall action and distinguishes them by the launch's
        // own recorded flag.
        Jump => {
            if fighter.locomotion.jump_backward {
                26
            } else {
                25
            }
        }
        JumpAerial => {
            if fighter.locomotion.jump_backward {
                28
            } else {
                27
            }
        }
        Fall => {
            if fighter.locomotion.fall_aerial {
                32
            } else {
                29
            }
        }
        // FallSpecialF/B are animation-only blends (`ftCo_Fall_Anim_Inner`
        // swaps the blend skeleton's figatree via `ftAnim_8006EDD0`, which
        // never writes `fp->anim_id`), so they are not Slippi-visible and
        // stay unmodeled; FallSpecial always reports its F id.
        FallSpecial => 35,
        DamageFall => 38,
        Squat => 39,
        SquatWait => 40,
        SquatRv => 41,
        Landing => 42,
        LandingFallSpecial => 43,
        Jab => 44,
        Attack12 => 45,
        Attack13 => 46,
        Attack100Start => 47,
        Attack100Loop => 48,
        Attack100End => 49,
        AttackDash => 50,
        AttackS3Hi => 51,
        AttackS3HiS => 52,
        AttackS3S => 53,
        AttackS3LwS => 54,
        AttackS3Lw => 55,
        AttackHi3 => 56,
        AttackLw3 => 57,
        AttackS4Hi => 58,
        AttackS4HiS => 59,
        AttackS4S => 60,
        AttackS4LwS => 61,
        AttackS4Lw => 62,
        AttackHi4 => 63,
        AttackLw4 => 64,
        AttackAirN => 65,
        AttackAirF => 66,
        AttackAirB => 67,
        AttackAirHi => 68,
        AttackAirLw => 69,
        LandingAirN => 70,
        LandingAirF => 71,
        LandingAirB => 72,
        LandingAirHi => 73,
        LandingAirLw => 74,
        Damage => match fighter.damage_motion {
            Some(skirmish::fighter::damage::DamageMotion::Ground { level, height }) => {
                let base = match height {
                    skirmish::fighter::damage::HurtHeight::High => 75,
                    skirmish::fighter::damage::HurtHeight::Middle => 78,
                    skirmish::fighter::damage::HurtHeight::Low => 81,
                };
                base + u16::from(level)
            }
            Some(skirmish::fighter::damage::DamageMotion::Air { level }) => 84 + u16::from(level),
            Some(skirmish::fighter::damage::DamageMotion::Fly { height }) => match height {
                skirmish::fighter::damage::HurtHeight::High => 87,
                skirmish::fighter::damage::HurtHeight::Middle => 88,
                skirmish::fighter::damage::HurtHeight::Low => 89,
            },
            // Legacy synthetic profiles predate sampled damage poses. Their
            // single Damage action corresponds to the neutral light reaction.
            None if fighter.grounded => 78,
            None => 84,
        },
        GuardOn => 178,
        Guard => 179,
        GuardOff => 180,
        GuardSetOff => 181,
        GuardReflect => 182,
        DownBound => prone_state(fighter, 183, 191),
        DownWait => prone_state(fighter, 184, 192),
        DownDamage => prone_state(fighter, 185, 193),
        DownStand => prone_state(fighter, 186, 194),
        DownAttack => prone_state(fighter, 187, 195),
        DownForward => prone_state(fighter, 188, 196),
        DownBack => prone_state(fighter, 189, 197),
        Passive => 199,
        PassiveStandF => 200,
        PassiveStandB => 201,
        PassiveWall => 202,
        PassiveWallJump => 203,
        PassiveCeiling => 204,
        ShieldBreakFly => 205,
        ShieldBreakFall => 206,
        ShieldBreakDown => prone_state(fighter, 207, 208),
        ShieldBreakStand => prone_state(fighter, 209, 210),
        Furafura => 211,
        EscapeF => 233,
        EscapeB => 234,
        EscapeN => 235,
        EscapeAir => 236,
        Catch => 212,
        CatchPull => 213,
        CatchDash => 214,
        CatchDashPull => 215,
        CatchWait => 216,
        CatchAttack => 217,
        CatchCut => 218,
        ThrowF => 219,
        ThrowB => 220,
        ThrowHi => 221,
        ThrowLw => 222,
        CapturePulledHi => 223,
        CaptureWaitHi => 224,
        CaptureDamageHi => 225,
        CapturePulledLw => 226,
        CaptureWaitLw => 227,
        CaptureDamageLw => 228,
        CaptureCut => 229,
        ReboundStop => 237,
        Rebound => 238,
        ThrownF => 239,
        ThrownB => 240,
        ThrownHi => 241,
        ThrownLw => 242,
        Pass => 244,
        Ottotto => 245,
        OttottoWait => 246,
        FlyReflectWall => 247,
        FlyReflectCeiling => 248,
        CliffCatch => 252,
        CliffWait => 253,
        CliffClimb => {
            if fighter.ledge.slow {
                254
            } else {
                255
            }
        }
        CliffAttack => {
            if fighter.ledge.slow {
                256
            } else {
                257
            }
        }
        CliffEscape => {
            if fighter.ledge.slow {
                258
            } else {
                259
            }
        }
        CliffJump => {
            if fighter.ledge.slow {
                260
            } else {
                262
            }
        }
        // The current character-specific resource slice is Fox. Its generic
        // neutral-special shell retains only the startup family distinction.
        SpecialN if character == Some(2) => 341,
        SpecialAirN if character == Some(2) => 344,
        SpecialN | SpecialAirN | Eliminated => return None,
    })
}

/// Resolve `Fighter::anim_id` from the exact common motion-state table. Values
/// without a figatree retain Melee's `-1` bit pattern in Slippi's `u32` field.
pub fn animation_index(fighter: &game::Fighter, character: Option<u8>) -> Option<u32> {
    let state = action_state(fighter, character)?;
    Some(match state {
        0..=3 | 5 | 9..=11 | 237 => u32::MAX,
        4 | 6 | 38 => 29,
        7 => 0,
        8 => 1,
        // Rebirth/RebirthWait keep the pre-batch constant 2; only Wait (14)
        // reports the tracked idle animation (optional idle-animation
        // profile: 2 -- Wait1_0 -- while absent, matching the constant this
        // replaced).
        12 | 13 => 2,
        14 => fighter.idle.animation,
        // WalkSlow/WalkMiddle/WalkFast sub-motions 7/8/9.
        15..=17 => u32::from(state - 8),
        18..=21 => u32::from(state - 8),
        // RunBrake..Fall (23..29) and FallAerial/FallSpecial (32, 35) share
        // one contiguous sub-motion block (`ftCo_Submotion`, forward.h:649+).
        23..=29 | 32 | 35 => u32::from(state - 9),
        39 | 40 => u32::from(state - 9),
        41..=43 => u32::from(state - 7),
        44..=64 => u32::from(state + 2),
        65..=74 => u32::from(state + 3),
        75..=91 => u32::from(state + 90),
        178..=181 => u32::from(state - 141),
        182 => 37,
        183..=204 => u32::from(state),
        205..=210 => u32::from(state + 81),
        211 => 205,
        212 | 213 => 242,
        214 | 215 => 243,
        216..=229 => u32::from(state + 28),
        233 | 234 => u32::from(state - 191),
        235 => 41,
        236 => 44,
        238 => 45,
        239..=242 => u32::from(state + 23),
        244 => 209,
        245 => 210,
        246 => 211,
        247 => 212,
        248 => 214,
        252 => 216,
        253 => 217,
        254..=260 | 262 => u32::from(state - 35),
        341 => 295,
        344 => 298,
        _ => return None,
    })
}

fn prone_state(fighter: &game::Fighter, face_up: u16, face_down: u16) -> u16 {
    match fighter.prone {
        Some(game::damage::ProneOrientation::FaceDown) => face_down,
        _ => face_up,
    }
}

fn difference(
    port: Port,
    field: &'static str,
    expected: u32,
    actual: u32,
    width: usize,
) -> Option<Difference> {
    (expected != actual).then(|| Difference {
        port,
        field,
        expected: format!("0x{expected:0width$x}"),
        actual: format!("0x{actual:0width$x}"),
    })
}

fn optional_difference(
    port: Port,
    field: &'static str,
    expected: Option<u16>,
    actual: Option<u16>,
) -> Option<Difference> {
    (expected != actual).then(|| Difference {
        port,
        field,
        expected: expected.map_or_else(|| "unmapped".into(), |v| format!("0x{v:04x}")),
        actual: actual.map_or_else(|| "unmapped".into(), |v| format!("0x{v:04x}")),
    })
}

fn float_difference(
    port: Port,
    field: &'static str,
    expected: f32,
    actual: f32,
) -> Option<Difference> {
    difference(port, field, expected.to_bits(), actual.to_bits(), 8)
}

pub fn compare(expected: &Observation, actual: &Observation) -> Option<Difference> {
    for (expected, actual) in expected.fighters.iter().zip(&actual.fighters) {
        let port = expected.port;
        if let Some(difference) =
            difference(port, "port", expected.port as u32, actual.port as u32, 2)
        {
            return Some(difference);
        }
        if let Some(difference) = optional_difference(
            port,
            BASE_FIELDS[0],
            expected.action_state,
            actual.action_state,
        ) {
            return Some(difference);
        }
        if let Some(expected) = expected.animation_index {
            let Some(actual) = actual.animation_index else {
                return Some(Difference {
                    port,
                    field: ANIMATION_FIELD,
                    expected: format!("0x{expected:08x}"),
                    actual: "unavailable".into(),
                });
            };
            if let Some(difference) = difference(port, ANIMATION_FIELD, expected, actual, 8) {
                return Some(difference);
            }
        }
        for (field, expected, actual) in [
            (BASE_FIELDS[1], expected.action_age, actual.action_age),
            (BASE_FIELDS[2], expected.position[0], actual.position[0]),
            (BASE_FIELDS[3], expected.position[1], actual.position[1]),
            (BASE_FIELDS[4], expected.direction, actual.direction),
            (BASE_FIELDS[5], expected.percent, actual.percent),
            (BASE_FIELDS[6], expected.shield, actual.shield),
        ] {
            if let Some(difference) = float_difference(port, field, expected, actual) {
                return Some(difference);
            }
        }
        for (field, expected, actual) in [
            (
                INSTANCE_FIELDS[0],
                expected.last_hit_by_instance,
                actual.last_hit_by_instance,
            ),
            (INSTANCE_FIELDS[1], expected.instance_id, actual.instance_id),
        ] {
            if let Some(expected) = expected {
                let Some(actual) = actual else {
                    return Some(Difference {
                        port,
                        field,
                        expected: format!("0x{expected:04x}"),
                        actual: "unavailable".into(),
                    });
                };
                if let Some(difference) =
                    difference(port, field, u32::from(expected), u32::from(actual), 4)
                {
                    return Some(difference);
                }
            }
        }
        for (field, expected, actual) in [
            (BASE_FIELDS[7], expected.stocks, actual.stocks),
            (
                BASE_FIELDS[8],
                u8::from(expected.airborne),
                u8::from(actual.airborne),
            ),
            (
                BASE_FIELDS[9],
                expected.jumps_remaining,
                actual.jumps_remaining,
            ),
        ] {
            if let Some(difference) =
                difference(port, field, u32::from(expected), u32::from(actual), 2)
            {
                return Some(difference);
            }
        }
        if let Some(difference) = difference(
            port,
            BASE_FIELDS[10],
            u32::from(expected.last_ground_id),
            u32::from(actual.last_ground_id),
            4,
        ) {
            return Some(difference);
        }
        if let Some(difference) = difference(
            port,
            BASE_FIELDS[11],
            u32::from(expected.l_cancel),
            u32::from(actual.l_cancel),
            2,
        ) {
            return Some(difference);
        }
        for (field, expected, actual) in [
            (PROVENANCE_FIELDS[0], expected.character, actual.character),
            (
                PROVENANCE_FIELDS[1],
                expected.last_attack_landed,
                actual.last_attack_landed,
            ),
            (
                PROVENANCE_FIELDS[2],
                expected.combo_count,
                actual.combo_count,
            ),
            (
                PROVENANCE_FIELDS[3],
                expected.last_hit_by,
                actual.last_hit_by,
            ),
        ] {
            if let Some(difference) =
                difference(port, field, u32::from(expected), u32::from(actual), 2)
            {
                return Some(difference);
            }
        }
        let selected_flags = [
            (0, 0x10),
            (1, 0x04),
            (1, 0x08),
            (1, 0x20),
            (2, 0x80),
            (3, 0x02),
            (3, 0x04),
            (3, 0x20),
            (4, 0x40),
            (4, 0x10),
        ];
        if let Some(expected_flags) = expected.state_flags {
            let Some(actual_flags) = actual.state_flags else {
                return Some(Difference {
                    port,
                    field: STATE_FLAG_FIELDS[0],
                    expected: format!("0x{:02x}", expected_flags[1] & 0x04),
                    actual: "unavailable".into(),
                });
            };
            for (field, (byte, mask)) in STATE_FLAG_FIELDS.iter().copied().zip(selected_flags) {
                if let Some(difference) = difference(
                    port,
                    field,
                    u32::from(expected_flags[byte] & mask != 0),
                    u32::from(actual_flags[byte] & mask != 0),
                    2,
                ) {
                    return Some(difference);
                }
            }
            if expected_flags[3] & 0x02 != 0 {
                let (Some(expected_misc), Some(actual_misc)) = (expected.misc_as, actual.misc_as)
                else {
                    return Some(Difference {
                        port,
                        field: MISC_HITSTUN_FIELD,
                        expected: expected.misc_as.map_or_else(
                            || "unavailable".into(),
                            |v| format!("0x{:08x}", v.to_bits()),
                        ),
                        actual: actual.misc_as.map_or_else(
                            || "unavailable".into(),
                            |v| format!("0x{:08x}", v.to_bits()),
                        ),
                    });
                };
                if let Some(difference) =
                    float_difference(port, MISC_HITSTUN_FIELD, expected_misc, actual_misc)
                {
                    return Some(difference);
                }
            }
        }
        if let Some(expected_hurtbox_state) = expected.hurtbox_state {
            let Some(actual_hurtbox_state) = actual.hurtbox_state else {
                return Some(Difference {
                    port,
                    field: HURTBOX_FIELD,
                    expected: format!("0x{expected_hurtbox_state:02x}"),
                    actual: "unavailable".into(),
                });
            };
            if let Some(difference) = difference(
                port,
                HURTBOX_FIELD,
                u32::from(expected_hurtbox_state),
                u32::from(actual_hurtbox_state),
                2,
            ) {
                return Some(difference);
            }
        }
        if let Some(expected_velocities) = expected.velocities {
            let Some(actual_velocities) = actual.velocities else {
                return Some(Difference {
                    port,
                    field: VELOCITY_FIELDS[0],
                    expected: format!("0x{:08x}", expected_velocities[0].to_bits()),
                    actual: "unavailable".into(),
                });
            };
            for ((field, expected), actual) in VELOCITY_FIELDS
                .iter()
                .copied()
                .zip(expected_velocities)
                .zip(actual_velocities)
            {
                if let Some(difference) = float_difference(port, field, expected, actual) {
                    return Some(difference);
                }
            }
        }
        if let Some(expected_hitlag) = expected.hitlag {
            let Some(actual_hitlag) = actual.hitlag else {
                return Some(Difference {
                    port,
                    field: HITLAG_FIELD,
                    expected: format!("0x{:08x}", expected_hitlag.to_bits()),
                    actual: "unavailable".into(),
                });
            };
            if let Some(difference) =
                float_difference(port, HITLAG_FIELD, expected_hitlag, actual_hitlag)
            {
                return Some(difference);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use slippi::row;

    const PORTS: [Port; 2] = [Port::P3, Port::P1];

    fn frame() -> slippi::Frame {
        slippi::Frame {
            id: -123,
            actors: [Port::P1, Port::P3]
                .map(|port| slippi::Actor {
                    port,
                    follower: false,
                    pre: row::Pre::default(),
                    post: row::Post {
                        state: 14,
                        state_age: Some(0.0),
                        airborne: Some(0),
                        jumps: Some(2),
                        ground: Some(u16::MAX),
                        l_cancel: Some(0),
                        character: 1,
                        last_attack_landed: 0,
                        combo_count: 0,
                        last_hit_by: 6,
                        last_hit_by_instance: Some(0),
                        instance_id: Some(1),
                        state_flags: Some(row::StateFlags(0, 0, 0, 2, 0)),
                        misc_as: Some(0.0),
                        hurtbox_state: Some(0),
                        stocks: 4,
                        velocities: Some(row::Velocities::default()),
                        hitlag: Some(0.0),
                        animation_index: Some(2),
                        ..Default::default()
                    },
                })
                .into(),
            start: None,
            end: None,
            items: None,
            fod_platforms: None,
            dreamland_whispys: None,
            stadium_transformations: None,
        }
    }

    #[test]
    fn controllers_preserve_stick_bits_and_do_not_copy_recorded_state() {
        let mut frame = frame();
        let pre = &mut frame.actors[1].pre;
        pre.joystick = row::Position {
            x: -0.0,
            y: f32::from_bits(0x3eaa_aaab),
        };
        pre.buttons_physical = game::BUTTON_A | game::BUTTON_B | game::BUTTON_X | game::BUTTON_Z;
        pre.buttons = u32::from(pre.buttons_physical) | MAIN_STICK_FLAGS;
        pre.cstick.x = -0.0;
        pre.triggers_physical.r = -0.0;
        pre.position = row::Position {
            x: f32::NAN,
            y: f32::INFINITY,
        };
        pre.random_seed = u32::MAX;
        pre.state = u16::MAX;
        let controllers = controllers(&frame, PORTS).unwrap();
        assert_eq!(
            controllers[0].stick.map(f32::to_bits),
            [0x8000_0000, 0x3eaa_aaab]
        );
        assert_eq!(
            controllers[0].buttons,
            game::BUTTON_A | game::BUTTON_B | game::BUTTON_X | game::BUTTON_Z
        );
        assert_eq!(controllers[1], game::Controller::default());
    }

    #[test]
    fn unsupported_inputs_and_actor_sets_are_errors() {
        for flag in [0x1, 0x80, 0x1000] {
            let mut frame = frame();
            frame.actors[0].pre.buttons_physical = flag;
            assert!(controllers(&frame, PORTS).is_err());
        }
        for flag in [0x1, 0x1000, 0x0100_0000, 0x8000_0000] {
            let mut frame = frame();
            frame.actors[0].pre.buttons = flag;
            assert!(controllers(&frame, PORTS).is_err());
        }
        for index in 0..5 {
            let mut frame = frame();
            let pre = &mut frame.actors[0].pre;
            let fields = [
                &mut pre.cstick.x,
                &mut pre.cstick.y,
                &mut pre.triggers,
                &mut pre.triggers_physical.l,
                &mut pre.triggers_physical.r,
            ];
            *fields[index] = f32::NAN;
            assert!(controllers(&frame, PORTS).is_err());
        }
        for value in [f32::NAN, f32::INFINITY, -1.001, 1.001] {
            let mut frame = frame();
            frame.actors[0].pre.joystick.x = value;
            assert!(controllers(&frame, PORTS).is_err());
        }
        assert!(controllers(&frame(), [Port::P1; 2]).is_err());
        let mut missing = frame();
        missing.actors.pop();
        assert!(controllers(&missing, PORTS).is_err());
        let mut follower = frame();
        follower.actors[1].follower = true;
        assert!(expected(&follower, PORTS).is_err());
    }

    #[test]
    fn processed_cstick_trigger_and_shoulder_channels_preserve_bits() {
        let mut frame = frame();
        let pre = &mut frame.actors[1].pre;
        pre.cstick = row::Position {
            x: -0.0,
            y: f32::from_bits(0x3eaa_aaab),
        };
        pre.triggers = f32::from_bits(0x3dcc_cccd);
        pre.triggers_physical.l = 0.75;
        pre.triggers_physical.r = 0.5;
        pre.buttons = LOGICAL_TRIGGER | CSTICK_FLAGS;
        let input = controllers(&frame, PORTS).unwrap()[0];
        assert_eq!(input.cstick.map(f32::to_bits), [0x8000_0000, 0x3eaa_aaab]);
        assert_eq!(input.trigger.to_bits(), 0x3dcc_cccd);
        assert_eq!(input.shield_pressure().to_bits(), 0x3dcc_cccd);
        for button in [game::BUTTON_L, game::BUTTON_R] {
            frame.actors[1].pre.buttons_physical = button;
            let input = controllers(&frame, PORTS).unwrap()[0];
            assert_eq!(input.buttons, button);
            assert_eq!(input.shield_pressure(), 1.0);
            assert_eq!(input.trigger.to_bits(), 0x3dcc_cccd);
        }
    }

    #[test]
    fn observations_map_ports_and_report_each_selected_field_by_bits() {
        let mut replay_frame = frame();
        replay_frame.actors[1].post.position.x = -0.0;
        let expected = expected(&replay_frame, PORTS).unwrap();
        assert_eq!(expected.fighters[0].port, Port::P3);
        assert_eq!(expected.fighters[0].position[0].to_bits(), 0x8000_0000);
        assert!(compare(&expected, &expected).is_none());
        for &field in BASE_FIELDS
            .iter()
            .chain(PROVENANCE_FIELDS)
            .chain(INSTANCE_FIELDS)
            .chain(STATE_FLAG_FIELDS)
            .chain([MISC_HITSTUN_FIELD].iter())
            .chain([HURTBOX_FIELD].iter())
            .chain(VELOCITY_FIELDS)
            .chain([HITLAG_FIELD].iter())
            .chain([ANIMATION_FIELD].iter())
        {
            let mut actual = expected.clone();
            let fighter = &mut actual.fighters[0];
            match field {
                "action_state" => fighter.action_state = None,
                "action_age" => fighter.action_age = 1.0,
                "position.x" => fighter.position[0] = 0.0,
                "position.y" => fighter.position[1] = f32::from_bits(1),
                "direction" => fighter.direction = -1.0,
                "percent" => fighter.percent = 1.0,
                "shield" => fighter.shield = 1.0,
                "stocks" => fighter.stocks -= 1,
                "airborne" => fighter.airborne = true,
                "jumps_remaining" => fighter.jumps_remaining -= 1,
                "last_ground_id" => fighter.last_ground_id = 7,
                "l_cancel" => fighter.l_cancel = 1,
                "character" => fighter.character ^= 1,
                "last_attack_landed" => fighter.last_attack_landed = 7,
                "combo_count" => fighter.combo_count = 1,
                "last_hit_by" => fighter.last_hit_by = 0,
                "last_hit_by_instance" => fighter.last_hit_by_instance = Some(7),
                "instance_id" => fighter.instance_id = Some(7),
                "state_flags.reflect" => fighter.state_flags.as_mut().unwrap()[0] ^= 0x10,
                "state_flags.protected" => fighter.state_flags.as_mut().unwrap()[1] ^= 0x04,
                "state_flags.fast_fall" => fighter.state_flags.as_mut().unwrap()[1] ^= 0x08,
                "state_flags.hitlag" => fighter.state_flags.as_mut().unwrap()[1] ^= 0x20,
                "state_flags.shield" => fighter.state_flags.as_mut().unwrap()[2] ^= 0x80,
                "state_flags.hitstun" => fighter.state_flags.as_mut().unwrap()[3] ^= 0x02,
                "state_flags.shield_touch" => fighter.state_flags.as_mut().unwrap()[3] ^= 0x04,
                "state_flags.powershield" => fighter.state_flags.as_mut().unwrap()[3] ^= 0x20,
                "state_flags.dead" => fighter.state_flags.as_mut().unwrap()[4] ^= 0x40,
                "state_flags.sleep" => fighter.state_flags.as_mut().unwrap()[4] ^= 0x10,
                "misc_as.hitstun" => fighter.misc_as = Some(1.0),
                "hurtbox_state" => fighter.hurtbox_state = Some(1),
                "velocities.self_x_air" => fighter.velocities.as_mut().unwrap()[0] = 1.0,
                "velocities.self_y" => fighter.velocities.as_mut().unwrap()[1] = 1.0,
                "velocities.knockback_x" => fighter.velocities.as_mut().unwrap()[2] = 1.0,
                "velocities.knockback_y" => fighter.velocities.as_mut().unwrap()[3] = 1.0,
                "velocities.self_x_ground" => fighter.velocities.as_mut().unwrap()[4] = 1.0,
                "hitlag" => fighter.hitlag = Some(1.0),
                "animation_index" => fighter.animation_index = None,
                _ => unreachable!(),
            }
            let difference = compare(&expected, &actual).unwrap();
            assert_eq!((difference.port, difference.field), (Port::P3, field));
            assert!(difference.to_string().contains(field));
            assert!(serde_json::to_string(&difference).unwrap().contains("0x"));
            if field == "position.x" {
                assert_eq!(difference.expected, "0x80000000");
                assert_eq!(difference.actual, "0x00000000");
            }
        }
        for airborne in [None, Some(2)] {
            replay_frame.actors[0].post.airborne = airborne;
            assert!(super::expected(&replay_frame, PORTS).is_err());
        }
        for (l_cancel, hurtbox_state) in [(Some(3), Some(0)), (Some(0), Some(3))] {
            let mut invalid = frame();
            invalid.actors[0].post.l_cancel = l_cancel;
            invalid.actors[0].post.hurtbox_state = hurtbox_state;
            assert!(super::expected(&invalid, PORTS).is_err());
        }
    }

    #[test]
    fn native_observation_uses_only_the_declared_post_fields() {
        let data = serde_json::from_str(include_str!(
            "../../../tests/fixtures/game/integration-match.json"
        ))
        .unwrap();
        let game = game::Match::new(data, 1).unwrap();
        let observed = observe(&game, PORTS, [2; 2]);
        for (index, fighter) in observed.fighters.iter().enumerate() {
            let native = &game.state().fighters[index];
            assert_eq!(fighter.port, PORTS[index]);
            assert_eq!(
                fighter.position.map(f32::to_bits),
                native.position.map(f32::to_bits)
            );
            assert_eq!(fighter.direction.to_bits(), native.facing.to_bits());
            assert_eq!(fighter.percent.to_bits(), native.percent.to_bits());
            assert_eq!(fighter.action_age, native.action_frame as f32);
            assert_eq!(fighter.action_state, action_state(native, Some(2)));
            assert_eq!(fighter.shield.to_bits(), native.shield.health.to_bits());
            assert_eq!(fighter.stocks, native.stocks);
            assert_eq!(fighter.airborne, !native.grounded);
            assert_eq!(fighter.jumps_remaining, 2 - native.locomotion.jumps_used);
            assert_eq!(
                fighter.last_ground_id,
                native.last_ground_line.map_or(u16::MAX, |line| line as u16)
            );
            assert_eq!(fighter.l_cancel, native.l_cancel_status);
            assert_eq!(fighter.character, 1);
            assert_eq!(
                fighter.last_attack_landed,
                native.combo.last_attack_landed as u8
            );
            assert_eq!(fighter.combo_count, native.combo.count as u8);
            assert_eq!(fighter.last_hit_by, 6);
            assert_eq!(fighter.last_hit_by_instance, Some(0));
            assert_eq!(fighter.instance_id, Some(native.action_instance.id));
            assert_eq!(fighter.state_flags, Some(state_flags(native)));
            assert_eq!(fighter.misc_as, Some(native.hitstun as f32));
            assert_eq!(fighter.hurtbox_state, Some(hurtbox_state(native)));
            assert_eq!(
                fighter.velocities.unwrap().map(f32::to_bits),
                [
                    native.velocity[0],
                    native.velocity[1],
                    native.knockback[0],
                    native.knockback[1],
                    native.ground_velocity,
                ]
                .map(f32::to_bits)
            );
            assert_eq!(fighter.hitlag.unwrap().to_bits(), native.hitlag.to_bits());
            assert_eq!(fighter.animation_index, animation_index(native, Some(2)));
        }
    }

    #[test]
    fn common_action_and_animation_ids_preserve_the_pinned_tables() {
        let data = serde_json::from_str(include_str!(
            "../../../tests/fixtures/game/integration-match.json"
        ))
        .unwrap();
        let game = game::Match::new(data, 1).unwrap();
        let mut fighter = game.state().fighters[0].clone();
        for (action, state, animation) in [
            (game::Action::Wait, 14, 2),
            (game::Action::Walk, 15, 7),
            (game::Action::Dash, 20, 12),
            (game::Action::JumpSquat, 24, 15),
            (game::Action::AttackAirLw, 69, 72),
            (game::Action::Guard, 179, 38),
            (game::Action::GuardReflect, 182, 37),
            (game::Action::EscapeF, 233, 42),
            (game::Action::EscapeB, 234, 43),
            (game::Action::EscapeN, 235, 41),
            (game::Action::EscapeAir, 236, 44),
            (game::Action::AttackS3S, 53, 55),
            (game::Action::AttackHi3, 56, 58),
            (game::Action::AttackLw3, 57, 59),
            (game::Action::AttackS4S, 60, 62),
            (game::Action::AttackHi4, 63, 65),
            (game::Action::AttackLw4, 64, 66),
            (game::Action::Attack12, 45, 47),
            (game::Action::Attack13, 46, 48),
            (game::Action::Attack100Start, 47, 49),
            (game::Action::Attack100Loop, 48, 50),
            (game::Action::Attack100End, 49, 51),
            (game::Action::AttackDash, 50, 52),
            (game::Action::FallSpecial, 35, 26),
            (game::Action::LandingFallSpecial, 43, 36),
            (game::Action::PassiveWallJump, 203, 203),
            (game::Action::ThrowLw, 222, 250),
            (game::Action::FlyReflectCeiling, 248, 214),
            (game::Action::DeadUpFallHitCameraIce, 10, u32::MAX),
        ] {
            fighter.action = action;
            assert_eq!(action_state(&fighter, Some(2)), Some(state), "{action:?}");
            assert_eq!(
                animation_index(&fighter, Some(2)),
                Some(animation),
                "{action:?}"
            );
        }

        fighter.action = game::Action::Walk;
        fighter.locomotion.walk.kind = skirmish::game::locomotion::WalkKind::Middle;
        assert_eq!(action_state(&fighter, Some(2)), Some(16));
        assert_eq!(animation_index(&fighter, Some(2)), Some(8));
        fighter.locomotion.walk.kind = skirmish::game::locomotion::WalkKind::Fast;
        assert_eq!(action_state(&fighter, Some(2)), Some(17));
        assert_eq!(animation_index(&fighter, Some(2)), Some(9));
        fighter.locomotion.walk.kind = skirmish::game::locomotion::WalkKind::Slow;

        fighter.action = game::Action::Jump;
        fighter.locomotion.jump_backward = false;
        assert_eq!(action_state(&fighter, Some(2)), Some(25));
        assert_eq!(animation_index(&fighter, Some(2)), Some(16));
        fighter.locomotion.jump_backward = true;
        assert_eq!(action_state(&fighter, Some(2)), Some(26));
        assert_eq!(animation_index(&fighter, Some(2)), Some(17));
        fighter.action = game::Action::JumpAerial;
        assert_eq!(action_state(&fighter, Some(2)), Some(28));
        assert_eq!(animation_index(&fighter, Some(2)), Some(19));
        fighter.locomotion.jump_backward = false;
        assert_eq!(action_state(&fighter, Some(2)), Some(27));
        assert_eq!(animation_index(&fighter, Some(2)), Some(18));
        fighter.action = game::Action::Fall;
        fighter.locomotion.fall_aerial = true;
        assert_eq!(action_state(&fighter, Some(2)), Some(32));
        assert_eq!(animation_index(&fighter, Some(2)), Some(23));
        fighter.locomotion.fall_aerial = false;
        assert_eq!(action_state(&fighter, Some(2)), Some(29));
        assert_eq!(animation_index(&fighter, Some(2)), Some(20));

        fighter.action = game::Action::Damage;
        fighter.damage_motion = Some(skirmish::fighter::damage::DamageMotion::Ground {
            level: 2,
            height: skirmish::fighter::damage::HurtHeight::Low,
        });
        assert_eq!(action_state(&fighter, Some(2)), Some(83));
        assert_eq!(animation_index(&fighter, Some(2)), Some(173));
        fighter.damage_motion = Some(skirmish::fighter::damage::DamageMotion::Fly {
            height: skirmish::fighter::damage::HurtHeight::High,
        });
        assert_eq!(action_state(&fighter, Some(2)), Some(87));
        assert_eq!(animation_index(&fighter, Some(2)), Some(177));

        fighter.action = game::Action::DownWait;
        fighter.prone = Some(game::damage::ProneOrientation::FaceDown);
        assert_eq!(action_state(&fighter, Some(2)), Some(192));
        assert_eq!(animation_index(&fighter, Some(2)), Some(192));
        fighter.action = game::Action::CliffAttack;
        fighter.ledge.slow = false;
        assert_eq!(action_state(&fighter, Some(2)), Some(257));
        assert_eq!(animation_index(&fighter, Some(2)), Some(222));
        fighter.ledge.slow = true;
        assert_eq!(action_state(&fighter, Some(2)), Some(256));
        assert_eq!(animation_index(&fighter, Some(2)), Some(221));

        fighter.action = game::Action::SpecialN;
        assert_eq!(action_state(&fighter, Some(2)), Some(341));
        assert_eq!(animation_index(&fighter, Some(2)), Some(295));
        fighter.action = game::Action::SpecialAirN;
        assert_eq!(action_state(&fighter, Some(2)), Some(344));
        assert_eq!(animation_index(&fighter, Some(2)), Some(298));

        fighter.action = game::Action::Respawn;
        assert_eq!(action_state(&fighter, Some(2)), Some(11));
        assert_eq!(animation_index(&fighter, Some(2)), Some(u32::MAX));
        fighter.action = game::Action::Eliminated;
        assert_eq!(action_state(&fighter, Some(2)), None);
        assert_eq!(animation_index(&fighter, Some(2)), None);
        fighter.action = game::Action::SpecialN;
        assert_eq!(action_state(&fighter, None), None);
        assert_eq!(animation_index(&fighter, None), None);
    }

    #[test]
    fn hurtbox_state_preserves_intangibility_priority() {
        let data = serde_json::from_str(include_str!(
            "../../../tests/fixtures/game/integration-match.json"
        ))
        .unwrap();
        let game = game::Match::new(data, 1).unwrap();
        let mut fighter = game.state().fighters[0].clone();
        assert_eq!(hurtbox_state(&fighter), 0);
        fighter.invincibility = 3;
        assert_eq!(hurtbox_state(&fighter), 1);
        fighter.intangibility = 2;
        assert_eq!(hurtbox_state(&fighter), 2);
        fighter.fast_fall = true;
        fighter.hitlag = 2.0;
        fighter.hitstun = 3;
        fighter.action = game::Action::Guard;
        assert_eq!(state_flags(&fighter), [0, 0x2c, 0x80, 0x02, 0]);
        fighter.shield.reflecting = true;
        fighter.shield.powershield = true;
        fighter.shield.touched = true;
        assert_eq!(state_flags(&fighter), [0x10, 0x2c, 0x80, 0x26, 0]);
        fighter.shield.reflecting = false;
        fighter.shield.powershield = false;
        fighter.shield.touched = false;
        fighter.death.hidden = true;
        assert_eq!(state_flags(&fighter)[4], 0x40);
        fighter.death.hidden = false;
        fighter.action = game::Action::Respawn;
        assert_eq!(state_flags(&fighter)[4], 0x50);
        fighter.action = game::Action::Eliminated;
        assert_eq!(state_flags(&fighter)[4], 0x40);
        fighter.action = game::Action::Rebirth;
        assert_eq!(state_flags(&fighter)[4], 0);
    }

    #[test]
    fn external_character_ids_map_to_the_original_internal_table() {
        let expected = [
            2, 3, 1, 24, 4, 5, 6, 17, 0, 18, 16, 8, 9, 12, 10, 15, 13, 14, 19, 7, 22, 20, 21, 26,
            23, 25,
        ];
        for (external, internal) in expected.into_iter().enumerate() {
            assert_eq!(internal_character(external as u8), Some(internal));
        }
        assert_eq!(internal_character(26), None);
        assert_eq!(internal_character(u8::MAX), None);
    }
}
