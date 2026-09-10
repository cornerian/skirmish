use super::{data::*, *};
use crate::collision::{
    bones::{BoneCapsule, Pose},
    stage,
};

fn require(condition: bool, message: &str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error::Data(message.into()))
    }
}

fn finite(values: impl IntoIterator<Item = f32>) -> bool {
    values
        .into_iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
}

fn nonnegative(values: impl IntoIterator<Item = f32>) -> bool {
    values.into_iter().all(|v| (0.0..=1_000_000.0).contains(&v))
}

pub(crate) fn validate(data: &MatchData) -> Result<(), Error> {
    require(data.schema == 1, "unsupported schema")?;
    require(!data.provenance.trim().is_empty(), "provenance is required")?;
    let stage = &data.stage;
    super::stage_motion::validate(stage)?;
    if let Some(geometry) = &stage.geometry {
        require(
            !geometry.lines.is_empty()
                && geometry.lines.len() <= 16384
                && !geometry.joints.is_empty()
                && geometry.joints.len() <= 1024,
            "stage requires bounded nonempty line and joint arrays",
        )?;
        stage::Stage::new(&geometry.lines, &geometry.joints)
            .map_err(|e| Error::Data(e.to_string()))?;
        for line in &geometry.lines {
            use crate::collision::stage::{CEILING, FLOOR, LEFT_WALL, RIGHT_WALL};
            let kind = line.flags & (FLOOR | CEILING | LEFT_WALL | RIGHT_WALL);
            let directed = match kind {
                FLOOR => line.start[0] < line.end[0],
                CEILING => line.start[0] > line.end[0],
                LEFT_WALL => line.start[1] < line.end[1],
                RIGHT_WALL => line.start[1] > line.end[1],
                _ => false,
            };
            require(
                directed && finite(line.start.into_iter().chain(line.end)),
                "stage lines require a finite directed surface",
            )?;
        }
    }
    let [left, right, bottom, top] = stage.blast;
    require(
        finite(stage.blast) && left < right && bottom < top,
        "invalid blast boundaries",
    )?;
    require(
        finite([stage.floor.left, stage.floor.right, stage.floor.y])
            && left < stage.floor.left
            && stage.floor.left < stage.floor.right
            && stage.floor.right < right
            && bottom < stage.floor.y
            && stage.floor.y < top,
        "invalid flat floor",
    )?;
    for spawn in stage.spawns {
        require(
            finite(spawn)
                && (stage.floor.left..=stage.floor.right).contains(&spawn[0])
                && (stage.floor.y..top).contains(&spawn[1]),
            "spawn must be on or above the floor inside blast boundaries",
        )?;
    }
    let rules = &data.rules;
    damage::validate_rules(&rules.damage)?;
    if let Some(staling) = &rules.staling {
        super::staling::validate(staling)?;
    }
    if let Some(grab) = &rules.grab {
        for fighter in &data.fighters {
            let denominator = fighter.weight * grab.throw_weight_scale;
            let rate = crate::fighter::grab::throw_animation_rate(
                false,
                fighter.weight,
                grab.throw_weight_scale,
            );
            require(
                denominator.is_finite() && denominator > 0.0 && rate.is_finite() && rate > 0.0,
                "invalid weight-dependent throw rate",
            )?;
        }
    }
    if let Some(nudge) = &rules.nudge {
        require(
            nonnegative([
                nudge.horizontal_step,
                nudge.depth_step,
                nudge.depth_limit,
                nudge.follower_depth_step,
                nudge.follower_depth_limit,
            ]),
            "invalid fighter nudge rules",
        )?;
    }
    if let Some(rebirth) = &rules.rebirth {
        super::rebirth::validate(rebirth, rules.respawn_invincibility_frames)?;
    }
    if let Some(death) = &rules.death {
        super::death::validate(death)?;
        require(
            rules.top_ko_min_knockback.is_some(),
            "blast-death rules require an explicit top KO threshold",
        )?;
    }
    require(
        rules
            .top_ko_min_knockback
            .is_none_or(|minimum| nonnegative([minimum])),
        "invalid top KO knockback threshold",
    )?;
    require(
        rules.stocks > 0
            && rules.time_limit_frames > 0
            && rules.time_limit_frames < i32::MAX as u32
            && rules.countdown_frames < 1_000_000
            && rules.respawn_frames < 1_000_000
            && rules.respawn_invincibility_frames < 1_000_000,
        "invalid match durations/stocks",
    )?;
    require(
        nonnegative([
            rules.friction_above_walk,
            rules.walk_accel_taper_gain,
            rules.knockback_decay,
            rules.knockback_speed,
            rules.hitstun_scale,
        ]) && (0.0..=1.0).contains(&rules.fast_fall_threshold)
            && rules.fast_fall_threshold > 0.0,
        "invalid common physics rules",
    )?;
    let kb = &rules.knockback;
    require(
        nonnegative([
            kb.weight_scale,
            kb.weight_base,
            kb.maximum,
            kb.percent_scale,
            kb.damage_percent_scale,
            kb.fixed_damage,
            kb.growth_scale,
            kb.growth_base,
        ]) && kb.maximum > 0.0
            && kb.maximum * rules.hitstun_scale < 1_000_000.0,
        "invalid knockback rules",
    )?;
    require(
        nonnegative([
            rules.hitlag.damage_scale,
            rules.hitlag.base,
            rules.hitlag.crouch_multiplier,
        ]),
        "invalid hitlag rules",
    )?;
    for fighter in &data.fighters {
        if let Some(rules) = &rules.clank {
            super::clank::validate(rules, fighter)?;
        } else {
            require(
                fighter.rebound.is_none(),
                "rebound animation requires a clank profile",
            )?;
        }
        match (&rules.nudge, &fighter.nudge) {
            (Some(_), Some(attributes)) => require(
                finite([attributes.center_offset]) && nonnegative([attributes.half_width]),
                "invalid fighter nudge attributes",
            )?,
            (Some(_), None) => {
                return Err(Error::Data(
                    "fighter nudge rules require attributes for every fighter".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(Error::Data(
                    "fighter nudge attributes require common rules".into(),
                ));
            }
            (None, None) => {}
        }
        match (&rules.grab, &fighter.grab) {
            (Some(grab_rules), Some(parameters)) => {
                grab::validate(grab_rules, parameters, fighter, rules.staling.is_some())?;
                require(
                    parameters.pummel.damage as f32 * rules.hitlag.damage_scale + rules.hitlag.base
                        < 1_000_000.0,
                    "pummel hitlag exceeds supported counter range",
                )?;
            }
            (Some(_), None) => {
                return Err(Error::Data(
                    "grab rules require parameters for every fighter".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(Error::Data("grab parameters require common rules".into()));
            }
            (None, None) => {}
        }
        match (&rules.ledge, &fighter.ledge) {
            (Some(rules), Some(parameters)) => ledge::validate(rules, parameters, fighter)?,
            (Some(_), None) => {
                return Err(Error::Data(
                    "ledge rules require parameters for every fighter".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(Error::Data("ledge parameters require common rules".into()));
            }
            (None, None) => {}
        }
        if let Some(rules) = &rules.shield {
            shield::validate(rules, fighter)?;
        } else {
            require(
                fighter.shield.is_none(),
                "shield attributes require common shield rules",
            )?;
        }
        if let Some(parameters) = &fighter.locomotion {
            locomotion::validate(parameters)?;
            require(
                rules
                    .damage
                    .displacement
                    .as_ref()
                    .is_none_or(|displacement| {
                        displacement.axis_thresholds
                            == [
                                parameters.horizontal_smash_deadzone,
                                parameters.vertical_smash_deadzone,
                            ]
                    }),
                "locomotion and displacement must use the same shared stick-age thresholds",
            )?;
        }
        if let Some(armor) = &fighter.armor {
            damage::validate_armor(armor)?;
        }
        match (
            rules
                .damage
                .floor_response
                .as_ref()
                .and_then(|profile| profile.tech_roll.as_ref()),
            &fighter.floor_tech,
        ) {
            (Some(_), Some(attributes)) => {
                let invincibility = rules
                    .damage
                    .floor_response
                    .as_ref()
                    .and_then(|profile| profile.recovery_invincibility.as_ref())
                    .map_or(0, |profile| profile.tech_roll_frames);
                damage::validate_floor_tech_attributes(attributes, fighter, invincibility)?
            }
            (Some(_), None) => {
                return Err(Error::Data(
                    "floor-tech roll rules require attributes for every fighter".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(Error::Data(
                    "floor-tech roll attributes require common rules".into(),
                ));
            }
            (None, None) => {}
        }
        match (
            rules.damage.floor_response.as_ref().filter(|profile| {
                profile.knockdown_options.is_some() || profile.down_damage.is_some()
            }),
            &fighter.knockdown,
        ) {
            (Some(_), Some(attributes)) => damage::validate_knockdown_attributes(
                attributes,
                fighter,
                rules.damage.floor_response.as_ref().unwrap(),
            )?,
            (Some(_), None) => {
                return Err(Error::Data(
                    "prone-recovery rules require attributes for every fighter".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(Error::Data(
                    "prone-recovery attributes require common rules".into(),
                ));
            }
            (None, None) => {}
        }
        match (&rules.damage.surface_tech, &fighter.surface_tech) {
            (Some(_), Some(attributes)) => damage::validate_surface_tech_attributes(attributes)?,
            (Some(_), None) => {
                return Err(Error::Data(
                    "damage-surface tech rules require attributes for every fighter".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(Error::Data(
                    "damage-surface tech attributes require common rules".into(),
                ));
            }
            (None, None) => {}
        }
        match (&rules.damage.damage_motion, &fighter.damage_poses) {
            (Some(_), Some(attributes)) => {
                damage::validate_damage_pose_attributes(attributes, fighter)?
            }
            (Some(_), None) => {
                return Err(Error::Data(
                    "damage-motion rules require poses for every fighter".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(Error::Data(
                    "damage poses require explicit common rules".into(),
                ));
            }
            (None, None) => {}
        }
        let m = &fighter.movement;
        require(
            nonnegative([
                m.ground_max_horizontal_velocity,
                m.air_max_horizontal_velocity,
                m.air_drift_stick_mul,
                m.aerial_drift_base,
                m.air_drift_max,
                m.aerial_friction,
                m.gravity,
                m.terminal_velocity,
                m.fast_fall_velocity,
                m.ground_friction,
                m.walk_acceleration_mul,
                m.walk_acceleration_base,
                m.walk_max_velocity,
                m.jump_vertical_velocity,
                m.short_hop_vertical_velocity,
                m.jump_horizontal_velocity,
                m.jump_horizontal_max,
                m.jump_momentum_multiplier,
                fighter.weight,
            ]) && m.jump_startup_frames > 0
                && m.jump_startup_frames < 1_000_000
                && m.landing_frames < 1_000_000
                && fighter.weight > 0.0,
            "invalid fighter attributes",
        )?;
        let pose = validate_bones(&fighter.bones)?;
        match &fighter.collision_box {
            CollisionBox::Fixed { source } => require(
                nonnegative([source.up, source.down, source.front, source.back])
                    && finite([source.angle]),
                "invalid fixed environmental collision box",
            )?,
            CollisionBox::Bones {
                indices,
                parameters,
                flags,
            } => require(
                indices.iter().all(|&index| index < fighter.bones.len())
                    && finite([parameters.side_y_offset])
                    && nonnegative([parameters.height_threshold, parameters.width_threshold])
                    && flags & !31 == 0,
                "invalid bone environmental collision box",
            )?,
        }
        require(
            !fighter.hurtboxes.is_empty() && fighter.hurtboxes.len() <= 128,
            "hurtboxes required (maximum 128)",
        )?;
        for hurt in &fighter.hurtboxes {
            require(
                hurt.bone < fighter.bones.len()
                    && finite(hurt.start)
                    && finite(hurt.end)
                    && nonnegative([hurt.radius]),
                "invalid hurtbox",
            )?;
            validate_shape(hurt.physics(), &pose)?;
        }
        if let Some(p) = &fighter.aerials {
            require(
                p.selection
                    .thresholds
                    .into_iter()
                    .all(|x| x > 0.0 && x <= 1.0)
                    && (0.0..=core::f32::consts::FRAC_PI_2).contains(&p.selection.vertical_angle)
                    && (0..=255).contains(&p.l_cancel_window)
                    && p.l_cancel_divisor.is_finite()
                    && p.l_cancel_divisor > 0.0,
                "invalid aerial selection or L-cancel parameters",
            )?;
            for movement in &p.moves {
                require(
                    movement.flags.len() == movement.attack.frames.len()
                        && movement.landing_lag > 0.0
                        && movement.landing_lag <= 1_000_000.0
                        && movement.landing_lag / p.l_cancel_divisor < 2_147_483_648.0
                        && (0.0..=4095.0).contains(&movement.landing_animation_end)
                        && !movement.landing_poses.is_empty()
                        && movement.landing_poses.len() <= 4096
                        && (movement.landing_animation_end as usize) < movement.landing_poses.len(),
                    "aerials require command flags, finite landing lag and complete landing poses",
                )?;
                for sample in &movement.landing_poses {
                    validate_animation_pose(sample, fighter)?;
                }
                let cancelled = crate::fighter::aerial::landing_lag(
                    movement.landing_lag,
                    0,
                    p.l_cancel_window,
                    p.l_cancel_divisor,
                )
                .map_err(|e| Error::Data(e.to_string()))?;
                require(
                    [movement.landing_lag, cancelled].into_iter().all(|lag| {
                        let rate = crate::fighter::aerial::landing_animation_rate(
                            movement.landing_animation_end,
                            lag,
                        );
                        rate.is_finite() && rate > 0.0
                    }),
                    "aerial landing animation rate must be finite and positive",
                )?;
            }
        }
        if let Some(p) = &fighter.special {
            require(
                p.neutral_thresholds
                    .into_iter()
                    .all(|threshold| threshold.is_finite() && threshold > 0.0 && threshold <= 1.0)
                    && p.ground.frames.len() == p.air.frames.len(),
                "specials require valid neutral thresholds and paired frame counts",
            )?;
        }
        for attack in core::iter::once(&fighter.jab)
            .chain(
                fighter
                    .aerials
                    .iter()
                    .flat_map(|p| p.moves.iter().map(|m| &m.attack)),
            )
            .chain(fighter.ledge.iter().map(|p| &p.attack.attack))
            .chain(fighter.special.iter().flat_map(|p| [&p.ground, &p.air]))
            .chain(
                fighter
                    .knockdown
                    .iter()
                    .flat_map(|p| [&p.face_up.attack, &p.face_down.attack]),
            )
        {
            require(
                rules.staling.is_none() || attack.move_id.is_some_and(|id| id != 0),
                "staling requires an explicit nonzero attack move_id",
            )?;
            require(
                !attack.frames.is_empty() && attack.frames.len() <= 4096,
                "attack must supply 1..4096 complete physics frames",
            )?;
            for frame in &attack.frames {
                let pose = validate_animation_pose(&frame.bones, fighter)?;
                require(frame.hitboxes.len() <= 4, "at most four hitboxes per frame")?;
                require(
                    frame.hurtbox_states.is_empty()
                        || frame.hurtbox_states.len() == fighter.hurtboxes.len(),
                    "attack hurtbox state samples must be empty or complete",
                )?;
                for hit in &frame.hitboxes {
                    require(
                        rules.clank.is_some() || !(hit.clank || hit.rebound),
                        "clank/rebound flags require an explicit ordinary profile",
                    )?;
                    require(
                        hit.bone < frame.bones.len()
                            && hit.group < 16
                            && finite(hit.center)
                            && nonnegative([hit.radius])
                            && hit.damage <= 999
                            && (-1000..=1000).contains(&hit.shield_damage)
                            && hit.growth <= 1000
                            && hit.fixed <= 1000
                            && hit.base <= 1000
                            && (0.0..=362.0).contains(&hit.angle_degrees)
                            && hit.angle_degrees.fract() == 0.0,
                        "invalid or unsupported hitbox",
                    )?;
                    validate_shape(BoneCapsule::sphere(hit.bone, hit.center, hit.radius), &pose)?;
                    require(
                        hit.damage as f32 * rules.hitlag.damage_scale + rules.hitlag.base
                            < 1_000_000.0,
                        "hitlag exceeds supported counter range",
                    )?;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_animation_pose(
    bones: &[Bone],
    fighter: &FighterData,
) -> Result<Pose, Error> {
    require(
        bones.len() == fighter.bones.len(),
        "animation changes bone count",
    )?;
    require(
        bones
            .iter()
            .zip(&fighter.bones)
            .all(|(a, b)| a.parent == b.parent && a.classical_scale == b.classical_scale),
        "animation changes skeleton topology",
    )?;
    let pose = validate_bones(bones)?;
    for hurt in &fighter.hurtboxes {
        validate_shape(hurt.physics(), &pose)?;
    }
    Ok(pose)
}

fn validate_bones(bones: &[Bone]) -> Result<Pose, Error> {
    require(
        !bones.is_empty() && bones.len() <= 256,
        "skeleton requires 1..256 bones",
    )?;
    require(
        bones.iter().all(|b| {
            finite(b.translation)
                && finite(b.rotation)
                && finite(b.scale)
                && b.scale.iter().all(|&s| s > 0.0)
        }),
        "invalid bone transform",
    )?;
    Pose::evaluate(&bones.iter().map(Bone::physics).collect::<Vec<_>>())
        .map_err(|e| Error::Data(e.to_string()))
}

fn validate_shape(shape: BoneCapsule, pose: &Pose) -> Result<(), Error> {
    let shape = shape
        .transform(pose, 1.0)
        .map_err(|e| Error::Data(e.to_string()))?;
    // Local bounds alone cannot bound a hierarchy's composed scales. Keep
    // sampled collision geometry in range before its squared-distance tests.
    require(
        finite(
            shape
                .start
                .into_iter()
                .chain(shape.end)
                .chain([shape.radius]),
        ),
        "transformed collision geometry exceeds the supported range",
    )
}

pub(crate) fn inputs(input: &[Controller; 2]) -> Result<(), Error> {
    for (player, input) in input.iter().enumerate() {
        if input.buttons
            & !(BUTTON_A | BUTTON_B | BUTTON_Z | BUTTON_X | BUTTON_Y | BUTTON_L | BUTTON_R)
            != 0
            || input
                .stick
                .iter()
                .chain(&input.cstick)
                .any(|x| !(-1.0..=1.0).contains(x))
            || !(0.0..=1.0).contains(&input.trigger)
        {
            return Err(Error::Input(player));
        }
    }
    Ok(())
}

pub(crate) fn state(state: &State) -> Result<(), Error> {
    for (player, f) in state.fighters.iter().enumerate() {
        require(
            grab::valid_relationship(&state.fighters, player),
            "invalid paired capture state",
        )?;
        require(ledge::valid_state(f), "invalid ledge attachment state")?;
        require(
            f.staling.transitions.is_empty(),
            "unflushed attack identity transition",
        )?;
        if f.position
            .into_iter()
            .chain([f.depth])
            .chain(f.deferred_position)
            .chain(f.nudge)
            .chain(f.velocity)
            .chain(f.knockback)
            .chain(f.floor_normal)
            .chain(f.hitboxes.into_iter().flat_map(|track| {
                track
                    .previous
                    .into_iter()
                    .chain(track.current)
                    .chain([track.radius])
            }))
            .chain(
                [
                    f.ecb.current,
                    f.ecb.previous,
                    f.ecb.desired,
                    f.ecb.before_load,
                    f.ecb.unsqueezed,
                ]
                .into_iter()
                .flat_map(|shape| {
                    shape
                        .top
                        .into_iter()
                        .chain(shape.bottom)
                        .chain(shape.left)
                        .chain(shape.right)
                }),
            )
            .chain([
                f.percent,
                f.ground_velocity,
                f.ground_knockback,
                f.hitlag,
                f.facing,
                f.grab.escape_timer,
            ])
            .chain([
                f.locomotion.turn_frames,
                f.locomotion.run_brake_frames,
                f.locomotion.dash_initial_delta,
                f.aerial.landing_elapsed,
                f.aerial.landing_rate,
                f.clank.clock,
                f.clank.rate,
                f.clank.impulse,
                f.clank.pending_ground_acceleration,
                f.clank.response.rebound_duration,
                f.clank.response.towards,
            ])
            .chain(f.locomotion.pass_delay)
            .chain(f.staling.hits.iter().flatten().map(|hit| hit.damage))
            .chain([
                f.shield.health,
                f.shield.strength,
                f.shield.minimum_hold,
                f.shield.raise_progress,
                f.shield.stun_progress,
                f.shield.stun_rate,
                f.shield.attacker_ground_push,
                f.shield.dizzy_timer,
            ])
            .chain(f.shield.attacker_push)
            .chain(f.death.camera_offset)
            .chain([f.death.depth_velocity])
            .any(|v| !v.is_finite())
        {
            return Err(Error::NonFinite);
        }
    }
    for event in &state.events {
        if let Event::ShieldHit { damage, .. } = event
            && !damage.is_finite()
        {
            return Err(Error::NonFinite);
        }
        if let Event::Hit {
            damage, knockback, ..
        } = event
            && (!damage.is_finite() || !knockback.is_finite())
        {
            return Err(Error::NonFinite);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonfinite_events_even_when_fighter_state_is_finite() {
        let data: MatchData = serde_json::from_str(include_str!(
            "../../tests/fixtures/game/integration-match.json"
        ))
        .unwrap();
        let mut snapshot = super::simulation::initial_state(&data, 0).unwrap();
        for (damage, knockback) in [(f32::NAN, 1.0), (1.0, f32::INFINITY)] {
            snapshot.events = vec![Event::Hit {
                attacker: 0,
                victim: 1,
                damage,
                knockback,
            }];
            assert!(matches!(state(&snapshot), Err(Error::NonFinite)));
        }
        snapshot.events.clear();
        snapshot.fighters[0].facing = f32::NAN;
        assert!(matches!(state(&snapshot), Err(Error::NonFinite)));
        snapshot.fighters[0].facing = 1.0;
        snapshot.fighters[0].ground_knockback = f32::NAN;
        assert!(matches!(state(&snapshot), Err(Error::NonFinite)));
    }
}
