//! Ordinary shield callbacks with caller-supplied native common/character data.
//! Yoshi shields, powershields, shield-tilt animation, rolls/grabs and projectile
//! reflection are separate unported branches. Shield break uses explicit native
//! animation durations rather than guessed character timing or render state.
use super::{
    Action, Controller, Error, Event, Fighter, State,
    data::{FighterData, Hitbox, MatchData},
};
use crate::{
    collision::bones,
    fighter::{combat, shield as math},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub maximum_health: f32,
    pub minimum_radius_scale: f32,
    pub size_scales: [f32; 2],
    pub analog_deadzone: f32,
    pub minimum_hold_frames: f32,
    pub drain_rate: f32,
    pub drain_scales: [f32; 2],
    pub regeneration: f32,
    pub damage_scale: f32,
    pub damage_base: f32,
    pub damage_scales: [f32; 2],
    pub stun_scale: f32,
    pub stun_base: f32,
    pub stun_scales: [f32; 2],
    pub push_scale: f32,
    pub push_multiplier: f32,
    pub push_maximum: f32,
    pub attacker_push_scale: f32,
    pub attacker_push_base: f32,
    pub attacker_air_decay: f32,
    pub attacker_ground_friction_multiplier: f32,
    pub hitlag_maximum: f32,
    pub sdi_horizontal_multiplier: f32,
    pub break_health: f32,
    pub dizzy_percent_base: f32,
    pub dizzy_base_frames: f32,
    pub dizzy_frame_decay: f32,
    pub dizzy_mash_rate: f32,
    pub mash_stick_threshold: f32,
    /// Only the ordinary GuardOn path is translated in this batch.
    pub powershield_input_window: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    pub bone: usize,
    pub offset: [f32; 3],
    pub initial_radius: f32,
    pub raise_frames: f32,
    pub release_frames: u32,
    pub stun_animation_end: f32,
    pub break_initial_velocity: f32,
    pub break_fly_frames: u32,
    pub break_down_frames: u32,
    pub break_stand_frames: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ShieldState {
    pub health: f32,
    pub strength: f32,
    pub minimum_hold: f32,
    pub release_latched: bool,
    pub raise_progress: f32,
    pub stun_progress: f32,
    pub stun_rate: f32,
    pub attacker_push: [f32; 2],
    pub attacker_ground_push: f32,
    pub dizzy_timer: f32,
    pub mash_directions: [i8; 2],
}

pub(crate) fn validate(r: &Rules, fighter: &FighterData) -> Result<(), Error> {
    let nonnegative = [
        r.maximum_health,
        r.minimum_hold_frames,
        r.drain_rate,
        r.regeneration,
        r.damage_scale,
        r.damage_base,
        r.stun_scale,
        r.stun_base,
        r.push_scale,
        r.push_multiplier,
        r.push_maximum,
        r.attacker_push_scale,
        r.attacker_push_base,
        r.attacker_air_decay,
        r.attacker_ground_friction_multiplier,
        r.hitlag_maximum,
        r.sdi_horizontal_multiplier,
        r.break_health,
        r.dizzy_percent_base,
        r.dizzy_base_frames,
        r.dizzy_frame_decay,
        r.dizzy_mash_rate,
    ];
    if !nonnegative
        .into_iter()
        .chain(r.size_scales)
        .chain(r.drain_scales)
        .all(|v| (0.0..=1_000_000.0).contains(&v))
        || !r
            .damage_scales
            .into_iter()
            .chain(r.stun_scales)
            .all(|v| (0.0..=1.0).contains(&v))
        || !(0.0..1.0).contains(&r.analog_deadzone)
        || !(0.0..=1.0).contains(&r.minimum_radius_scale)
        || r.minimum_radius_scale == 0.0
        || !(0.0..=1.0).contains(&r.mash_stick_threshold)
        || r.maximum_health == 0.0
        || r.stun_base == 0.0
        || r.dizzy_frame_decay == 0.0
        || r.break_health > r.maximum_health
        || r.powershield_input_window != 0
    {
        return Err(Error::Data(
            "invalid or unsupported ordinary shield rules".into(),
        ));
    }
    if let Some(a) = &fighter.shield {
        if a.bone >= fighter.bones.len()
            || !a
                .offset
                .into_iter()
                .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
            || ![
                a.initial_radius,
                a.raise_frames,
                a.stun_animation_end,
                a.break_initial_velocity,
            ]
            .into_iter()
            .all(|v| v > 0.0 && v <= 1_000_000.0)
            || ![
                a.release_frames,
                a.break_fly_frames,
                a.break_down_frames,
                a.break_stand_frames,
            ]
            .into_iter()
            .all(|v| (1..=1_000_000).contains(&v))
        {
            return Err(Error::Data(
                "invalid shield geometry or animation durations".into(),
            ));
        }
        if fighter.bones[a.bone].scale != [1.0; 3] {
            return Err(Error::Data(
                "shield bone requires unit local scale; its native radius replaces that scale"
                    .into(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn active(f: &Fighter) -> bool {
    f.grounded
        && matches!(
            f.action,
            Action::GuardOn | Action::Guard | Action::GuardSetOff
        )
}

pub(crate) fn break_invulnerable(action: Action) -> bool {
    matches!(
        action,
        Action::ShieldBreakFly
            | Action::ShieldBreakFall
            | Action::ShieldBreakDown
            | Action::ShieldBreakStand
    )
}

fn breaking(action: Action) -> bool {
    break_invulnerable(action) || action == Action::Furafura
}

fn enter(f: &mut Fighter, action: Action) {
    super::simulation::enter(f, action);
}

fn start_break(f: &mut Fighter, a: &Attributes) {
    f.grounded = false;
    f.ground_line = None;
    f.ground_velocity = 0.0;
    f.velocity = [0.0, a.break_initial_velocity];
    f.fast_fall = false;
    f.ecb_lock = 10;
    f.ecb.bottom_locked = true;
    f.locomotion.jumps_used = 1;
    enter(f, Action::ShieldBreakFly);
}

/// Shield actions consume their IASA callback. Entry honors the existing
/// supported attack priority; missing roll/grab/powershield paths are explicit.
pub(crate) fn update(
    f: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    input: Controller,
) -> bool {
    let (Some(r), Some(a)) = (rules, data.shield.as_ref()) else {
        return false;
    };
    let own_action = active(f) || matches!(f.action, Action::GuardOff) || breaking(f.action);
    if own_action {
        match f.action {
            Action::ShieldBreakFly if f.action_frame >= a.break_fly_frames => {
                enter(f, Action::ShieldBreakFall)
            }
            Action::ShieldBreakDown if f.action_frame >= a.break_down_frames => {
                enter(f, Action::ShieldBreakStand)
            }
            Action::ShieldBreakStand if f.action_frame >= a.break_stand_frames => {
                enter(f, Action::Furafura);
                let mut percent = r.dizzy_percent_base - f.percent;
                if percent < 0.0 {
                    percent = 0.0;
                }
                f.shield.dizzy_timer = percent + r.dizzy_base_frames;
                f.shield.mash_directions = [0; 2];
                f.shield.health = r.break_health;
            }
            Action::Furafura => {
                f.shield.health = r.break_health;
                f.shield.dizzy_timer -= r.dizzy_frame_decay;
                math::mash(
                    &mut f.shield.dizzy_timer,
                    &mut f.shield.mash_directions,
                    input.stick,
                    input.buttons & !f.previous_input.buttons,
                    r.mash_stick_threshold,
                    r.dizzy_mash_rate,
                );
                if f.shield.dizzy_timer <= 0.0 {
                    enter(f, Action::Wait);
                }
            }
            Action::GuardOff if f.action_frame >= a.release_frames => {
                enter(f, Action::Wait);
                return false;
            }
            Action::GuardOn | Action::Guard => {
                f.shield.strength = math::strength(
                    input.shield_pressure(),
                    r.analog_deadzone,
                    f.shield.strength,
                );
                let (health, broken) = math::drain(
                    f.shield.health,
                    f.shield.strength,
                    r.drain_scales,
                    r.drain_rate,
                );
                f.shield.health = health;
                if broken {
                    start_break(f, a);
                    return true;
                }
                if f.shield.minimum_hold > 0.0 {
                    f.shield.minimum_hold = (f.shield.minimum_hold - 1.0).max(0.0);
                }
                f.shield.raise_progress += 1.0;
                if f.action == Action::GuardOn && f.shield.raise_progress >= a.raise_frames {
                    enter(f, Action::Guard);
                }
                f.shield.release_latched |= !input.shield_held();
                if f.shield.release_latched && f.shield.minimum_hold == 0.0 {
                    enter(f, Action::GuardOff);
                    return true;
                }
            }
            Action::GuardSetOff => {
                f.shield.stun_progress += f.shield.stun_rate;
                if f.shield.stun_progress >= a.stun_animation_end {
                    enter(
                        f,
                        if f.shield.release_latched {
                            Action::GuardOff
                        } else {
                            Action::Guard
                        },
                    );
                }
                return true;
            }
            _ => {}
        }
        if matches!(f.action, Action::GuardOn | Action::Guard | Action::GuardOff) {
            let pressed = input.buttons & !f.previous_input.buttons;
            let stick_jump = data.locomotion.as_ref().is_some_and(|p| {
                input.stick[1] >= p.tap_jump_threshold
                    && f.locomotion.tilt_y_age < p.tap_jump_window
            });
            if stick_jump || pressed & (super::BUTTON_X | super::BUTTON_Y) != 0 {
                f.short_hop = false;
                f.locomotion.jump_input = if stick_jump {
                    super::locomotion::JumpInput::Stick
                } else {
                    super::locomotion::JumpInput::Buttons
                };
                enter(f, Action::JumpSquat);
            }
        }
        return true;
    }
    if f.grounded
        && matches!(
            f.action,
            Action::Wait
                | Action::Walk
                | Action::Dash
                | Action::Run
                | Action::Turn
                | Action::Squat
                | Action::SquatWait
        )
        && input.shield_held()
        && f.shield.health != 0.0
        && input.buttons & !f.previous_input.buttons & super::BUTTON_A == 0
    {
        f.shield.strength = math::strength(input.shield_pressure(), r.analog_deadzone, 0.0);
        f.shield.minimum_hold = r.minimum_hold_frames;
        f.shield.release_latched = false;
        f.shield.raise_progress = 0.0;
        enter(f, Action::GuardOn);
        return true;
    }
    false
}

/// ProcessHit regenerates outside guard, and subtracts its base cost even with
/// zero shield-damage accumulation. Hit damage is applied by `apply_contact`.
pub(crate) fn finish_frame(f: &mut Fighter, rules: Option<&Rules>, shield_contact: bool) {
    let Some(r) = rules else { return };
    if shield_contact {
        return;
    }
    if active(f) {
        f.shield.health -= r.damage_base;
        if f.shield.health < 0.0 {
            f.shield.health = r.break_health;
        }
    } else if f.shield.health < r.maximum_health {
        f.shield.health += r.regeneration;
        if f.shield.health > r.maximum_health {
            f.shield.health = r.maximum_health;
        }
    }
}

pub(crate) fn hitlag(
    f: &mut Fighter,
    input: Controller,
    rules: &super::data::Rules,
    exiting: bool,
) {
    let (Some(r), Some(d)) = (&rules.shield, &rules.damage.displacement) else {
        return;
    };
    if f.action != Action::GuardSetOff || !f.grounded {
        return;
    }
    if (exiting || f.locomotion.tilt_x_age < d.sdi_window)
        && math::displacement(
            &mut f.position,
            f.floor_normal,
            input.stick[0],
            d.minimum_stick_magnitude,
            if exiting {
                d.asdi_distance
            } else {
                d.sdi_distance
            },
            r.sdi_horizontal_multiplier,
        )
        && !exiting
    {
        f.locomotion.tilt_x_age = 254;
    }
}

pub(crate) fn apply_contact(
    data: &MatchData,
    state: &mut State,
    attacker: usize,
    hit: &Hitbox,
    staled: super::staling::Hit,
) -> Result<(), Error> {
    let victim = 1 - attacker;
    let r = data
        .rules
        .shield
        .as_ref()
        .ok_or_else(|| Error::Physics("shield contact without common data".into()))?;
    let a = data.fighters[victim]
        .shield
        .as_ref()
        .ok_or_else(|| Error::Physics("shield contact without attributes".into()))?;
    let towards = if state.fighters[victim].position[0] > state.fighters[attacker].position[0] {
        1.0
    } else {
        -1.0
    };
    let amount = state.fighters[victim].shield.strength;
    let damage = math::environment_damage(staled.damage);
    let amount_damage = (damage + hit.shield_damage).max(0);
    let loss = math::damage_loss(
        amount_damage,
        amount,
        r.damage_scales,
        r.damage_scale,
        r.damage_base,
    );
    let f = &mut state.fighters[victim];
    f.shield.health -= loss;
    let broken = f.shield.health < 0.0;
    if damage == 0 {
        if broken {
            f.shield.health = r.break_health;
        }
        state.events.push(Event::ShieldHit {
            attacker,
            victim,
            damage: loss,
            broken: false,
        });
        return Ok(());
    }
    if broken {
        f.shield.health = r.break_health;
        start_break(f, a);
    } else {
        let stun = math::stun(damage, amount, r.stun_scales, r.stun_scale, r.stun_base);
        let (rate, velocity) = math::response(
            stun,
            a.stun_animation_end,
            r.push_scale,
            r.push_multiplier,
            r.push_maximum,
            -towards,
        );
        f.shield.stun_rate = rate;
        f.shield.stun_progress = 0.0;
        f.ground_velocity = velocity;
        f.locomotion.tilt_x_age = 254;
        enter(f, Action::GuardSetOff);
    }
    let hitlag = combat::hitlag(damage, false, 1.0, &data.rules.hitlag.physics())
        .map_err(|e| Error::Physics(e.to_string()))?
        .min(r.hitlag_maximum);
    f.hitlag = f.hitlag.max(hitlag);
    let source = &mut state.fighters[attacker];
    source.hitlag = source.hitlag.max(hitlag);
    if source.grounded {
        let damage = amount * damage as f32;
        if damage != 0.0 {
            let value = damage * r.attacker_push_scale + r.attacker_push_base;
            source.shield.attacker_ground_push = if towards < 0.0 { value } else { -value };
            source.shield.attacker_push = [
                source.floor_normal[1] * source.shield.attacker_ground_push,
                -source.floor_normal[0] * source.shield.attacker_ground_push,
            ];
        }
    }
    state.events.push(Event::ShieldHit {
        attacker,
        victim,
        damage: loss,
        broken,
    });
    Ok(())
}

/// Source attacker recoil remains separate from self velocity and knockback.
/// Includes the original airborne decay typo that clears knockback Y rather
/// than shield-recoil Y when the vector is smaller than the decay amount.
pub(crate) fn recoil(f: &mut Fighter, data: &FighterData, rules: Option<&Rules>) {
    let Some(r) = rules else { return };
    let [x, y] = f.shield.attacker_push;
    if x == 0.0 && y == 0.0 {
        return;
    }
    if f.grounded {
        if f.shield.attacker_ground_push == 0.0 {
            f.shield.attacker_ground_push = x;
        }
        f.shield.attacker_ground_push = crate::fighter::decrement_toward_zero(
            f.shield.attacker_ground_push,
            data.movement.ground_friction * r.attacker_ground_friction_multiplier,
        );
        f.shield.attacker_push = [
            f.floor_normal[1] * f.shield.attacker_ground_push,
            -f.floor_normal[0] * f.shield.attacker_ground_push,
        ];
    } else {
        let angle = libm::atan2f(y, x);
        if libm::sqrtf(x * x + y * y) < r.attacker_air_decay {
            f.shield.attacker_push[0] = 0.0;
            f.knockback[1] = 0.0;
        } else {
            f.shield.attacker_push[0] -= r.attacker_air_decay * libm::cosf(angle);
            f.shield.attacker_push[1] -= r.attacker_air_decay * libm::sinf(angle);
        }
        f.shield.attacker_ground_push = 0.0;
    }
}

/// Matrix supplied to the original shield narrowphase. The shield bone's scale
/// receives inlineB0's radius, while its center follows the supplied bone pose.
pub(crate) fn geometry(
    f: &Fighter,
    data: &FighterData,
    r: &Rules,
    pose: &bones::Pose,
) -> Result<([f32; 3], bones::Matrix), Error> {
    let a = data
        .shield
        .as_ref()
        .ok_or_else(|| Error::Physics("missing shield geometry".into()))?;
    let mut matrix = *pose
        .world_matrix(a.bone)
        .map_err(|e| Error::Physics(e.to_string()))?;
    let radius = math::radius(
        f.shield.health,
        r.maximum_health,
        f.shield.strength,
        r.size_scales,
        r.minimum_radius_scale,
        a.initial_radius,
    );
    for row in &mut matrix {
        for value in &mut row[..3] {
            *value *= radius;
        }
    }
    let center = bones::transform_point(&matrix, a.offset);
    if center
        .into_iter()
        .chain(matrix.into_iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(Error::NonFinite);
    }
    Ok((center, matrix))
}
