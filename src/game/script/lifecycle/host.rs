//! NativeHost implementation for one transactional lifecycle dispatch.
//!
//! The host owns only private clones and small projected metadata. Starlark
//! receives path-backed objects, so reading a resource container does not
//! clone the container; every subsequent attribute lookup resolves through
//! this host. The original fighter and movement are committed by `finish`
//! only after callback execution and all validation succeed.

use super::lifecycle_movement;
use super::lifecycle_resources::ResourceCache;
use super::lifecycle_state;
use super::starlark::value::{
    Error as StarError, NativeHost, NativeKind, NativeObject, NativeValue,
};
use super::{LocalState, LocalValue, StateSchema, StateType};
use crate::game::data::{FighterData, Rules, StageGeometry};
use crate::{
    fighter::{Attributes, Movement},
    game,
};
use serde_json::{Map, Value as JsonValue};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) struct NativeHelperParams {
    pub attributes: Attributes,
    pub locomotion: Option<crate::fighter::locomotion::Parameters>,
    pub jump_vertical_velocity: f32,
}

pub(crate) struct LifecycleHost {
    pub fighter: game::Fighter,
    pub movement: Option<Movement>,
    pub context: JsonValue,
    pub pre_landing: game::Fighter,
    pub geometry: Option<StageGeometry>,
    pub helper_params: Option<NativeHelperParams>,
    pub resources: Arc<ResourceCache>,
    pub persistent: LocalState,
    pub action_state: LocalState,
    /// Resource root owned by the currently selected special-move behavior.
    /// The dispatcher supplies this only for callbacks whose descriptor
    /// guarantees the root exists; explicit `resource(path)` remains optional.
    pub owned_resource: Option<String>,
    /// Behavior owner of the callback currently executing, for input hooks.
    /// Root callbacks intentionally leave this unset.
    pub callback_owner: Option<usize>,
    pub metadata: Option<Arc<crate::game::script::definition::FighterDefinition>>,
    pub persistent_schema: Option<StateSchema>,
    pub action_schema: Option<StateSchema>,
    pub bone_count: Option<usize>,
    /// The already-sampled current fighter pose.  Lifecycle callbacks are
    /// transactional, so retaining this private snapshot cannot observe or
    /// mutate live match state.
    pub pose: Option<crate::collision::bones::Pose>,
    /// Immutable snapshot of generic entities visible to this callback.
    pub entities: Option<game::entity::EntityStore>,
    pub entity_owner_port: Option<u8>,
}

impl LifecycleHost {
    pub(crate) fn new(
        fighter: &game::Fighter,
        movement: Option<&Movement>,
        context: JsonValue,
        native: super::lifecycle::NativeContext<'_>,
        data: Option<&FighterData>,
        rules: Option<&Rules>,
    ) -> Result<Self, StarError> {
        let resources = ResourceCache::build(data, rules, false)
            .map_err(|error| StarError::Host(error.to_string()))?;
        let mut context = context;
        if let JsonValue::Object(values) = &mut context {
            values
                .entry("input")
                .or_insert_with(|| JsonValue::Object(Map::new()));
        }
        Ok(Self {
            fighter: fighter.clone(),
            movement: movement.copied(),
            context,
            pre_landing: native.pre_landing.unwrap_or(fighter).clone(),
            geometry: native.geometry.cloned(),
            helper_params: data.map(|data| NativeHelperParams {
                attributes: data.movement.physics(),
                locomotion: data.locomotion,
                jump_vertical_velocity: data.movement.jump_vertical_velocity,
            }),
            resources: Arc::new(resources),
            persistent: fighter.script_state.clone(),
            action_state: fighter.action_state.clone(),
            owned_resource: None,
            callback_owner: None,
            metadata: None,
            persistent_schema: None,
            action_schema: None,
            bone_count: data.map(|data| data.bones.len()),
            pose: data.and_then(|data| crate::game::simulation::pose(fighter, data).ok()),
            entities: native.entities.cloned(),
            entity_owner_port: native.entity_owner_port,
        })
    }

    /// Hot path constructor used once the match-level resource index is
    /// available. It deliberately accepts the shared cache so dispatch does
    /// not walk or sanitize resource trees again.
    pub(crate) fn new_with_cache(
        fighter: &game::Fighter,
        movement: Option<&Movement>,
        context: JsonValue,
        native: super::lifecycle::NativeContext<'_>,
        data: Option<&FighterData>,
        resources: Arc<ResourceCache>,
    ) -> Self {
        let mut context = context;
        if let JsonValue::Object(values) = &mut context {
            values
                .entry("input")
                .or_insert_with(|| JsonValue::Object(Map::new()));
        }
        Self {
            fighter: fighter.clone(),
            movement: movement.copied(),
            context,
            pre_landing: native.pre_landing.unwrap_or(fighter).clone(),
            geometry: native.geometry.cloned(),
            helper_params: data.map(|data| NativeHelperParams {
                attributes: data.movement.physics(),
                locomotion: data.locomotion,
                jump_vertical_velocity: data.movement.jump_vertical_velocity,
            }),
            resources,
            persistent: fighter.script_state.clone(),
            action_state: fighter.action_state.clone(),
            owned_resource: None,
            callback_owner: None,
            metadata: None,
            persistent_schema: None,
            action_schema: None,
            bone_count: data.map(|data| data.bones.len()),
            pose: data.and_then(|data| crate::game::simulation::pose(fighter, data).ok()),
            entities: native.entities.cloned(),
            entity_owner_port: native.entity_owner_port,
        }
    }

    fn entity_path(&self, path: &str) -> Option<NativeValue> {
        Self::project_entity_path(self.entities.as_ref()?, self.entity_owner_port?, path)
    }

    pub(crate) fn project_entity_path(
        entities: &game::entity::EntityStore,
        owner_port: u8,
        path: &str,
    ) -> Option<NativeValue> {
        let rest = path.strip_prefix("context.entity_at_index[")?;
        let (ordinal, field) = rest.split_once("]")?;
        let ordinal = ordinal.parse::<u8>().ok()?;
        let payload = entities.payload_for_owner(owner_port, ordinal)?;
        match field {
            "" => Some(NativeValue::Object(NativeObject {
                kind: NativeKind::Value,
                path: path.into(),
            })),
            ".available" => Some(NativeValue::Bool(true)),
            ".lifecycle" => Some(NativeValue::String("active".into())),
            ".motion_state" => Some(NativeValue::Int(i64::from(payload.motion_state))),
            ".position" => Some(NativeValue::Vec2([
                payload.position[0],
                payload.position[1],
            ])),
            ".velocity" => Some(NativeValue::Vec2([
                payload.velocity[0],
                payload.velocity[1],
            ])),
            ".facing" => Some(NativeValue::F32(payload.facing)),
            _ => None,
        }
    }

    /// Attach immutable definition metadata retained by the program cache.
    /// This keeps state schemas available without cloning or reparsing them
    /// on each callback.
    pub(crate) fn with_metadata(
        mut self,
        metadata: Arc<crate::game::script::definition::FighterDefinition>,
    ) -> Self {
        self.metadata = Some(metadata);
        let persistent_defaults = self
            .persistent_schema()
            .map(|schema| {
                schema
                    .fields
                    .iter()
                    .map(|(key, field)| (key.clone(), field.default.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for (key, default) in persistent_defaults {
            self.persistent.entry(key).or_insert(default);
        }
        let action_defaults = self
            .action_schema()
            .map(|schema| {
                schema
                    .fields
                    .iter()
                    .map(|(key, field)| (key.clone(), field.default.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for (key, default) in action_defaults {
            self.action_state.entry(key).or_insert(default);
        }
        self
    }

    fn persistent_schema(&self) -> Option<&StateSchema> {
        self.metadata
            .as_deref()
            .map(|metadata| &metadata.state)
            .or(self.persistent_schema.as_ref())
    }

    fn action_schema(&self) -> Option<&StateSchema> {
        self.metadata
            .as_deref()
            .map(|metadata| &metadata.action_state)
            .or(self.action_schema.as_ref())
    }

    pub(crate) fn commit_into(
        &mut self,
        fighter: &mut game::Fighter,
        movement: Option<&mut Movement>,
    ) -> Result<(), StarError> {
        // Validate both destinations before committing either side. This
        // keeps a malformed movement projection from partially committing the
        // fighter state.
        if let Some(source) = self.movement.as_ref() {
            lifecycle_movement::validate_native(source)
                .map_err(|error| StarError::Host(error.to_string()))?;
        }
        lifecycle_state::validate(&self.fighter)
            .map_err(|error| StarError::Host(error.to_string()))?;
        lifecycle_state::validate_state(&self.persistent)
            .map_err(|error| StarError::Host(error.to_string()))?;
        lifecycle_state::validate_state(&self.action_state)
            .map_err(|error| StarError::Host(error.to_string()))?;
        if let Some(schema) = self.persistent_schema() {
            schema
                .validate(&self.persistent)
                .map_err(|error| StarError::Host(error.to_string()))?;
        }
        if let Some(schema) = self.action_schema() {
            schema
                .validate(&self.action_state)
                .map_err(|error| StarError::Host(error.to_string()))?;
        }
        let mut candidate = self.fighter.clone();
        candidate.script_state = self.persistent.clone();
        candidate.action_state = self.action_state.clone();
        lifecycle_state::commit(&candidate, fighter)
            .map_err(|error| StarError::Host(error.to_string()))?;
        if let (Some(source), Some(destination)) = (self.movement, movement) {
            lifecycle_movement::commit(&source, destination)
                .map_err(|error| StarError::Host(error.to_string()))?;
        }
        Ok(())
    }

    fn fighter_path(&self, path: &str) -> Result<NativeValue, StarError> {
        if path == "fighter.state" {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::State,
                path: path.into(),
            }));
        }
        if path == "fighter.action_state" {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::State,
                path: path.into(),
            }));
        }
        if path == "fighter.previous_input" || path == "fighter.input" {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::Input,
                path: path.into(),
            }));
        }
        match path {
            "fighter.action" => {
                return Ok(NativeValue::String(action_reference(
                    self.fighter.action,
                    &self.resources,
                )));
            }
            "fighter.action_frame" => {
                return Ok(NativeValue::Int(i64::from(self.fighter.action_frame)));
            }
            "fighter.position" => return Ok(NativeValue::Vec2(self.fighter.position)),
            "fighter.velocity" => return Ok(NativeValue::Vec2(self.fighter.velocity)),
            "fighter.knockback" => return Ok(NativeValue::Vec2(self.fighter.knockback)),
            "fighter.ground_velocity" => return Ok(NativeValue::F32(self.fighter.ground_velocity)),
            "fighter.ground_knockback" => {
                return Ok(NativeValue::F32(self.fighter.ground_knockback));
            }
            "fighter.depth" => return Ok(NativeValue::F32(self.fighter.depth)),
            "fighter.facing" => return Ok(NativeValue::F32(self.fighter.facing)),
            "fighter.percent" => return Ok(NativeValue::F32(self.fighter.percent)),
            "fighter.hitlag" => return Ok(NativeValue::F32(self.fighter.hitlag)),
            "fighter.hitstun" => return Ok(NativeValue::Int(i64::from(self.fighter.hitstun))),
            "fighter.grounded" => return Ok(NativeValue::Bool(self.fighter.grounded)),
            "fighter.short_hop" => return Ok(NativeValue::Bool(self.fighter.short_hop)),
            "fighter.fast_fall" => return Ok(NativeValue::Bool(self.fighter.fast_fall)),
            "fighter.floor_normal" => return list(self.fighter.floor_normal),
            "fighter.ground_line" => {
                return Ok(self
                    .fighter
                    .ground_line
                    .map_or(NativeValue::None, |x| NativeValue::Int(x as i64)));
            }
            _ => {}
        }
        if let Some(value) = state_get(path, "fighter.state", &self.persistent)
            .or_else(|| state_get(path, "fighter.action_state", &self.action_state))
        {
            return Ok(value);
        }
        if path == "fighter.locomotion"
            || path == "fighter.shield"
            || path == "fighter.aerial"
            || path == "fighter.ecb"
            || path == "fighter.flags"
        {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::Value,
                path: path.into(),
            }));
        }
        // ECB shapes are immutable, scoped snapshots. Keep the shape and its
        // coordinate vectors behind the host path so Pon cannot retain a Rust
        // reference into the transactional fighter clone.
        if let Some(value) = Self::project_ecb_path(&self.fighter.ecb, path) {
            return Ok(value);
        }
        // Nested native fields that are commonly read by policy callbacks.
        match path {
            "fighter.flags.reflecting" => Ok(NativeValue::Bool(self.fighter.shield.reflecting)),
            "fighter.shield.reflecting" => Ok(NativeValue::Bool(self.fighter.shield.reflecting)),
            "fighter.shield.health" => Ok(NativeValue::F32(self.fighter.shield.health)),
            "fighter.aerial.mobility" => Ok(NativeValue::F32(self.fighter.aerial.mobility)),
            "fighter.aerial.landing_lag" => Ok(self
                .fighter
                .aerial
                .landing_lag
                .map_or(NativeValue::None, NativeValue::F32)),
            "fighter.aerial.allow_interrupt" => {
                Ok(NativeValue::Bool(self.fighter.aerial.allow_interrupt))
            }
            "fighter.locomotion.jumps_used" => Ok(NativeValue::Int(i64::from(
                self.fighter.locomotion.jumps_used,
            ))),
            "fighter.locomotion.side_special_b_age" => Ok(NativeValue::Int(i64::from(
                self.fighter.locomotion.side_special_b_age,
            ))),
            "fighter.locomotion.up_special_b_age" => Ok(NativeValue::Int(i64::from(
                self.fighter.locomotion.up_special_b_age,
            ))),
            "fighter.locomotion.tilt_y_age" => Ok(NativeValue::Int(i64::from(
                self.fighter.locomotion.tilt_y_age,
            ))),
            "fighter.locomotion.jump_input" => Ok(NativeValue::String(
                format!("{:?}", self.fighter.locomotion.jump_input).to_ascii_lowercase(),
            )),
            _ => Err(StarError::Host(format!("unknown fighter field `{path}`"))),
        }
    }

    fn project_ecb_path(ecb: &crate::collision::ecb::State, path: &str) -> Option<NativeValue> {
        let (shape_path, shape) = if path.starts_with("fighter.ecb.current") {
            ("fighter.ecb.current", &ecb.current)
        } else if path.starts_with("fighter.ecb.previous") {
            ("fighter.ecb.previous", &ecb.previous)
        } else {
            return None;
        };
        if path == shape_path {
            return Some(NativeValue::Object(NativeObject {
                kind: NativeKind::Value,
                path: shape_path.into(),
            }));
        }
        let coordinate = match path.strip_prefix(shape_path)?.strip_prefix('.')? {
            "top" => shape.top,
            "bottom" => shape.bottom,
            "left" => shape.left,
            "right" => shape.right,
            _ => return None,
        };
        Some(NativeValue::Vec2(coordinate))
    }
    fn context_path(&self, path: &str) -> Result<NativeValue, StarError> {
        if let Some(value) = self.entity_path(path) {
            return Ok(value);
        }
        // Async MoveContext fields are rebound for every pre/post step. Keep
        // them as scoped host objects so `action.fighter.state` and related
        // writes use the same transactional fighter host as ordinary hooks.
        if path == "context.fighter" {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::Fighter,
                path: "fighter".into(),
            }));
        }
        if path == "context.action" {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::Value,
                path: "context.action".into(),
            }));
        }
        if path == "context.next_action" || path == "next_action" {
            let value = json_path(&self.context, "event.to")
                .or_else(|| json_path(&self.context, "next_action"))
                .ok_or_else(|| {
                    StarError::Host("next_action is unavailable in this callback".into())
                })?;
            return json_to_native(value, path);
        }
        if path == "context.input" {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::Input,
                path: path.into(),
            }));
        }
        if path == "context" {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::Context,
                path: path.into(),
            }));
        }
        // Fighter class parameters are immutable registration metadata. Keep
        // them behind scoped handles so callbacks can read nested records
        // without cloning the definition into the VM on every dispatch.
        if path == "context.parameters" {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::Value,
                path: path.into(),
            }));
        }
        if let Some(relative) = path.strip_prefix("context.parameters.") {
            if let Some(value) = self
                .metadata
                .as_deref()
                .and_then(|metadata| project_parameter_path(&metadata.parameters, relative))
            {
                return parameter_to_native(value, path);
            }
            return Err(StarError::Host(format!("unknown context field `{path}`")));
        }
        // Resource records are path-backed. Resolve these before the generic
        // JSON context so a lookup such as `c.resource("neutral").frames`
        // remains lazy and does not materialize the resource tree.
        if let Some(resource) = path.strip_prefix("context.") {
            if let Some(move_id) = self.resources.projectile_move_id(resource) {
                return Ok(NativeValue::Int(i64::from(move_id)));
            }
            if let Some(resource) = resource.strip_suffix(".length")
                && let Some(length) = self.resources.array_length_path(resource)
            {
                return Ok(NativeValue::Int(length as i64));
            }
            if let Some(value) = self.resources.value_path(resource) {
                return match value {
                    JsonValue::Object(_) | JsonValue::Array(_) => {
                        Ok(NativeValue::Object(NativeObject {
                            kind: NativeKind::Value,
                            path: path.into(),
                        }))
                    }
                    _ => json_to_native(value, path),
                };
            }
            // The cache indexes every declared path, but attack frame trees
            // intentionally stop at the attack root. Resolve projected
            // scalar metadata such as `attack.frame_count` from that root.
            let parts: Vec<_> = resource.split('.').collect();
            for split in (1..parts.len()).rev() {
                let root = parts[..split].join(".");
                let suffix = parts[split..].join(".");
                if let Some(root_value) = self.resources.value(&root)
                    && let Some(value) = json_path(root_value, &suffix)
                {
                    return json_to_native(value, path);
                }
            }
        }
        let relative = path.strip_prefix("context.").unwrap_or(path);
        if let Some(value) = json_path(&self.context, relative) {
            return json_to_native(value, path);
        }
        Err(StarError::Host(format!("unknown context field `{path}`")))
    }

    fn set_fighter(&mut self, path: &str, value: NativeValue) -> Result<(), StarError> {
        match path {
            "fighter.position" => self.fighter.position = vec2(value, "position")?,
            "fighter.velocity" => {
                let velocity = vec2(value, "velocity")?;
                self.fighter.velocity = velocity;
                if let Some(movement) = &mut self.movement {
                    lifecycle_movement::set_velocity(movement, velocity[0], velocity[1])
                        .map_err(StarError::Host)?;
                }
            }
            "fighter.knockback" => self.fighter.knockback = vec2(value, "knockback")?,
            "fighter.depth" => self.fighter.depth = f32_value(value, "depth")?,
            "fighter.ground_velocity" => {
                let value = f32_value(value, "ground_velocity")?;
                self.fighter.ground_velocity = value;
                if let Some(movement) = &mut self.movement {
                    movement.ground_velocity = value;
                }
            }
            "fighter.ground_knockback" => {
                let value = f32_value(value, "ground_knockback")?;
                self.fighter.ground_knockback = value;
                if let Some(movement) = &mut self.movement {
                    movement.ground_knockback = value;
                }
            }
            "fighter.facing" => self.fighter.facing = f32_value(value, "facing")?,
            "fighter.percent" => self.fighter.percent = f32_value(value, "percent")?,
            "fighter.hitlag" => self.fighter.hitlag = f32_value(value, "hitlag")?,
            "fighter.hitstun" => {
                self.fighter.hitstun = u32::try_from(int_value(value, "hitstun")?)
                    .map_err(|_| StarError::Host("hitstun is out of range".into()))?
            }
            "fighter.grounded" => self.fighter.grounded = bool_value(value, "grounded")?,
            "fighter.short_hop" => self.fighter.short_hop = bool_value(value, "short_hop")?,
            "fighter.fast_fall" => self.fighter.fast_fall = bool_value(value, "fast_fall")?,
            "fighter.flags.reflecting" => {
                let value = bool_value(value, "flags.reflecting")?;
                self.fighter.shield.reflecting = value
            }
            "fighter.shield.reflecting" => {
                self.fighter.shield.reflecting = bool_value(value, "shield.reflecting")?
            }
            "fighter.shield.health" => {
                self.fighter.shield.health = f32_value(value, "shield.health")?
            }
            "fighter.floor_normal" => {
                let value = vec3(value, "floor_normal")?;
                self.fighter.floor_normal = value;
                if let Some(movement) = &mut self.movement {
                    movement.floor_normal = value;
                }
            }
            "fighter.ground_line" => {
                self.fighter.ground_line = match value {
                    NativeValue::None => None,
                    NativeValue::Int(value) => Some(usize::try_from(value).map_err(|_| {
                        StarError::Host("ground_line must be a non-negative line id".into())
                    })?),
                    other => {
                        return Err(StarError::Host(format!(
                            "ground_line expects None or an integer, got {other:?}"
                        )));
                    }
                };
            }
            "fighter.action_frame" => {
                self.fighter.action_frame = u32::try_from(int_value(value, "action_frame")?)
                    .map_err(|_| StarError::Host("action_frame is out of range".into()))?
            }
            _ if path.starts_with("fighter.state.") => {
                let schema = self
                    .metadata
                    .as_deref()
                    .map(|metadata| &metadata.state)
                    .or(self.persistent_schema.as_ref());
                state_set(path, "fighter.state", value, &mut self.persistent, schema)?
            }
            _ if path.starts_with("fighter.action_state.") => {
                let schema = self
                    .metadata
                    .as_deref()
                    .map(|metadata| &metadata.action_state)
                    .or(self.action_schema.as_ref());
                state_set(
                    path,
                    "fighter.action_state",
                    value,
                    &mut self.action_state,
                    schema,
                )?
            }
            "fighter.aerial.mobility" => {
                self.fighter.aerial.mobility = f32_value(value, "aerial.mobility")?
            }
            "fighter.aerial.allow_interrupt" => {
                self.fighter.aerial.allow_interrupt = bool_value(value, "aerial.allow_interrupt")?
            }
            "fighter.locomotion.jumps_used" => {
                self.fighter.locomotion.jumps_used =
                    u8::try_from(int_value(value, "locomotion.jumps_used")?).map_err(|_| {
                        StarError::Host("locomotion.jumps_used is out of range".into())
                    })?
            }
            // These ages are native input latches.  They are exposed to
            // fighter callbacks for source-style command bookkeeping, so
            // writes must round-trip through the same transactional host.
            "fighter.locomotion.side_special_b_age" => {
                self.fighter.locomotion.side_special_b_age =
                    u8_value(value, "locomotion.side_special_b_age")?
            }
            "fighter.locomotion.up_special_b_age" => {
                self.fighter.locomotion.up_special_b_age =
                    u8_value(value, "locomotion.up_special_b_age")?
            }
            "fighter.locomotion.tilt_y_age" => {
                self.fighter.locomotion.tilt_y_age = u8_value(value, "locomotion.tilt_y_age")?
            }
            "fighter.locomotion.jump_input" => {
                self.fighter.locomotion.jump_input = match value {
                    NativeValue::String(value) => match value.to_ascii_lowercase().as_str() {
                        "buttons" => crate::fighter::locomotion::JumpInput::Buttons,
                        "stick" => crate::fighter::locomotion::JumpInput::Stick,
                        "c_stick" | "cstick" => crate::fighter::locomotion::JumpInput::CStick,
                        _ => {
                            return Err(StarError::Host("locomotion.jump_input is invalid".into()));
                        }
                    },
                    other => {
                        return Err(StarError::Host(format!(
                            "locomotion.jump_input expects a string, got {other:?}"
                        )));
                    }
                };
            }
            "fighter.action" => {
                return Err(StarError::Host(
                    "fighter.action is read-only; use change_action".into(),
                ));
            }
            _ => {
                return Err(StarError::Host(format!(
                    "unsupported fighter write `{path}`"
                )));
            }
        }
        Ok(())
    }

    fn call_fighter(&mut self, path: &str, args: &[NativeValue]) -> Result<NativeValue, StarError> {
        match path {
            "fighter.part_position" => {
                if args.len() != 1 {
                    return Err(StarError::Host(
                        "part_position expects one numeric fighter-part ordinal".into(),
                    ));
                }
                let part = match args[0] {
                    NativeValue::Int(value) => usize::try_from(value).map_err(|_| {
                        StarError::Host("fighter part ordinal must be non-negative".into())
                    })?,
                    _ => {
                        return Err(StarError::Host(
                            "fighter part ordinal must be an integer".into(),
                        ));
                    }
                };
                if self.bone_count.is_some_and(|count| part >= count) {
                    return Err(StarError::Host(
                        "fighter part ordinal is outside the loaded skeleton".into(),
                    ));
                }
                let pose = self
                    .pose
                    .as_ref()
                    .ok_or_else(|| StarError::Host("fighter pose is unavailable".into()))?;
                let matrix = pose
                    .world_matrix(part)
                    .map_err(|error| StarError::Host(error.to_string()))?;
                Ok(list(crate::collision::bones::transform_point(
                    matrix, [0.0; 3],
                ))?)
            }
            "fighter.spawn_article" => {
                if !(3..=4).contains(&args.len()) {
                    return Err(StarError::Host(
                        "spawn_article expects article_id, position, and facing, or angle and speed".into(),
                    ));
                }
                if self.fighter.pending_article_spawns.len()
                    + self.fighter.pending_projectiles.len()
                    >= crate::game::projectile::MAX_PENDING_PROJECTILES
                {
                    return Err(StarError::Host("article emission queue is full".into()));
                }
                let article_id = match args[0] {
                    NativeValue::Int(value) => {
                        let value = u16::try_from(value).map_err(|_| {
                            StarError::Host("article_id must be a non-negative u16".into())
                        })?;
                        crate::game::script::resources::ArticleId(value)
                    }
                    NativeValue::String(_) => {
                        return Err(StarError::Host(
                            "spawn_article article_id must be numeric, not a string".into(),
                        ));
                    }
                    _ => return Err(StarError::Host("article_id must be an integer".into())),
                };
                if self.resources.article(article_id).is_none() {
                    return Err(StarError::Host(format!(
                        "article {article_id:?} is unavailable or invalid"
                    )));
                }
                let position = vec3(args[1].clone(), "article position")?;
                let launch = if args.len() == 3 {
                    crate::game::projectile::ArticleLaunch::Facing(f32_value(
                        args[2].clone(),
                        "article facing",
                    )?)
                } else {
                    crate::game::projectile::ArticleLaunch::Explicit {
                        angle: f32_value(args[2].clone(), "article angle")?,
                        speed: f32_value(args[3].clone(), "article speed")?,
                    }
                };
                self.fighter.pending_article_spawns.push(
                    crate::game::projectile::PendingArticleSpawn {
                        article_id,
                        position,
                        launch,
                    },
                );
                Ok(NativeValue::None)
            }
            "fighter.emit_projectile" => {
                if args.len() != 7 {
                    return Err(StarError::Host(
                        "emit_projectile expects kind, position, angle, speed, lifetime, hitboxes, and move_id".into(),
                    ));
                }
                if self.fighter.pending_projectiles.len()
                    + self.fighter.pending_article_spawns.len()
                    >= crate::game::projectile::MAX_PENDING_PROJECTILES
                {
                    return Err(StarError::Host("projectile emission queue is full".into()));
                }
                let kind = match &args[0] {
                    NativeValue::String(value) => match value.as_str() {
                        "laser" | "fox_laser" => crate::game::projectile::ProjectileKind::FoxLaser,
                        "falco_laser" => crate::game::projectile::ProjectileKind::FalcoLaser,
                        other => {
                            return Err(StarError::Host(format!(
                                "unknown projectile kind {other:?}"
                            )));
                        }
                    },
                    _ => return Err(StarError::Host("projectile kind must be a string".into())),
                };
                let position = vec3(args[1].clone(), "projectile position")?;
                let angle = f32_value(args[2].clone(), "projectile angle")?;
                let speed = f32_value(args[3].clone(), "projectile speed")?;
                let lifetime = f32_value(args[4].clone(), "projectile lifetime")?;
                let hitbox_path = match &args[5] {
                    NativeValue::Object(object) if object.kind == NativeKind::Value => {
                        object.path.strip_prefix("context.").ok_or_else(|| {
                            StarError::Host("projectile hitboxes must be a resource handle".into())
                        })?
                    }
                    _ => {
                        return Err(StarError::Host(
                            "projectile hitboxes must be a resource handle".into(),
                        ));
                    }
                };
                let hitboxes = self
                    .resources
                    .projectile_hitboxes(hitbox_path)
                    .ok_or_else(|| {
                        StarError::Host(
                            "projectile hitboxes resource is unavailable or invalid".into(),
                        )
                    })?
                    .to_vec();
                let move_id = match args[6] {
                    NativeValue::Int(value) => u16::try_from(value).map_err(|_| {
                        StarError::Host("projectile move_id is out of range".into())
                    })?,
                    _ => {
                        return Err(StarError::Host(
                            "projectile move_id must be an integer".into(),
                        ));
                    }
                };
                if let Some(article_id) =
                    self.resources
                        .legacy_article(kind, hitbox_path, lifetime, move_id)
                {
                    self.fighter.pending_article_spawns.push(
                        crate::game::projectile::PendingArticleSpawn {
                            article_id,
                            position,
                            launch: crate::game::projectile::ArticleLaunch::Explicit {
                                angle,
                                speed,
                            },
                        },
                    );
                } else {
                    self.fighter.pending_projectiles.push(
                        crate::game::projectile::PendingProjectile {
                            kind,
                            position,
                            angle,
                            speed,
                            lifetime,
                            hitboxes,
                            move_id,
                        },
                    );
                }
                Ok(NativeValue::None)
            }
            "fighter.spawn_special_effect" => {
                if !(1..=2).contains(&args.len()) {
                    return Err(StarError::Host(
                        "spawn_special_effect expects one or two arguments".into(),
                    ));
                }
                let resource = match args.first() {
                    Some(NativeValue::String(value)) if !value.is_empty() => value.clone(),
                    _ => {
                        return Err(StarError::Host(
                            "spawn_special_effect requires a resource path".into(),
                        ));
                    }
                };
                if self.resources.value_path(&resource).is_none() {
                    return Err(StarError::Host(format!(
                        "effect resource `{resource}` is unavailable"
                    )));
                }
                let bone = match args.get(1).unwrap_or(&NativeValue::None) {
                    NativeValue::None => None,
                    NativeValue::Int(value) => {
                        let bone = usize::try_from(*value)
                            .ok()
                            .and_then(|value| u16::try_from(value).ok())
                            .ok_or_else(|| StarError::Host("effect bone is out of range".into()))?;
                        if self
                            .bone_count
                            .is_some_and(|count| usize::from(bone) >= count)
                        {
                            return Err(StarError::Host(
                                "effect bone is outside the loaded skeleton".into(),
                            ));
                        }
                        Some(bone)
                    }
                    _ => {
                        return Err(StarError::Host(
                            "effect bone must be an integer or None".into(),
                        ));
                    }
                };
                let id = self
                    .fighter
                    .effects
                    .spawn(resource, bone)
                    .map_err(|error| StarError::Host(error.into()))?;
                Ok(NativeValue::Int(i64::from(id)))
            }
            "fighter.clear_special_effect" => {
                if !args.is_empty() {
                    return Err(StarError::Host(
                        "clear_special_effect expects no arguments".into(),
                    ));
                }
                self.fighter.effects.clear();
                Ok(NativeValue::None)
            }
            "fighter.change_action" | "context.next_action" | "next_action" => {
                if args.len() > 4 {
                    return Err(StarError::Host(
                        "fighter.change_action expects at most four arguments".into(),
                    ));
                }
                let action = action_arg(args.first())?;
                let old_frame = self.fighter.action_frame;
                let preserve = args
                    .get(1)
                    .filter(|value| !matches!(value, NativeValue::None))
                    .map(|value| bool_value(value.clone(), "preserve_state"))
                    .transpose()?
                    .unwrap_or(false);
                let keep_frame = args
                    .get(2)
                    .filter(|value| !matches!(value, NativeValue::None))
                    .map(|value| bool_value(value.clone(), "keep_frame"))
                    .transpose()?
                    .unwrap_or(false);
                let preserve_fields =
                    decode_preserve_fields(args.get(3), self.action_schema(), preserve)?;
                let action = crate::game::script::parse_action(&action)
                    .ok_or_else(|| StarError::Host(format!("unsupported action `{action}`")))?;
                let prior_state = self.action_state.clone();
                let reset_state = self
                    .action_schema()
                    .map(StateSchema::defaults)
                    .unwrap_or_default();
                if self.fighter.script_events.pending_transitions.len()
                    >= crate::game::script::events::MAX_PENDING_TRANSITIONS
                {
                    return Err(StarError::Host(
                        "native action transition queue is full".into(),
                    ));
                }
                let retains_move = preserve
                    && self.fighter.script_events.pending_move_selection.is_none()
                    && self.fighter.script_events.active_move.is_some_and(|owner| {
                        self.callback_owner
                            .is_none_or(|callback_owner| callback_owner == owner.behavior_index)
                            && self.metadata.as_deref().is_some_and(|metadata| {
                                metadata.behavior_owns_action(owner.behavior_index, action)
                            })
                    });
                if let Some(owner) = self.callback_owner
                    && self
                        .resources
                        .program()
                        .is_some_and(|program| program.moves().is_registered_behavior(owner))
                    && self
                        .metadata
                        .as_deref()
                        .is_some_and(|metadata| metadata.behavior_owns_action(owner, action))
                    && !retains_move
                {
                    if let Some(pending) = self.fighter.script_events.pending_move_selection {
                        if pending.behavior_index != owner {
                            return Err(StarError::Host(
                                "pending move selection belongs to another behavior".into(),
                            ));
                        }
                    } else {
                        self.fighter
                            .script_events
                            .stage_move_selection(
                                crate::game::script::move_registry::MoveEntry {
                                    behavior_index: owner,
                                    canonical: None,
                                },
                                action,
                            )
                            .map_err(|error| StarError::Host(error.into()))?;
                    }
                }
                crate::game::simulation::enter(&mut self.fighter, action);
                // `enter` stages the transition before action-local writes.
                // Record the two host options immediately so any subsequent
                // native writes in this callback observe the destination
                // transition metadata without re-entering the VM.
                self.fighter
                    .script_events
                    .set_last_transition_flags(preserve, keep_frame);
                // A running class move may explicitly retain ownership while
                // changing its native phase. This is tied to the active
                // owner and the declared preserve option; ordinary action
                // changes cancel the move at the transition boundary.
                self.fighter
                    .script_events
                    .set_last_transition_move_retention(retains_move);
                self.action_state = action_state_after_change(
                    &prior_state,
                    reset_state,
                    preserve,
                    preserve_fields.as_deref(),
                );
                if keep_frame {
                    self.fighter.action_frame = old_frame;
                }
                self.sync_movement_from_fighter();
                Ok(NativeValue::None)
            }
            "fighter.set_velocity" => {
                let velocity = if args.len() == 1 {
                    vec2(args[0].clone(), "velocity")?
                } else {
                    [
                        f32_value(
                            args.first().cloned().unwrap_or(NativeValue::None),
                            "velocity.x",
                        )?,
                        f32_value(
                            args.get(1).cloned().unwrap_or(NativeValue::None),
                            "velocity.y",
                        )?,
                    ]
                };
                self.set_fighter("fighter.velocity", NativeValue::Vec2(velocity))?;
                Ok(NativeValue::None)
            }
            "fighter.set_motion_angle" => {
                if args.len() != 1 {
                    return Err(StarError::Host("set_motion_angle expects one angle".into()));
                }
                let angle = f32_value(args[0].clone(), "motion angle")?;
                let cosine = crate::compat::math::trig::cosf(angle);
                let sine = crate::compat::math::trig::sinf(angle);
                self.fighter.script_events.motion_binding =
                    crate::game::script::motion::MotionBinding {
                        facing: self.fighter.facing,
                        cosine,
                        sine,
                        ground_scale: 1.0,
                    };
                Ok(NativeValue::None)
            }
            "fighter.launch_from_angle" => {
                if args.len() != 2 {
                    return Err(StarError::Host(
                        "launch_from_angle expects speed and angle".into(),
                    ));
                }
                let speed = f32_value(args[0].clone(), "launch speed")?;
                let angle = f32_value(args[1].clone(), "launch angle")?;
                let cosine = crate::compat::math::trig::cosf(angle);
                let sine = crate::compat::math::trig::sinf(angle);
                let facing = self.fighter.facing;
                self.fighter.script_events.motion_binding =
                    crate::game::script::motion::MotionBinding {
                        facing,
                        cosine,
                        sine,
                        ground_scale: 1.0,
                    };
                self.set_fighter(
                    "fighter.velocity",
                    NativeValue::Vec2(source_launch_velocity(speed, cosine, sine, facing)),
                )?;
                Ok(NativeValue::None)
            }
            "fighter.set_motion_binding" => {
                let Some(NativeValue::Dict(values)) = args.first() else {
                    return Err(StarError::Host(
                        "set_motion_binding expects a binding dictionary".into(),
                    ));
                };
                if args.len() != 1 {
                    return Err(StarError::Host(
                        "set_motion_binding expects one argument".into(),
                    ));
                }
                for name in values.keys() {
                    if !matches!(name.as_str(), "facing" | "cosine" | "sine" | "ground_scale") {
                        return Err(StarError::Host(format!(
                            "unknown motion binding field `{name}`"
                        )));
                    }
                }
                let number = |name: &str| {
                    f32_value(values.get(name).cloned().unwrap_or(NativeValue::None), name)
                };
                let binding = crate::game::script::motion::MotionBinding {
                    facing: number("facing")?,
                    cosine: number("cosine")?,
                    sine: number("sine")?,
                    ground_scale: number("ground_scale")?,
                };
                self.fighter.script_events.motion_binding = binding;
                Ok(NativeValue::None)
            }
            "fighter.restore_pre_landing" => {
                self.fighter = self.pre_landing.clone();
                self.sync_movement_from_fighter();
                Ok(NativeValue::None)
            }
            "fighter.max_jumps" => {
                let max = self
                    .helper_params
                    .as_ref()
                    .and_then(|params| params.locomotion.as_ref())
                    .map(|x| x.max_jumps)
                    .ok_or_else(|| StarError::Host("max_jumps requires fighter data".into()))?;
                self.fighter.locomotion.jumps_used = max;
                Ok(NativeValue::None)
            }
            "fighter.enter_fall_special" => {
                let mobility = f32_value(
                    args.first().cloned().unwrap_or(NativeValue::None),
                    "mobility",
                )?;
                let lag = args
                    .get(1)
                    .map(|x| f32_value(x.clone(), "landing_lag"))
                    .transpose()?;
                let max = self
                    .helper_params
                    .as_ref()
                    .and_then(|params| params.locomotion.as_ref())
                    .map(|x| x.max_jumps)
                    .ok_or_else(|| {
                        StarError::Host("enter_fall_special requires fighter data".into())
                    })?;
                crate::game::simulation::enter(&mut self.fighter, game::Action::FallSpecial);
                let (grounded, ground_velocity) =
                    fall_special_ground_state(self.fighter.grounded, self.fighter.ground_velocity);
                self.fighter.grounded = grounded;
                self.fighter.ground_velocity = ground_velocity;
                if let Some(movement) = &mut self.movement {
                    movement.ground_velocity = ground_velocity;
                }
                self.fighter.aerial.allow_interrupt = true;
                self.fighter.aerial.mobility = mobility;
                self.fighter.aerial.landing_lag = lag;
                self.fighter.locomotion.jumps_used = max;
                self.sync_movement_from_fighter();
                Ok(NativeValue::None)
            }
            "fighter.enter_landing_special" => {
                let end = f32_value(
                    args.first().cloned().unwrap_or(NativeValue::None),
                    "animation_end",
                )?;
                let lag = f32_value(
                    args.get(1).cloned().unwrap_or(NativeValue::None),
                    "landing_lag",
                )?;
                crate::fighter::helpers::enter_landing_fall_special(&mut self.fighter, end, lag);
                self.sync_movement_from_fighter();
                Ok(NativeValue::None)
            }
            "fighter.action_finished" => {
                if let Some(frame_count) = json_path(&self.context, "action_metadata.frame_count")
                    .and_then(JsonValue::as_u64)
                {
                    return Ok(NativeValue::Bool(
                        self.fighter.action_frame as u64 >= frame_count,
                    ));
                }
                let action =
                    action_reference(self.fighter.action, &self.resources).to_ascii_lowercase();
                let paths = [
                    action.clone(),
                    format!("attacks.{action}"),
                    format!("specials.{action}"),
                ];
                let finished = paths.iter().any(|path| {
                    let frames = self.resources.frame_count(path);
                    frames != 0 && self.fighter.action_frame as usize >= frames
                });
                Ok(NativeValue::Bool(finished))
            }
            "fighter.has_complete_animation" => {
                if args.len() != 1 {
                    return Err(StarError::Host(
                        "has_complete_animation expects one motion state id".into(),
                    ));
                }
                let state_id = match args[0] {
                    NativeValue::Int(value) => u32::try_from(value).map_err(|_| {
                        StarError::Host("motion state id must be a non-negative integer".into())
                    })?,
                    _ => {
                        return Err(StarError::Host(
                            "motion state id must be a non-negative integer".into(),
                        ));
                    }
                };
                Ok(NativeValue::Bool(
                    self.resources.has_complete_animation(state_id),
                ))
            }
            "fighter.special_attribute" => {
                if args.len() != 1 {
                    return Err(StarError::Host(
                        "special_attribute expects one typed attribute id".into(),
                    ));
                }
                let packed = match args[0] {
                    NativeValue::Int(value) => u32::try_from(value).map_err(|_| {
                        StarError::Host(
                            "special attribute id must be a non-negative integer".into(),
                        )
                    })?,
                    _ => {
                        return Err(StarError::Host(
                            "special attribute id must be an integer".into(),
                        ));
                    }
                };
                let layout = u8::try_from(packed >> 16).unwrap_or(0);
                let field_id = packed as u16;
                self.resources
                    .special_attribute(layout, field_id)
                    .map_or(Ok(NativeValue::None), |value| Ok(NativeValue::F32(value)))
            }
            _ => Err(StarError::Host(format!("unknown fighter method `{path}`"))),
        }
    }

    fn controller(&self) -> game::Controller {
        json_path(&self.context, "input")
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or_default()
    }

    fn refresh_movement_view(&mut self) {
        if let Some(movement) = self.movement {
            self.fighter.velocity = [movement.self_velocity[0], movement.self_velocity[1]];
            self.fighter.ground_velocity = movement.ground_velocity;
            self.fighter.ground_knockback = movement.ground_knockback;
            self.fighter.floor_normal = movement.floor_normal;
        }
    }

    fn sync_movement_from_fighter(&mut self) {
        if let Some(movement) = &mut self.movement {
            movement.self_velocity[0] = self.fighter.velocity[0];
            movement.self_velocity[1] = self.fighter.velocity[1];
            movement.ground_velocity = self.fighter.ground_velocity;
            movement.ground_knockback = self.fighter.ground_knockback;
            movement.floor_normal = self.fighter.floor_normal;
        }
    }
}

fn action_reference(
    action: crate::game::Action,
    resources: &super::lifecycle_resources::ResourceCache,
) -> String {
    if let Some(program) = resources.program() {
        if let Some(definition) = program.metadata().action(action)
            && let Some(reference) = definition.action.as_deref()
        {
            return reference
                .strip_prefix("Action.")
                .unwrap_or(reference)
                .to_owned();
        }
        if let Some(reference) = linked_action_reference(&program, action) {
            return reference;
        }
    }
    if let crate::game::Action::Custom(id) = action {
        // A custom action without a linked definition is invalid at resource
        // registration, but keep its wire shape deterministic for diagnostics.
        return format!("Custom.unlinked:{:016x}", id.get());
    }
    format!("{action:?}")
}

fn linked_action_reference(
    program: &super::Program,
    action: crate::game::Action,
) -> Option<String> {
    let candidate = |name: &str, definition: &super::definition::ActionDefinition| {
        let reference = definition.action.as_deref().unwrap_or(name);
        let reference = reference.strip_prefix("Action.").unwrap_or(reference);
        (crate::game::script::parse_action(reference) == Some(action)).then(|| reference.to_owned())
    };
    program
        .metadata()
        .actions
        .iter()
        .find_map(|(name, definition)| candidate(name, definition))
        .or_else(|| {
            program.metadata().behaviors.iter().find_map(|behavior| {
                behavior
                    .actions
                    .iter()
                    .find_map(|(name, definition)| candidate(name, definition))
            })
        })
}

impl NativeHost for LifecycleHost {
    fn get(&mut self, path: &str) -> Result<NativeValue, StarError> {
        if path == "fighter" {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::Fighter,
                path: path.into(),
            }));
        }
        if path.starts_with("fighter.") {
            return self.fighter_path(path);
        }
        if path == "context" {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::Context,
                path: path.into(),
            }));
        }
        if path.starts_with("context.") {
            return self.context_path(path);
        }
        if path == "input" {
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::Input,
                path: path.into(),
            }));
        }
        if path.starts_with("input.") {
            return self.context_path(&format!("context.{path}"));
        }
        Err(StarError::Host(format!("unknown native path `{path}`")))
    }

    fn set(&mut self, path: &str, value: NativeValue) -> Result<(), StarError> {
        if path.starts_with("fighter.") {
            return self.set_fighter(path, value);
        }
        Err(StarError::Host(format!(
            "unsupported native write `{path}`"
        )))
    }

    fn call_named(
        &mut self,
        path: &str,
        args: &[NativeValue],
        named: &BTreeMap<String, NativeValue>,
    ) -> Result<NativeValue, StarError> {
        if let Some(value) = super::builtins::call_named(self, path, args, named)? {
            return Ok(value);
        }
        if named.is_empty() {
            return self.call(path, args);
        }
        let mut positional = args.to_vec();
        match path {
            "fighter.spawn_article" => {
                const NAMES: [&str; 5] = ["article_id", "position", "facing", "angle", "speed"];
                for name in named.keys() {
                    if !NAMES.contains(&name.as_str()) {
                        return Err(StarError::Host(format!(
                            "unknown spawn_article argument `{name}`"
                        )));
                    }
                }
                if !args.is_empty() {
                    return Err(StarError::Host(
                        "spawn_article expects named arguments".into(),
                    ));
                }
                let compact = ["article_id", "position", "facing"];
                let explicit = ["article_id", "position", "angle", "speed"];
                let names = if named.keys().all(|name| compact.contains(&name.as_str()))
                    && compact.iter().all(|name| named.contains_key(*name))
                {
                    &compact[..]
                } else if explicit.iter().all(|name| named.contains_key(*name))
                    && named.keys().all(|name| explicit.contains(&name.as_str()))
                {
                    &explicit[..]
                } else {
                    return Err(StarError::Host(
                        "spawn_article requires article_id, position, facing or angle and speed"
                            .into(),
                    ));
                };
                positional = names
                    .iter()
                    .map(|name| named.get(*name).cloned().expect("checked above"))
                    .collect();
            }
            "fighter.change_action" => {
                for name in named.keys() {
                    if !matches!(
                        name.as_str(),
                        "preserve_state" | "keep_frame" | "preserve_fields"
                    ) {
                        return Err(StarError::Host(format!(
                            "unknown change_action argument `{name}`"
                        )));
                    }
                }
                if let Some(value) = named.get("preserve_state") {
                    while positional.len() < 2 {
                        positional.push(NativeValue::None);
                    }
                    positional[1] = value.clone();
                }
                if let Some(value) = named.get("keep_frame") {
                    while positional.len() < 3 {
                        positional.push(NativeValue::None);
                    }
                    positional[2] = value.clone();
                }
                if let Some(value) = named.get("preserve_fields") {
                    while positional.len() < 4 {
                        positional.push(NativeValue::None);
                    }
                    positional[3] = value.clone();
                }
            }
            "context.pass_as" | "pass_as" => {
                for name in named.keys() {
                    if !matches!(name.as_str(), "action" | "keep_frame") {
                        return Err(StarError::Host(format!(
                            "unknown pass_as argument `{name}`"
                        )));
                    }
                }
                if let Some(value) = named.get("action") {
                    while positional.len() < 2 {
                        positional.push(NativeValue::None);
                    }
                    positional[1] = value.clone();
                }
                if let Some(value) = named.get("keep_frame") {
                    while positional.len() < 3 {
                        positional.push(NativeValue::None);
                    }
                    positional[2] = value.clone();
                }
            }
            "fighter.enter_fall_special" => {
                for name in named.keys() {
                    if !matches!(name.as_str(), "mobility" | "landing_lag") {
                        return Err(StarError::Host(format!(
                            "unknown enter_fall_special argument `{name}`"
                        )));
                    }
                }
                if let Some(value) = named.get("mobility") {
                    while positional.is_empty() {
                        positional.push(NativeValue::None);
                    }
                    positional[0] = value.clone();
                }
                if let Some(value) = named.get("landing_lag") {
                    while positional.len() < 2 {
                        positional.push(NativeValue::None);
                    }
                    positional[1] = value.clone();
                }
            }
            "fighter.emit_projectile" => {
                const NAMES: [&str; 7] = [
                    "kind", "position", "angle", "speed", "lifetime", "hitboxes", "move_id",
                ];
                for name in named.keys() {
                    if !NAMES.contains(&name.as_str()) {
                        return Err(StarError::Host(format!(
                            "unknown emit_projectile argument `{name}`"
                        )));
                    }
                }
                if !args.is_empty() {
                    return Err(StarError::Host(
                        "emit_projectile expects named arguments".into(),
                    ));
                }
                if named.len() != NAMES.len() {
                    let missing = NAMES
                        .iter()
                        .find(|name| !named.contains_key(**name))
                        .copied()
                        .unwrap_or("argument");
                    return Err(StarError::Host(format!(
                        "emit_projectile missing required argument `{missing}`"
                    )));
                }
                positional = NAMES
                    .iter()
                    .map(|name| named.get(*name).cloned().expect("checked above"))
                    .collect();
            }
            _ => {
                return Err(StarError::Host(format!(
                    "native call `{path}` does not accept named arguments"
                )));
            }
        }
        self.call(path, &positional)
    }

    fn call(&mut self, path: &str, args: &[NativeValue]) -> Result<NativeValue, StarError> {
        if let Some(value) = super::builtins::call(self, path, args)? {
            return Ok(value);
        }
        if path == "context.next_action" || path == "next_action" {
            // MoveContext convenience API: action selection is still staged
            // through the same transactional native change_action path.
            return self.call_fighter("fighter.change_action", args);
        }
        if path == "context.entity_at_index" || path == "entity_at_index" {
            let Some(NativeValue::Int(ordinal)) = args.first() else {
                return Ok(NativeValue::None);
            };
            let Ok(ordinal) = u8::try_from(*ordinal) else {
                return Ok(NativeValue::None);
            };
            let Some(entities) = self.entities.as_ref() else {
                return Ok(NativeValue::None);
            };
            if entities
                .payload_for_owner(self.entity_owner_port.unwrap_or(u8::MAX), ordinal)
                .is_none()
            {
                return Ok(NativeValue::None);
            }
            return Ok(NativeValue::Object(NativeObject {
                kind: NativeKind::Value,
                path: format!("context.entity_at_index[{ordinal}]"),
            }));
        }
        if path.starts_with("fighter.") {
            return self.call_fighter(path, args);
        }
        if path == "context.fall" || path == "fall" {
            let movement = self
                .movement
                .as_mut()
                .ok_or_else(|| StarError::Host("movement state unavailable".into()))?;
            lifecycle_movement::fall(
                movement,
                f32_value(
                    args.first().cloned().unwrap_or(NativeValue::None),
                    "gravity",
                )?,
                f32_value(
                    args.get(1).cloned().unwrap_or(NativeValue::None),
                    "terminal velocity",
                )?,
            )
            .map_err(StarError::Host)?;
            self.refresh_movement_view();
            return Ok(NativeValue::None);
        }
        if path == "context.friction_air" || path == "friction_air" {
            let movement = self
                .movement
                .as_mut()
                .ok_or_else(|| StarError::Host("movement state unavailable".into()))?;
            lifecycle_movement::friction_air(
                movement,
                f32_value(
                    args.first().cloned().unwrap_or(NativeValue::None),
                    "air friction",
                )?,
            )
            .map_err(StarError::Host)?;
            self.refresh_movement_view();
            return Ok(NativeValue::None);
        }
        if path == "context.drift_clamp" || path == "drift_clamp" {
            let movement = self
                .movement
                .as_mut()
                .ok_or_else(|| StarError::Host("movement state unavailable".into()))?;
            return lifecycle_movement::drift_clamp(
                movement,
                f32_value(
                    args.first().cloned().unwrap_or(NativeValue::None),
                    "drift maximum",
                )?,
                f32_value(
                    args.get(1).cloned().unwrap_or(NativeValue::None),
                    "drift acceleration",
                )?,
            )
            .map(NativeValue::Bool)
            .map_err(StarError::Host)
            .inspect(|_| self.refresh_movement_view());
        }
        if path == "context.drift_or_friction" || path == "drift_or_friction" {
            let movement = self
                .movement
                .as_mut()
                .ok_or_else(|| StarError::Host("movement state unavailable".into()))?;
            return lifecycle_movement::drift_or_friction(
                movement,
                f32_value(
                    args.first().cloned().unwrap_or(NativeValue::None),
                    "drift step",
                )?,
            )
            .map(NativeValue::Bool)
            .map_err(StarError::Host)
            .inspect(|_| self.refresh_movement_view());
        }
        if path == "context.jump_input" || path == "jump_input" {
            let params = self
                .helper_params
                .as_ref()
                .ok_or_else(|| StarError::Host("jump_input requires fighter data".into()))?;
            let parameters = params.locomotion.as_ref().ok_or_else(|| {
                StarError::Host("jump_input requires locomotion parameters".into())
            })?;
            let input = self.controller();
            let source =
                crate::fighter::locomotion::jump_input(&self.fighter, parameters, input, false);
            return Ok(source.map_or(NativeValue::None, |source| {
                NativeValue::String(format!("{source:?}").to_ascii_lowercase())
            }));
        }
        if path == "context.aerial_jump" || path == "aerial_jump" {
            let params = self
                .helper_params
                .as_ref()
                .ok_or_else(|| StarError::Host("aerial_jump requires fighter data".into()))?;
            let parameters = params.locomotion.as_ref().ok_or_else(|| {
                StarError::Host("aerial_jump requires locomotion parameters".into())
            })?;
            let input = self.controller();
            let jumped = crate::fighter::locomotion::try_aerial_jump_with_parameters(
                &mut self.fighter,
                parameters,
                params.jump_vertical_velocity,
                input,
            );
            self.sync_movement_from_fighter();
            return Ok(NativeValue::Bool(jumped));
        }
        if path == "context.pass_as" || path == "pass_as" {
            let params = self
                .helper_params
                .as_ref()
                .ok_or_else(|| StarError::Host("pass_as requires fighter data".into()))?;
            let geometry = self
                .geometry
                .as_ref()
                .ok_or_else(|| StarError::Host("pass_as requires stage geometry".into()))?;
            let (destination, velocity_arg, keep_arg) = if args
                .first()
                .is_some_and(|value| matches!(value, NativeValue::String(_)))
            {
                (args.first(), args.get(1), args.get(2))
            } else {
                (args.get(1), args.first(), args.get(2))
            };
            let velocity_y = f32_value(
                velocity_arg.cloned().unwrap_or(NativeValue::None),
                "pass velocity",
            )?;
            let destination = if let Some(value) = destination {
                let name = action_arg(Some(value))?;
                crate::game::script::parse_action(&name)
                    .ok_or_else(|| StarError::Host("unsupported pass action".into()))?
            } else {
                game::Action::Pass
            };
            let keep_frame = keep_arg
                .map(|value| bool_value(value.clone(), "keep_frame"))
                .transpose()?
                .unwrap_or(false);
            let passed = crate::game::collision::begin_pass_as_with_attributes(
                &mut self.fighter,
                params.attributes,
                geometry,
                velocity_y,
                destination,
                keep_frame,
            );
            self.sync_movement_from_fighter();
            return Ok(NativeValue::Bool(passed));
        }
        if path == "context.input.just_pressed"
            || path == "input.just_pressed"
            || path == "context.input.held"
            || path == "input.held"
        {
            let mask = args
                .first()
                .map(button_mask)
                .transpose()?
                .ok_or_else(|| StarError::Host("input query requires a button".into()))?;
            let input = self.controller();
            let value = if path.ends_with("just_pressed") {
                input.buttons & !self.fighter.previous_input.buttons & mask != 0
            } else {
                input.buttons & mask != 0
            };
            return Ok(NativeValue::Bool(value));
        }
        if path == "context.resource" || path == "resource" {
            if args.is_empty() {
                let resource = self.owned_resource.as_deref().ok_or_else(|| {
                    StarError::Host("no owned resource is available for this callback".into())
                })?;
                let value = self.resources.value_path(resource).ok_or_else(|| {
                    StarError::Host(format!("owned resource `{resource}` is unavailable"))
                })?;
                return match value {
                    JsonValue::Object(_) | JsonValue::Array(_) => {
                        Ok(NativeValue::Object(NativeObject {
                            kind: NativeKind::Value,
                            path: format!("context.{resource}"),
                        }))
                    }
                    _ => json_to_native(value, &format!("context.{resource}")),
                };
            }
            let resource = args
                .first()
                .and_then(|x| match x {
                    NativeValue::String(x) => Some(x.as_str()),
                    _ => None,
                })
                .ok_or_else(|| StarError::Host("resource path must be a string".into()))?;
            if let Some(value) = self.resources.value_path(resource) {
                return match value {
                    JsonValue::Object(_) | JsonValue::Array(_) => {
                        Ok(NativeValue::Object(NativeObject {
                            kind: NativeKind::Value,
                            path: format!("context.{resource}"),
                        }))
                    }
                    _ => json_to_native(value, &format!("context.{resource}")),
                };
            }
            return Ok(NativeValue::None);
        }
        if path.ends_with(".frames") || path == "frames" {
            let resource = args
                .first()
                .and_then(|x| match x {
                    NativeValue::String(x) => Some(x.as_str()),
                    _ => None,
                })
                .ok_or_else(|| StarError::Host("frames path must be a string".into()))?;
            return Ok(NativeValue::Int(self.resources.frame_count(resource) as i64));
        }
        if path.ends_with(".validate_attack") || path == "validate_attack" {
            let resource = args
                .first()
                .and_then(|x| match x {
                    NativeValue::String(x) => Some(x.as_str()),
                    _ => None,
                })
                .ok_or_else(|| StarError::Host("attack path must be a string".into()))?;
            return self
                .resources
                .validate_attack(resource)
                .map(NativeValue::Bool)
                .map_err(|error| StarError::Host(error.to_string()));
        }
        if path.ends_with(".array_length") || path == "array_length" {
            let resource = args
                .first()
                .and_then(|x| match x {
                    NativeValue::String(x) => Some(x.as_str()),
                    _ => None,
                })
                .ok_or_else(|| StarError::Host("array path must be a string".into()))?;
            let index = args.get(1).and_then(|x| match x {
                NativeValue::Int(x) => usize::try_from(*x).ok(),
                _ => None,
            });
            return Ok(self
                .resources
                .array_length(resource, index)
                .map_or(NativeValue::None, |x| NativeValue::Int(x as i64)));
        }
        if path.ends_with(".sample") || path == "sample" {
            let resource = args
                .first()
                .and_then(|x| match x {
                    NativeValue::String(x) => Some(x.as_str()),
                    _ => None,
                })
                .ok_or_else(|| StarError::Host("sample path must be a string".into()))?;
            let frame = args
                .get(1)
                .and_then(|x| match x {
                    NativeValue::Int(x) => usize::try_from(*x).ok(),
                    _ => None,
                })
                .ok_or_else(|| {
                    StarError::Host("sample frame must be a nonnegative integer".into())
                })?;
            return self
                .resources
                .sample(resource, frame)
                .map(|value| json_to_native(value, &format!("context.{resource}")))
                .transpose()?
                .ok_or_else(|| StarError::Host(format!("sample out of range for `{resource}`")));
        }
        Err(StarError::Host(format!("unknown native method `{path}`")))
    }
}

fn decode_preserve_fields(
    value: Option<&NativeValue>,
    schema: Option<&StateSchema>,
    preserve_state: bool,
) -> Result<Option<Vec<String>>, StarError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let NativeValue::List(values) = value else {
        if matches!(value, NativeValue::None) {
            return Ok(None);
        }
        return Err(StarError::Host(
            "preserve_fields must be a sequence of state field names".into(),
        ));
    };
    if preserve_state && !values.is_empty() {
        return Err(StarError::Host(
            "preserve_state and preserve_fields are mutually exclusive".into(),
        ));
    }
    let Some(schema) = schema else {
        if values.is_empty() {
            return Ok(Some(Vec::new()));
        }
        return Err(StarError::Host(
            "preserve_fields requires a declared action_state schema".into(),
        ));
    };
    let mut fields = Vec::with_capacity(values.len());
    for value in values {
        let NativeValue::String(field) = value else {
            return Err(StarError::Host(
                "preserve_fields entries must be strings".into(),
            ));
        };
        if field.is_empty() || !schema.fields.contains_key(field) {
            return Err(StarError::Host(format!(
                "preserve_fields contains undeclared field `{field}`"
            )));
        }
        if fields.iter().any(|existing| existing == field) {
            return Err(StarError::Host(format!(
                "preserve_fields contains duplicate field `{field}`"
            )));
        }
        fields.push(field.clone());
    }
    Ok(Some(fields))
}

fn action_state_after_change(
    prior: &LocalState,
    reset: LocalState,
    preserve_state: bool,
    preserve_fields: Option<&[String]>,
) -> LocalState {
    if preserve_state {
        return prior.clone();
    }
    let mut state = reset;
    if let Some(fields) = preserve_fields {
        for field in fields {
            if let Some(value) = prior.get(field) {
                state.insert(field.clone(), value.clone());
            }
        }
    }
    state
}

/// `ftCo_80096900` calls `ftCommon_8007D60C` when FallSpecial is entered from
/// the ground; that native branch clears the grounded flag and gr_vel. Air
/// entries retain their existing ground velocity bookkeeping.
fn fall_special_ground_state(grounded: bool, ground_velocity: f32) -> (bool, f32) {
    if grounded {
        (false, 0.0)
    } else {
        (false, ground_velocity)
    }
}

fn state_get(path: &str, root: &str, state: &LocalState) -> Option<NativeValue> {
    let key = path.strip_prefix(&format!("{root}."))?;
    state.get(key).map(local_to_native)
}

fn state_set(
    path: &str,
    root: &str,
    value: NativeValue,
    state: &mut LocalState,
    schema: Option<&StateSchema>,
) -> Result<(), StarError> {
    let key = path
        .strip_prefix(&format!("{root}."))
        .ok_or_else(|| StarError::Host("invalid state path".into()))?;
    let value = match value {
        NativeValue::Bool(x) => LocalValue::Bool(x),
        NativeValue::Int(x) => LocalValue::Integer(x),
        NativeValue::F32(x) => LocalValue::Number(f64::from(x)),
        NativeValue::String(x) => LocalValue::String(x),
        NativeValue::Tuple(values) => LocalValue::Tuple(
            values
                .iter()
                .map(native_to_local)
                .collect::<Result<_, _>>()?,
        ),
        // The SDK encoder exposes tuples as JSON-compatible sequences at the
        // host boundary. Copy it immediately into the immutable persistent
        // form; lists remain rejected for scalar declarations and never
        // persist.
        NativeValue::List(values) if matches!(schema.map(|s| s.fields.get(key)), Some(Some(field)) if matches!(&field.value_type, StateType::FixedTuple { .. })) => {
            LocalValue::Tuple(
                values
                    .iter()
                    .map(native_to_local)
                    .collect::<Result<_, _>>()?,
            )
        }
        _ => {
            return Err(StarError::Host(
                "state fields must be scalar or fixed tuple".into(),
            ));
        }
    };
    if let Some(schema) = schema {
        let field = schema
            .fields
            .get(key)
            .ok_or_else(|| StarError::Host(format!("undeclared state field `{key}`")))?;
        let matches = super::state_value_matches(field.value_type.clone(), &value);
        if !matches {
            return Err(StarError::Host(format!(
                "state field `{key}` has the wrong type"
            )));
        }
    }
    state.insert(key.to_owned(), value);
    Ok(())
}

fn local_to_native(value: &LocalValue) -> NativeValue {
    match value {
        LocalValue::Bool(x) => NativeValue::Bool(*x),
        LocalValue::Integer(x) => NativeValue::Int(*x),
        LocalValue::Number(x) => NativeValue::F32(*x as f32),
        LocalValue::String(x) => NativeValue::String(x.clone()),
        LocalValue::Tuple(values) => {
            NativeValue::Tuple(values.iter().map(local_to_native).collect())
        }
    }
}

fn native_to_local(value: &NativeValue) -> Result<LocalValue, StarError> {
    Ok(match value {
        NativeValue::Bool(x) => LocalValue::Bool(*x),
        NativeValue::Int(x) => LocalValue::Integer(*x),
        NativeValue::F32(x) => LocalValue::Number(f64::from(*x)),
        NativeValue::String(x) => LocalValue::String(x.clone()),
        NativeValue::Tuple(values) => LocalValue::Tuple(
            values
                .iter()
                .map(native_to_local)
                .collect::<Result<_, _>>()?,
        ),
        _ => {
            return Err(StarError::Host(
                "tuple elements must be scalar or fixed tuple".into(),
            ));
        }
    })
}
fn action_arg(value: Option<&NativeValue>) -> Result<String, StarError> {
    match value {
        Some(NativeValue::String(x)) => Ok(x.clone()),
        _ => Err(StarError::Host("action must be a string".into())),
    }
}

/// The source launch expression is `facing * (speed * cosf(angle))`.
/// Keeping this grouping explicit matters because these are binary32 values.
fn source_launch_velocity(speed: f32, cosine: f32, sine: f32, facing: f32) -> [f32; 2] {
    [facing * (speed * cosine), speed * sine]
}

fn f32_value(value: NativeValue, name: &str) -> Result<f32, StarError> {
    let value = match value {
        NativeValue::F32(x) => x,
        NativeValue::Int(x) => x as f32,
        _ => return Err(StarError::Host(format!("{name} must be a number"))),
    };
    lifecycle_state::validate_f32(name, value).map_err(|x| StarError::Host(x.to_string()))
}
fn int_value(value: NativeValue, name: &str) -> Result<i64, StarError> {
    match value {
        NativeValue::Int(x) => Ok(x),
        _ => Err(StarError::Host(format!("{name} must be an integer"))),
    }
}
fn u8_value(value: NativeValue, name: &str) -> Result<u8, StarError> {
    u8::try_from(int_value(value, name)?)
        .map_err(|_| StarError::Host(format!("{name} is out of range")))
}
fn bool_value(value: NativeValue, name: &str) -> Result<bool, StarError> {
    match value {
        NativeValue::Bool(x) => Ok(x),
        _ => Err(StarError::Host(format!("{name} must be a boolean"))),
    }
}
fn button_mask(value: &NativeValue) -> Result<u16, StarError> {
    match value {
        NativeValue::Int(value) => {
            u16::try_from(*value).map_err(|_| StarError::Host("button mask is out of range".into()))
        }
        NativeValue::String(name) => match name.to_ascii_uppercase().as_str() {
            "A" => Ok(game::BUTTON_A),
            "B" => Ok(game::BUTTON_B),
            "Z" => Ok(game::BUTTON_Z),
            "L" => Ok(game::BUTTON_L),
            "R" => Ok(game::BUTTON_R),
            "X" => Ok(game::BUTTON_X),
            "Y" => Ok(game::BUTTON_Y),
            "DPAD_LEFT" => Ok(game::BUTTON_DPAD_LEFT),
            "DPAD_RIGHT" => Ok(game::BUTTON_DPAD_RIGHT),
            "DPAD_DOWN" => Ok(game::BUTTON_DPAD_DOWN),
            "DPAD_UP" => Ok(game::BUTTON_DPAD_UP),
            _ => Err(StarError::Host(format!("unknown button `{name}`"))),
        },
        _ => Err(StarError::Host("button must be a name or mask".into())),
    }
}
pub(crate) fn vec2(value: NativeValue, name: &str) -> Result<[f32; 2], StarError> {
    match value {
        NativeValue::Vec2(x) => {
            f32_value(NativeValue::F32(x[0]), name)?;
            f32_value(NativeValue::F32(x[1]), name)?;
            Ok(x)
        }
        NativeValue::List(x) if x.len() == 2 => Ok([
            f32_value(x[0].clone(), name)?,
            f32_value(x[1].clone(), name)?,
        ]),
        _ => Err(StarError::Host(format!(
            "{name} must be a two element vector"
        ))),
    }
}
fn vec3(value: NativeValue, name: &str) -> Result<[f32; 3], StarError> {
    match value {
        NativeValue::List(x) if x.len() == 3 => Ok([
            f32_value(x[0].clone(), name)?,
            f32_value(x[1].clone(), name)?,
            f32_value(x[2].clone(), name)?,
        ]),
        _ => Err(StarError::Host(format!(
            "{name} must be a three element vector"
        ))),
    }
}
fn list<const N: usize>(value: [f32; N]) -> Result<NativeValue, StarError> {
    Ok(NativeValue::List(
        value.into_iter().map(NativeValue::F32).collect(),
    ))
}
pub(crate) fn json_path<'a>(value: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    path.split('.')
        .filter(|x| !x.is_empty())
        .try_fold(value, |value, key| value.as_object()?.get(key))
}

fn project_parameter_path<'a>(
    parameters: &'a BTreeMap<String, JsonValue>,
    relative: &str,
) -> Option<&'a JsonValue> {
    if let Some(value) = parameters.get(relative) {
        return Some(value);
    }
    let (root, suffix) = relative.split_once('.')?;
    json_path(parameters.get(root)?, suffix)
}

fn parameter_to_native(value: &JsonValue, path: &str) -> Result<NativeValue, StarError> {
    Ok(match value {
        JsonValue::Null => NativeValue::None,
        JsonValue::Bool(value) => NativeValue::Bool(*value),
        JsonValue::Number(value) => {
            if let Some(value) = value.as_i64() {
                NativeValue::Int(value)
            } else {
                NativeValue::F32(value.as_f64().ok_or_else(|| {
                    StarError::Host(format!("parameter `{path}` is not a finite number"))
                })? as f32)
            }
        }
        JsonValue::String(value) => NativeValue::String(value.clone()),
        JsonValue::Object(_) => NativeValue::Object(NativeObject {
            kind: NativeKind::Value,
            path: path.into(),
        }),
        JsonValue::Array(_) => {
            return Err(StarError::Host(format!(
                "parameter `{path}` must be a scalar or record"
            )));
        }
    })
}

pub(crate) fn json_to_native(value: &JsonValue, path: &str) -> Result<NativeValue, StarError> {
    Ok(match value {
        JsonValue::Null => NativeValue::None,
        JsonValue::Bool(x) => NativeValue::Bool(*x),
        JsonValue::Number(x) => NativeValue::F32(x.as_f64().unwrap_or(0.0) as f32),
        JsonValue::String(x) => NativeValue::String(x.clone()),
        JsonValue::Array(x) => NativeValue::List(
            x.iter()
                .map(|x| json_to_native(x, path))
                .collect::<Result<_, _>>()?,
        ),
        JsonValue::Object(_) => NativeValue::Object(NativeObject {
            kind: NativeKind::Value,
            path: path.into(),
        }),
    })
}

pub(crate) fn native_to_json_with_resources(
    value: NativeValue,
    resources: Option<&ResourceCache>,
) -> Result<JsonValue, StarError> {
    Ok(match value {
        NativeValue::None => JsonValue::Null,
        NativeValue::Bool(value) => JsonValue::Bool(value),
        NativeValue::Int(value) => JsonValue::from(value),
        NativeValue::F32(value) => serde_json::Number::from_f64(f64::from(value))
            .map(JsonValue::Number)
            .ok_or_else(|| StarError::Invalid("non-finite callback number".into()))?,
        NativeValue::String(value) => JsonValue::String(value),
        NativeValue::Vec2(value) => {
            if value.iter().any(|component| !component.is_finite()) {
                return Err(StarError::Invalid("non-finite callback vector".into()));
            }
            JsonValue::Array(value.into_iter().map(JsonValue::from).collect())
        }
        NativeValue::List(values) => JsonValue::Array(
            values
                .into_iter()
                .map(|value| native_to_json_with_resources(value, resources))
                .collect::<Result<_, _>>()?,
        ),
        NativeValue::Dict(values) => JsonValue::Object(
            values
                .into_iter()
                .map(|(key, value)| Ok((key, native_to_json_with_resources(value, resources)?)))
                .collect::<Result<_, StarError>>()?,
        ),
        NativeValue::Tuple(values) => JsonValue::Array(
            values
                .into_iter()
                .map(|value| native_to_json_with_resources(value, resources))
                .collect::<Result<_, _>>()?,
        ),
        NativeValue::Object(object) => {
            if object.kind != NativeKind::Value {
                return Err(StarError::Invalid(
                    "callback returned a live native object".into(),
                ));
            }
            let resource_path = object.path.strip_prefix("context.").ok_or_else(|| {
                StarError::Invalid("callback returned a non-resource object".into())
            })?;
            resources
                .and_then(|resources| resources.value_path(resource_path))
                .cloned()
                .ok_or_else(|| {
                    StarError::Invalid(format!("unknown callback resource `{resource_path}`"))
                })?
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        LifecycleHost, NativeKind, NativeValue, parameter_to_native, project_parameter_path,
        state_set,
    };
    use super::{action_state_after_change, decode_preserve_fields};
    use crate::collision::ecb::{Shape, State as EcbState};
    use crate::game::entity::{EntityOwner, EntityPayload, EntityStore};
    use crate::game::script::{LocalState, LocalValue, StateField, StateSchema, StateType};
    use std::collections::BTreeMap;

    fn schema() -> StateSchema {
        StateSchema {
            fields: BTreeMap::from([
                (
                    "release_lag".into(),
                    StateField::from_default(LocalValue::Integer(0)),
                ),
                (
                    "is_release".into(),
                    StateField::from_default(LocalValue::Bool(false)),
                ),
                (
                    "gravity_delay".into(),
                    StateField::from_default(LocalValue::Integer(0)),
                ),
                (
                    "reset_me".into(),
                    StateField::from_default(LocalValue::Integer(7)),
                ),
            ]),
        }
    }

    #[test]
    fn selective_preserve_copies_declared_fields_and_resets_the_rest() {
        let prior = LocalState::from([
            ("release_lag".into(), LocalValue::Integer(12)),
            ("is_release".into(), LocalValue::Bool(true)),
            ("gravity_delay".into(), LocalValue::Integer(4)),
            ("reset_me".into(), LocalValue::Integer(99)),
        ]);
        let selected = vec!["release_lag".into(), "is_release".into()];
        let reset = schema().defaults();
        let state = action_state_after_change(&prior, reset, false, Some(&selected));
        assert_eq!(state.get("release_lag"), Some(&LocalValue::Integer(12)));
        assert_eq!(state.get("is_release"), Some(&LocalValue::Bool(true)));
        assert_eq!(state.get("gravity_delay"), Some(&LocalValue::Integer(0)));
        assert_eq!(state.get("reset_me"), Some(&LocalValue::Integer(7)));
    }

    #[test]
    fn malformed_preserve_fields_are_rejected_before_transition() {
        let prior_state = LocalState::from([("release_lag".into(), LocalValue::Integer(12))]);
        let before = prior_state.clone();
        let malformed =
            super::NativeValue::List(vec![super::NativeValue::String("unknown".into())]);
        let error = decode_preserve_fields(Some(&malformed), Some(&schema()), false)
            .expect_err("unknown fields must be rejected");
        assert!(error.to_string().contains("undeclared field"));
        assert_eq!(prior_state, before);
    }

    #[test]
    fn ecb_getter_projects_opaque_shapes_and_copied_corner_vectors() {
        let current = Shape {
            top: [0.25, 8.0],
            bottom: [-0.5, -1.0],
            left: [-3.0, 3.5],
            right: [2.0, 3.5],
        };
        let previous = Shape {
            top: [0.0, 7.5],
            bottom: [0.0, -1.5],
            left: [-2.5, 3.0],
            right: [2.5, 3.0],
        };
        let mut ecb = EcbState::new(current);
        ecb.previous = previous;

        assert_eq!(
            LifecycleHost::project_ecb_path(&ecb, "fighter.ecb.current"),
            Some(NativeValue::Object(super::NativeObject {
                kind: NativeKind::Value,
                path: "fighter.ecb.current".into(),
            }))
        );
        assert_eq!(
            LifecycleHost::project_ecb_path(&ecb, "fighter.ecb.current.bottom"),
            Some(NativeValue::Vec2(current.bottom))
        );
        assert_eq!(
            LifecycleHost::project_ecb_path(&ecb, "fighter.ecb.previous.right"),
            Some(NativeValue::Vec2(previous.right))
        );
        assert!(LifecycleHost::project_ecb_path(&ecb, "fighter.ecb.current.center").is_none());
    }

    #[test]
    fn parameter_getter_reads_declared_scalars_and_nested_values_only() {
        let parameters = BTreeMap::from([
            ("projectile_kind".into(), serde_json::json!("laser")),
            ("tuning".into(), serde_json::json!({"speed": 1.5})),
        ]);
        assert_eq!(
            project_parameter_path(&parameters, "projectile_kind"),
            Some(&serde_json::json!("laser"))
        );
        assert_eq!(
            project_parameter_path(&parameters, "tuning.speed"),
            Some(&serde_json::json!(1.5))
        );
        assert_eq!(
            parameter_to_native(&serde_json::json!(7), "context.parameters.frames").unwrap(),
            NativeValue::Int(7)
        );
        assert_eq!(
            parameter_to_native(&serde_json::json!(1.5), "context.parameters.speed").unwrap(),
            NativeValue::F32(1.5)
        );
        assert!(project_parameter_path(&parameters, "missing").is_none());
    }

    #[test]
    fn launch_velocity_keeps_source_binary32_grouping() {
        let speed = 44_126.313_f32;
        let cosine = 0.15130243_f32;
        let sine = 0.25_f32;
        let facing = 14338.76_f32;
        let velocity = super::source_launch_velocity(speed, cosine, sine, facing);
        assert_eq!(velocity[0].to_bits(), 95731552.0_f32.to_bits());
        assert_ne!(
            velocity[0].to_bits(),
            (speed * (facing * cosine)).to_bits(),
            "the associative rewrite must remain observably different"
        );
        assert_eq!(velocity[1], speed * sine);
    }

    #[test]
    fn native_input_age_writes_accept_u8_and_reject_signed_overflow() {
        assert_eq!(super::u8_value(NativeValue::Int(254), "age").unwrap(), 254);
        for value in [NativeValue::Int(-1), NativeValue::Int(256)] {
            assert!(super::u8_value(value, "age").is_err());
        }
    }

    #[test]
    fn entity_projection_dispatches_by_owner_and_fails_closed() {
        let mut entities = EntityStore::default();
        entities
            .insert_with_payload(
                EntityOwner::new(0, 1),
                EntityPayload {
                    position: [3.0, 4.0, 0.0],
                    velocity: [1.0, -2.0, 0.0],
                    motion_state: 7,
                    facing: -1.0,
                    lifetime: None,
                },
            )
            .unwrap();
        assert_eq!(
            LifecycleHost::project_entity_path(&entities, 0, "context.entity_at_index[1].position"),
            Some(NativeValue::Vec2([3.0, 4.0]))
        );
        assert_eq!(
            LifecycleHost::project_entity_path(&entities, 1, "context.entity_at_index[1].position"),
            None
        );
        assert_eq!(
            LifecycleHost::project_entity_path(&entities, 0, "context.entity_at_index[255]"),
            None
        );
    }

    #[test]
    fn fall_special_clears_ground_velocity_only_for_ground_entry() {
        assert_eq!(super::fall_special_ground_state(true, 2.5), (false, 0.0));
        assert_eq!(super::fall_special_ground_state(false, 2.5), (false, 2.5));
    }

    #[test]
    fn fixed_tuple_state_writes_are_typed_arity_checked_and_atomic() {
        let schema = StateSchema {
            fields: BTreeMap::from([(
                "command".into(),
                StateField {
                    value_type: StateType::FixedTuple {
                        element: Box::new(StateType::Integer),
                        length: 4,
                    },
                    default: LocalValue::Tuple(vec![LocalValue::Integer(0); 4]),
                },
            )]),
        };
        let mut state = LocalState::from([(
            "command".into(),
            LocalValue::Tuple(vec![LocalValue::Integer(7); 4]),
        )]);
        state_set(
            "fighter.action_state.command",
            "fighter.action_state",
            NativeValue::Tuple(vec![NativeValue::Int(1); 4]),
            &mut state,
            Some(&schema),
        )
        .expect("valid tuple write");
        let before = state.clone();
        for value in [
            NativeValue::Tuple(vec![NativeValue::Int(1); 3]),
            NativeValue::Tuple(vec![
                NativeValue::Int(1),
                NativeValue::Bool(false),
                NativeValue::Int(1),
                NativeValue::Int(1),
            ]),
        ] {
            assert!(
                state_set(
                    "fighter.action_state.command",
                    "fighter.action_state",
                    value,
                    &mut state,
                    Some(&schema)
                )
                .is_err()
            );
            assert_eq!(
                state, before,
                "rejected tuple write must not partially commit"
            );
        }
    }
}
