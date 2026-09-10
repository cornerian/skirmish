//! Experimental scheduler. Exact helper arithmetic does not certify this order
//! against Melee's complete GObj/action pipeline. Unsupported rules are listed
//! in docs/match.md, and every input resource carries an experimental profile.
use super::{data::*, *};
use crate::{
    collision::{bones, ecb, shield as body_collision, stage},
    fighter::{
        Movement, combat, damage as damage_math, locomotion as movement_math, nudge as push,
    },
};

pub(crate) fn initial_state(data: &MatchData, seed: u32) -> Result<State, Error> {
    let stage_state = stage_motion::State::default();
    let geometry = stage_motion::geometry(&data.stage, stage_state.frame);
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
        stage: stage_state,
        fighters: [
            spawn(data, &geometry, 0, data.rules.stocks, 0)?,
            spawn(data, &geometry, 1, data.rules.stocks, 0)?,
        ],
        rng_seed: seed,
        attack_instances: crate::fighter::stale::InstanceCounter::default(),
        events: vec![],
    })
}

fn spawn(
    data: &MatchData,
    geometry: &StageGeometry,
    player: usize,
    stocks: u8,
    invincibility: u32,
) -> Result<Fighter, Error> {
    let position = data.stage.spawns[player];
    let mut fighter = Fighter {
        position,
        depth: 0.0,
        deferred_position: [0.0; 3],
        nudge: [0.0; 2],
        velocity: [0.0; 2],
        knockback: [0.0; 2],
        ground_knockback: 0.0,
        ground_velocity: 0.0,
        facing: if player == 0 { 1.0 } else { -1.0 },
        grounded: false,
        ground_line: None,
        skip_floor: None,
        floor_normal: [0.0, 1.0, 0.0],
        contacts: [None; 4],
        ecb: ecb::State::default(),
        ecb_lock: 0,
        locomotion: locomotion::State::default(),
        shield: shield::ShieldState {
            health: data.rules.shield.as_ref().map_or(0.0, |r| r.maximum_health),
            ..Default::default()
        },
        aerial: aerial::State::default(),
        clank: clank::State::default(),
        grab: grab::State::default(),
        ledge: ledge::State::default(),
        death: death::State::default(),
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
        tumbling: false,
        prone: None,
        down_timer: 0,
        damage_motion: None,
        last_damage_surface: None,
        reflect_lockout: 0,
        surface_tech: damage::SurfaceTechState::default(),
        invincibility,
        short_hop: false,
        fast_fall: false,
        hit_groups: 0,
        hitboxes: [hitboxes::Track::default(); 4],
        staling: staling::State::default(),
        previous_input: Controller::default(),
    };
    collision::initialize(&mut fighter, &data.fighters[player], geometry)?;
    if !fighter.grounded {
        fighter.locomotion.jumps_used = 1;
    }
    Ok(fighter)
}

pub(crate) fn enter(fighter: &mut Fighter, action: Action) {
    clank::transition(fighter, action);
    fighter.aerial = aerial::State::default();
    staling::transition(fighter, action);
    fighter.action = action;
    fighter.action_frame = 0;
    // Fighter_ChangeMotionState unconditionally calls mpClearFloorSkip.
    fighter.skip_floor = None;
    // An attack's contact history lasts through its active frames and hitlag.
    fighter.hit_groups = 0;
    fighter.hitboxes = [hitboxes::Track::default(); 4];
    if !matches!(
        action,
        Action::Damage
            | Action::DamageFall
            | Action::DownDamage
            | Action::FlyReflectWall
            | Action::FlyReflectCeiling
    ) {
        fighter.tumbling = false;
        fighter.last_damage_surface = None;
        fighter.reflect_lockout = 0;
    }
    if !matches!(
        action,
        Action::DownBound
            | Action::DownWait
            | Action::DownDamage
            | Action::DownForward
            | Action::DownBack
            | Action::DownAttack
            | Action::DownStand
    ) {
        fighter.prone = None;
    }
    if !matches!(action, Action::DownWait | Action::DownDamage) {
        fighter.down_timer = 0;
    }
    if action != Action::Damage {
        fighter.damage_motion = None;
    }
    if !matches!(
        action,
        Action::PassiveWall | Action::PassiveWallJump | Action::PassiveCeiling
    ) {
        fighter.surface_tech = damage::SurfaceTechState::default();
    }
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

    let previous_stage_frame = state.stage.frame;
    stage_motion::advance(&data.stage, &mut state.stage)?;

    // The source runs priority-1 animation callbacks and push sampling in stable
    // entity order, then priority-3 action input and priority-4/6 physics/map.
    // Positions do not advance until every subject's nudge has been sampled.
    let mut frozen = [false; 2];
    let mut active = [false; 2];
    let previous_geometry = stage_motion::geometry(&data.stage, previous_stage_frame);
    let geometry = stage_motion::geometry(&data.stage, state.stage.frame);
    let stage_moved = geometry.lines != previous_geometry.lines;
    let stage = stage::Stage::new(&geometry.lines, &geometry.joints).map_err(physics)?;
    let nudge_neighbors = if data.rules.nudge.is_some() {
        Some(
            (0..geometry.lines.len())
                .map(|line| {
                    Ok(push::Neighbors {
                        previous: stage.neighbor(line, false).map_err(physics)?,
                        next: stage.neighbor(line, true).map_err(physics)?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?,
        )
    } else {
        None
    };
    for player in 0..2 {
        let fighter = &mut state.fighters[player];
        let input = inputs[player];
        fighter.nudge = [0.0; 2];
        if fighter.action == Action::Eliminated {
            continue;
        }
        if fighter.action == Action::Respawn {
            if fighter.action_frame >= data.rules.respawn_frames {
                *fighter = spawn(
                    data,
                    &geometry,
                    player,
                    fighter.stocks,
                    data.rules.respawn_invincibility_frames,
                )?;
                if let Some(rules) = &data.rules.rebirth {
                    rebirth::enter(
                        fighter,
                        rules,
                        player,
                        data.rules.respawn_invincibility_frames,
                    );
                }
                state.events.push(Event::Respawned { player });
            } else {
                fighter.action_frame += 1;
            }
            fighter.previous_input = input;
            frozen[player] = true;
            continue;
        }
        if death::owns_action(fighter.action) {
            let rules = data
                .rules
                .death
                .as_ref()
                .ok_or_else(|| Error::Data("death action requires explicit rules".into()))?;
            let update = death::update(fighter, rules);
            if update == death::Update::Complete {
                fighter.death = death::State::default();
                enter(fighter, Action::Respawn);
            } else {
                death::move_fighter(fighter, rules);
            }
            fighter.previous_input = input;
            frozen[player] = true;
            if update == death::Update::LoseStock {
                lose_stock(data, state, player, true)?;
            }
            continue;
        }
        sample_input_history(fighter, &data.fighters[player], &data.rules, input);
        if fighter.hitlag > 0.0 {
            let previous_position = fighter.position;
            fighter.hitlag = (fighter.hitlag - 1.0).max(0.0);
            shield::hitlag(fighter, input, &data.rules, fighter.hitlag == 0.0);
            if fighter.hitlag == 0.0 {
                damage::exit_hitlag(fighter, input, &data.rules.damage)?;
            } else {
                damage::during_hitlag(fighter, input.stick, &data.rules.damage)?;
            }
            stage_motion::carry(&data.stage, &state.stage, fighter)?;
            advance_ecb_lock(fighter);
            collision::sample(
                fighter,
                &data.fighters[player],
                &pose(fighter, &data.fighters[player])?,
            )?;
            if stage_moved
                || fighter.position != previous_position
                || fighter.ecb.current != fighter.ecb.desired
            {
                collision::resolve(
                    fighter,
                    previous_position,
                    (&stage, &geometry, &previous_geometry),
                    player,
                    &mut state.events,
                    (&data.fighters[player], &data.rules, input),
                )?;
            }
            staling::flush(
                fighter,
                &data.fighters[player],
                data.rules.staling.as_ref(),
                &mut state.attack_instances,
            )?;
            fighter.previous_input = input;
            frozen[player] = true;
            continue;
        }
        active[player] = true;
    }

    let mut just_turned = [false; 2];
    let mut clank_owns = [false; 2];
    let mut shield_owns = [false; 2];
    for player in 0..2 {
        if !active[player] {
            continue;
        }
        (just_turned[player], clank_owns[player], shield_owns[player]) = update_animation(
            &mut state.fighters[player],
            &data.fighters[player],
            &data.rules,
            &geometry,
            player,
            inputs[player],
        )?;
        update_nudge(
            data,
            state,
            nudge_neighbors.as_deref().unwrap_or_default(),
            player,
        )?;
    }

    let pair_frozen = grab::update_pairs(data, state, inputs, active)?;
    for player in 0..2 {
        if pair_frozen[player] {
            active[player] = false;
            frozen[player] = true;
        }
    }

    for player in 0..2 {
        if !active[player] {
            continue;
        }
        let fighter = &mut state.fighters[player];
        let input = inputs[player];
        update_actions(
            fighter,
            &data.fighters[player],
            &data.rules,
            input,
            just_turned[player],
            clank_owns[player],
            shield_owns[player],
        );
        if let Some(velocity_y) = locomotion::pass_request(
            fighter,
            &data.fighters[player],
            input,
            collision::on_platform(fighter, &geometry),
        ) {
            collision::begin_pass(fighter, &data.fighters[player], &geometry, velocity_y);
        }
    }
    grab::synchronize_actions(data, state)?;

    for player in 0..2 {
        if !active[player] {
            continue;
        }
        let fighter = &mut state.fighters[player];
        let input = inputs[player];
        if fighter.grab.captor.is_some() {
            fighter.nudge = [0.0; 2];
            staling::flush(
                fighter,
                &data.fighters[player],
                data.rules.staling.as_ref(),
                &mut state.attack_instances,
            )?;
            fighter.previous_input = input;
            continue;
        }
        if rebirth::owns_action(fighter.action) {
            let rules = data
                .rules
                .rebirth
                .as_ref()
                .ok_or_else(|| Error::Data("rebirth state requires explicit rules".into()))?;
            rebirth::move_fighter(fighter, rules, player);
            collision::sample(
                fighter,
                &data.fighters[player],
                &pose(fighter, &data.fighters[player])?,
            )?;
            staling::flush(
                fighter,
                &data.fighters[player],
                data.rules.staling.as_ref(),
                &mut state.attack_instances,
            )?;
            fighter.previous_input = input;
            continue;
        }
        if ledge::attached(fighter) {
            ledge::attach(fighter, &data.fighters[player], &geometry)?;
            collision::sample(
                fighter,
                &data.fighters[player],
                &pose(fighter, &data.fighters[player])?,
            )?;
            staling::flush(
                fighter,
                &data.fighters[player],
                data.rules.staling.as_ref(),
                &mut state.attack_instances,
            )?;
            fighter.previous_input = input;
            continue;
        }
        if fighter.damage_elapsed >= 0 {
            fighter.damage_elapsed = fighter.damage_elapsed.saturating_add(1);
        }
        let previous_position = fighter.position;
        move_fighter(fighter, &data.fighters[player], &data.rules, input);
        stage_motion::carry(&data.stage, &state.stage, fighter)?;
        advance_ecb_lock(fighter);
        collision::sample(
            fighter,
            &data.fighters[player],
            &pose(fighter, &data.fighters[player])?,
        )?;
        collision::resolve(
            fighter,
            previous_position,
            (&stage, &geometry, &previous_geometry),
            player,
            &mut state.events,
            (&data.fighters[player], &data.rules, input),
        )?;
        staling::flush(
            fighter,
            &data.fighters[player],
            data.rules.staling.as_ref(),
            &mut state.attack_instances,
        )?;
        fighter.previous_input = input;
    }
    let capture_previous = state.fighters.each_ref().map(|fighter| fighter.position);
    grab::release_broken_pairs(state);
    grab::attach_all(data, state)?;
    let captured_before_scan = state
        .fighters
        .each_ref()
        .map(|fighter| fighter.grab.captor.is_some());
    resolve_captured_collisions(
        data,
        state,
        inputs,
        &stage,
        &geometry,
        &previous_geometry,
        capture_previous,
        core::array::from_fn(|player| captured_before_scan[player] && !frozen[player]),
    )?;

    // Contact decisions are collected from the same post-movement state. Apply
    // damage afterward so a lower port cannot suppress a simultaneous trade.
    let poses = [
        pose(&state.fighters[0], &data.fighters[0])?,
        pose(&state.fighters[1], &data.fighters[1])?,
    ];
    ledge::scan(data, state, &stage, &geometry, inputs)?;
    grab::scan(data, state, frozen)?;
    resolve_captured_collisions(
        data,
        state,
        inputs,
        &stage,
        &geometry,
        &previous_geometry,
        capture_previous,
        core::array::from_fn(|player| {
            !captured_before_scan[player] && state.fighters[player].grab.captor.is_some()
        }),
    )?;
    for player in 0..2 {
        staling::flush(
            &mut state.fighters[player],
            &data.fighters[player],
            data.rules.staling.as_ref(),
            &mut state.attack_instances,
        )?;
    }
    let mut swept = [[None; 4]; 2];
    for player in 0..2 {
        let fighter = &mut state.fighters[player];
        let frame = if data.fighters[player]
            .attack(fighter.action, fighter.prone)
            .is_some()
        {
            Some(attack_frame(fighter, &data.fighters[player])?)
        } else {
            None
        };
        swept[player] = hitboxes::update_tracks(&mut fighter.hitboxes, frame, &poses[player])?;
        staling::sample(&mut fighter.staling, frame, data.rules.staling.as_ref())?;
        if data.rules.clank.is_some() {
            clank::sample(&mut fighter.clank, frame);
        }
    }
    clank::scan(data, state, &swept)?;
    #[derive(Clone, Copy)]
    enum HitContact {
        Shield,
        Fighter {
            height: damage_math::HurtHeight,
            geometry: damage::FighterContact,
        },
    }
    let mut hits = Vec::with_capacity(2);
    for attacker in 0..2 {
        let victim = 1 - attacker;
        let (source, target) = (&state.fighters[attacker], &state.fighters[victim]);
        if frozen[attacker]
            || data.fighters[attacker]
                .attack(source.action, source.prone)
                .is_none()
            || target.invincibility > 0
            || target.grab.captor.is_some()
            || shield::break_invulnerable(target.action)
            || matches!(target.action, Action::Respawn | Action::Eliminated)
            || rebirth::invulnerable(target.action)
            || death::owns_action(target.action)
        {
            continue;
        }
        let frame = attack_frame(source, &data.fighters[attacker])?;
        for (slot, hit) in frame.hitboxes.iter().enumerate() {
            if if data.rules.clank.is_some() {
                clank::blocked(source, slot, victim)
            } else {
                source.hit_groups & (1 << hit.group) != 0
            } {
                continue;
            }
            let attack = swept[attacker][slot]
                .as_ref()
                .ok_or_else(|| Error::Physics("active hitbox has no tracked sweep".into()))?;
            let staled = source.staling.hits[slot]
                .ok_or_else(|| Error::Physics("active hitbox has no damage sample".into()))?;
            if shield::active(target)
                && let Some(rules) = &data.rules.shield
            {
                let (center, matrix) =
                    shield::geometry(target, &data.fighters[victim], rules, &poses[victim])?;
                if crate::collision::shield::shield_contact(attack, center, &matrix, 1.0, 20.0)
                    .map_err(physics)?
                    .is_some()
                {
                    hits.push((attacker, hit, staled, HitContact::Shield));
                    break;
                }
            }
            let mut body_contact = None;
            for (index, hurtbox) in data.fighters[victim].hurtboxes.iter().enumerate() {
                if !hurtbox_state(target, &data.fighters[victim], index)?.accepts_contact() {
                    continue;
                }
                let hurt = hurtbox
                    .physics()
                    .transform(&poses[victim], 1.0)
                    .map_err(physics)?;
                let capsule = combat::Capsule {
                    start: hurt.start,
                    end: hurt.end,
                    radius: hurt.radius,
                };
                let matrix = poses[victim]
                    .world_matrix(hurtbox.bone)
                    .map_err(|error| Error::Physics(error.to_string()))?;
                let mut contact = body_collision::Contact::default();
                let overlaps =
                    body_collision::capsule_matrix(attack, &capsule, matrix, 3.0, &mut contact)
                        .map_err(physics)?;
                if overlaps {
                    let height = data.fighters[victim]
                        .damage_poses
                        .as_ref()
                        .map_or_default(|poses| poses.hurtbox_heights[index]);
                    body_contact = Some(HitContact::Fighter {
                        height,
                        geometry: damage::FighterContact {
                            hurt_start: hurt.start,
                            hurt_end: hurt.end,
                            position: contact.position,
                        },
                    });
                    break;
                }
            }
            if let Some(contact) = body_contact {
                hits.push((attacker, hit, staled, contact));
                break;
            }
        }
    }
    // Preserve both action counters during a simultaneous trade before Damage
    // replaces their action; attacks that connected start hitlag on this step.
    for &(attacker, hit, _, _) in &hits {
        state.fighters[attacker].hit_groups |= 1 << hit.group;
        if data.rules.clank.is_some() {
            clank::record(&mut state.fighters[attacker], hit.group, 1 - attacker)?;
        }
    }
    let mut newly_hit = [false; 2];
    let mut shield_contact = [false; 2];
    for (attacker, hit, staled, contact) in hits {
        newly_hit[1 - attacker] = true;
        match contact {
            HitContact::Shield => {
                shield_contact[1 - attacker] = true;
                shield::apply_contact(data, state, attacker, hit, staled)?;
            }
            HitContact::Fighter { height, geometry } => damage::apply_hit(
                data,
                state,
                attacker,
                hit,
                staled,
                height,
                damage::HitDirection::FighterContact(geometry),
            )?,
        }
        if matches!(contact, HitContact::Fighter { .. }) && data.rules.staling.is_some() {
            state.fighters[attacker]
                .staling
                .queue
                .record(staled.identity, false);
        }
        staling::flush(
            &mut state.fighters[1 - attacker],
            &data.fighters[1 - attacker],
            data.rules.staling.as_ref(),
            &mut state.attack_instances,
        )?;
    }

    clank::finish(data, state, newly_hit)?;

    let mut blast_deaths = [None; 2];
    let mut blast_knockouts = [false; 2];
    if let Some(rules) = &data.rules.death {
        let mut rng = crate::random::HsdRng::new(state.rng_seed);
        for player in 0..2 {
            let fighter = &state.fighters[player];
            blast_deaths[player] = crate::fighter::death::select(
                crate::fighter::death::Query {
                    excluded: [
                        death::owns_action(fighter.action),
                        fighter.action == Action::Respawn,
                        fighter.action == Action::Eliminated,
                        fighter.action == Action::Rebirth,
                        fighter.action == Action::RebirthWait,
                    ],
                    position: fighter.position,
                    blast: data.stage.blast,
                    grounded: fighter.grounded,
                    forced_top_eligible: false,
                    knockback_y: fighter.knockback[1],
                    top_knockback_threshold: data
                        .rules
                        .top_ko_min_knockback
                        .expect("validated death rules require a top threshold"),
                    force_normal_top: rules.force_normal_top[player],
                    camera_disables_screen: rules.camera_disables_screen,
                    screen_chance_percent: rules.screen_chance_percent,
                    ice: false,
                },
                &mut rng,
            );
            blast_knockouts[player] = blast_deaths[player].is_some();
        }
        state.rng_seed = rng.seed();
    } else {
        blast_knockouts = core::array::from_fn(|player| {
            let fighter = &state.fighters[player];
            let [left, right, bottom, top] = data.stage.blast;
            let [x, y] = fighter.position;
            let top_eligible = data
                .rules
                .top_ko_min_knockback
                .is_none_or(|minimum| fighter.grounded || fighter.knockback[1] > minimum);
            !matches!(
                fighter.action,
                Action::Respawn | Action::Eliminated | Action::Rebirth | Action::RebirthWait
            ) && (x < left || x > right || y < bottom || y > top && top_eligible)
        });
    }
    for (player, &knocked_out) in blast_knockouts.iter().enumerate() {
        if knocked_out {
            grab::break_for_player(state, player);
        }
    }
    for player in 0..2 {
        staling::flush(
            &mut state.fighters[player],
            &data.fighters[player],
            data.rules.staling.as_ref(),
            &mut state.attack_instances,
        )?;
    }

    for player in 0..2 {
        if !matches!(
            state.fighters[player].action,
            Action::Respawn | Action::Eliminated
        ) {
            shield::finish_frame(
                &mut state.fighters[player],
                data.rules.shield.as_ref(),
                shield_contact[player],
            );
            if let Some(kind) = blast_deaths[player] {
                death::begin(
                    &mut state.fighters[player],
                    kind,
                    data.rules.death.as_ref().unwrap(),
                );
                state.events.push(Event::DeathStarted {
                    player,
                    death: kind,
                });
                if !matches!(
                    kind,
                    crate::fighter::death::Kind::UpStar
                        | crate::fighter::death::Kind::UpStarIce
                        | crate::fighter::death::Kind::UpScreen
                        | crate::fighter::death::Kind::UpScreenIce
                ) {
                    lose_stock(data, state, player, true)?;
                } else {
                    staling::flush(
                        &mut state.fighters[player],
                        &data.fighters[player],
                        data.rules.staling.as_ref(),
                        &mut state.attack_instances,
                    )?;
                }
                continue;
            }
            if blast_knockouts[player] {
                lose_stock(data, state, player, false)?;
                continue;
            }
            let throw_release = grab::paired_throw_release(data, state, player);
            let fighter = &mut state.fighters[player];
            if !frozen[player] {
                // The last invincible frame still protects this frame's contacts.
                fighter.invincibility = fighter.invincibility.saturating_sub(1);
            }
            if !frozen[player] && !newly_hit[player] && fighter.hitlag == 0.0 {
                if !grab::advance_action_frame(fighter, throw_release)
                    && !locomotion::hold_action_frame(fighter)
                {
                    fighter.action_frame = fighter.action_frame.saturating_add(1);
                }
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

#[allow(clippy::too_many_arguments)]
fn resolve_captured_collisions(
    data: &MatchData,
    state: &mut State,
    inputs: [Controller; 2],
    stage: &stage::Stage<'_>,
    geometry: &StageGeometry,
    previous_geometry: &StageGeometry,
    previous_positions: [[f32; 2]; 2],
    players: [bool; 2],
) -> Result<(), Error> {
    for player in 0..2 {
        if !players[player] || state.fighters[player].grab.captor.is_none() {
            continue;
        }
        let fighter = &mut state.fighters[player];
        collision::sample(
            fighter,
            &data.fighters[player],
            &pose(fighter, &data.fighters[player])?,
        )?;
        collision::resolve(
            fighter,
            previous_positions[player],
            (stage, geometry, previous_geometry),
            player,
            &mut state.events,
            (&data.fighters[player], &data.rules, inputs[player]),
        )?;
    }
    Ok(())
}

fn lose_stock(
    data: &MatchData,
    state: &mut State,
    player: usize,
    preserve_death: bool,
) -> Result<(), Error> {
    let fighter = &mut state.fighters[player];
    // ftCo_800D34E0 resets only the deceased player's queue.
    fighter.staling.queue.reset();
    fighter.stocks -= 1;
    fighter.velocity = [0.0; 2];
    fighter.knockback = [0.0; 2];
    fighter.ground_knockback = 0.0;
    fighter.hitlag = 0.0;
    fighter.hitstun = 0;
    fighter.di_pending = false;
    fighter.ledge = ledge::State::default();
    if fighter.stocks == 0 {
        enter(fighter, Action::Eliminated);
    } else if !preserve_death {
        enter(fighter, Action::Respawn);
    }
    state.events.push(Event::Knockout {
        player,
        stocks: fighter.stocks,
    });
    staling::flush(
        fighter,
        &data.fighters[player],
        data.rules.staling.as_ref(),
        &mut state.attack_instances,
    )?;
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

fn update_nudge(
    data: &MatchData,
    state: &mut State,
    neighbors: &[push::Neighbors],
    subject: usize,
) -> Result<(), Error> {
    let Some(rules) = &data.rules.nudge else {
        return Ok(());
    };
    if matches!(
        state.fighters[subject].action,
        Action::Respawn | Action::Eliminated | Action::Rebirth | Action::RebirthWait
    ) || death::owns_action(state.fighters[subject].action)
    {
        return Ok(());
    }
    let attributes = [
        data.fighters[0]
            .nudge
            .ok_or_else(|| Error::Data("nudge rules require fighter attributes".into()))?,
        data.fighters[1]
            .nudge
            .ok_or_else(|| Error::Data("nudge rules require fighter attributes".into()))?,
    ];
    let bodies: [push::Body; 2] = core::array::from_fn(|player| {
        let fighter = &state.fighters[player];
        let attributes = attributes[player];
        push::Body {
            position: [fighter.position[0], fighter.position[1], fighter.depth],
            deferred_position: fighter.deferred_position,
            facing: fighter.facing,
            center_offset: attributes.center_offset,
            half_width: attributes.half_width,
            player_id: player as u8,
            floor: fighter.ground_line,
            follower_of: None,
            inactive: matches!(
                fighter.action,
                Action::Respawn | Action::Eliminated | Action::Rebirth | Action::RebirthWait
            ) || death::owns_action(fighter.action)
                || fighter.grab.captor.is_some()
                || ledge::attached(fighter),
            holds_victim: fighter.grab.victim.is_some(),
            nudge_disabled: attributes.nudge_disabled,
            hitlag: fighter.hitlag > 0.0,
            overlap_disabled: attributes.overlap_disabled,
        }
    });
    state.fighters[subject].nudge =
        push::velocity(subject, &bodies, neighbors, rules).map_err(physics)?;
    Ok(())
}

// The original x670/x671 timers are shared by movement and damage callbacks.
// Input sampling continues during hitlag; SDI/jump transitions consume the
// same history, so old held inputs cannot become fresh after a state change.
fn sample_input_history(f: &mut Fighter, data: &FighterData, rules: &Rules, input: Controller) {
    let pressed = input.buttons & !f.previous_input.buttons;
    f.locomotion.jump_press_age = if pressed & (BUTTON_X | BUTTON_Y) != 0 {
        0
    } else {
        f.locomotion.jump_press_age.saturating_add(1)
    };
    f.locomotion.attack_a_age = if pressed & BUTTON_A != 0 {
        0
    } else {
        f.locomotion.attack_a_age.saturating_add(1)
    };
    f.locomotion.attack_b_age = if pressed & BUTTON_B != 0 {
        0
    } else {
        f.locomotion.attack_b_age.saturating_add(1)
    };
    if pressed & (BUTTON_L | BUTTON_R) != 0 {
        f.locomotion.previous_tech_press_age = f.locomotion.tech_press_age;
        f.locomotion.tech_press_age = 0;
    } else {
        f.locomotion.tech_press_age = f.locomotion.tech_press_age.saturating_add(1);
    }
    f.locomotion.trigger_age = if input.shield_held() && !f.previous_input.shield_held() {
        0
    } else {
        f.locomotion.trigger_age.saturating_add(1)
    };
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
        f.locomotion.tilt_x_age = damage_math::tilt_timer(
            f.locomotion.tilt_x_age,
            input.stick[0],
            f.previous_input.stick[0],
            x,
        );
        f.locomotion.tilt_y_age = damage_math::tilt_timer(
            f.locomotion.tilt_y_age,
            input.stick[1],
            f.previous_input.stick[1],
            y,
        );
    }
}

fn update_animation(
    f: &mut Fighter,
    data: &FighterData,
    rules: &Rules,
    geometry: &StageGeometry,
    player: usize,
    input: Controller,
) -> Result<(bool, bool, bool), Error> {
    let attrs = &data.movement;
    rebirth::update_animation(
        f,
        rules.rebirth.as_ref(),
        player,
        rules.respawn_invincibility_frames,
    );
    special::update_animation(f, data.special.as_ref());
    ledge::update_animation(f, data, geometry, rules.ledge.as_ref())?;
    grab::update_fighter_animation(f, data);
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
    damage::update_animation(f, data, &rules.damage, input);
    // Anim transitions install the destination state's input callback before
    // dispatch. This includes fresh aerial input on the ground-jump launch.
    let just_turned = locomotion::update_animation(f, data, input);
    aerial::update_animation(f, data);
    let clank_owns = clank::update_animation(f, data);
    let shield_owns = if clank_owns {
        false
    } else {
        shield::update_animation(f, data, rules.shield.as_ref(), input)
    };
    Ok((just_turned, clank_owns, shield_owns))
}

fn update_actions(
    f: &mut Fighter,
    data: &FighterData,
    rules: &Rules,
    input: Controller,
    just_turned: bool,
    clank_owns: bool,
    shield_owns: bool,
) {
    if rebirth::update_actions(
        f,
        rules.rebirth.as_ref(),
        input,
        rules.respawn_invincibility_frames,
    ) {
        return;
    }
    if damage::update_actions(f, &rules.damage, input) {
        return;
    }
    if ledge::update_actions(f, data, rules.ledge.as_ref(), input) {
        return;
    }
    if clank_owns {
        return;
    }
    if special::update_actions(f, data.special.as_ref(), input) {
        return;
    }
    if grab::update_actions(f, data, rules.grab.as_ref(), input) {
        return;
    }
    if shield::update_actions(f, data, rules.shield.as_ref(), input, shield_owns) {
        return;
    }
    if aerial::update(f, data, input) {
        return;
    }
    if data.locomotion.is_some() {
        locomotion::update_actions(f, data, input, just_turned);
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
        ground_knockback: f.ground_knockback,
        floor_normal: f.floor_normal,
        stick_x: input.stick[0],
        ..Movement::default()
    };
    if f.grounded {
        if f.action == Action::Rebound
            && !crate::fighter::clank::apply_rebound_friction(&mut f.clank.impulse)
        {
            // Rebound's first physics callback retains projected self velocity.
        } else if let Some(target) = damage::ground_recovery_velocity(f, data) {
            // ft_80085030 converts the animation's local TransN delta into the
            // exact target ground velocity before projecting it onto the floor.
            movement.ground_acceleration = target - movement.ground_velocity;
            movement.project_ground();
        } else if locomotion::ground_motion(f, data, &mut movement, input) {
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
            // Rebound_Phys uses ApplyGroundMovement, which scales the already
            // clamped acceleration on slippery surfaces before projection.
            if f.action == Action::Rebound
                && let Some(clank) = &rules.clank
                && clank.surface_friction_multiplier < 1.0
            {
                movement.ground_acceleration *= clank.surface_friction_multiplier;
            }
            movement.project_ground();
        }
    } else if !(f.action == Action::Jump && f.action_frame == 0)
        && !ledge::skip_jump_physics(f, data)
    {
        // ftCo_Jump_Phys_Inner skips gravity/drift on the launch callback.
        // The launch velocity is still integrated below on that frame.
        if matches!(f.action, Action::PassiveWall | Action::PassiveWallJump)
            && f.surface_tech.timer != 0
        {
            // Wall techs remain fixed until the source timer releases them.
        } else if !matches!(
            f.action,
            Action::Damage
                | Action::DownDamage
                | Action::FlyReflectWall
                | Action::FlyReflectCeiling
                | Action::PassiveWall
                | Action::PassiveWallJump
                | Action::PassiveCeiling
        ) && !shield::break_invulnerable(f.action)
        {
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
    f.ground_velocity += f.clank.pending_ground_acceleration;
    f.clank.pending_ground_acceleration = 0.0;
    for (axis, velocity) in f.velocity.iter_mut().enumerate() {
        *velocity = movement.self_velocity[axis] + movement.animation_velocity[axis];
    }
    if f.grounded
        && let Some(profile) = &rules.damage.ground_launch
    {
        movement.decay_ground_knockback(
            attrs.ground_friction * profile.ground_knockback_friction_multiplier,
        );
        f.ground_knockback = movement.ground_knockback;
        f.knockback = [
            f.floor_normal[1] * f.ground_knockback,
            -f.floor_normal[0] * f.ground_knockback,
        ];
    } else {
        f.ground_knockback = 0.0;
        f.knockback = damage_math::decay_air_knockback(f.knockback, rules.knockback_decay);
    }
    shield::recoil(f, data, rules.shield.as_ref());
    // Fighter_procUpdate adds the priority-1 push result before self velocity,
    // knockback and shield recoil. Depth is gameplay state used by bone contact.
    f.position[0] += f.nudge[0];
    f.depth += f.nudge[1];
    for axis in 0..2 {
        f.position[axis] += f.velocity[axis];
        f.position[axis] += f.knockback[axis];
        f.position[axis] += f.shield.attacker_push[axis];
    }
}

pub(crate) fn pose(fighter: &Fighter, data: &FighterData) -> Result<bones::Pose, Error> {
    let local = if let Some(pose) = grab::pose(fighter, data) {
        pose
    } else if let Some(pose) = ledge::pose(fighter, data) {
        pose
    } else if let Some(pose) = damage::ground_recovery_pose(fighter, data) {
        pose
    } else if let Some(pose) = damage::damage_pose(fighter, data) {
        pose
    } else if matches!(fighter.action, Action::ReboundStop | Action::Rebound) {
        clank::pose(fighter, data).ok_or_else(|| Error::Data("missing rebound pose".into()))?
    } else if data.attack(fighter.action, fighter.prone).is_some() {
        &attack_frame(fighter, data)?.bones
    } else if aerial::landing_index(fighter.action).is_some() {
        aerial::landing_pose(fighter, data)
            .ok_or_else(|| Error::Data("landing pose is outside supplied samples".into()))?
    } else {
        &data.bones
    };
    let bones = local.iter().map(Bone::physics).collect::<Vec<_>>();
    // Native match coordinates: local +X faces forward, +Y up, +Z depth.
    let root = [
        [
            fighter.facing,
            0.0,
            0.0,
            fighter.position[0] + fighter.death.camera_offset[0],
        ],
        [
            0.0,
            1.0,
            0.0,
            fighter.position[1] + fighter.death.camera_offset[1],
        ],
        [
            0.0,
            0.0,
            fighter.facing,
            fighter.depth + fighter.death.camera_offset[2],
        ],
    ];
    bones::Pose::evaluate_with_root(&bones, &root).map_err(physics)
}

fn attack_frame<'a>(fighter: &Fighter, data: &'a FighterData) -> Result<&'a AttackFrame, Error> {
    data.attack(fighter.action, fighter.prone)
        .ok_or_else(|| Error::Data("missing attack resources".into()))?
        .frames
        .get(fighter.action_frame as usize)
        .ok_or_else(|| Error::Physics("attack pose frame is outside the supplied animation".into()))
}

pub(crate) fn hurtbox_state(
    fighter: &Fighter,
    data: &FighterData,
    index: usize,
) -> Result<HurtboxState, Error> {
    let base = data
        .hurtboxes
        .get(index)
        .ok_or_else(|| Error::Physics("hurtbox index is outside supplied resources".into()))?
        .state;
    let Some(_) = data.attack(fighter.action, fighter.prone) else {
        return Ok(base);
    };
    let frame = attack_frame(fighter, data)?;
    if frame.hurtbox_states.is_empty() {
        Ok(base)
    } else {
        frame
            .hurtbox_states
            .get(index)
            .copied()
            .ok_or_else(|| Error::Physics("incomplete attack hurtbox state sample".into()))
    }
}

fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}
