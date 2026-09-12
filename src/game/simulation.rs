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

pub(crate) fn initial_state(data: &MatchData, seed: u32, slots: [u32; 2]) -> Result<State, Error> {
    let stage_state = stage_motion::State::default();
    let geometry = stage_motion::geometry(&data.stage, stage_state.frame);
    let mut action_instances = crate::fighter::instance::Counter::default();
    let mut attack_instances = crate::fighter::stale::InstanceCounter::default();
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
            spawn(
                data,
                &geometry,
                0,
                data.rules.stocks,
                0,
                slots[0],
                true,
                &mut action_instances,
                &mut attack_instances,
            )?,
            spawn(
                data,
                &geometry,
                1,
                data.rules.stocks,
                0,
                slots[1],
                true,
                &mut action_instances,
                &mut attack_instances,
            )?,
        ],
        projectiles: vec![],
        rng_seed: seed,
        attack_instances,
        action_instances,
        events: vec![],
    })
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    data: &MatchData,
    geometry: &StageGeometry,
    player: usize,
    stocks: u8,
    invincibility: u32,
    slot: u32,
    // The match-start warp-in (`rules.entry`) only ever applies to the
    // initial match spawn, never a mid-match stock respawn (Melee's own
    // respawn uses the unrelated, already-implemented Rebirth platform,
    // not `ftCo_MS_Entry`).
    is_match_start: bool,
    action_instances: &mut crate::fighter::instance::Counter,
    attack_instances: &mut crate::fighter::stale::InstanceCounter,
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
        // gmvs.c:1780-1830 (`Player_GetFacingDirection`'s own assignment
        // rule), applied only when the match-start warp-in is modeled: a
        // number of pre-existing fixtures park the "other" fighter far off
        // to one side purely as a non-interacting dummy (its position was
        // never meant to influence fighter 0's facing), which the general
        // rule -- correctly -- would read as a real opponent position. The
        // plain `player == 0` hardcode is kept as the default so every such
        // fixture is unaffected; `docs/match-start.md` records this scoping.
        facing: if data.rules.entry.is_some() {
            crate::fighter::entry::spawn_facing(data.stage.spawns, player)
        } else if player == 0 {
            1.0
        } else {
            -1.0
        },
        grounded: false,
        ground_line: None,
        last_ground_line: None,
        skip_floor: None,
        floor_normal: [0.0, 1.0, 0.0],
        contacts: [None; 4],
        edge_contact: None,
        ecb: ecb::State::default(),
        ecb_lock: 0,
        locomotion: locomotion::State::default(),
        shield: shield::ShieldState {
            health: data.rules.shield.as_ref().map_or(0.0, |r| r.maximum_health),
            ..Default::default()
        },
        aerial: aerial::State::default(),
        fox_side_special: characters::fox::side::State::default(),
        fox_up_special: characters::fox::up::State::default(),
        down_special: characters::fox::down::State::default(),
        fox_neutral_special: characters::fox::neutral::State::default(),
        tilt: tilt::State::default(),
        smash: smash::State::default(),
        dash: dash::State::default(),
        jab: jab::State::default(),
        idle: idle::State::default(),
        clank: clank::State::default(),
        grab: grab::State::default(),
        ledge: ledge::State::default(),
        death: death::State::default(),
        entry: entry::State::default(),
        action: Action::Fall,
        action_frame: 0,
        percent: 0.0,
        stocks,
        hitlag: 0.0,
        hitstun: 0,
        action_instance: crate::fighter::action_instance::State::default(),
        combo: crate::fighter::combo::State::default(),
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
        wall_jump: wall_jump::State::default(),
        invincibility,
        intangibility: 0,
        body_state: BodyState::default(),
        l_cancel_status: 0,
        landing_allow_interrupt: false,
        short_hop: false,
        fast_fall: false,
        hit_groups: 0,
        hitboxes: [hitboxes::Track::default(); 4],
        staling: staling::State::default(),
        previous_input: Controller::default(),
    };
    collision::initialize(&mut fighter, &data.fighters[player], geometry)?;
    fighter.last_ground_line = fighter.ground_line;
    if !fighter.grounded {
        fighter.locomotion.jumps_used = 1;
    }
    // `ftCo_800C61B0`: overrides `collision::initialize`'s own Wait/Fall
    // spawn action with Entry when the match-start warp-in is modeled.
    // `docs/match-start.md`: `rules.entry.is_none()` keeps today's Fall/Wait
    // start unchanged. `entry::enter` calls `simulation::enter`, which
    // already queues its own motion identity and staling transition (unlike
    // `collision::initialize`'s raw field assignment); `staling::flush`
    // drains both, matching every other `simulation::enter` call site.
    if is_match_start && data.rules.entry.is_some() {
        entry::enter(&mut fighter, slot);
        staling::flush(
            &mut fighter,
            &data.fighters[player],
            data.rules.staling.as_ref(),
            attack_instances,
            action_instances,
        )?;
    } else {
        let identity =
            crate::fighter::action_instance::motion_identity(fighter.action, None, false);
        crate::fighter::action_instance::queue(&mut fighter.action_instance, identity);
        crate::fighter::action_instance::flush(&mut fighter.action_instance, action_instances);
    }
    Ok(fighter)
}

pub(crate) fn enter(fighter: &mut Fighter, action: Action) {
    let identity =
        crate::fighter::action_instance::motion_identity(action, fighter.prone, fighter.ledge.slow);
    let leaving_down_tilt = fighter.action == Action::AttackLw3;
    if leaving_down_tilt {
        // The down tilt's deferred x21EC callback (ft_800892A0 then
        // ft_80089824) allocates twice with identity 0 before the ordinary
        // motion-change accounting.
        crate::fighter::action_instance::queue(&mut fighter.action_instance, 0);
        crate::fighter::action_instance::queue(&mut fighter.action_instance, 0);
    }
    if !matches!(action, Action::AttackLw3 | Action::Attack100Loop) {
        // ftCo_AttackLw3 and Attack100Loop enter with Ft_MF_SkipAttackCount.
        crate::fighter::action_instance::queue(&mut fighter.action_instance, identity);
    }
    clank::transition(fighter, action);
    fighter.aerial = aerial::State::default();
    // The side special's gravity delay is freshly assigned by every phase's
    // own entry; a mid-phase ground<->air conversion preserves it explicitly
    // around this reset (`specials::transfer_ground_air`).
    fighter.fox_side_special = characters::fox::side::State::default();
    // Same convention as the side special's own reset above: the up
    // special's gravity delay, rotate/launch angle and Travel counters are
    // all freshly assigned by their own phase's entry, with mid-phase
    // ground<->air conversions preserving them explicitly around this reset
    // (`specials::transfer_ground_air`, the up special's own `land`).
    fighter.fox_up_special = characters::fox::up::State::default();
    // Fighter_ChangeMotionState unconditionally clears `fp->mv.fx.SpecialLw`;
    // every internal Reflector transition (`characters::fox::down`) restores
    // the whole-move fields (release_lag/is_release/gravity_delay) it
    // preserves across phase changes explicitly around this reset, the same
    // pattern as `fox_side_special` above.
    fighter.down_special = characters::fox::down::State::default();
    // Blaster keeps no whole-move state across a `simulation::enter` at
    // all: `repeat_armed` is freshly re-evaluated every Loop cycle, and a
    // mid-move ground<->air conversion never happens for this move (see
    // `characters::fox::neutral`'s own module doc), so there is nothing to
    // preserve around this reset, unlike the other three specials above.
    fighter.fox_neutral_special = characters::fox::neutral::State::default();
    if !ledge::owns_action(action) {
        fighter.ledge.slow = false;
    }
    staling::transition(fighter, action);
    if leaving_down_tilt {
        // ft_800890D0 runs before the x21EC restart of the stale instance.
        staling::restart_identity(fighter);
    }
    fighter.action = action;
    fighter.action_frame = 0;
    // Fighter_ChangeMotionState unconditionally calls mpClearFloorSkip.
    fighter.skip_floor = None;
    // Ordinary transitions (Ft_MF_None) reset the scripted collision state.
    fighter.body_state = BodyState::default();
    // `fighter.c:1066`: `Fighter_ChangeMotionState` unconditionally clears
    // `Fighter::x221F_b1` (the Slippi `state_flags.dead` bit,
    // `crates/skirmish-replay/src/observation.rs`'s `state_flags`) on every
    // motion change. `death::begin` (blast deaths) and `entry::enter`
    // (`ft_0C31.c:46`'s `ftCo_800C61B0`) each call this function first and
    // then explicitly set `fighter.death.hidden = true` afterward, matching
    // the source's own "ChangeMotionState, then set the flag back" order.
    fighter.death.hidden = false;
    // ftCo_800DEEA8: every motion change clears the smash charge.
    fighter.smash = smash::State::default();
    // mv.co.attackdash.x0 is cleared by doEnter; the AttackDash entry callers
    // (Dash/Run) arm it to x68 immediately after this.
    fighter.dash = dash::State::default();
    // Fighter_ChangeMotionState sets fp->anim_id from the destination motion
    // state's own table entry; every Wait entry's own entry is assumed 2
    // (Wait1_0, see game::idle::State). idle::State is read only while
    // Action::Wait is current, so resetting it unconditionally here (like
    // dash/smash above) is harmless for every other destination.
    fighter.idle = idle::State::default();
    // mv.co.landing.allow_interrupt is set only by the ordinary Landing entry
    // (`ftCo_Landing_Enter_Basic`), immediately after this reset.
    fighter.landing_allow_interrupt = false;
    // jump_backward and fall_aerial distinguish JumpF/JumpB, JumpAerialF/B
    // and the aerial-jump variant of Fall for Slippi's reported motion id;
    // the source keeps no such field on Fighter, since it stores the chosen
    // state id directly. Set immediately after this reset by the
    // ground/aerial jump launch and by JumpAerial's own animation-end Fall
    // entry, respectively.
    fighter.locomotion.jump_backward = false;
    fighter.locomotion.fall_aerial = false;
    // Fighter_ChangeMotionState keeps the jab timer only for Wait and walks.
    if !jab::keeps_window(action) {
        fighter.jab.window = 0.0;
    }
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
    // Ordinary wall-jump entry reclaims this shared motion immediately after
    // the transition; damage-tech entry must never inherit that ownership.
    fighter.wall_jump.active = false;
    fighter.wall_jump.startup_timer = 0;
    fighter.wall_jump.vertical_exponent = 0;
}

pub(crate) fn advance(
    data: &MatchData,
    state: &mut State,
    inputs: [Controller; 2],
) -> Result<(), Error> {
    // `docs/input-lock.md`: with `rules.entry` present, the match simulates
    // every frame fully from the first frame (no `Phase::Countdown` freeze);
    // Phase/`Event::Started` still land on `rules.countdown_frames`, exactly
    // as before this batch. `rules.entry.is_none()` keeps the legacy frozen
    // Countdown byte-for-byte (every pre-existing fixture that asserts
    // frozen positions through Countdown is unaffected).
    let simulate_through_countdown = data.rules.entry.is_some();
    // Captured before the phase transition below mutates `state.phase`, so
    // the match clock (`remaining_frames`) stays frozen through every
    // Countdown frame -- including this one, when `simulate_through_countdown`
    // runs the full pipeline instead of returning early -- and only starts
    // decrementing once a frame begins already in `Phase::Playing`, exactly
    // matching `docs/input-lock.md`'s "clock starts after countdown_frames".
    let was_countdown = matches!(state.phase, Phase::Countdown { .. });
    if let Phase::Countdown { remaining } = state.phase {
        state.phase = if remaining == 1 {
            state.events.push(Event::Started);
            Phase::Playing
        } else {
            Phase::Countdown {
                remaining: remaining - 1,
            }
        };
        if !simulate_through_countdown {
            for (player, (fighter, input)) in state.fighters.iter_mut().zip(inputs).enumerate() {
                // `docs/match-start.md`: the match-start warp-in still
                // progresses during Phase::Countdown (Melee's own pre-"GO"
                // period genuinely shows Entry/EntryStart/EntryEnd on
                // screen), scoped to only the entry-owned fighters so a
                // `rules.entry.is_none()` match is byte-for-byte unaffected
                // (still fully frozen, as before this batch). No landing
                // check here: stage geometry/collision are not set up this
                // early, and every fixture that exercises this spawns well
                // above any floor.
                if entry::owns_action(fighter.action) {
                    entry::update_animation(
                        fighter,
                        &data.fighters[player],
                        data.rules.entry.as_ref(),
                    )?;
                }
                fighter.previous_input = input;
            }
            return Ok(());
        }
    }

    // `docs/input-lock.md`: the pre-"GO" input lock. Confirmed against the
    // replay, decomp citation pending (see the doc): a held stick produces
    // no drift even in ordinary Fall during this window, so the gate is not
    // scoped to Entry/EntryStart/EntryEnd's own (empty) IASA callbacks --
    // every fighter's *dispatched* controller is replaced by neutral for the
    // first `input_lock_frames` frames, upstream of every other use of
    // `inputs` this frame (dispatch, hitlag sampling). `raw_inputs` keeps the
    // real, un-neutralized samples so `previous_input` bookkeeping (below)
    // can still track them: `fox-fd-4.slp` (a second real recording,
    // `docs/parity.md`) held its stick down continuously from before the
    // lock through the unlock frame, and matching `previous_input` to the
    // neutralized `inputs` made that continuously-held input look like a
    // fresh press exactly at unlock, wrongly edge-triggering fast-fall
    // (`fast_fall_threshold`'s `f.previous_input.stick[1] >
    // -rules.fast_fall_threshold` check below) on a frame the recording
    // shows falling under ordinary gravity. This resolves `docs/
    // input-lock.md`'s "open question" the other way from its original,
    // explicitly-flagged guess (neutral `previous_input` throughout the
    // lock): the real pad copy does not stop tracking during the lock, only
    // the fighters' own dispatch does.
    let raw_inputs = inputs;
    let inputs = match &data.rules.entry {
        Some(entry) if state.next_frame <= entry.input_lock_frames => [Controller::default(); 2],
        _ => inputs,
    };

    // Slippi's recorder clears these transient fields before their producer
    // callbacks. Contacts and landings later in the frame replace them.
    for fighter in &mut state.fighters {
        fighter.l_cancel_status = 0;
        fighter.shield.touched = false;
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
                    0,
                    false,
                    &mut state.action_instances,
                    &mut state.attack_instances,
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
            fighter.previous_input = raw_inputs[player];
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
            fighter.previous_input = raw_inputs[player];
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
            combat_history::push(fighter, &data.rules.damage.combo);
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
                specials::update_ground_contact(fighter);
            }
            staling::flush(
                fighter,
                &data.fighters[player],
                data.rules.staling.as_ref(),
                &mut state.attack_instances,
                &mut state.action_instances,
            )?;
            fighter.previous_input = raw_inputs[player];
            frozen[player] = true;
            continue;
        }
        active[player] = true;
    }
    combat_history::update(&mut state.fighters, active);

    let mut just_turned = [false; 2];
    let mut clank_owns = [false; 2];
    let mut shield_owns = [false; 2];
    // ftCo_Wait_Anim's HSD_Randi draw runs within the same animation-phase
    // callback order Melee dispatches in (player order); the seed is handed
    // off to the blast-zone death draw further below exactly as today.
    let mut idle_rng = crate::random::HsdRng::new(state.rng_seed);
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
            &mut idle_rng,
        )?;
        update_nudge(
            data,
            state,
            nudge_neighbors.as_deref().unwrap_or_default(),
            player,
        )?;
    }
    state.rng_seed = idle_rng.seed();

    let pair_frozen = grab::update_pairs(data, state, inputs, active)?;
    for player in 0..2 {
        if pair_frozen[player] {
            if active[player] {
                combat_history::push(&mut state.fighters[player], &data.rules.damage.combo);
            }
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
        )?;
        if let Some(velocity_y) = locomotion::pass_request_after_actions(
            fighter,
            &data.fighters[player],
            input,
            collision::on_platform(fighter, &geometry),
            shield_owns[player],
        ) {
            collision::begin_pass(fighter, &data.fighters[player], &geometry, velocity_y);
        } else {
            characters::fox::down::platform_drop(fighter, &data.fighters[player], &geometry, input);
        }
    }
    grab::synchronize_actions(data, state)?;

    // Snapshot before this frame's own movement/collision resolution below,
    // for `ledge::scan`'s `cd->prev_pos` (`mp/mpcoll.c`'s ledge-catch query
    // reads the position from before this frame's physics -- exactly what
    // each active fighter's own `previous_position` local further down is
    // computed from; fighters that skip movement this frame -- rebirth,
    // already ledge-attached, captured -- don't move in this loop either, so
    // one snapshot here matches every fighter's own value).
    let frame_start_positions = state.fighters.each_ref().map(|fighter| fighter.position);

    for player in 0..2 {
        if !active[player] {
            continue;
        }
        let fighter = &mut state.fighters[player];
        let input = inputs[player];
        if fighter.grab.captor.is_some() {
            fighter.nudge = [0.0; 2];
            combat_history::push(fighter, &data.rules.damage.combo);
            staling::flush(
                fighter,
                &data.fighters[player],
                data.rules.staling.as_ref(),
                &mut state.attack_instances,
                &mut state.action_instances,
            )?;
            fighter.previous_input = raw_inputs[player];
            continue;
        }
        if rebirth::owns_action(fighter.action) {
            let rules = data
                .rules
                .rebirth
                .as_ref()
                .ok_or_else(|| Error::Data("rebirth state requires explicit rules".into()))?;
            rebirth::move_fighter(fighter, rules, player);
            combat_history::push(fighter, &data.rules.damage.combo);
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
                &mut state.action_instances,
            )?;
            fighter.previous_input = raw_inputs[player];
            continue;
        }
        if ledge::attached(fighter) {
            ledge::attach(fighter, &data.fighters[player], &geometry)?;
            combat_history::push(fighter, &data.rules.damage.combo);
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
                &mut state.action_instances,
            )?;
            fighter.previous_input = raw_inputs[player];
            continue;
        }
        if fighter.damage_elapsed >= 0 {
            fighter.damage_elapsed = fighter.damage_elapsed.saturating_add(1);
        }
        let previous_position = fighter.position;
        move_fighter(fighter, &data.fighters[player], &data.rules, input);
        combat_history::push(fighter, &data.rules.damage.combo);
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
        // After this frame's own collision resolution, so a move's own
        // per-frame contact state (Fox's up special's `rotateModel`) reads
        // this frame's fresh `floor_normal`/`grounded`, not last frame's
        // (unlike `tick_ground_timers`, ticked from `move_fighter` above,
        // before this call).
        specials::update_ground_contact(fighter);
        staling::flush(
            fighter,
            &data.fighters[player],
            data.rules.staling.as_ref(),
            &mut state.attack_instances,
            &mut state.action_instances,
        )?;
        fighter.previous_input = raw_inputs[player];
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
    // Item logic runs after fighters, at its own GObj priority: any pending
    // shot from this frame's own fighter dispatch (`characters::fox::
    // neutral`) spawns now, then every active projectile (a freshly
    // spawned one included, matching the source's own same-frame item
    // Anim/Phys/Coll run) advances once.
    for player in 0..2 {
        if let Some(projectile) = characters::fox::neutral::drain_pending_shot(
            &mut state.fighters[player],
            &data.fighters[player],
            player,
            &mut state.attack_instances,
        ) {
            let projectile_kind = projectile.kind;
            state.projectiles.push(projectile);
            state.events.push(Event::ProjectileSpawned {
                owner: player,
                projectile_kind,
            });
        }
    }
    projectile::advance(data, state, &poses)?;
    ledge::scan(
        data,
        state,
        &stage,
        &geometry,
        inputs,
        frame_start_positions,
    )?;
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
            &mut state.action_instances,
        )?;
    }
    let mut swept = [[None; 4]; 2];
    for player in 0..2 {
        let fighter = &mut state.fighters[player];
        let frame = if data.fighters[player]
            .attack(fighter.action, fighter.prone, fighter.ledge.slow)
            .is_some()
        {
            Some(attack_frame(fighter, &data.fighters[player])?)
        } else {
            None
        };
        swept[player] = hitboxes::update_tracks(&mut fighter.hitboxes, frame, &poses[player])?;
        let charge = fighter.smash;
        staling::sample(
            &mut fighter.staling,
            frame,
            data.rules.staling.as_ref(),
            |damage| {
                crate::fighter::smash::charge_damage(
                    damage,
                    charge.charge,
                    charge.frames,
                    charge.hold_frames,
                    charge.damage_multiplier,
                )
            },
        )?;
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
    // At most one hit per attacker (the inner loop `break`s on the first
    // connecting hitbox), so a fixed two-slot array indexed by attacker
    // replaces the unconditional per-frame `Vec` allocation with none at all.
    let mut hits = [None, None];
    let mut shield_touches = [false; 2];
    for attacker in 0..2 {
        let victim = 1 - attacker;
        let (source, target) = (&state.fighters[attacker], &state.fighters[victim]);
        if frozen[attacker]
            || data.fighters[attacker]
                .attack(source.action, source.prone, source.ledge.slow)
                .is_none()
            || target.invincibility > 0
            || target.intangibility > 0
            || !target.body_state.accepts_contact()
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
                    if hit.element == HitElement::Inert {
                        shield_touches[victim] = true;
                        continue;
                    }
                    hits[attacker] = Some((hit, staled, HitContact::Shield));
                    break;
                }
            }
            if hit.element == HitElement::Inert {
                continue;
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
                hits[attacker] = Some((hit, staled, contact));
                break;
            }
        }
    }
    for (fighter, touched) in state.fighters.iter_mut().zip(shield_touches) {
        fighter.shield.touched = touched;
    }
    // Preserve both action counters during a simultaneous trade before Damage
    // replaces their action; attacks that connected start hitlag on this step.
    for (attacker, hit) in hits
        .iter()
        .enumerate()
        .filter_map(|(attacker, hit)| Some((attacker, hit.as_ref()?.0)))
    {
        state.fighters[attacker].hit_groups |= 1 << hit.group;
        if data.rules.clank.is_some() {
            clank::record(&mut state.fighters[attacker], hit.group, 1 - attacker)?;
        }
    }
    let mut newly_hit = [false; 2];
    let mut shield_contact = [false; 2];
    for (attacker, hit, staled, contact) in
        hits.into_iter().enumerate().filter_map(|(attacker, hit)| {
            hit.map(|(hit, staled, contact)| (attacker, hit, staled, contact))
        })
    {
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
            &mut state.action_instances,
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
            &mut state.action_instances,
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
                        &mut state.action_instances,
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
                fighter.intangibility = fighter.intangibility.saturating_sub(1);
            }
            if !frozen[player] && !newly_hit[player] && fighter.hitlag == 0.0 {
                if !grab::advance_action_frame(fighter, throw_release)
                    && !locomotion::hold_action_frame(fighter)
                    && !smash::charging(fighter)
                {
                    fighter.action_frame = fighter.action_frame.saturating_add(1);
                }
                if fighter.hitstun == 1 {
                    crate::fighter::combo::finish_hitstun(
                        &mut fighter.combo,
                        &data.rules.damage.combo,
                    );
                }
                fighter.hitstun = fighter.hitstun.saturating_sub(1);
            }
        }
    }
    for fighter in &mut state.fighters {
        if fighter.ground_line.is_some() {
            fighter.last_ground_line = fighter.ground_line;
        }
    }
    if !was_countdown {
        state.remaining_frames -= 1;
    }
    if state.fighters.iter().any(|f| f.stocks == 0) {
        let winner = match (state.fighters[0].stocks > 0, state.fighters[1].stocks > 0) {
            (true, false) => Some(0),
            (false, true) => Some(1),
            _ => None,
        };
        finish(state, winner, FinishReason::Stocks);
    } else if !was_countdown && state.remaining_frames == 0 {
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
    fighter.action_instance = crate::fighter::action_instance::State::default();
    fighter.combo = crate::fighter::combo::State::default();
    fighter.di_pending = false;
    fighter.ledge = ledge::State::default();
    if preserve_death {
        fighter.death.stock_lost = true;
    }
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
        &mut state.action_instances,
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
            // ftCo_80099314 and ftCo_800998EC set x221D_b5 for the escape.
            overlap_disabled: attributes.overlap_disabled || escape::owns_action(fighter.action),
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
    // fighter.c:1735-1739 (`x688`): distinct from x67D/attack_b_age above,
    // since ftCo_SpecialS_HasInput also requires the stick past the side
    // threshold. A missing `rules.specials` keeps the gate permanently open
    // (never 0), matching "None keeps B+side inert".
    let side_threshold = rules
        .specials
        .as_ref()
        .map_or(f32::INFINITY, |specials| specials.side_stick_threshold);
    f.locomotion.side_special_b_age =
        if pressed & BUTTON_B != 0 && input.stick[0].abs() >= side_threshold {
            0
        } else {
            f.locomotion.side_special_b_age.saturating_add(1)
        };
    // fighter.c:1723-1727 (`x686`, `ftCo_800D6928`): the up-special age,
    // shared with the down-special one (x687) in gating on x21C alone rather
    // than a per-move threshold. A missing `rules.specials` keeps the gate
    // permanently open, matching the side age's own convention above.
    let vertical_threshold = rules
        .specials
        .as_ref()
        .map_or(f32::INFINITY, |specials| specials.vertical_threshold);
    f.locomotion.up_special_b_age =
        if pressed & BUTTON_B != 0 && input.stick[1] >= vertical_threshold {
            0
        } else {
            f.locomotion.up_special_b_age.saturating_add(1)
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
        })
        .or_else(|| rules.wall_jump.as_ref().map(|p| [p.tilt_deadzone; 2]));
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
    idle_rng: &mut crate::random::HsdRng,
) -> Result<(bool, bool, bool), Error> {
    let attrs = &data.movement;
    rebirth::update_animation(
        f,
        rules.rebirth.as_ref(),
        player,
        rules.respawn_invincibility_frames,
    );
    entry::update_animation(f, data, rules.entry.as_ref())?;
    specials::update_animation(f, data, input, collision::on_platform(f, geometry));
    ledge::update_animation(f, data, geometry, rules.ledge.as_ref())?;
    wall_jump::update_animation(f, data, rules.wall_jump.as_ref());
    grab::update_fighter_animation(f, data);
    jab::update_animation(f, data)?;
    // Read f.action before any of this call's own transitions (the Landing/
    // JumpSquat match arms below, and every later module's own Action::Wait
    // entries) so a same-frame entry into Wait is not also animated this
    // frame -- Melee's own Anim callback for the destination motion state
    // is not re-invoked within the same frame's callback that produced the
    // transition, the same ordering `game::locomotion::advance_run_animation`
    // relies on for Dash/RunTurn-to-Run.
    idle::update_animation(f, data, idle_rng);
    match f.action {
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
    edge::update_animation(f, data)?;
    escape::update_animation(f, data)?;
    escape_air::update_animation(f, data, rules.escape_air.as_ref())?;
    tilt::update_animation(f, data)?;
    smash::update_animation(f, data)?;
    dash::update_animation(f, data)?;
    taunt::update_animation(f, data)?;
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
) -> Result<(), Error> {
    // ftCo_Entry_IASA/ftCo_EntryStart_IASA/ftCo_EntryEnd_IASA are all empty:
    // no input callback of any kind runs during the match-start warp-in.
    if entry::owns_action(f.action) {
        return Ok(());
    }
    // ftCo_800DF0D0 precedes every input callback.
    smash::update_charge_input(f, input);
    if rebirth::update_actions(
        f,
        rules.rebirth.as_ref(),
        input,
        rules.respawn_invincibility_frames,
    ) {
        return Ok(());
    }
    if damage::update_actions(f, &rules.damage, input) {
        return Ok(());
    }
    if ledge::update_actions(f, data, rules.ledge.as_ref(), input) {
        return Ok(());
    }
    // ftCo_Escape_IASA only serves item throws; ftCo_EscapeN_IASA is empty.
    // EscapeAir, FallSpecial and the uninterruptible special landing likewise
    // offer nothing modeled here.
    if escape::owns_action(f.action) || escape_air::owns_action(f.action) {
        return Ok(());
    }
    // Uninterruptible tilt frames own dispatch; interruptible ones expose
    // their source chains below. ftCo_AttackLw3_IASA still runs checkPadA.
    if tilt::owns_action(f.action) && tilt::interrupt_chain(f, data).is_none() {
        tilt::update_ground_attacks(f, data, (rules.tilt.as_ref(), rules.smash.as_ref()), input);
        return Ok(());
    }
    if smash::owns_action(f.action) && tilt::interrupt_chain(f, data).is_none() {
        return Ok(());
    }
    // Uninterruptible jab frames still count rapid presses and buffer the
    // follow-up; interruptible ones run those inside their chains.
    if jab::owns_action(f.action) && tilt::interrupt_chain(f, data).is_none() {
        jab::update_actions(f, data, input);
        return Ok(());
    }
    // ftCo_Dash_IASA / ftCo_Run_IASA / ftCo_AttackDash_IASA in one place;
    // when rules.dash is Some this always consumes Dash and Run frames, and
    // AttackDash's own catch-buffer check always runs before its Wait chain
    // opens (below) or its uninterruptible frames stop here.
    if dash::update_actions(f, data, rules, input)? {
        return Ok(());
    }
    if dash::owns_action(f.action) && tilt::interrupt_chain(f, data).is_none() {
        return Ok(());
    }
    if clank_owns {
        return Ok(());
    }
    let dash_before_special = f.action == Action::Dash;
    if specials::update_actions(f, data, rules, input) {
        // A special starting from Dash falls through to the same friction
        // tail the ordinary dash-to-something-else transition applies,
        // even though specials::update_actions runs after that module
        // already returned this frame.
        if dash_before_special && let Some(dash_rules) = rules.dash.as_ref() {
            dash::apply_transition_friction(f, dash_rules);
        }
        return Ok(());
    }
    if grab::update_actions(f, data, rules.grab.as_ref(), input) {
        return Ok(());
    }
    // ftCo_Wait.c:58 precedes the ordinary shield check ftCo_80091A4C at
    // line 59; ftCo_AppealS_IASA reaches ftCo_80099794 at the same relative
    // position. In both chains only (Walk's chain has no ftCo_80099794 call),
    // a held shoulder with a fresh downward main stick enters EscapeN
    // directly, before this frame ever raises GuardOn.
    if (f.action == Action::Wait || tilt::interrupt_chain(f, data) == Some(tilt::Chain::Taunt))
        && escape::try_wait_chain_spot_dodge(f, data, rules.escape.as_ref(), input)?
    {
        return Ok(());
    }
    if shield::update_actions(f, data, rules, input, shield_owns)? {
        return Ok(());
    }
    // ftCo_800DE9D8, checked after the shield entry and before the jump
    // dispatch in every chain that lists it (Wait, Walk, Squat, SquatWait,
    // SquatRv, Turn, Landing, Ottotto/OttottoWait, AttackS4's interruptible
    // chain and the down tilt's interruptible block; Run and Dash reach it
    // from dash::update_dash_or_run's own block_42 instead, before this
    // point is ever reached for them).
    if f.grounded
        && (matches!(
            f.action,
            Action::Wait
                | Action::Walk
                | Action::Squat
                | Action::SquatWait
                | Action::SquatRv
                | Action::Turn
        ) || edge::owns_action(f.action)
            || matches!(
                tilt::interrupt_chain(f, data),
                Some(tilt::Chain::Wait) | Some(tilt::Chain::DownTilt)
            ))
        && taunt::try_taunt(f, data, input)?
    {
        return Ok(());
    }
    // Every airborne chain checks ftCo_80099A58 before aerial attacks and jumps.
    if escape_air::try_air_dodge(f, data, rules.escape_air.as_ref(), input)? {
        return Ok(());
    }
    if aerial::update(f, data, input) {
        return Ok(());
    }
    if data.locomotion.is_some() {
        locomotion::update_actions(
            f,
            data,
            (rules.tilt.as_ref(), rules.smash.as_ref()),
            rules.edge.as_ref(),
            rules.walk.as_ref(),
            input,
            just_turned,
        );
        return Ok(());
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
    Ok(())
}

fn move_fighter(f: &mut Fighter, data: &FighterData, rules: &Rules, input: Controller) {
    if entry::owns_action(f.action) {
        // ftCo_EntryStart_Phys/ftCo_EntryEnd_Phys: position is written
        // directly from the timer curve, not integrated from velocity.
        entry::move_fighter(f, rules.entry.as_ref());
        return;
    }
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
        specials::tick_ground_timers(f);
        if f.action == Action::Rebound
            && !crate::fighter::clank::apply_rebound_friction(&mut f.clank.impulse)
        {
            // Rebound's first physics callback retains projected self velocity.
        } else if let Some(target) = damage::ground_recovery_velocity(f, data)
            .or_else(|| escape::ground_target_velocity(f, data))
            .or_else(|| smash::ground_target_velocity(f, data))
            .or_else(|| jab::ground_target_velocity(f, data))
            .or_else(|| dash::ground_target_velocity(f, data, rules.dash.as_ref()))
            .or_else(|| taunt::ground_target_velocity(f, data))
            .or_else(|| specials::ground_target_velocity(f, data))
        {
            // ft_80085030 converts the animation's local TransN delta into the
            // exact target ground velocity before projecting it onto the floor.
            // Escape rolls share it; the spot dodge uses ordinary friction.
            movement.ground_acceleration = target - movement.ground_velocity;
            movement.project_ground();
        } else if locomotion::ground_motion(f, data, &mut movement, input) {
            // Explicit locomotion parameters supply dash/run acceleration.
        } else if edge::owns_action(f.action) {
            // ftCo_Ottotto_Phys / ftCo_OttottoWait_Phys: empty. No friction,
            // no movement; velocity was already zeroed on Ottotto's entry.
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
        } else if let Some(friction) = specials::ground_friction_override(f, data) {
            // A move's own dedicated ground friction, distinct from the
            // fighter's ordinary attribute (the side special's End phase).
            movement.friction_ground(friction);
            movement.project_ground();
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
        if specials::air_physics(f, data, rules, &mut movement) {
            // A move that owns this action's air phase drives the frame's
            // airborne physics entirely (gravity-delayed fall plus a fixed
            // air friction, or a root-motion dash's velocity set).
        } else if let Some(false) = escape_air::skip_decay(f, data) {
            // ftCo_EscapeAir_Phys decays both axes without gravity or drift
            // until the script raises its skip-decay flag.
            let decayed = crate::fighter::escape_air::decay(
                f.velocity,
                rules
                    .escape_air
                    .as_ref()
                    .expect("validated air-dodge samples require common rules")
                    .decay,
            );
            movement.self_velocity = [decayed[0], decayed[1], 0.0];
        } else if matches!(f.action, Action::PassiveWall | Action::PassiveWallJump)
            && (f.surface_tech.timer != 0 || f.wall_jump.startup_timer != 0)
        {
            // Wall techs remain fixed until the source timer releases them.
        } else if (!matches!(
            f.action,
            Action::Damage
                | Action::DownDamage
                | Action::PassiveWall
                | Action::PassiveWallJump
                | Action::PassiveCeiling
        ) || damage::damage_air_interruptible(f))
            && !shield::break_invulnerable(f.action)
        {
            let damage_input_locked = matches!(
                f.action,
                Action::Damage
                    | Action::DamageFall
                    | Action::FlyReflectWall
                    | Action::FlyReflectCeiling
            ) && f.hitstun != 0;
            if !damage_input_locked
                && !f.fast_fall
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
            if !locomotion::multi_jump_drift(f, data, &mut movement) {
                if f.action == Action::FallSpecial {
                    // ftCo_80096900: mv.co.fallspecial.mobility is
                    // ca->air_drift_max * mobility. Ordinary FallSpecial
                    // callers pass mobility == 1 (unchanged from ordinary
                    // drift); the Fox/Falco side-special End sets x4C.
                    movement.drift_air_scaled(attrs.air_drift_max * f.aerial.mobility);
                } else {
                    movement.drift_air();
                }
            }
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
    } else if let Some(pose) = wall_jump::pose(fighter, data) {
        pose
    } else if let Some(pose) = damage::surface_response_pose(fighter, data) {
        pose
    } else if let Some(pose) = damage::surface_tech_pose(fighter, data) {
        pose
    } else if let Some(pose) = damage::ground_recovery_pose(fighter, data) {
        pose
    } else if let Some(pose) = damage::damage_pose(fighter, data) {
        pose
    } else if let Some(pose) = escape::pose(fighter, data) {
        pose
    } else if let Some(pose) = escape_air::pose(fighter, data) {
        pose
    } else if let Some(pose) = edge::pose(fighter, data) {
        pose
    } else if let Some(pose) = taunt::pose(fighter, data) {
        pose
    } else if matches!(fighter.action, Action::ReboundStop | Action::Rebound) {
        clank::pose(fighter, data).ok_or_else(|| Error::Data("missing rebound pose".into()))?
    } else if data
        .attack(fighter.action, fighter.prone, fighter.ledge.slow)
        .is_some()
    {
        &attack_frame(fighter, data)?.bones
    } else if aerial::landing_index(fighter.action).is_some() {
        aerial::landing_pose(fighter, data)
            .ok_or_else(|| Error::Data("landing pose is outside supplied samples".into()))?
    } else if let Some(pose) = movement::pose(fighter, data) {
        pose
    } else {
        &data.bones
    };
    let mut bones = local.iter().map(Bone::physics).collect::<Vec<_>>();
    if fighter.action == Action::JumpAerial
        && data
            .locomotion
            .as_ref()
            .is_some_and(|parameters| parameters.multi_jump.is_some())
        && let Some(root) = bones.first_mut()
    {
        root.local.rotation[1] += fighter.locomotion.multi_jump_yaw;
    }
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
    data.attack(fighter.action, fighter.prone, fighter.ledge.slow)
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
    if let Some(states) = edge::hurtbox_frame(fighter, data) {
        return if states.is_empty() {
            Ok(base)
        } else {
            states
                .get(index)
                .copied()
                .ok_or_else(|| Error::Physics("incomplete teeter hurtbox state sample".into()))
        };
    }
    let Some(_) = data.attack(fighter.action, fighter.prone, fighter.ledge.slow) else {
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
