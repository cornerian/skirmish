//! Experimental scheduler. Exact helper arithmetic does not certify this order
//! against Melee's complete GObj/action pipeline. Unsupported rules are listed
//! in docs/match.md, and every input resource carries an experimental profile.
use crate::{data::*, *};
use melee_physics::{Movement, bones, combat, locomotion};

pub(crate) fn initial_state(data: &MatchData, seed: u32) -> State {
    State {
        next_frame: 0,
        remaining_frames: data.rules.time_limit_frames,
        phase: if data.rules.countdown_frames == 0 {
            Phase::Playing
        } else {
            Phase::Countdown {
                remaining: data.rules.countdown_frames,
            }
        },
        fighters: std::array::from_fn(|player| spawn(data, player, data.rules.stocks, 0)),
        rng_seed: seed,
        events: vec![],
    }
}

fn spawn(data: &MatchData, player: usize, stocks: u8, invincibility: u32) -> Fighter {
    let position = data.stage.spawns[player];
    let grounded = position[1] == data.stage.floor.y;
    Fighter {
        position,
        velocity: [0.0; 2],
        knockback: [0.0; 2],
        ground_velocity: 0.0,
        facing: if player == 0 { 1.0 } else { -1.0 },
        grounded,
        action: if grounded { Action::Wait } else { Action::Fall },
        action_frame: 0,
        percent: 0.0,
        stocks,
        hitlag: 0.0,
        hitstun: 0,
        invincibility,
        short_hop: false,
        fast_fall: false,
        hit_groups: 0,
        previous_input: Controller::default(),
    }
}

fn enter(fighter: &mut Fighter, action: Action) {
    fighter.action = action;
    fighter.action_frame = 0;
    // An attack's contact history lasts through its active frames and hitlag.
    fighter.hit_groups = 0;
}

pub(crate) fn advance(
    data: &MatchData,
    state: &mut State,
    inputs: [Controller; 2],
) -> Result<(), Error> {
    if let Phase::Countdown { remaining } = state.phase {
        state.phase = if remaining == 1 {
            state.events.push(Event::Started);
            Phase::Playing
        } else {
            Phase::Countdown {
                remaining: remaining - 1,
            }
        };
        for (fighter, input) in state.fighters.iter_mut().zip(inputs) {
            fighter.previous_input = input;
        }
        return Ok(());
    }

    // Resolve both players' movement before observing either player's contacts.
    // Frozen players cannot re-activate hitboxes when their last hitlag tick ends.
    let mut frozen = [false; 2];
    for player in 0..2 {
        let fighter = &mut state.fighters[player];
        let input = inputs[player];
        if fighter.action == Action::Eliminated {
            continue;
        }
        if fighter.action == Action::Respawn {
            if fighter.action_frame >= data.rules.respawn_frames {
                *fighter = spawn(
                    data,
                    player,
                    fighter.stocks,
                    data.rules.respawn_invincibility_frames,
                );
                state.events.push(Event::Respawned { player });
            } else {
                fighter.action_frame += 1;
            }
            fighter.previous_input = input;
            frozen[player] = true;
            continue;
        }
        if fighter.hitlag > 0.0 {
            fighter.hitlag = (fighter.hitlag - 1.0).max(0.0);
            fighter.previous_input = input;
            frozen[player] = true;
            continue;
        }
        update_action(fighter, &data.fighters[player], input);
        let previous_position = fighter.position;
        move_fighter(fighter, &data.fighters[player], &data.rules, input);
        collide_floor(
            fighter,
            previous_position,
            &data.stage.floor,
            player,
            &mut state.events,
        );
        fighter.previous_input = input;
    }

    // Contact decisions are collected from the same post-movement state. Apply
    // damage afterward so a lower port cannot suppress a simultaneous trade.
    let poses = [
        pose(&state.fighters[0], &data.fighters[0])?,
        pose(&state.fighters[1], &data.fighters[1])?,
    ];
    let mut hits = Vec::with_capacity(2);
    for attacker in 0..2 {
        let victim = 1 - attacker;
        let (source, target) = (&state.fighters[attacker], &state.fighters[victim]);
        if frozen[attacker]
            || source.action != Action::Jab
            || target.invincibility > 0
            || matches!(target.action, Action::Respawn | Action::Eliminated)
        {
            continue;
        }
        let frame = attack_frame(source, &data.fighters[attacker])?;
        for hit in &frame.hitboxes {
            if source.hit_groups & (1 << hit.group) != 0 {
                continue;
            }
            let sphere = bones::BoneCapsule::sphere(hit.bone, hit.center, hit.radius)
                .transform(&poses[attacker], 1.0)
                .map_err(physics)?;
            let mut collided = false;
            for hurtbox in &data.fighters[victim].hurtboxes {
                let hurt = hurtbox
                    .physics()
                    .transform(&poses[victim], 1.0)
                    .map_err(physics)?;
                let capsule = combat::Capsule {
                    start: hurt.start,
                    end: hurt.end,
                    radius: hurt.radius,
                };
                let mut closest = [0.0; 3];
                let overlaps =
                    combat::capsule_sphere(&capsule, sphere.start, sphere.radius, &mut closest);
                if closest.iter().any(|v| !v.is_finite()) {
                    return Err(Error::NonFinite);
                }
                if overlaps {
                    collided = true;
                    break;
                }
            }
            if collided {
                hits.push((attacker, hit));
                break;
            }
        }
    }
    // Preserve both action counters during a simultaneous trade before Damage
    // replaces their action; attacks that connected start hitlag on this step.
    for &(attacker, hit) in &hits {
        state.fighters[attacker].hit_groups |= 1 << hit.group;
    }
    let mut newly_hit = [false; 2];
    for (attacker, hit) in hits {
        newly_hit[1 - attacker] = true;
        apply_hit(data, state, attacker, hit)?;
    }

    for (player, fighter) in state.fighters.iter_mut().enumerate() {
        if !matches!(fighter.action, Action::Respawn | Action::Eliminated) {
            let [left, right, bottom, top] = data.stage.blast;
            let [x, y] = fighter.position;
            if x < left || x > right || y < bottom || y > top {
                fighter.stocks -= 1;
                fighter.velocity = [0.0; 2];
                fighter.knockback = [0.0; 2];
                fighter.hitlag = 0.0;
                fighter.hitstun = 0;
                enter(
                    fighter,
                    if fighter.stocks == 0 {
                        Action::Eliminated
                    } else {
                        Action::Respawn
                    },
                );
                state.events.push(Event::Knockout {
                    player,
                    stocks: fighter.stocks,
                });
                continue;
            }
            if !frozen[player] {
                // The last invincible frame still protects this frame's contacts.
                fighter.invincibility = fighter.invincibility.saturating_sub(1);
            }
            if !frozen[player] && !newly_hit[player] && fighter.hitlag == 0.0 {
                fighter.action_frame = fighter.action_frame.saturating_add(1);
                fighter.hitstun = fighter.hitstun.saturating_sub(1);
            }
        }
    }
    state.remaining_frames -= 1;
    if state.fighters.iter().any(|f| f.stocks == 0) {
        let winner = match (state.fighters[0].stocks > 0, state.fighters[1].stocks > 0) {
            (true, false) => Some(0),
            (false, true) => Some(1),
            _ => None,
        };
        finish(state, winner, FinishReason::Stocks);
    } else if state.remaining_frames == 0 {
        let [a, b] = &state.fighters;
        let order = a
            .stocks
            .cmp(&b.stocks)
            .then_with(|| b.percent.total_cmp(&a.percent));
        let winner = match order {
            std::cmp::Ordering::Greater => Some(0),
            std::cmp::Ordering::Less => Some(1),
            _ => None,
        };
        finish(state, winner, FinishReason::Time);
    }
    Ok(())
}

fn finish(state: &mut State, winner: Option<usize>, reason: FinishReason) {
    state.phase = Phase::Finished { winner, reason };
    state.events.push(Event::Finished { winner, reason });
}

fn update_action(f: &mut Fighter, data: &FighterData, input: Controller) {
    let attrs = &data.movement;
    match f.action {
        Action::Jab if f.action_frame as usize >= data.jab.frames.len() => enter(
            f,
            if f.grounded {
                Action::Wait
            } else {
                Action::Fall
            },
        ),
        Action::Landing if f.action_frame >= attrs.landing_frames => enter(f, Action::Wait),
        Action::Damage if f.hitstun == 0 => enter(
            f,
            if f.grounded {
                Action::Wait
            } else {
                Action::Fall
            },
        ),
        Action::JumpSquat if f.action_frame >= attrs.jump_startup_frames => {
            let velocity = locomotion::jump_velocity(
                [f.velocity[0], f.velocity[1], 0.0],
                input.stick[0],
                f.short_hop,
                1.0,
                &locomotion::JumpAttributes {
                    momentum_multiplier: attrs.jump_momentum_multiplier,
                    horizontal_initial_velocity: attrs.jump_horizontal_velocity,
                    horizontal_max_velocity: attrs.jump_horizontal_max,
                    full_hop_velocity: attrs.jump_vertical_velocity,
                    short_hop_velocity: attrs.short_hop_vertical_velocity,
                },
            );
            f.velocity = [velocity[0], velocity[1]];
            f.ground_velocity = 0.0;
            f.grounded = false;
            f.fast_fall = false;
            enter(f, Action::Jump);
        }
        _ => {}
    }
    let pressed = input.buttons & !f.previous_input.buttons;
    if matches!(f.action, Action::Wait | Action::Walk) {
        if pressed & BUTTON_A != 0 {
            enter(f, Action::Jab);
        } else if pressed & (BUTTON_X | BUTTON_Y) != 0 {
            f.short_hop = false;
            enter(f, Action::JumpSquat);
        } else if input.stick[0] != 0.0 {
            // Turn/dash state machines are unported. This profile walks directly.
            f.facing = input.stick[0].signum();
            if f.action != Action::Walk {
                enter(f, Action::Walk);
            }
        } else if f.action == Action::Walk {
            enter(f, Action::Wait);
        }
    }
    if f.action == Action::JumpSquat && input.buttons & (BUTTON_X | BUTTON_Y) == 0 {
        f.short_hop = true;
    }
}

fn move_fighter(f: &mut Fighter, data: &FighterData, rules: &Rules, input: Controller) {
    let attrs = &data.movement;
    let mut movement = Movement {
        attributes: attrs.physics(),
        self_velocity: [f.velocity[0], f.velocity[1], 0.0],
        ground_velocity: f.ground_velocity,
        floor_normal: [0.0, 1.0, 0.0],
        stick_x: input.stick[0],
        ..Movement::default()
    };
    if f.grounded {
        if f.action == Action::Walk {
            locomotion::walk(
                &mut movement,
                &locomotion::WalkParameters {
                    accel_mul: 1.0,
                    acceleration_mul: attrs.walk_acceleration_mul,
                    acceleration_base: attrs.walk_acceleration_base,
                    max_velocity: attrs.walk_max_velocity,
                    ground_friction: attrs.ground_friction,
                    taper_gain: rules.walk_accel_taper_gain,
                    ground_friction_multiplier: 1.0,
                    animation_speed_multiplier: 1.0,
                },
            );
        } else {
            let mut friction = attrs.ground_friction;
            if f.ground_velocity.abs() > attrs.walk_max_velocity {
                friction *= rules.friction_above_walk;
            }
            movement.friction_ground(friction);
            movement.project_ground();
        }
    } else if !(f.action == Action::Jump && f.action_frame == 0) {
        // ftCo_Jump_Phys_Inner skips gravity/drift on the launch callback.
        // The launch velocity is still integrated below on that frame.
        if f.action != Action::Damage {
            if !f.fast_fall
                && f.velocity[1] < 0.0
                && input.stick[1] <= -rules.fast_fall_threshold
                && f.previous_input.stick[1] > -rules.fast_fall_threshold
            {
                f.fast_fall = true;
            }
            if f.fast_fall {
                movement.fall_fast();
            } else {
                movement.fall_basic();
            }
            movement.drift_air();
            if f.action == Action::Jump && movement.self_velocity[1] < 0.0 {
                enter(f, Action::Fall);
            }
        } else {
            movement.fall_basic();
            movement.friction_air_basic();
        }
    }
    // Ordinary no-wind/no-shield branch of Fighter_procUpdate's integration:
    // apply acceleration, clear it, then add self velocity and knockback.
    f.ground_velocity = movement.ground_velocity + movement.ground_acceleration;
    for (axis, velocity) in f.velocity.iter_mut().enumerate() {
        *velocity = movement.self_velocity[axis] + movement.animation_velocity[axis];
    }
    if f.knockback != [0.0; 2] {
        let [x, y] = f.knockback;
        let angle = libm::atan2f(y, x);
        if libm::sqrtf(x * x + y * y) < rules.knockback_decay {
            f.knockback = [0.0; 2];
        } else {
            f.knockback[0] -= rules.knockback_decay * libm::cosf(angle);
            f.knockback[1] -= rules.knockback_decay * libm::sinf(angle);
        }
    }
    for axis in 0..2 {
        f.position[axis] += f.velocity[axis];
        f.position[axis] += f.knockback[axis];
    }
}

fn collide_floor(
    f: &mut Fighter,
    previous_position: [f32; 2],
    floor: &Floor,
    player: usize,
    events: &mut Vec<Event>,
) {
    let on_segment = (floor.left..=floor.right).contains(&f.position[0]);
    if f.grounded {
        if !on_segment {
            f.grounded = false;
            f.ground_velocity = 0.0;
            f.fast_fall = false;
            enter(f, Action::Fall);
        }
    } else {
        // Experimental point-foot sweep; native ECB geometry is still missing.
        let descending = f.velocity[1] + f.knockback[1] <= 0.0;
        if on_segment && descending && previous_position[1] >= floor.y && f.position[1] <= floor.y {
            f.position[1] = floor.y;
            f.velocity[1] = 0.0;
            f.knockback = [0.0; 2];
            f.ground_velocity = f.velocity[0];
            f.grounded = true;
            f.fast_fall = false;
            // Damage landing/tech/bounce states are not implemented in this slice.
            if f.action != Action::Damage {
                enter(f, Action::Landing);
            }
            events.push(Event::Landed { player });
        }
    }
}

fn pose(fighter: &Fighter, data: &FighterData) -> Result<bones::Pose, Error> {
    let local = if fighter.action == Action::Jab {
        &attack_frame(fighter, data)?.bones
    } else {
        &data.bones
    };
    let bones = local.iter().map(Bone::physics).collect::<Vec<_>>();
    // Native match coordinates: local +X faces forward, +Y up, +Z depth.
    let root = [
        [fighter.facing, 0.0, 0.0, fighter.position[0]],
        [0.0, 1.0, 0.0, fighter.position[1]],
        [0.0, 0.0, fighter.facing, 0.0],
    ];
    bones::Pose::evaluate_with_root(&bones, &root).map_err(physics)
}

fn attack_frame<'a>(fighter: &Fighter, data: &'a FighterData) -> Result<&'a AttackFrame, Error> {
    data.jab
        .frames
        .get(fighter.action_frame as usize)
        .ok_or_else(|| Error::Physics("attack pose frame is outside the supplied animation".into()))
}

fn apply_hit(
    data: &MatchData,
    state: &mut State,
    attacker: usize,
    hit: &Hitbox,
) -> Result<(), Error> {
    let victim = 1 - attacker;
    let target = &state.fighters[victim];
    let knockback = combat::knockback(
        &data.rules.knockback.physics(),
        combat::KnockbackHit {
            growth: hit.growth,
            fixed: hit.fixed,
            base: hit.base,
        },
        combat::DamageState {
            percent: target.percent,
            pending_damage: hit.damage as f32,
            count_override: None,
        },
        hit.damage,
        combat::KnockbackModifiers {
            stage: 1.0,
            attack: 1.0,
            defense: 1.0,
            weight: data.fighters[victim].weight,
        },
    )
    .map_err(physics)?;
    let hitlag = combat::hitlag(hit.damage as i32, false, 1.0, &data.rules.hitlag.physics())
        .map_err(physics)?;
    let hitstun = combat::initial_hitstun(knockback, data.rules.hitstun_scale).map_err(physics)?;
    if !knockback.is_finite() || !hitlag.is_finite() {
        return Err(Error::NonFinite);
    }
    if knockback < 0.0 || hitlag < 0.0 || hitstun < 1 {
        return Err(Error::Physics(
            "combat rules produced a negative magnitude or invalid timer".into(),
        ));
    }
    let facing = state.fighters[attacker].facing;
    state.fighters[attacker].hitlag = state.fighters[attacker].hitlag.max(hitlag);
    let target = &mut state.fighters[victim];
    target.percent = (target.percent + hit.damage as f32).min(999.0);
    target.hitlag = target.hitlag.max(hitlag);
    target.hitstun = hitstun as u32;
    target.velocity = [0.0; 2];
    target.ground_velocity = 0.0;
    let angle = hit.angle_degrees * (core::f32::consts::PI / 180.0);
    let speed = knockback * data.rules.knockback_speed;
    target.knockback = [
        speed * libm::cosf(angle) * facing,
        speed * libm::sinf(angle),
    ];
    target.grounded = false;
    target.fast_fall = false;
    enter(target, Action::Damage);
    state.events.push(Event::Hit {
        attacker,
        victim,
        damage: hit.damage as f32,
        knockback,
    });
    Ok(())
}

fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}
