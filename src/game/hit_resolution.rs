//! Cross-fighter hit preparation, resolution, and shield contact.
//! Fighter-local arithmetic and state machines remain in `crate::fighter`.
use crate::fighter::damage::ProneOrientation;
use crate::fighter::{combat, damage, shield};
use crate::game::{
    Action, Error, Event, Fighter, State,
    data::{Hitbox, MatchData},
    script,
};
fn physics(error: impl core::fmt::Display) -> Error {
    Error::Physics(error.to_string())
}

// The original damage transition assigns/merges launch velocity before hitlag,
/// installs a post-hitlag callback, then resets the elapsed-hit counter. The
/// caller resolves simultaneous contacts before invoking this helper.
#[derive(Clone, Copy)]
pub(crate) enum HitDirection {
    FighterContact(FighterContact),
    Throw,
}

#[derive(Clone, Copy)]
pub(crate) struct FighterContact {
    pub hurt_start: [f32; 3],
    pub hurt_end: [f32; 3],
    pub position: [f32; 3],
}

/// A hit after its pre hooks have run.  Preparation is deliberately separate
/// from resolution so simultaneous contacts can run hooks once, in contact
/// order, before any one of them changes the defender's action.
pub(crate) struct PreparedHit {
    attacker: usize,
    victim: usize,
    hit: Hitbox,
    staled: crate::game::staling::Hit,
    hurt_height: damage::HurtHeight,
    projectile: bool,
    target: Fighter,
    was_grounded: bool,
    previous_facing: f32,
    damage_facing: f32,
    down_damage_face_up: Option<bool>,
    patch: script::HitPatch,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_hit(
    data: &MatchData,
    state: &mut State,
    attacker: usize,
    hit: &Hitbox,
    staled: crate::game::staling::Hit,
    hurt_height: damage::HurtHeight,
    direction: HitDirection,
    projectile: bool,
) -> Result<bool, Error> {
    let Some(prepared) = prepare_hit(
        data,
        state,
        attacker,
        hit,
        staled,
        hurt_height,
        direction,
        projectile,
    )?
    else {
        return Ok(false);
    };
    resolve_prepared_hit(data, state, prepared)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_hit(
    data: &MatchData,
    state: &mut State,
    attacker: usize,
    hit: &Hitbox,
    staled: crate::game::staling::Hit,
    hurt_height: damage::HurtHeight,
    direction: HitDirection,
    projectile: bool,
) -> Result<Option<PreparedHit>, Error> {
    let victim = 1 - attacker;
    let rules = &data.rules;
    let target = state.fighters[victim].clone();
    let was_grounded = target.grounded;
    let previous_facing = target.facing;
    let (damage_facing, angle_degrees) = match direction {
        HitDirection::FighterContact(contact) if hit.angle_degrees == 362.0 => {
            let launch =
                damage::positional_launch(contact.hurt_start, contact.hurt_end, contact.position);
            (launch.direction, launch.angle_degrees)
        }
        HitDirection::FighterContact(_) => (
            damage::fighter_hit_direction(target.position[0], state.fighters[attacker].position[0]),
            hit.angle_degrees as i32,
        ),
        HitDirection::Throw => (
            damage::throw_hit_direction(state.fighters[attacker].facing),
            hit.angle_degrees as i32,
        ),
    };
    let down_damage_face_up = rules
        .damage
        .floor_response
        .as_ref()
        .and_then(|floor| floor.down_damage.as_ref())
        .and_then(|profile| {
            damage::down_damage_face_up(
                matches!(
                    target.action,
                    Action::DownBound | Action::DownWait | Action::DownDamage
                ),
                target.action == Action::DownWait && target.prone == Some(ProneOrientation::FaceUp),
                false,
                staled.damage,
                profile.pending_damage_threshold,
            )
        });
    let knockback = combat::knockback(
        &rules.knockback.physics(),
        combat::KnockbackHit {
            growth: hit.growth,
            fixed: hit.fixed,
            base: hit.base,
        },
        combat::DamageState {
            percent: target.percent,
            pending_damage: staled.damage,
            count_override: None,
        },
        staled.base_damage,
        combat::KnockbackModifiers {
            stage: 1.0,
            attack: 1.0,
            defense: 1.0,
            weight: data.fighters[victim].weight,
        },
    )
    .map_err(physics)?;
    if !knockback.is_finite() {
        return Err(Error::NonFinite);
    }
    if knockback < 0.0 {
        return Err(Error::Physics(
            "combat rules produced negative knockback".into(),
        ));
    }
    // ftCo_Damage_CalcKnockback scales a charging victim before armor.
    let knockback = match &data.rules.smash {
        Some(smash) if crate::fighter::smash::charging(&target) => {
            knockback * smash.charging_knockback_multiplier
        }
        _ => knockback,
    };
    let knockback = data.fighters[victim]
        .armor
        .as_ref()
        .map_or(knockback, |armor| {
            damage::subtract_armor(
                knockback,
                [armor.armor0, armor.armor1],
                armor.minimum_knockback,
            )
        });
    let damage_motion = rules.damage.damage_motion.as_ref().map(|profile| {
        damage::damage_motion(
            knockback,
            rules.hitstun_scale,
            profile.thresholds,
            !target.grounded,
            hurt_height,
        )
    });
    let attacker_hitlag = combat::hitlag(staled.damage as i32, false, 1.0, &rules.hitlag.physics())
        .map_err(physics)?;
    let hitlag = combat::hitlag(
        staled.damage as i32,
        matches!(target.action, Action::Squat | Action::SquatWait),
        1.0,
        &rules.hitlag.physics(),
    )
    .map_err(physics)?;
    let hitstun = combat::initial_hitstun(knockback, rules.hitstun_scale).map_err(physics)?;
    if !knockback.is_finite() || !hitlag.is_finite() || !attacker_hitlag.is_finite() {
        return Err(Error::NonFinite);
    }
    if knockback < 0.0 || hitlag < 0.0 || attacker_hitlag < 0.0 || hitstun < 1 {
        return Err(Error::Physics(
            "combat rules produced a negative magnitude or invalid timer".into(),
        ));
    }
    let angle = damage::launch_angle(
        angle_degrees,
        knockback,
        !target.grounded,
        &rules.damage.angle_rules(),
    );
    let speed = knockback * rules.knockback_speed;
    let incoming = [
        -speed * crate::compat::math::trig::cosf(angle.radians) * damage_facing,
        speed * crate::compat::math::trig::sinf(angle.radians),
    ];
    let ground_launch = (was_grounded && rules.damage.ground_launch.is_some()).then(|| {
        damage::ground_launch(
            incoming,
            [target.floor_normal[0], target.floor_normal[1]],
            down_damage_face_up.is_some()
                || matches!(damage_motion, Some(damage::DamageMotion::Fly { .. })),
            &rules.damage.ground_launch.as_ref().unwrap().physics(),
        )
    });
    let incoming = ground_launch.map_or(incoming, |launch| launch.knockback);
    let merged = damage::merge_knockback(
        target.knockback,
        incoming,
        target.damage_elapsed,
        rules.damage.knockback_replace_window,
    );
    if !angle.radians.is_finite() || merged.into_iter().any(|value| !value.is_finite()) {
        return Err(Error::NonFinite);
    }
    let mut patch = script::HitPatch {
        cancelled: false,
        damage: staled.damage,
        angle: angle_degrees as f32,
        knockback,
        apply_damage: true,
        apply_knockback: true,
        apply_hitlag: true,
        apply_hitstun: true,
        reflect: false,
    };
    let view = |id: usize, fighter: &Fighter| script::FighterView {
        id: id as u8,
        action: format!("{:?}", fighter.action),
        action_frame: fighter.action_frame,
        velocity: fighter.velocity,
        ground_velocity: fighter.ground_velocity,
        grounded: fighter.grounded,
        percent: fighter.percent,
        hitlag: fighter.hitlag,
        hitstun: fighter.hitstun,
        flags: Default::default(),
    };
    let mut hit_view = script::HitView {
        frame: state.next_frame,
        attacker: attacker as u8,
        defender: victim as u8,
        damage: staled.damage,
        angle: angle_degrees as f32,
        base_knockback: hit.base,
        knockback_growth: hit.growth,
        knockback,
        hitbox_group: hit.group,
        projectile,
        max_damage: 0,
    };
    for (id, hook) in [
        (attacker, script::Hook::BeforeHit),
        (victim, script::Hook::BeforeReceiveHit),
    ] {
        let Some(program) = crate::game::script::definition::cached_program(&data.fighters[id])
        else {
            continue;
        };
        let fighter_view = view(id, &state.fighters[id]);
        let result = if hook == script::Hook::BeforeReceiveHit {
            program.dispatch_with_context_with_patch(
                hook,
                &fighter_view,
                &hit_view,
                &patch,
                script::CombatContext {
                    persistent: &state.fighters[id].script_state,
                    action_state: &state.fighters[id].action_state,
                    resources: data.fighters[id].script_resources.get(),
                },
            )
        } else {
            program.dispatch_with_context(
                hook,
                &fighter_view,
                Some(&hit_view),
                script::CombatContext {
                    persistent: &state.fighters[id].script_state,
                    action_state: &state.fighters[id].action_state,
                    resources: data.fighters[id].script_resources.get(),
                },
            )
        }
        .map_err(|error| Error::Data(format!("fighter script hook failed: {error}")))?;
        state.fighters[id].script_state = result.locals;
        state.fighters[id].action_state = result.action_state;
        script::apply_commands(state, id, &result.commands)?;
        if let Some(next) = result.hit {
            let prior = patch;
            patch = next;
            patch.cancelled |= prior.cancelled;
            patch.apply_damage &= prior.apply_damage;
            patch.apply_knockback &= prior.apply_knockback;
            patch.apply_hitlag &= prior.apply_hitlag;
            patch.apply_hitstun &= prior.apply_hitstun;
            hit_view.damage = patch.damage;
            hit_view.angle = patch.angle;
            hit_view.knockback = patch.knockback;
        }
    }
    if patch.cancelled {
        return Ok(None);
    }
    let prepared = PreparedHit {
        attacker,
        victim,
        hit: hit.clone(),
        staled,
        hurt_height,
        projectile,
        target,
        was_grounded,
        previous_facing,
        damage_facing,
        down_damage_face_up,
        patch,
    };
    Ok(Some(prepared))
}

pub(crate) fn resolve_prepared_hit(
    data: &MatchData,
    state: &mut State,
    prepared: PreparedHit,
) -> Result<bool, Error> {
    let PreparedHit {
        attacker,
        victim,
        hit,
        staled,
        hurt_height,
        projectile,
        target,
        was_grounded,
        previous_facing,
        damage_facing,
        down_damage_face_up,
        patch,
    } = prepared;
    let rules = &data.rules;
    if !patch.damage.is_finite()
        || patch.damage < 0.0
        || !patch.angle.is_finite()
        || !patch.knockback.is_finite()
        || patch.knockback < 0.0
    {
        return Err(Error::Data(
            "fighter script returned invalid hit values".into(),
        ));
    }
    let apply_knockback = patch.apply_knockback && patch.knockback != 0.0;
    let has_knockback = patch.knockback != 0.0;
    let apply_hitstun = patch.apply_hitstun && has_knockback;
    let apply_hitlag = patch.apply_hitlag && has_knockback;
    let damage_motion = rules.damage.damage_motion.as_ref().map(|profile| {
        damage::damage_motion(
            patch.knockback,
            rules.hitstun_scale,
            profile.thresholds,
            !target.grounded,
            hurt_height,
        )
    });
    let attacker_hitlag = combat::hitlag(patch.damage as i32, false, 1.0, &rules.hitlag.physics())
        .map_err(physics)?;
    let hitlag = combat::hitlag(
        patch.damage as i32,
        matches!(target.action, Action::Squat | Action::SquatWait),
        1.0,
        &rules.hitlag.physics(),
    )
    .map_err(physics)?;
    let hitstun = combat::initial_hitstun(patch.knockback, rules.hitstun_scale).map_err(physics)?;
    let angle = damage::launch_angle(
        patch.angle as i32,
        patch.knockback,
        !target.grounded,
        &rules.damage.angle_rules(),
    );
    let speed = patch.knockback * rules.knockback_speed;
    let incoming = [
        -speed * crate::compat::math::trig::cosf(angle.radians) * damage_facing,
        speed * crate::compat::math::trig::sinf(angle.radians),
    ];
    let ground_launch = (was_grounded && rules.damage.ground_launch.is_some()).then(|| {
        damage::ground_launch(
            incoming,
            [target.floor_normal[0], target.floor_normal[1]],
            down_damage_face_up.is_some()
                || matches!(damage_motion, Some(damage::DamageMotion::Fly { .. })),
            &rules.damage.ground_launch.as_ref().unwrap().physics(),
        )
    });
    let incoming = ground_launch.map_or(incoming, |launch| launch.knockback);
    let merged = damage::merge_knockback(
        target.knockback,
        incoming,
        target.damage_elapsed,
        rules.damage.knockback_replace_window,
    );
    if !angle.radians.is_finite()
        || incoming.into_iter().any(|value| !value.is_finite())
        || merged.into_iter().any(|value| !value.is_finite())
        || !attacker_hitlag.is_finite()
        || !hitlag.is_finite()
        || attacker_hitlag < 0.0
        || hitlag < 0.0
        || hitstun < 1
        || ground_launch.is_some_and(|launch| {
            launch.knockback.into_iter().any(|value| !value.is_finite())
                || !launch.ground_knockback.is_finite()
        })
    {
        return Err(Error::Data(
            "fighter script produced invalid resolved hit values".into(),
        ));
    }
    // `Fighter_UnkTakeDamage_8006CC30` (percent) and `ftCo_Damage_CalcKnockback`
    // (this hit's own knockback magnitude) both run unconditionally in
    // `Fighter_ProcessHit_8006D1EC`, before its `switch (fp->x1828)` reaches
    // `ftCo_8008EC90` (the ordinary ground/air reaction entry, `ftCo_
    // Damage.c:838`): that function's own first check --
    // `if (fp->x2220_b3 || fp->x2220_b4 || !fp->dmg.kb_applied) { inlineB2(gobj);
    // return; }` -- skips its entire motion-state transition (`ftCo_8008E908`
    // -> `ftCo_8008DCE0`, `:668,266`) whenever the computed knockback is
    // exactly zero, applying only cosmetic hit-effects (`inlineB2`'s own
    // `ftCo_8008DA4C` flash/vibration, not modeled) instead. A hit with zero
    // growth, zero base and zero weight-independent knockback -- e.g. Fox/
    // Falco's own Blaster laser, confirmed zero across all three fields
    // (`docs/fox-neutral-special.md`) -- always computes exactly this,
    // regardless of percent or weight: it "flinches" cosmetically without
    // ever forcing a reaction, so a fighter mid-JumpSquat (or any other
    // action) simply keeps going, only its damage percent ticking up --
    // `combat_history::record_hit`'s own victim-side `last_hit_by`/
    // `last_hit_by_instance` bookkeeping stays at its own prior (no-hit)
    // value too, since decomp's skipped reaction never reaches the code
    // that would update it. The attacker's own last-move-landed/combo-count
    // bookkeeping (the same function's `combo::record`) is unaffected --
    // it is the attacker's own accounting, set independently of whether the
    // victim ever flinches -- so `record_hit` itself still runs every hit.
    // The attacker's own hitlag, though, is part of the same skipped
    // reaction (`ftCo_8008EC90`'s gate runs before `ftCo_Damage_
    // CalcHitlag` would otherwise apply it to either side), so it stays
    // gated alongside the victim's.
    let reacted = apply_knockback;
    if apply_hitlag {
        state.fighters[attacker].hitlag = state.fighters[attacker].hitlag.max(attacker_hitlag);
    }
    crate::game::combat_history::record_hit(
        state,
        attacker,
        victim,
        staled.identity.move_id,
        &data.rules.damage.combo,
        reacted,
    );
    if patch.apply_damage {
        state.fighters[victim].percent = (state.fighters[victim].percent + patch.damage).min(999.0);
    }
    if apply_hitlag {
        state.fighters[victim].hitlag = state.fighters[victim].hitlag.max(hitlag);
    }
    if reacted {
        let target = &mut state.fighters[victim];
        // ftCo_8008DCE0 first installs the hit direction. Its prone
        // low-damage caller then supplies the old facing as the
        // transition's final override.
        target.facing = if down_damage_face_up.is_some() {
            previous_facing
        } else {
            damage_facing
        };
        target.hitstun = if apply_hitstun { hitstun as u32 } else { 0 };
        target.velocity = [0.0; 2];
        target.ground_velocity = 0.0;
        target.knockback = merged;
        target.ground_knockback = ground_launch.map_or(0.0, |launch| launch.ground_knockback);
        if was_grounded && ground_launch.is_none_or(|launch| launch.airborne) {
            // ftCommon_8007D5D4 consumes the ground jump when damage leaves ground.
            target.locomotion.jumps_used = 1;
        }
        if ground_launch.is_none_or(|launch| launch.airborne) {
            target.grounded = false;
            target.ground_line = None;
        }
        target.fast_fall = false;
        crate::fighter::ledge::release_on_damage(target, rules.ledge.as_ref());
        crate::game::simulation::enter(
            target,
            if down_damage_face_up.is_some() {
                Action::DownDamage
            } else {
                Action::Damage
            },
        );
        target.damage_motion = if down_damage_face_up.is_none() {
            damage_motion
        } else {
            None
        };
        if let Some(face_up) = down_damage_face_up {
            target.prone = Some(if face_up {
                ProneOrientation::FaceUp
            } else {
                ProneOrientation::FaceDown
            });
            // The source stores hitstun in the same union slot used by DownWait.
            target.down_timer = if apply_hitstun { hitstun as u32 } else { 0 };
        }
        target.damage_elapsed = 0;
        target.locomotion.tilt_x_age = 254;
        target.locomotion.tilt_y_age = 254;
        target.damage_angle_flag = 0;
        target.last_damage_surface = None;
        target.reflect_lockout = 0;
        if let Some(timer) = angle.special_timer {
            target.damage_angle_flag = 1;
            target.damage_angle_timer = timer;
        }
        // A zero-hitlag hit has no expiry callback in the examined upstream path.
        target.di_pending = target.hitlag > 0.0;
        target.tumbling = rules
            .damage
            .floor_response
            .as_ref()
            .is_some_and(|profile| patch.knockback >= profile.tumble_knockback_threshold);
    }
    state.events.push(Event::Hit {
        attacker,
        victim,
        damage: patch.damage,
        knockback: patch.knockback,
    });
    // Post-resolution hooks observe the committed hit outcome. Their locals
    // are still part of the transactional Match::step state, while returned
    // hit edits are intentionally ignored after resolution.
    let hit_view = script::HitView {
        frame: state.next_frame,
        attacker: attacker as u8,
        defender: victim as u8,
        damage: patch.damage,
        angle: patch.angle,
        base_knockback: hit.base,
        knockback_growth: hit.growth,
        knockback: patch.knockback,
        hitbox_group: hit.group,
        projectile,
        max_damage: 0,
    };
    for (id, hook) in [
        (attacker, script::Hook::AfterHit),
        (victim, script::Hook::AfterReceiveHit),
    ] {
        let Some(program) = crate::game::script::definition::cached_program(&data.fighters[id])
        else {
            continue;
        };
        let fighter = state.fighters[id].clone();
        let view = script::FighterView {
            id: id as u8,
            action: format!("{:?}", fighter.action),
            action_frame: fighter.action_frame,
            velocity: fighter.velocity,
            ground_velocity: fighter.ground_velocity,
            grounded: fighter.grounded,
            percent: fighter.percent,
            hitlag: fighter.hitlag,
            hitstun: fighter.hitstun,
            flags: Default::default(),
        };
        let result = program
            .dispatch_with_context_with_patch(
                hook,
                &view,
                &hit_view,
                &patch,
                script::CombatContext {
                    persistent: &fighter.script_state,
                    action_state: &fighter.action_state,
                    resources: data.fighters[id].script_resources.get(),
                },
            )
            .map_err(|error| Error::Data(format!("fighter script hook failed: {error}")))?;
        state.fighters[id].script_state = result.locals;
        state.fighters[id].action_state = result.action_state;
        script::apply_commands(state, id, &result.commands)?;
    }
    Ok(true)
}

// Shield contact is cross-fighter because it mutates both participants and emits match events.
pub(crate) fn apply_shield_contact(
    data: &MatchData,
    state: &mut State,
    attacker: usize,
    hit: &Hitbox,
    staled: crate::game::staling::Hit,
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
    let damage = shield::environment_damage(staled.damage);
    let amount_damage = (damage + hit.shield_damage).max(0);
    let loss = shield::damage_loss(
        amount_damage,
        amount,
        r.damage_scales,
        r.damage_scale,
        r.damage_base,
    );
    let f = &mut state.fighters[victim];
    let powershield = f.shield.powershield;
    let applied_loss = if powershield { 0.0 } else { loss };
    f.shield.health -= applied_loss;
    let broken = f.shield.health < 0.0;
    if damage == 0 {
        if broken {
            f.shield.health = r.break_health;
        }
        state.events.push(Event::ShieldHit {
            attacker,
            victim,
            damage: applied_loss,
            broken: false,
        });
        return Ok(());
    }
    if broken {
        f.shield.health = r.break_health;
        shield::start_break(f, a);
    } else {
        let stun = shield::stun(damage, amount, r.stun_scales, r.stun_scale, r.stun_base);
        let (rate, velocity) = shield::response(
            stun,
            a.stun_animation_end,
            r.push_scale,
            if powershield { 1.0 } else { r.push_multiplier },
            r.push_maximum,
            -towards,
        );
        f.shield.stun_rate = rate;
        f.shield.stun_progress = 0.0;
        f.ground_velocity = velocity;
        f.locomotion.tilt_x_age = 254;
        shield::enter(f, Action::GuardSetOff);
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
        damage: applied_loss,
        broken,
    });
    Ok(())
}
