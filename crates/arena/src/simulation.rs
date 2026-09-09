//! Experimental scheduler. Exact helper arithmetic does not certify this order
//! against Melee's complete GObj/action pipeline. Unsupported rules are listed
//! in docs/match.md, and every input resource carries an experimental profile.
use crate::{data::*, *};
use physics::{Movement, bones, combat, locomotion as movement_math, sweep};

pub(crate) fn initial_state(data: &MatchData, seed: u32) -> Result<State, Error> {
    Ok(State {
        next_frame: 0,
        remaining_frames: data.rules.time_limit_frames,
        phase: if data.rules.countdown_frames == 0 {
            Phase::Playing
        } else {
            Phase::Countdown {
                remaining: data.rules.countdown_frames,
            }
        },
        fighters: [
            spawn(data, 0, data.rules.stocks, 0)?,
            spawn(data, 1, data.rules.stocks, 0)?,
        ],
        rng_seed: seed,
        events: vec![],
    })
}

fn spawn(
    data: &MatchData,
    player: usize,
    stocks: u8,
    invincibility: u32,
) -> Result<Fighter, Error> {
    let position = data.stage.spawns[player];
    let mut fighter = Fighter {
        position,
        velocity: [0.0; 2],
        knockback: [0.0; 2],
        ground_velocity: 0.0,
        facing: if player == 0 { 1.0 } else { -1.0 },
        grounded: false,
        ground_line: None,
        skip_floor: None,
        floor_normal: [0.0, 1.0, 0.0],
        contacts: [None; 4],
        ecb: physics::ecb::State::default(),
        ecb_lock: 0,
        locomotion: locomotion::State::default(),
        action: Action::Fall,
        action_frame: 0,
        percent: 0.0,
        stocks,
        hitlag: 0.0,
        hitstun: 0,
        damage_elapsed: -1,
        damage_angle_flag: 0,
        damage_angle_timer: 0,
        di_pending: false,
        invincibility,
        short_hop: false,
        fast_fall: false,
        hit_groups: 0,
        hitboxes: [hitboxes::Track::default(); 4],
        previous_input: Controller::default(),
    };
    collision::initialize(&mut fighter, &data.fighters[player], &data.stage)?;
    if !fighter.grounded {
        fighter.locomotion.jumps_used = 1;
    }
    Ok(fighter)
}

pub(crate) fn enter(fighter: &mut Fighter, action: Action) {
    fighter.action = action;
    fighter.action_frame = 0;
    // Fighter_ChangeMotionState unconditionally calls mpClearFloorSkip.
    fighter.skip_floor = None;
    // An attack's contact history lasts through its active frames and hitlag.
    fighter.hit_groups = 0;
    fighter.hitboxes = [hitboxes::Track::default(); 4];
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
    let geometry = collision::geometry(&data.stage);
    let stage = physics::stage::Stage::new(&geometry.lines, &geometry.joints).map_err(physics)?;
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
                )?;
                state.events.push(Event::Respawned { player });
            } else {
                fighter.action_frame += 1;
            }
            fighter.previous_input = input;
            frozen[player] = true;
            continue;
        }
        sample_input_history(fighter, &data.fighters[player], &data.rules, input);
        if fighter.hitlag > 0.0 {
            let previous_position = fighter.position;
            fighter.hitlag = (fighter.hitlag - 1.0).max(0.0);
            if fighter.hitlag == 0.0 {
                damage::exit_hitlag(fighter, input.stick, &data.rules.damage)?;
            } else {
                damage::during_hitlag(fighter, input.stick, &data.rules.damage)?;
            }
            advance_ecb_lock(fighter);
            collision::sample(
                fighter,
                &data.fighters[player],
                &pose(fighter, &data.fighters[player])?,
            )?;
            if fighter.position != previous_position || fighter.ecb.current != fighter.ecb.desired {
                collision::resolve(
                    fighter,
                    previous_position,
                    &stage,
                    player,
                    &mut state.events,
                )?;
            }
            fighter.previous_input = input;
            frozen[player] = true;
            continue;
        }
        update_action(fighter, &data.fighters[player], input);
        if let Some(velocity_y) = locomotion::pass_request(
            fighter,
            &data.fighters[player],
            input,
            collision::on_platform(fighter, &data.stage),
        ) {
            collision::begin_pass(fighter, &data.fighters[player], &data.stage, velocity_y);
        }
        if fighter.damage_elapsed >= 0 {
            fighter.damage_elapsed = fighter.damage_elapsed.saturating_add(1);
        }
        let previous_position = fighter.position;
        move_fighter(fighter, &data.fighters[player], &data.rules, input);
        advance_ecb_lock(fighter);
        collision::sample(
            fighter,
            &data.fighters[player],
            &pose(fighter, &data.fighters[player])?,
        )?;
        collision::resolve(
            fighter,
            previous_position,
            &stage,
            player,
            &mut state.events,
        )?;
        fighter.previous_input = input;
    }

    // Contact decisions are collected from the same post-movement state. Apply
    // damage afterward so a lower port cannot suppress a simultaneous trade.
    let poses = [
        pose(&state.fighters[0], &data.fighters[0])?,
        pose(&state.fighters[1], &data.fighters[1])?,
    ];
    let mut swept = [[None; 4]; 2];
    for player in 0..2 {
        let fighter = &mut state.fighters[player];
        let frame = if fighter.action == Action::Jab {
            Some(attack_frame(fighter, &data.fighters[player])?)
        } else {
            None
        };
        swept[player] = hitboxes::update_tracks(&mut fighter.hitboxes, frame, &poses[player])?;
    }
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
        for (slot, hit) in frame.hitboxes.iter().enumerate() {
            if source.hit_groups & (1 << hit.group) != 0 {
                continue;
            }
            let attack = swept[attacker][slot]
                .as_ref()
                .ok_or_else(|| Error::Physics("active hitbox has no tracked sweep".into()))?;
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
                let mut closest = sweep::ClosestPair::default();
                let overlaps = sweep::capsule_capsule(attack, &capsule, &mut closest);
                if closest
                    .first
                    .into_iter()
                    .chain(closest.second)
                    .any(|v| !v.is_finite())
                {
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
        damage::apply_hit(data, state, attacker, hit)?;
    }

    for (player, fighter) in state.fighters.iter_mut().enumerate() {
        if !matches!(fighter.action, Action::Respawn | Action::Eliminated) {
            let [left, right, bottom, top] = data.stage.blast;
            let [x, y] = fighter.position;
            // Ordinary supported branch of ftCo_800D3158. Scripted death
            // overrides and star/screen animation selection remain unported.
            let top_eligible = data
                .rules
                .top_ko_min_knockback
                .is_none_or(|minimum| fighter.grounded || fighter.knockback[1] > minimum);
            if x < left || x > right || y < bottom || (y > top && top_eligible) {
                fighter.stocks -= 1;
                fighter.velocity = [0.0; 2];
                fighter.knockback = [0.0; 2];
                fighter.hitlag = 0.0;
                fighter.hitstun = 0;
                fighter.di_pending = false;
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

// Fighter_procMap releases the bottom lock before sampling, including during
// hitlag. One map callback is one tick, regardless of collision substep count.
fn advance_ecb_lock(fighter: &mut Fighter) {
    if fighter.ecb_lock > 0 {
        fighter.ecb_lock -= 1;
        if fighter.ecb_lock == 0 {
            fighter.ecb.bottom_locked = false;
        }
    }
}

// The original x670/x671 timers are shared by movement and damage callbacks.
// Input sampling continues during hitlag; SDI/jump transitions consume the
// same history, so old held inputs cannot become fresh after a state change.
fn sample_input_history(f: &mut Fighter, data: &FighterData, rules: &Rules, input: Controller) {
    let thresholds = data
        .locomotion
        .as_ref()
        .map(|p| [p.horizontal_smash_deadzone, p.vertical_smash_deadzone])
        .or_else(|| {
            rules
                .damage
                .displacement
                .as_ref()
                .map(|p| p.axis_thresholds)
        });
    if let Some([x, y]) = thresholds {
        f.locomotion.tilt_x_age = physics::damage::tilt_timer(
            f.locomotion.tilt_x_age,
            input.stick[0],
            f.previous_input.stick[0],
            x,
        );
        f.locomotion.tilt_y_age = physics::damage::tilt_timer(
            f.locomotion.tilt_y_age,
            input.stick[1],
            f.previous_input.stick[1],
            y,
        );
    }
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
        Action::JumpSquat
            if data.locomotion.is_none() && f.action_frame >= attrs.jump_startup_frames =>
        {
            let velocity = movement_math::jump_velocity(
                [f.velocity[0], f.velocity[1], 0.0],
                input.stick[0],
                f.short_hop,
                1.0,
                &movement_math::JumpAttributes {
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
            f.ground_line = None;
            f.fast_fall = false;
            enter(f, Action::Jump);
        }
        _ => {}
    }
    if data.locomotion.is_some() {
        locomotion::update_actions(f, data, input);
        return;
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
        floor_normal: f.floor_normal,
        stick_x: input.stick[0],
        ..Movement::default()
    };
    if f.grounded {
        if locomotion::ground_motion(f, data, &mut movement, input) {
            // Explicit locomotion parameters supply dash/run acceleration.
        } else if f.action == Action::Walk {
            movement_math::walk(
                &mut movement,
                &movement_math::WalkParameters {
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
    f.knockback = physics::damage::decay_air_knockback(f.knockback, rules.knockback_decay);
    for axis in 0..2 {
        f.position[axis] += f.velocity[axis];
        f.position[axis] += f.knockback[axis];
    }
}

pub(crate) fn pose(fighter: &Fighter, data: &FighterData) -> Result<bones::Pose, Error> {
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

fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}
