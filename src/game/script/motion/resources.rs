//! Resource linking for generic native motion profiles.
//!
//! The script compiler owns the syntax and static checks for `motion.*`
//! constructors.  This module owns the load-time seam between that metadata
//! and [`super::motion::MotionProfile`].  A descriptor contains typed paths,
//! rather than strings which a callback could evaluate later.  Linking walks
//! those paths once, copies the resulting scalar values and tracks into an
//! immutable profile, and leaves only [`super::motion::MotionBinding`] as
//! per-fighter state.
//!
//! No fighter or move names belong here.  A resource path is opaque to this
//! module; character-specific meaning is supplied by the descriptor and by
//! the parameter source passed by the native integration layer.

use super::lifecycle_resources::ResourceCache;
use super::motion::{
    AirOperation, CommandBranch, GravityMultiplier, GroundOperation, MotionProfile, ScalarTrack,
    StickSteering, TrackEnd, TrackTransform, VelocitySample, VelocityTrack, COMMAND_SLOTS,
    MAX_COMMAND_VALUE,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

/// Resource and native-parameter paths are validated when the compiler makes
/// a field reference.  This prevents an expression such as `foo[bar]` from
/// crossing the static descriptor boundary as an apparently typed path.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldRef(String);

/// Semantic aliases used by frontends when retaining the distinction between
/// an animation resource field and a native parameter field in their AST.
pub type ResourceField = FieldRef;
pub type ParameterField = FieldRef;

impl FieldRef {
    pub fn new(path: impl Into<String>) -> Result<Self, MotionLinkError> {
        let path = path.into();
        validate_path(&path)?;
        Ok(Self(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn standard_parameter(path: &'static str) -> ScalarRef {
    ScalarRef::Parameter(FieldRef(path.to_owned()))
}

impl TryFrom<String> for FieldRef {
    type Error = MotionLinkError;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        Self::new(path)
    }
}

impl TryFrom<&str> for FieldRef {
    type Error = MotionLinkError;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        Self::new(path)
    }
}

/// A typed native fighter-specific attribute slot. The layout is part of the
/// identity so a profile cannot accidentally read an offset from another
/// fighter's packed attribute record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecialAttributeRef {
    pub layout: u8,
    pub field_id: u16,
}

impl SpecialAttributeRef {
    pub const fn new(layout: u8, field_id: u16) -> Self {
        Self { layout, field_id }
    }
}

/// A scalar in a motion declaration. `Resource` resolves through the
/// immutable animation/resource cache, while `Parameter` resolves through a
/// native parameter view (for example a fighter's movement attributes).
#[derive(Clone, Debug, PartialEq)]
pub enum ScalarRef {
    Literal(f32),
    Resource(FieldRef),
    Parameter(FieldRef),
    SpecialAttribute(SpecialAttributeRef),
}

impl ScalarRef {
    pub fn literal(value: f32) -> Self {
        Self::Literal(value)
    }

    pub fn resource(path: impl Into<String>) -> Result<Self, MotionLinkError> {
        Ok(Self::Resource(FieldRef::new(path)?))
    }

    pub fn parameter(path: impl Into<String>) -> Result<Self, MotionLinkError> {
        Ok(Self::Parameter(FieldRef::new(path)?))
    }

    pub const fn special_attribute(reference: SpecialAttributeRef) -> Self {
        Self::SpecialAttribute(reference)
    }
}

impl AirOperationDescriptor {
    pub fn gravity(
        acceleration: ScalarRef,
        terminal_velocity: ScalarRef,
        delay: ScalarRef,
    ) -> Self {
        Self::Gravity {
            acceleration,
            terminal_velocity,
            delay,
        }
    }

    pub fn friction(amount: ScalarRef) -> Self {
        Self::Friction { amount }
    }

    pub fn vertical_gravity(gravity: ScalarRef) -> Self {
        Self::VerticalGravity { gravity }
    }

    pub fn gravity_multiplier(index: usize, value: u32, multiplier: ScalarRef) -> Self {
        Self::GravityMultiplier {
            index,
            value,
            multiplier,
        }
    }

    pub fn stick_steering(
        threshold: ScalarRef,
        acceleration: ScalarRef,
        target: ScalarRef,
    ) -> Self {
        Self::StickSteering {
            threshold,
            acceleration,
            target,
        }
    }

    pub fn velocity_track(track: VelocityTrackDescriptor) -> Self {
        Self::VelocityTrack(track)
    }

    pub fn directional_acceleration(starts_at: ScalarRef, magnitude: ScalarRef) -> Self {
        Self::DirectionalAcceleration {
            starts_at,
            magnitude,
        }
    }

    pub fn drift_clamp(maximum: ScalarRef, acceleration: ScalarRef) -> Self {
        Self::DriftClamp {
            maximum,
            acceleration,
        }
    }

    pub fn drift_or_friction(recovery_step: ScalarRef) -> Self {
        Self::DriftOrFriction { recovery_step }
    }

    pub fn command_velocity_scale(index: usize, value: u32, multiplier: ScalarRef) -> Self {
        Self::CommandVelocityScale {
            index,
            value,
            multiplier,
        }
    }

    pub fn command_branch(index: usize, cases: BTreeMap<u32, Vec<Self>>) -> Self {
        Self::CommandBranch(CommandBranchDescriptor::new(index, cases))
    }
}

impl GroundOperationDescriptor {
    pub fn friction(amount: ScalarRef) -> Self {
        Self::Friction { amount }
    }

    pub fn friction_above_walk(
        amount: ScalarRef,
        walk_max_velocity: ScalarRef,
        above_walk_multiplier: ScalarRef,
    ) -> Self {
        Self::FrictionAboveWalk {
            amount,
            walk_max_velocity,
            above_walk_multiplier,
        }
    }

    pub fn native_ground_friction() -> Self {
        Self::friction_above_walk(
            standard_parameter("movement.ground_friction"),
            standard_parameter("movement.walk_max_velocity"),
            standard_parameter("rules.friction_above_walk"),
        )
    }

    pub fn stick_steering(
        threshold: ScalarRef,
        acceleration: ScalarRef,
        target: ScalarRef,
    ) -> Self {
        Self::StickSteering {
            threshold,
            acceleration,
            target,
        }
    }

    pub fn friction_after(starts_at: ScalarRef, before: ScalarRef, after: ScalarRef) -> Self {
        Self::FrictionAfter {
            starts_at,
            before,
            after,
        }
    }

    pub fn target_track(track: ScalarTrackDescriptor) -> Self {
        Self::TargetTrack(track)
    }

    pub fn command_branch(index: usize, cases: BTreeMap<u32, Vec<Self>>) -> Self {
        Self::CommandBranch(CommandBranchDescriptor::new(index, cases))
    }
}

/// Which component a velocity resource provides.  `Both` expects each
/// sample to be a two-element array; `X` and `Y` accept either a scalar list
/// or a two-element list and select the requested component.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrackComponent {
    X,
    Y,
    #[default]
    Both,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VelocityTrackDescriptor {
    pub path: FieldRef,
    pub component: TrackComponent,
    pub end: TrackEnd,
    pub x_transform: TrackTransform,
    pub y_transform: TrackTransform,
}

impl VelocityTrackDescriptor {
    pub fn new(path: FieldRef) -> Self {
        Self {
            path,
            component: TrackComponent::Both,
            end: TrackEnd::NoOp,
            x_transform: TrackTransform::Absolute,
            y_transform: TrackTransform::Absolute,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScalarTrackDescriptor {
    pub path: FieldRef,
    pub end: TrackEnd,
    pub transform: TrackTransform,
}

impl ScalarTrackDescriptor {
    pub fn new(path: FieldRef) -> Self {
        Self {
            path,
            end: TrackEnd::NoOp,
            transform: TrackTransform::Absolute,
        }
    }
}

/// Descriptor form of a mutually exclusive numeric command branch. Cases are
/// ordinary operation lists, so each scalar is linked and validated through
/// the same path as an operation authored directly in the profile.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandBranchDescriptor<T> {
    pub index: usize,
    pub cases: BTreeMap<u32, Vec<T>>,
}

impl<T> CommandBranchDescriptor<T> {
    pub fn new(index: usize, cases: BTreeMap<u32, Vec<T>>) -> Self {
        Self { index, cases }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AirOperationDescriptor {
    Gravity {
        acceleration: ScalarRef,
        terminal_velocity: ScalarRef,
        delay: ScalarRef,
    },
    GravityMultiplier {
        index: usize,
        value: u32,
        multiplier: ScalarRef,
    },
    Friction {
        amount: ScalarRef,
    },
    VerticalGravity {
        gravity: ScalarRef,
    },
    StickSteering {
        threshold: ScalarRef,
        acceleration: ScalarRef,
        target: ScalarRef,
    },
    VelocityTrack(VelocityTrackDescriptor),
    DirectionalAcceleration {
        starts_at: ScalarRef,
        magnitude: ScalarRef,
    },
    DriftClamp {
        maximum: ScalarRef,
        acceleration: ScalarRef,
    },
    DriftOrFriction {
        recovery_step: ScalarRef,
    },
    CommandVelocityScale {
        index: usize,
        value: u32,
        multiplier: ScalarRef,
    },
    CommandBranch(CommandBranchDescriptor<AirOperationDescriptor>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroundOperationDescriptor {
    Friction {
        amount: ScalarRef,
    },
    FrictionAboveWalk {
        amount: ScalarRef,
        walk_max_velocity: ScalarRef,
        above_walk_multiplier: ScalarRef,
    },
    StickSteering {
        threshold: ScalarRef,
        acceleration: ScalarRef,
        target: ScalarRef,
    },
    FrictionAfter {
        starts_at: ScalarRef,
        before: ScalarRef,
        after: ScalarRef,
    },
    TargetTrack(ScalarTrackDescriptor),
    CommandBranch(CommandBranchDescriptor<GroundOperationDescriptor>),
}

/// Generic descriptor emitted by the class-contract/static motion parser.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MotionDescriptor {
    pub air: Vec<AirOperationDescriptor>,
    pub ground: Vec<GroundOperationDescriptor>,
}

impl MotionDescriptor {
    pub fn profile(
        air: impl IntoIterator<Item = AirOperationDescriptor>,
        ground: impl IntoIterator<Item = GroundOperationDescriptor>,
    ) -> Self {
        Self {
            air: air.into_iter().collect(),
            ground: ground.into_iter().collect(),
        }
    }

    /// Decode the JSON form emitted by `CompiledProgram`'s immutable manifest.
    /// This is a registration-time operation; callers should retain the
    /// resulting descriptor/profile rather than decode it during dispatch.
    pub fn from_compiled_constructor(value: &Value) -> Result<Self, MotionLinkError> {
        parse_profile_constructor(value)
    }
}

/// Alias used by callers that refer to the resulting profile as metadata.
pub type MotionProfileDescriptor = MotionDescriptor;

/// Read-only scalar/track view used by the linker.  The game cache implements
/// this with its dotted path lookup; tests and other resource pack loaders can
/// implement it without constructing a `FighterData` value.
pub trait MotionResourceSource {
    fn value_path(&self, path: &str) -> Option<&Value>;
}

impl MotionResourceSource for ResourceCache {
    fn value_path(&self, path: &str) -> Option<&Value> {
        self.value_path(path)
    }
}

/// Native scalar parameter view. Parameters intentionally use a separate
/// source from resources so a declaration cannot smuggle a fighter field into
/// the resource tree or perform a dynamic lookup during simulation.
pub trait MotionParameterSource {
    fn number_path(&self, path: &str) -> Option<f32>;

    fn special_attribute(&self, _reference: SpecialAttributeRef) -> Option<f32> {
        None
    }
}

/// Convenience adapter for JSON-backed native parameter records. The native
/// integration may instead implement [`MotionParameterSource`] directly over
/// its typed attributes.
impl MotionParameterSource for Value {
    fn number_path(&self, path: &str) -> Option<f32> {
        json_path(self, path).and_then(number)
    }
}

impl MotionParameterSource for BTreeMap<String, f32> {
    fn number_path(&self, path: &str) -> Option<f32> {
        self.get(path).copied()
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum MotionLinkError {
    #[error("invalid motion field path `{0}`")]
    InvalidPath(String),
    #[error("motion field `{path}` is missing")]
    MissingField { path: String },
    #[error("motion field `{path}` must be a finite bounded number")]
    InvalidNumber { path: String },
    #[error("motion track `{path}` must be an array")]
    TrackNotArray { path: String },
    #[error("motion track `{path}` has too many samples ({count}, maximum {maximum})")]
    TrackTooLong {
        path: String,
        count: usize,
        maximum: usize,
    },
    #[error("motion track `{path}` sample {index} has the wrong shape")]
    InvalidSample { path: String, index: usize },
    #[error("motion profile is invalid: {0}")]
    InvalidProfile(String),
    #[error("motion constructor must be an object")]
    ConstructorNotObject,
    #[error("unknown motion constructor `{0}`")]
    UnknownConstructor(String),
    #[error("motion constructor `{constructor}` has an invalid argument count")]
    InvalidArgumentCount { constructor: String },
    #[error("motion constructor `{constructor}` has unknown keyword `{keyword}`")]
    UnknownKeyword {
        constructor: String,
        keyword: String,
    },
    #[error("motion constructor `{constructor}` is missing keyword `{keyword}`")]
    MissingKeyword {
        constructor: String,
        keyword: String,
    },
    #[error("motion constructor `{constructor}` keyword `{keyword}` has the wrong type")]
    InvalidKeywordType {
        constructor: String,
        keyword: String,
    },
    #[error("motion scalar must use a static resource(...) or parameter(...) reference")]
    UntypedScalarReference,
    #[error("motion scalar reference constructor is malformed")]
    MalformedScalarReference,
    #[error("special attribute layout {layout} field {field_id} is missing or mismatched")]
    MissingSpecialAttribute { layout: u8, field_id: u16 },
}

/// Keep resource input bounded even when a resource pack contains a very
/// large otherwise valid array. The runtime track itself remains allocation
/// free after this one-time linking step.
pub const MAX_TRACK_SAMPLES: usize = 4096;

fn parse_profile_constructor(value: &Value) -> Result<MotionDescriptor, MotionLinkError> {
    let (constructor, args, keywords) = constructor_parts(value)?;
    require_constructor(&constructor, "motion.profile")?;
    if !args.is_empty() {
        return Err(MotionLinkError::InvalidArgumentCount { constructor });
    }
    reject_unknown(&constructor, &keywords, &["air", "ground"])?;
    let air = optional_keyword(&keywords, "air")
        .map(parse_air_operations)
        .transpose()?
        .unwrap_or_default();
    let ground = optional_keyword(&keywords, "ground")
        .map(parse_ground_operations)
        .transpose()?
        .unwrap_or_default();
    Ok(MotionDescriptor { air, ground })
}

fn parse_air_operations(value: &Value) -> Result<Vec<AirOperationDescriptor>, MotionLinkError> {
    let values = value
        .as_array()
        .ok_or_else(|| invalid_type("motion.profile", "air"))?;
    values.iter().map(parse_air_operation).collect()
}

fn parse_ground_operations(
    value: &Value,
) -> Result<Vec<GroundOperationDescriptor>, MotionLinkError> {
    let values = value
        .as_array()
        .ok_or_else(|| invalid_type("motion.profile", "ground"))?;
    values.iter().map(parse_ground_operation).collect()
}

fn parse_air_operation(value: &Value) -> Result<AirOperationDescriptor, MotionLinkError> {
    let (constructor, args, keywords) = constructor_parts(value)?;
    if !args.is_empty() {
        return Err(MotionLinkError::InvalidArgumentCount { constructor });
    }
    match constructor.as_str() {
        "motion.gravity" => {
            reject_unknown(
                &constructor,
                &keywords,
                &["acceleration", "terminal_velocity", "delay"],
            )?;
            Ok(AirOperationDescriptor::gravity(
                required_scalar(&keywords, &constructor, "acceleration")?,
                required_scalar(&keywords, &constructor, "terminal_velocity")?,
                required_scalar(&keywords, &constructor, "delay")?,
            ))
        }
        "motion.gravity_multiplier" => {
            reject_unknown(&constructor, &keywords, &["index", "value", "multiplier"])?;
            Ok(AirOperationDescriptor::gravity_multiplier(
                required_command_index(&keywords, &constructor, "index")?,
                required_command_value(&keywords, &constructor, "value")?,
                required_scalar(&keywords, &constructor, "multiplier")?,
            ))
        }
        "motion.friction" | "motion.air_friction" => {
            reject_unknown(&constructor, &keywords, &["amount", "path"])?;
            let amount = scalar_keyword_or_path(&keywords, &constructor, "amount", "path")?;
            Ok(AirOperationDescriptor::friction(amount))
        }
        "motion.vertical_gravity" => {
            reject_unknown(&constructor, &keywords, &["value", "path"])?;
            Ok(AirOperationDescriptor::vertical_gravity(
                scalar_keyword_or_path(&keywords, &constructor, "value", "path")?,
            ))
        }
        "motion.stick_steering" => {
            reject_unknown(
                &constructor,
                &keywords,
                &["threshold", "acceleration", "target"],
            )?;
            Ok(AirOperationDescriptor::stick_steering(
                required_scalar(&keywords, &constructor, "threshold")?,
                required_scalar(&keywords, &constructor, "acceleration")?,
                required_scalar(&keywords, &constructor, "target")?,
            ))
        }
        "motion.velocity_track" => {
            let track = parse_velocity_track(&constructor, &keywords)?;
            Ok(AirOperationDescriptor::velocity_track(track))
        }
        "motion.directional_acceleration" => {
            reject_unknown(&constructor, &keywords, &["starts_at", "magnitude"])?;
            Ok(AirOperationDescriptor::directional_acceleration(
                required_scalar(&keywords, &constructor, "starts_at")?,
                required_scalar(&keywords, &constructor, "magnitude")?,
            ))
        }
        "motion.drift_clamp" => {
            reject_unknown(&constructor, &keywords, &["maximum", "acceleration"])?;
            Ok(AirOperationDescriptor::drift_clamp(
                required_scalar(&keywords, &constructor, "maximum")?,
                required_scalar(&keywords, &constructor, "acceleration")?,
            ))
        }
        "motion.drift_or_friction" => {
            reject_unknown(&constructor, &keywords, &["recovery_step"])?;
            Ok(AirOperationDescriptor::drift_or_friction(required_scalar(
                &keywords,
                &constructor,
                "recovery_step",
            )?))
        }
        "motion.command_velocity_scale" => {
            reject_unknown(&constructor, &keywords, &["index", "value", "multiplier"])?;
            Ok(AirOperationDescriptor::command_velocity_scale(
                required_command_index(&keywords, &constructor, "index")?,
                required_command_value(&keywords, &constructor, "value")?,
                required_scalar(&keywords, &constructor, "multiplier")?,
            ))
        }
        "motion.command_branch" => {
            reject_unknown(&constructor, &keywords, &["index", "cases"])?;
            Ok(AirOperationDescriptor::command_branch(
                required_command_index(&keywords, &constructor, "index")?,
                parse_command_cases(
                    &constructor,
                    keywords
                        .get("cases")
                        .ok_or_else(|| MotionLinkError::MissingKeyword {
                            constructor: constructor.clone(),
                            keyword: "cases".to_owned(),
                        })?,
                    parse_air_operation,
                )?,
            ))
        }
        _ => Err(MotionLinkError::UnknownConstructor(constructor)),
    }
}

fn parse_ground_operation(value: &Value) -> Result<GroundOperationDescriptor, MotionLinkError> {
    let (constructor, args, keywords) = constructor_parts(value)?;
    if !args.is_empty() {
        return Err(MotionLinkError::InvalidArgumentCount { constructor });
    }
    match constructor.as_str() {
        "motion.ground_friction_above_walk" => {
            reject_unknown(&constructor, &keywords, &[])?;
            Ok(GroundOperationDescriptor::native_ground_friction())
        }
        "motion.friction" | "motion.ground_friction" => {
            reject_unknown(&constructor, &keywords, &["amount", "path"])?;
            Ok(GroundOperationDescriptor::friction(scalar_keyword_or_path(
                &keywords,
                &constructor,
                "amount",
                "path",
            )?))
        }
        "motion.stick_steering" => {
            reject_unknown(
                &constructor,
                &keywords,
                &["threshold", "acceleration", "target"],
            )?;
            Ok(GroundOperationDescriptor::stick_steering(
                required_scalar(&keywords, &constructor, "threshold")?,
                required_scalar(&keywords, &constructor, "acceleration")?,
                required_scalar(&keywords, &constructor, "target")?,
            ))
        }
        "motion.friction_after" | "motion.ground_friction_after" => {
            reject_unknown(&constructor, &keywords, &["starts_at", "before", "after"])?;
            Ok(GroundOperationDescriptor::friction_after(
                required_scalar(&keywords, &constructor, "starts_at")?,
                required_scalar(&keywords, &constructor, "before")?,
                required_scalar(&keywords, &constructor, "after")?,
            ))
        }
        "motion.target_track" => Ok(GroundOperationDescriptor::target_track(parse_scalar_track(
            &constructor,
            &keywords,
        )?)),
        "motion.command_branch" => {
            reject_unknown(&constructor, &keywords, &["index", "cases"])?;
            Ok(GroundOperationDescriptor::command_branch(
                required_command_index(&keywords, &constructor, "index")?,
                parse_command_cases(
                    &constructor,
                    keywords
                        .get("cases")
                        .ok_or_else(|| MotionLinkError::MissingKeyword {
                            constructor: constructor.clone(),
                            keyword: "cases".to_owned(),
                        })?,
                    parse_ground_operation,
                )?,
            ))
        }
        _ => Err(MotionLinkError::UnknownConstructor(constructor)),
    }
}

fn parse_command_cases<T>(
    constructor: &str,
    value: &Value,
    parse_operation: impl Fn(&Value) -> Result<T, MotionLinkError>,
) -> Result<BTreeMap<u32, Vec<T>>, MotionLinkError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_type(constructor, "cases"))?;
    if object.is_empty() {
        return Err(invalid_type(constructor, "cases"));
    }
    let mut cases = BTreeMap::new();
    for (raw_value, operations) in object {
        let value = raw_value
            .parse::<u32>()
            .ok()
            .filter(|value| *value <= MAX_COMMAND_VALUE)
            .ok_or_else(|| invalid_type(constructor, "cases"))?;
        let operations = operations
            .as_array()
            .ok_or_else(|| invalid_type(constructor, "cases"))?;
        if operations.is_empty() {
            return Err(invalid_type(constructor, "cases"));
        }
        let operations = operations
            .iter()
            .map(&parse_operation)
            .collect::<Result<Vec<_>, _>>()?;
        if cases.insert(value, operations).is_some() {
            return Err(invalid_type(constructor, "cases"));
        }
    }
    Ok(cases)
}

fn parse_velocity_track(
    constructor: &str,
    keywords: &BTreeMap<String, Value>,
) -> Result<VelocityTrackDescriptor, MotionLinkError> {
    reject_unknown(
        constructor,
        keywords,
        &[
            "path",
            "component",
            "end",
            "transform",
            "x_transform",
            "y_transform",
            "multiply_x_by_facing",
            "multiply_y_by_facing",
        ],
    )?;
    let path = required_field(keywords, constructor, "path")?;
    let component = optional_string(keywords, constructor, "component")?
        .map(|value| parse_component(constructor, value))
        .transpose()?
        .unwrap_or_default();
    let end = optional_string(keywords, constructor, "end")?
        .map(|value| parse_end(constructor, value))
        .transpose()?
        .unwrap_or_default();
    let default_transform = optional_string(keywords, constructor, "transform")?
        .map(|value| parse_transform(constructor, value))
        .transpose()?;
    let x_transform = optional_transform(
        keywords,
        constructor,
        "x_transform",
        "multiply_x_by_facing",
        default_transform.unwrap_or(TrackTransform::Absolute),
    )?;
    let y_transform = optional_transform(
        keywords,
        constructor,
        "y_transform",
        "multiply_y_by_facing",
        default_transform.unwrap_or(TrackTransform::Absolute),
    )?;
    Ok(VelocityTrackDescriptor {
        path,
        component,
        end,
        x_transform,
        y_transform,
    })
}

fn parse_scalar_track(
    constructor: &str,
    keywords: &BTreeMap<String, Value>,
) -> Result<ScalarTrackDescriptor, MotionLinkError> {
    reject_unknown(
        constructor,
        keywords,
        &["path", "end", "transform", "multiply_by_facing"],
    )?;
    let path = required_field(keywords, constructor, "path")?;
    let end = optional_string(keywords, constructor, "end")?
        .map(|value| parse_end(constructor, value))
        .transpose()?
        .unwrap_or_default();
    let transform = optional_transform(
        keywords,
        constructor,
        "transform",
        "multiply_by_facing",
        TrackTransform::Absolute,
    )?;
    Ok(ScalarTrackDescriptor {
        path,
        end,
        transform,
    })
}

fn optional_transform(
    keywords: &BTreeMap<String, Value>,
    constructor: &str,
    transform_key: &str,
    boolean_key: &str,
    default: TrackTransform,
) -> Result<TrackTransform, MotionLinkError> {
    let transform = optional_string(keywords, constructor, transform_key)?
        .map(|value| parse_transform(constructor, value))
        .transpose()?;
    let boolean = keywords
        .get(boolean_key)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| invalid_type(constructor, boolean_key))
        })
        .transpose()?;
    match (transform, boolean) {
        (Some(transform), Some(flag)) => {
            let binding_scale = transform == TrackTransform::BindingScale;
            if binding_scale == flag {
                Ok(transform)
            } else {
                Err(MotionLinkError::InvalidKeywordType {
                    constructor: constructor.to_owned(),
                    keyword: boolean_key.to_owned(),
                })
            }
        }
        // The long-form transform and legacy boolean are aliases. Permit
        // them together when they express the same binding-scale mode.
        (Some(transform), None) => Ok(transform),
        (None, Some(true)) => Ok(TrackTransform::BindingScale),
        (None, Some(false)) | (None, None) => Ok(default),
    }
}

fn parse_component(constructor: &str, value: &str) -> Result<TrackComponent, MotionLinkError> {
    match value {
        "x" => Ok(TrackComponent::X),
        "y" => Ok(TrackComponent::Y),
        "xy" | "both" => Ok(TrackComponent::Both),
        _ => Err(MotionLinkError::InvalidKeywordType {
            constructor: constructor.to_owned(),
            keyword: "component".to_owned(),
        }),
    }
}

fn parse_end(constructor: &str, value: &str) -> Result<TrackEnd, MotionLinkError> {
    match value {
        "no_op" => Ok(TrackEnd::NoOp),
        "hold_last" => Ok(TrackEnd::HoldLast),
        "loop" => Ok(TrackEnd::Loop),
        _ => Err(MotionLinkError::InvalidKeywordType {
            constructor: constructor.to_owned(),
            keyword: "end".to_owned(),
        }),
    }
}

fn parse_transform(constructor: &str, value: &str) -> Result<TrackTransform, MotionLinkError> {
    match value {
        "absolute" => Ok(TrackTransform::Absolute),
        "binding_scale" | "facing" => Ok(TrackTransform::BindingScale),
        _ => Err(MotionLinkError::InvalidKeywordType {
            constructor: constructor.to_owned(),
            keyword: "transform".to_owned(),
        }),
    }
}

fn scalar_keyword_or_path(
    keywords: &BTreeMap<String, Value>,
    constructor: &str,
    scalar_key: &str,
    path_key: &str,
) -> Result<ScalarRef, MotionLinkError> {
    match (keywords.get(scalar_key), keywords.get(path_key)) {
        (Some(_), Some(_)) => Err(MotionLinkError::InvalidKeywordType {
            constructor: constructor.to_owned(),
            keyword: scalar_key.to_owned(),
        }),
        (Some(value), None) => parse_scalar(value),
        (None, Some(value)) => parse_resource_path(value),
        (None, None) => Err(MotionLinkError::MissingKeyword {
            constructor: constructor.to_owned(),
            keyword: scalar_key.to_owned(),
        }),
    }
}

fn required_scalar(
    keywords: &BTreeMap<String, Value>,
    constructor: &str,
    keyword: &str,
) -> Result<ScalarRef, MotionLinkError> {
    let value = keywords
        .get(keyword)
        .ok_or_else(|| MotionLinkError::MissingKeyword {
            constructor: constructor.to_owned(),
            keyword: keyword.to_owned(),
        })?;
    parse_scalar(value)
}

fn required_command_index(
    keywords: &BTreeMap<String, Value>,
    constructor: &str,
    keyword: &str,
) -> Result<usize, MotionLinkError> {
    let value = keywords
        .get(keyword)
        .ok_or_else(|| MotionLinkError::MissingKeyword {
            constructor: constructor.to_owned(),
            keyword: keyword.to_owned(),
        })?;
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value < COMMAND_SLOTS)
        .ok_or_else(|| invalid_type(constructor, keyword))
}

fn required_command_value(
    keywords: &BTreeMap<String, Value>,
    constructor: &str,
    keyword: &str,
) -> Result<u32, MotionLinkError> {
    let value = keywords
        .get(keyword)
        .ok_or_else(|| MotionLinkError::MissingKeyword {
            constructor: constructor.to_owned(),
            keyword: keyword.to_owned(),
        })?;
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value <= MAX_COMMAND_VALUE)
        .ok_or_else(|| invalid_type(constructor, keyword))
}

fn parse_scalar(value: &Value) -> Result<ScalarRef, MotionLinkError> {
    if let Some(value) = value.as_f64() {
        let value = value as f32;
        return checked_number("literal", value).map(ScalarRef::Literal);
    }
    let (constructor, args, keywords) =
        constructor_parts(value).map_err(|_| MotionLinkError::UntypedScalarReference)?;
    if !keywords.is_empty() || args.len() != 1 {
        return Err(MotionLinkError::MalformedScalarReference);
    }
    if constructor == "special_attribute" || constructor == "motion.special_attribute" {
        let reference: SpecialAttributeRef = serde_json::from_value(args[0].clone())
            .map_err(|_| MotionLinkError::MalformedScalarReference)?;
        return Ok(ScalarRef::SpecialAttribute(reference));
    }
    if !args.iter().all(Value::is_string) {
        return Err(MotionLinkError::UntypedScalarReference);
    }
    let path = FieldRef::new(args[0].as_str().unwrap_or_default())?;
    match constructor.as_str() {
        "resource" | "motion.resource" => Ok(ScalarRef::Resource(path)),
        "parameter" | "motion.parameter" => Ok(ScalarRef::Parameter(path)),
        _ => Err(MotionLinkError::UntypedScalarReference),
    }
}

fn parse_resource_path(value: &Value) -> Result<ScalarRef, MotionLinkError> {
    let path = value
        .as_str()
        .ok_or_else(|| invalid_type("motion", "path"))?;
    Ok(ScalarRef::Resource(FieldRef::new(path)?))
}

fn parse_field_value(
    value: &Value,
    constructor: &str,
    keyword: &str,
) -> Result<FieldRef, MotionLinkError> {
    // A path keyword is itself a field position in the constructor schema, so
    // the baseline manifest represents it as a string.  Scalar values use
    // `resource(...)`/`parameter(...)` wrappers and are rejected as bare
    // strings by `parse_scalar`.
    if let Some(path) = value.as_str() {
        return FieldRef::new(path);
    }
    let (reference, args, kwargs) =
        constructor_parts(value).map_err(|_| invalid_type(constructor, keyword))?;
    if !matches!(reference.as_str(), "resource" | "motion.resource")
        || !kwargs.is_empty()
        || args.len() != 1
        || !args[0].is_string()
    {
        return Err(invalid_type(constructor, keyword));
    }
    FieldRef::new(args[0].as_str().unwrap_or_default())
}

fn required_field(
    keywords: &BTreeMap<String, Value>,
    constructor: &str,
    keyword: &str,
) -> Result<FieldRef, MotionLinkError> {
    let value = keywords
        .get(keyword)
        .ok_or_else(|| MotionLinkError::MissingKeyword {
            constructor: constructor.to_owned(),
            keyword: keyword.to_owned(),
        })?;
    parse_field_value(value, constructor, keyword)
}

fn optional_string<'a>(
    keywords: &'a BTreeMap<String, Value>,
    constructor: &str,
    keyword: &str,
) -> Result<Option<&'a str>, MotionLinkError> {
    keywords
        .get(keyword)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| invalid_type(constructor, keyword))
        })
        .transpose()
}

type ConstructorParts = (String, Vec<Value>, BTreeMap<String, Value>);

fn constructor_parts(value: &Value) -> Result<ConstructorParts, MotionLinkError> {
    let object = value
        .as_object()
        .ok_or(MotionLinkError::ConstructorNotObject)?;
    let constructor_key = "callee";
    let constructor = object
        .get(constructor_key)
        .and_then(Value::as_str)
        .ok_or(MotionLinkError::ConstructorNotObject)?
        .to_owned();
    let args = object
        .get("args")
        .map(|value| {
            value
                .as_array()
                .cloned()
                .ok_or_else(|| invalid_type(&constructor, "args"))
        })
        .transpose()?
        .unwrap_or_default();
    let keywords = object
        .get("kwargs")
        .map(|value| {
            value
                .as_object()
                .cloned()
                .map(|value| value.into_iter().collect::<BTreeMap<_, _>>())
                .ok_or_else(|| invalid_type(&constructor, "kwargs"))
        })
        .transpose()?
        .unwrap_or_default();
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "callee" | "args" | "kwargs"))
    {
        return Err(MotionLinkError::UnknownKeyword {
            constructor,
            keyword: "object field".to_owned(),
        });
    }
    Ok((constructor, args, keywords))
}

fn require_constructor(actual: &str, expected: &str) -> Result<(), MotionLinkError> {
    if actual == expected {
        Ok(())
    } else {
        Err(MotionLinkError::UnknownConstructor(actual.to_owned()))
    }
}

fn reject_unknown(
    constructor: &str,
    keywords: &BTreeMap<String, Value>,
    allowed: &[&str],
) -> Result<(), MotionLinkError> {
    if let Some(keyword) = keywords
        .keys()
        .find(|keyword| !allowed.contains(&keyword.as_str()))
    {
        return Err(MotionLinkError::UnknownKeyword {
            constructor: constructor.to_owned(),
            keyword: keyword.clone(),
        });
    }
    Ok(())
}

fn optional_keyword<'a>(keywords: &'a BTreeMap<String, Value>, key: &str) -> Option<&'a Value> {
    keywords.get(key)
}

fn invalid_type(constructor: &str, keyword: &str) -> MotionLinkError {
    MotionLinkError::InvalidKeywordType {
        constructor: constructor.to_owned(),
        keyword: keyword.to_owned(),
    }
}

/// Resolve one static descriptor into the executable immutable native profile.
pub fn link_profile<R: MotionResourceSource, P: MotionParameterSource>(
    descriptor: &MotionDescriptor,
    resources: &R,
    parameters: &P,
) -> Result<MotionProfile, MotionLinkError> {
    let profile = MotionProfile {
        air: descriptor
            .air
            .iter()
            .map(|operation| link_air(operation, resources, parameters))
            .collect::<Result<_, _>>()?,
        ground: descriptor
            .ground
            .iter()
            .map(|operation| link_ground(operation, resources, parameters))
            .collect::<Result<_, _>>()?,
    };
    profile
        .validate()
        .map_err(|error| MotionLinkError::InvalidProfile(error.to_string()))?;
    Ok(profile)
}

fn link_air<R: MotionResourceSource, P: MotionParameterSource>(
    operation: &AirOperationDescriptor,
    resources: &R,
    parameters: &P,
) -> Result<AirOperation, MotionLinkError> {
    Ok(match operation {
        AirOperationDescriptor::Gravity {
            acceleration,
            terminal_velocity,
            delay,
        } => AirOperation::Gravity(super::motion::Gravity {
            acceleration: resolve(acceleration, resources, parameters)?,
            terminal_velocity: resolve(terminal_velocity, resources, parameters)?,
            delay: resolve(delay, resources, parameters)?,
        }),
        AirOperationDescriptor::GravityMultiplier {
            index,
            value,
            multiplier,
        } => AirOperation::GravityMultiplier(GravityMultiplier::new(
            *index,
            *value,
            resolve(multiplier, resources, parameters)?,
        )),
        AirOperationDescriptor::Friction { amount } => AirOperation::Friction {
            amount: resolve(amount, resources, parameters)?,
        },
        AirOperationDescriptor::VerticalGravity { gravity } => AirOperation::VerticalGravity {
            gravity: resolve(gravity, resources, parameters)?,
        },
        AirOperationDescriptor::StickSteering {
            threshold,
            acceleration,
            target,
        } => AirOperation::StickSteering(StickSteering::new(
            resolve(threshold, resources, parameters)?,
            resolve(acceleration, resources, parameters)?,
            resolve(target, resources, parameters)?,
        )),
        AirOperationDescriptor::VelocityTrack(track) => {
            AirOperation::VelocityTrack(link_velocity_track(track, resources)?)
        }
        AirOperationDescriptor::DirectionalAcceleration {
            starts_at,
            magnitude,
        } => AirOperation::DirectionalAcceleration {
            starts_at: resolve(starts_at, resources, parameters)?,
            magnitude: resolve(magnitude, resources, parameters)?,
        },
        AirOperationDescriptor::DriftClamp {
            maximum,
            acceleration,
        } => AirOperation::DriftClamp {
            maximum: resolve(maximum, resources, parameters)?,
            acceleration: resolve(acceleration, resources, parameters)?,
        },
        AirOperationDescriptor::DriftOrFriction { recovery_step } => {
            AirOperation::DriftOrFriction {
                recovery_step: resolve(recovery_step, resources, parameters)?,
            }
        }
        AirOperationDescriptor::CommandVelocityScale {
            index,
            value,
            multiplier,
        } => AirOperation::CommandVelocityScale {
            index: *index,
            value: *value,
            multiplier: resolve(multiplier, resources, parameters)?,
        },
        AirOperationDescriptor::CommandBranch(branch) => {
            let cases = branch
                .cases
                .iter()
                .map(|(value, operations)| {
                    Ok((
                        *value,
                        operations
                            .iter()
                            .map(|operation| link_air(operation, resources, parameters))
                            .collect::<Result<Vec<_>, _>>()?,
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, MotionLinkError>>()?;
            AirOperation::CommandBranch(CommandBranch::new(branch.index, cases))
        }
    })
}

fn link_ground<R: MotionResourceSource, P: MotionParameterSource>(
    operation: &GroundOperationDescriptor,
    resources: &R,
    parameters: &P,
) -> Result<GroundOperation, MotionLinkError> {
    Ok(match operation {
        GroundOperationDescriptor::Friction { amount } => GroundOperation::Friction {
            amount: resolve(amount, resources, parameters)?,
        },
        GroundOperationDescriptor::FrictionAboveWalk {
            amount,
            walk_max_velocity,
            above_walk_multiplier,
        } => GroundOperation::FrictionAboveWalk {
            amount: resolve(amount, resources, parameters)?,
            walk_max_velocity: resolve(walk_max_velocity, resources, parameters)?,
            above_walk_multiplier: resolve(above_walk_multiplier, resources, parameters)?,
        },
        GroundOperationDescriptor::StickSteering {
            threshold,
            acceleration,
            target,
        } => GroundOperation::StickSteering(StickSteering::new(
            resolve(threshold, resources, parameters)?,
            resolve(acceleration, resources, parameters)?,
            resolve(target, resources, parameters)?,
        )),
        GroundOperationDescriptor::FrictionAfter {
            starts_at,
            before,
            after,
        } => GroundOperation::FrictionAfter {
            starts_at: resolve(starts_at, resources, parameters)?,
            before: resolve(before, resources, parameters)?,
            after: resolve(after, resources, parameters)?,
        },
        GroundOperationDescriptor::TargetTrack(track) => {
            GroundOperation::TargetTrack(link_scalar_track(track, resources)?)
        }
        GroundOperationDescriptor::CommandBranch(branch) => {
            let cases = branch
                .cases
                .iter()
                .map(|(value, operations)| {
                    Ok((
                        *value,
                        operations
                            .iter()
                            .map(|operation| link_ground(operation, resources, parameters))
                            .collect::<Result<Vec<_>, _>>()?,
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, MotionLinkError>>()?;
            GroundOperation::CommandBranch(CommandBranch::new(branch.index, cases))
        }
    })
}

fn resolve<R: MotionResourceSource, P: MotionParameterSource>(
    value: &ScalarRef,
    resources: &R,
    parameters: &P,
) -> Result<f32, MotionLinkError> {
    let result = match value {
        ScalarRef::Literal(value) => return checked_number("literal", *value),
        ScalarRef::Resource(path) => {
            let value = resources.value_path(path.as_str()).ok_or_else(|| {
                MotionLinkError::MissingField {
                    path: path.as_str().to_owned(),
                }
            })?;
            number(value).ok_or_else(|| MotionLinkError::InvalidNumber {
                path: path.as_str().to_owned(),
            })?
        }
        ScalarRef::Parameter(path) => {
            let value = parameters.number_path(path.as_str()).ok_or_else(|| {
                MotionLinkError::MissingField {
                    path: path.as_str().to_owned(),
                }
            })?;
            return checked_number(path.as_str(), value);
        }
        ScalarRef::SpecialAttribute(reference) => {
            let value = parameters.special_attribute(*reference).ok_or({
                MotionLinkError::MissingSpecialAttribute {
                    layout: reference.layout,
                    field_id: reference.field_id,
                }
            })?;
            return checked_number("special attribute", value);
        }
    };
    Ok(result)
}

fn link_velocity_track<R: MotionResourceSource>(
    descriptor: &VelocityTrackDescriptor,
    resources: &R,
) -> Result<VelocityTrack, MotionLinkError> {
    let path = descriptor.path.as_str();
    let value = resources
        .value_path(path)
        .ok_or_else(|| MotionLinkError::MissingField {
            path: path.to_owned(),
        })?;
    let values = value
        .as_array()
        .ok_or_else(|| MotionLinkError::TrackNotArray {
            path: path.to_owned(),
        })?;
    check_track_length(path, values.len())?;
    let samples = values
        .iter()
        .enumerate()
        .map(|(index, value)| velocity_sample(path, index, value, descriptor.component))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(VelocityTrack::with_transforms(
        samples,
        descriptor.end,
        descriptor.x_transform,
        descriptor.y_transform,
    ))
}

fn velocity_sample(
    path: &str,
    index: usize,
    value: &Value,
    component: TrackComponent,
) -> Result<VelocitySample, MotionLinkError> {
    if let Some(number) = number(value) {
        return Ok(match component {
            TrackComponent::X => VelocitySample::x(number),
            TrackComponent::Y => VelocitySample::y(number),
            TrackComponent::Both => {
                return Err(MotionLinkError::InvalidSample {
                    path: path.to_owned(),
                    index,
                });
            }
        });
    }
    let Some(values) = value.as_array() else {
        return Err(MotionLinkError::InvalidSample {
            path: path.to_owned(),
            index,
        });
    };
    if values.len() != 2 {
        return Err(MotionLinkError::InvalidSample {
            path: path.to_owned(),
            index,
        });
    }
    let x = values.first().and_then(number);
    let y = values.get(1).and_then(number);
    Ok(match component {
        TrackComponent::X => {
            VelocitySample::x(x.ok_or_else(|| MotionLinkError::InvalidSample {
                path: path.to_owned(),
                index,
            })?)
        }
        TrackComponent::Y => {
            VelocitySample::y(y.ok_or_else(|| MotionLinkError::InvalidSample {
                path: path.to_owned(),
                index,
            })?)
        }
        TrackComponent::Both => VelocitySample::xy(
            x.ok_or_else(|| MotionLinkError::InvalidSample {
                path: path.to_owned(),
                index,
            })?,
            y.ok_or_else(|| MotionLinkError::InvalidSample {
                path: path.to_owned(),
                index,
            })?,
        ),
    })
}

fn link_scalar_track<R: MotionResourceSource>(
    descriptor: &ScalarTrackDescriptor,
    resources: &R,
) -> Result<ScalarTrack, MotionLinkError> {
    let path = descriptor.path.as_str();
    let value = resources
        .value_path(path)
        .ok_or_else(|| MotionLinkError::MissingField {
            path: path.to_owned(),
        })?;
    let values = value
        .as_array()
        .ok_or_else(|| MotionLinkError::TrackNotArray {
            path: path.to_owned(),
        })?;
    check_track_length(path, values.len())?;
    let samples = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if value.is_null() {
                Ok(None)
            } else {
                number(value)
                    .map(Some)
                    .ok_or_else(|| MotionLinkError::InvalidSample {
                        path: path.to_owned(),
                        index,
                    })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ScalarTrack::with_transform(
        samples,
        descriptor.end,
        descriptor.transform,
    ))
}

fn check_track_length(path: &str, count: usize) -> Result<(), MotionLinkError> {
    if count > MAX_TRACK_SAMPLES {
        Err(MotionLinkError::TrackTooLong {
            path: path.to_owned(),
            count,
            maximum: MAX_TRACK_SAMPLES,
        })
    } else {
        Ok(())
    }
}

fn number(value: &Value) -> Option<f32> {
    value.as_f64().and_then(|value| {
        let value = value as f32;
        (value.is_finite() && value.abs() <= 1_000_000.0).then_some(value)
    })
}

fn checked_number(path: &str, value: f32) -> Result<f32, MotionLinkError> {
    (value.is_finite() && value.abs() <= 1_000_000.0)
        .then_some(value)
        .ok_or_else(|| MotionLinkError::InvalidNumber {
            path: path.to_owned(),
        })
}

fn validate_path(path: &str) -> Result<(), MotionLinkError> {
    if path.is_empty()
        || path.len() > 256
        || path.split('.').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        })
    {
        Err(MotionLinkError::InvalidPath(path.to_owned()))
    } else {
        Ok(())
    }
}

fn json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(value, |value, part| value.get(part))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Default)]
    struct SyntheticResources(BTreeMap<String, Value>);

    impl MotionResourceSource for SyntheticResources {
        fn value_path(&self, path: &str) -> Option<&Value> {
            self.0.get(path).or_else(|| {
                let parts = path.split('.').collect::<Vec<_>>();
                (1..parts.len()).rev().find_map(|split| {
                    self.0
                        .get(&parts[..split].join("."))
                        .and_then(|value| json_path(value, &parts[split..].join(".")))
                })
            })
        }
    }

    fn field(path: &str) -> FieldRef {
        FieldRef::new(path).expect("test field path")
    }

    fn constructor(callee: &str, kwargs: Value) -> Value {
        json!({
            "callee": callee,
            "args": [],
            "kwargs": kwargs,
        })
    }

    fn resource(path: &str) -> Value {
        json!({"callee": "resource", "args": [path], "kwargs": {}})
    }

    fn parameter(path: &str) -> Value {
        json!({"callee": "parameter", "args": [path], "kwargs": {}})
    }

    fn special_attribute(layout: u8, field_id: u16) -> Value {
        json!({
            "callee": "special_attribute",
            "args": [{"layout": layout, "field_id": field_id}],
            "kwargs": {}
        })
    }

    #[derive(Default)]
    struct SyntheticParameters {
        values: BTreeMap<String, f32>,
        attribute: Option<(SpecialAttributeRef, f32)>,
    }

    impl MotionParameterSource for SyntheticParameters {
        fn number_path(&self, path: &str) -> Option<f32> {
            self.values.get(path).copied()
        }

        fn special_attribute(&self, reference: SpecialAttributeRef) -> Option<f32> {
            self.attribute
                .filter(|(actual, _)| *actual == reference)
                .map(|(_, value)| value)
        }
    }

    #[test]
    fn links_generic_numeric_operations_without_character_names() {
        let resources = SyntheticResources(BTreeMap::from([(
            "move.attributes".into(),
            json!({"gravity": 0.25, "delay": 2.0, "friction": 0.1}),
        )]));
        let descriptor = MotionDescriptor::profile(
            [AirOperationDescriptor::Gravity {
                acceleration: ScalarRef::Resource(field("move.attributes.gravity")),
                terminal_velocity: ScalarRef::Parameter(field("movement.terminal_velocity")),
                delay: ScalarRef::Resource(field("move.attributes.delay")),
            }],
            [GroundOperationDescriptor::Friction {
                amount: ScalarRef::Resource(field("move.attributes.friction")),
            }],
        );
        let parameters = BTreeMap::from([(String::from("movement.terminal_velocity"), 9.0)]);
        let profile = link_profile(&descriptor, &resources, &parameters).expect("profile");
        assert_eq!(profile.air.len(), 1);
        assert_eq!(profile.ground.len(), 1);
        assert_eq!(profile.initial_gravity_delay(), Some(2.0));
    }

    #[test]
    fn links_optional_scalar_and_vector_tracks_once() {
        let resources = SyntheticResources(BTreeMap::from([
            ("dash.ground".into(), json!([2.0, null, 3.0])),
            ("dash.air".into(), json!([[2.0, 0.0], [2.5, -0.1]])),
        ]));
        let descriptor = MotionDescriptor::profile(
            [AirOperationDescriptor::VelocityTrack(
                VelocityTrackDescriptor {
                    path: field("dash.air"),
                    component: TrackComponent::Both,
                    end: TrackEnd::NoOp,
                    x_transform: TrackTransform::BindingScale,
                    y_transform: TrackTransform::Absolute,
                },
            )],
            [GroundOperationDescriptor::TargetTrack(
                ScalarTrackDescriptor::new(field("dash.ground")),
            )],
        );
        let profile = link_profile(&descriptor, &resources, &BTreeMap::new()).expect("profile");
        match &profile.air[0] {
            AirOperation::VelocityTrack(track) => assert_eq!(track.samples.len(), 2),
            operation => panic!("unexpected operation {operation:?}"),
        }
        match &profile.ground[0] {
            GroundOperation::TargetTrack(track) => {
                assert_eq!(track.samples, vec![Some(2.0), None, Some(3.0)])
            }
            operation => panic!("unexpected operation {operation:?}"),
        }
    }

    #[test]
    fn component_specific_velocity_tracks_allow_null_unused_axes() {
        let resources = SyntheticResources(BTreeMap::from([
            ("x.track".into(), json!([[2.0, null], [3.0, null]])),
            ("y.track".into(), json!([[null, -1.0], [null, -2.0]])),
        ]));
        let descriptor = MotionDescriptor::profile(
            [
                AirOperationDescriptor::VelocityTrack(VelocityTrackDescriptor {
                    path: field("x.track"),
                    component: TrackComponent::X,
                    end: TrackEnd::NoOp,
                    x_transform: TrackTransform::Absolute,
                    y_transform: TrackTransform::Absolute,
                }),
                AirOperationDescriptor::VelocityTrack(VelocityTrackDescriptor {
                    path: field("y.track"),
                    component: TrackComponent::Y,
                    end: TrackEnd::NoOp,
                    x_transform: TrackTransform::Absolute,
                    y_transform: TrackTransform::Absolute,
                }),
            ],
            [],
        );
        let profile = link_profile(&descriptor, &resources, &BTreeMap::new())
            .expect("component-specific tracks link");
        assert!(matches!(
            &profile.air[0],
            AirOperation::VelocityTrack(track)
                if track.samples == vec![VelocitySample::x(2.0), VelocitySample::x(3.0)]
        ));
        assert!(matches!(
            &profile.air[1],
            AirOperation::VelocityTrack(track)
                if track.samples == vec![VelocitySample::y(-1.0), VelocitySample::y(-2.0)]
        ));
    }

    #[test]
    fn rejects_dynamic_paths_missing_fields_and_bad_track_shapes() {
        assert!(matches!(
            FieldRef::new("move[frame]"),
            Err(MotionLinkError::InvalidPath(_))
        ));
        let resources = SyntheticResources(BTreeMap::new());
        let descriptor = MotionDescriptor::profile(
            [AirOperationDescriptor::Friction {
                amount: ScalarRef::Resource(field("missing.value")),
            }],
            [],
        );
        assert!(matches!(
            link_profile(&descriptor, &resources, &BTreeMap::new()),
            Err(MotionLinkError::MissingField { .. })
        ));

        let resources = SyntheticResources(BTreeMap::from([("track".into(), json!([[1.0]]))]));
        let descriptor = MotionDescriptor::profile(
            [AirOperationDescriptor::VelocityTrack(
                VelocityTrackDescriptor::new(field("track")),
            )],
            [],
        );
        assert!(matches!(
            link_profile(&descriptor, &resources, &BTreeMap::new()),
            Err(MotionLinkError::InvalidSample { .. })
        ));
    }

    #[test]
    fn decodes_compiled_motion_constructor_and_preserves_typed_refs() {
        let gravity = constructor(
            "motion.gravity",
            json!({
                "acceleration": resource("move.gravity"),
                "terminal_velocity": parameter("movement.terminal_velocity"),
                "delay": 2.0,
            }),
        );
        let profile = constructor(
            "motion.profile",
            json!({
                "air": [gravity],
                "ground": [constructor(
                    "motion.friction",
                    json!({"path": "move.friction"}),
                )],
            }),
        );
        let descriptor = MotionDescriptor::from_compiled_constructor(&profile).expect("decode");
        assert!(matches!(
            descriptor.air.first(),
            Some(AirOperationDescriptor::Gravity {
                acceleration: ScalarRef::Resource(_),
                terminal_velocity: ScalarRef::Parameter(_),
                ..
            })
        ));
        let resources = SyntheticResources(BTreeMap::from([
            ("move.gravity".into(), json!(0.25)),
            ("move.friction".into(), json!(0.1)),
        ]));
        let parameters = BTreeMap::from([(String::from("movement.terminal_velocity"), 9.0)]);
        assert!(link_profile(&descriptor, &resources, &parameters).is_ok());
    }

    #[test]
    fn parses_and_links_native_ground_friction_without_script_paths() {
        let profile = constructor(
            "motion.profile",
            json!({
                "ground": [constructor("motion.ground_friction_above_walk", json!({}))]
            }),
        );
        let descriptor = MotionDescriptor::from_compiled_constructor(&profile).expect("decode");
        assert!(matches!(
            descriptor.ground.first(),
            Some(GroundOperationDescriptor::FrictionAboveWalk {
                amount: ScalarRef::Parameter(amount),
                walk_max_velocity: ScalarRef::Parameter(walk_max),
                above_walk_multiplier: ScalarRef::Parameter(multiplier),
            }) if amount.as_str() == "movement.ground_friction"
                && walk_max.as_str() == "movement.walk_max_velocity"
                && multiplier.as_str() == "rules.friction_above_walk"
        ));

        let parameters = BTreeMap::from([
            (String::from("movement.ground_friction"), 0.2),
            (String::from("movement.walk_max_velocity"), 1.5),
            (String::from("rules.friction_above_walk"), 2.0),
        ]);
        let linked =
            link_profile(&descriptor, &SyntheticResources::default(), &parameters).expect("link");
        assert!(matches!(
            linked.ground.first(),
            Some(GroundOperation::FrictionAboveWalk {
                amount,
                walk_max_velocity,
                above_walk_multiplier,
            }) if (*amount, *walk_max_velocity, *above_walk_multiplier) == (0.2, 1.5, 2.0)
        ));
    }

    #[test]
    fn native_ground_friction_rejects_invalid_linked_values() {
        let descriptor =
            MotionDescriptor::profile([], [GroundOperationDescriptor::native_ground_friction()]);
        let parameters = BTreeMap::from([
            (String::from("movement.ground_friction"), 0.2),
            (String::from("movement.walk_max_velocity"), 1.5),
            (String::from("rules.friction_above_walk"), -1.0),
        ]);
        assert!(matches!(
            link_profile(&descriptor, &SyntheticResources::default(), &parameters),
            Err(MotionLinkError::InvalidProfile(message))
                if message.contains("ground friction multiplier")
        ));
    }

    #[test]
    fn special_attribute_reference_is_typed_and_resolved_once() {
        let reference = SpecialAttributeRef::new(3, 6);
        assert_eq!(
            serde_json::to_value(reference).expect("serialize typed reference"),
            json!({"layout": 3, "field_id": 6})
        );
        let descriptor = MotionDescriptor::profile(
            [AirOperationDescriptor::Gravity {
                acceleration: ScalarRef::special_attribute(reference),
                terminal_velocity: ScalarRef::literal(9.0),
                delay: ScalarRef::literal(0.0),
            }],
            [],
        );
        let value = constructor(
            "motion.gravity",
            json!({
                "acceleration": special_attribute(3, 6),
                "terminal_velocity": 9.0,
                "delay": 0.0,
            }),
        );
        let parsed = MotionDescriptor::from_compiled_constructor(&constructor(
            "motion.profile",
            json!({"air": [value]}),
        ))
        .expect("typed special attribute descriptor");
        assert!(matches!(
            parsed.air.first(),
            Some(AirOperationDescriptor::Gravity {
                acceleration: ScalarRef::SpecialAttribute(actual),
                ..
            }) if *actual == reference
        ));
        let parameters = SyntheticParameters {
            attribute: Some((reference, 0.75)),
            ..SyntheticParameters::default()
        };
        let profile = link_profile(&descriptor, &SyntheticResources::default(), &parameters)
            .expect("special attribute links");
        assert_eq!(profile.initial_gravity_delay(), Some(0.0));
        assert!(
            matches!(profile.air.first(), Some(AirOperation::Gravity(gravity)) if gravity.acceleration == 0.75)
        );
    }

    #[test]
    fn special_attribute_missing_or_mismatched_layout_fails_closed() {
        let descriptor = MotionDescriptor::profile(
            [AirOperationDescriptor::Friction {
                amount: ScalarRef::special_attribute(SpecialAttributeRef::new(4, 2)),
            }],
            [],
        );
        let absent = SyntheticParameters::default();
        assert!(matches!(
            link_profile(&descriptor, &SyntheticResources::default(), &absent),
            Err(MotionLinkError::MissingSpecialAttribute {
                layout: 4,
                field_id: 2
            })
        ));
        let mismatched = SyntheticParameters {
            attribute: Some((SpecialAttributeRef::new(3, 2), 1.0)),
            ..SyntheticParameters::default()
        };
        assert!(matches!(
            link_profile(&descriptor, &SyntheticResources::default(), &mismatched),
            Err(MotionLinkError::MissingSpecialAttribute {
                layout: 4,
                field_id: 2
            })
        ));
    }

    #[test]
    fn gravity_multiplier_decodes_gates_and_links_typed_multiplier() {
        let reference = SpecialAttributeRef::new(3, 3);
        let operation = constructor(
            "motion.gravity_multiplier",
            json!({
                "index": 0,
                "value": 0,
                "multiplier": special_attribute(3, 3),
            }),
        );
        let descriptor = MotionDescriptor::from_compiled_constructor(&constructor(
            "motion.profile",
            json!({"air": [operation]}),
        ))
        .expect("gravity multiplier descriptor");
        assert!(matches!(
            descriptor.air.first(),
            Some(AirOperationDescriptor::GravityMultiplier {
                index: 0,
                value: 0,
                multiplier: ScalarRef::SpecialAttribute(actual),
            }) if *actual == reference
        ));
        let parameters = SyntheticParameters {
            attribute: Some((reference, 1.5)),
            ..SyntheticParameters::default()
        };
        let profile = link_profile(&descriptor, &SyntheticResources::default(), &parameters)
            .expect("gravity multiplier links");
        assert_eq!(
            profile.air,
            vec![AirOperation::GravityMultiplier(GravityMultiplier::new(
                0, 0, 1.5
            ))]
        );

        let missing = SyntheticParameters::default();
        assert!(matches!(
            link_profile(&descriptor, &SyntheticResources::default(), &missing),
            Err(MotionLinkError::MissingSpecialAttribute {
                layout: 3,
                field_id: 3
            })
        ));
    }

    #[test]
    fn compiled_constructor_rejects_unknown_keywords_and_untyped_scalars() {
        let bad_keyword = constructor("motion.profile", json!({"side": []}));
        assert!(matches!(
            MotionDescriptor::from_compiled_constructor(&bad_keyword),
            Err(MotionLinkError::UnknownKeyword { .. })
        ));
        let untyped = constructor(
            "motion.gravity",
            json!({
                "acceleration": "move.gravity",
                "terminal_velocity": 9.0,
                "delay": 0.0,
            }),
        );
        let profile = constructor("motion.profile", json!({"air": [untyped]}));
        assert!(matches!(
            MotionDescriptor::from_compiled_constructor(&profile),
            Err(MotionLinkError::UntypedScalarReference)
        ));
    }

    #[test]
    fn equivalent_transform_aliases_are_accepted_together() {
        let operation = constructor(
            "motion.velocity_track",
            json!({
                "path": "motion.velocity",
                "transform": "binding_scale",
                "multiply_x_by_facing": true,
            }),
        );
        let profile = constructor("motion.profile", json!({"air": [operation]}));
        let descriptor = MotionDescriptor::from_compiled_constructor(&profile)
            .expect("equivalent transform aliases should agree");
        assert!(matches!(
            descriptor.air.first(),
            Some(AirOperationDescriptor::VelocityTrack(track))
                if track.x_transform == TrackTransform::BindingScale
        ));
    }

    #[test]
    fn absolute_transform_and_false_legacy_alias_are_accepted_together() {
        let operation = constructor(
            "motion.velocity_track",
            json!({
                "path": "motion.velocity",
                "transform": "absolute",
                "multiply_x_by_facing": false,
            }),
        );
        let profile = constructor("motion.profile", json!({"air": [operation]}));
        let descriptor = MotionDescriptor::from_compiled_constructor(&profile)
            .expect("equivalent absolute aliases should agree");
        assert!(matches!(
            descriptor.air.first(),
            Some(AirOperationDescriptor::VelocityTrack(track))
                if track.x_transform == TrackTransform::Absolute
        ));
    }

    #[test]
    fn decodes_and_links_command_velocity_scale() {
        let operation = constructor(
            "motion.command_velocity_scale",
            json!({
                "index": 1,
                "value": 1,
                "multiplier": resource("move.attributes.specialn_vel_mul"),
            }),
        );
        let descriptor = MotionDescriptor::from_compiled_constructor(&constructor(
            "motion.profile",
            json!({"air": [operation]}),
        ))
        .expect("descriptor");
        assert!(matches!(
            descriptor.air.first(),
            Some(AirOperationDescriptor::CommandVelocityScale {
                index: 1,
                value: 1,
                multiplier: ScalarRef::Resource(_),
            })
        ));
        let resources = SyntheticResources(BTreeMap::from([(
            "move.attributes.specialn_vel_mul".into(),
            json!(0.5),
        )]));
        let profile = link_profile(&descriptor, &resources, &BTreeMap::new()).expect("profile");
        assert_eq!(
            profile.air,
            vec![AirOperation::CommandVelocityScale {
                index: 1,
                value: 1,
                multiplier: 0.5,
            }]
        );
    }

    #[test]
    fn decodes_and_links_numeric_command_branch_cases() {
        let operation = constructor(
            "motion.command_branch",
            json!({
                "index": 1,
                "cases": {
                    "0": [constructor("motion.gravity", json!({
                        "acceleration": 0.25,
                        "terminal_velocity": 9.0,
                        "delay": 0.0,
                    }))],
                    "1": [constructor("motion.command_velocity_scale", json!({
                        "index": 1,
                        "value": 1,
                        "multiplier": 0.5,
                    }))],
                },
            }),
        );
        let descriptor = MotionDescriptor::from_compiled_constructor(&constructor(
            "motion.profile",
            json!({"air": [operation]}),
        ))
        .expect("command branch descriptor");
        let profile = link_profile(
            &descriptor,
            &SyntheticResources::default(),
            &BTreeMap::new(),
        )
        .expect("command branch profile");
        assert!(matches!(
            profile.air.first(),
            Some(AirOperation::CommandBranch(branch))
                if branch.index == 1
                    && branch.cases.len() == 2
                    && matches!(branch.cases.get(&1).and_then(|ops| ops.first()),
                        Some(AirOperation::CommandVelocityScale { value: 1, .. }))
        ));
    }

    #[test]
    fn command_branch_wire_validation_rejects_non_numeric_and_empty_cases() {
        for cases in [json!({"fast": []}), json!({"0": []})] {
            let operation =
                constructor("motion.command_branch", json!({"index": 0, "cases": cases}));
            let profile = constructor("motion.profile", json!({"air": [operation]}));
            assert!(matches!(
                MotionDescriptor::from_compiled_constructor(&profile),
                Err(MotionLinkError::InvalidKeywordType { keyword, .. }) if keyword == "cases"
            ));
        }

        let operation = constructor(
            "motion.command_branch",
            json!({"index": COMMAND_SLOTS, "cases": {"0": [
                constructor("motion.friction", json!({"amount": 0.2}))
            ]}}),
        );
        let profile = constructor("motion.profile", json!({"air": [operation]}));
        assert!(matches!(
            MotionDescriptor::from_compiled_constructor(&profile),
            Err(MotionLinkError::InvalidKeywordType { keyword, .. }) if keyword == "index"
        ));
    }

    #[test]
    fn rejects_unbounded_command_constructor_values() {
        for (field, value) in [
            ("index", json!(COMMAND_SLOTS)),
            ("value", json!(u64::from(MAX_COMMAND_VALUE) + 1)),
        ] {
            let operation = constructor(
                "motion.command_velocity_scale",
                json!({
                    "index": if field == "index" { value.clone() } else { json!(0) },
                    "value": if field == "value" { value } else { json!(0) },
                    "multiplier": 1.0,
                }),
            );
            let profile = constructor("motion.profile", json!({"air": [operation]}));
            assert!(matches!(
                MotionDescriptor::from_compiled_constructor(&profile),
                Err(MotionLinkError::InvalidKeywordType { .. })
            ));
        }
    }
}
