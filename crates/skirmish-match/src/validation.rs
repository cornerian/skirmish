use crate::{data::*, *};
use melee_physics::bones::{BoneCapsule, Pose};

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
    if let Some(geometry) = &stage.geometry {
        require(
            !geometry.lines.is_empty()
                && geometry.lines.len() <= 16384
                && !geometry.joints.is_empty()
                && geometry.joints.len() <= 1024,
            "stage requires bounded nonempty line and joint arrays",
        )?;
        melee_physics::stage::Stage::new(&geometry.lines, &geometry.joints)
            .map_err(|e| Error::Data(e.to_string()))?;
        for line in &geometry.lines {
            use melee_physics::stage::{CEILING, FLOOR, LEFT_WALL, RIGHT_WALL};
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
        require(
            !fighter.jab.frames.is_empty() && fighter.jab.frames.len() <= 4096,
            "jab must supply 1..4096 complete physics frames",
        )?;
        for frame in &fighter.jab.frames {
            require(
                frame.bones.len() == fighter.bones.len(),
                "animation changes bone count",
            )?;
            require(
                frame
                    .bones
                    .iter()
                    .zip(&fighter.bones)
                    .all(|(a, b)| a.parent == b.parent && a.classical_scale == b.classical_scale),
                "animation changes skeleton topology",
            )?;
            let pose = validate_bones(&frame.bones)?;
            for hurt in &fighter.hurtboxes {
                validate_shape(hurt.physics(), &pose)?;
            }
            require(frame.hitboxes.len() <= 4, "at most four hitboxes per frame")?;
            for hit in &frame.hitboxes {
                require(
                    hit.bone < frame.bones.len()
                        && hit.group < 16
                        && finite(hit.center)
                        && nonnegative([hit.radius])
                        && hit.damage <= 999
                        && hit.growth <= 1000
                        && hit.fixed <= 1000
                        && hit.base <= 1000
                        && (0.0..=361.0).contains(&hit.angle_degrees)
                        && hit.angle_degrees.fract() == 0.0,
                    "invalid or unsupported hitbox",
                )?;
                validate_shape(BoneCapsule::sphere(hit.bone, hit.center, hit.radius), &pose)?;
                require(
                    hit.damage as f32 * rules.hitlag.damage_scale + rules.hitlag.base < 1_000_000.0,
                    "hitlag exceeds supported counter range",
                )?;
            }
        }
    }
    Ok(())
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
        if input.buttons & !(BUTTON_A | BUTTON_X | BUTTON_Y) != 0
            || input.stick.iter().any(|x| !(-1.0..=1.0).contains(x))
        {
            return Err(Error::Input(player));
        }
    }
    Ok(())
}

pub(crate) fn state(state: &State) -> Result<(), Error> {
    for f in &state.fighters {
        if f.position
            .into_iter()
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
            .chain([f.percent, f.ground_velocity, f.hitlag, f.facing])
            .any(|v| !v.is_finite())
        {
            return Err(Error::NonFinite);
        }
    }
    for event in &state.events {
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
        let data: MatchData =
            serde_json::from_str(include_str!("../tests/fixtures/integration-match.json")).unwrap();
        let mut snapshot = crate::simulation::initial_state(&data, 0).unwrap();
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
    }
}
