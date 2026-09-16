//! Immutable, generic motion profiles selected by lifecycle policy.
//!
//! A profile describes the small set of continuous movement operations that
//! the native simulation can advance after a script has selected a phase.
//! Selection, direction, thresholds, state transitions, and the timing of
//! phase changes remain script/event-engine concerns.  In particular, this
//! module never names a fighter or an action and never calls the script VM.
//!
//! [`MotionProfile::apply`] deliberately takes a caller-owned [`MotionState`]
//! and does not advance its clock.  The event engine owns the exact ordering
//! of phase-clock advancement, grounded timer ticks, and physics.  This is
//! needed for transitions which preserve a frame, re-enter an action, or
//! transfer between ground and air during the same simulation frame.

use crate::fighter::{Movement, helpers};
use serde::{Deserialize, Serialize};

/// A native profile identifier.  The resource cache owns the corresponding
/// immutable [`MotionProfile`]; rollback state only needs this small handle
/// together with [`MotionState`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MotionProfileId(pub u16);

impl MotionProfileId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }
}

/// Mutable native clock state associated with one selected profile.
///
/// Both fields are `f32` because the source state stores its action timers as
/// floats and performs the increment/decrement/comparison operations in that
/// type.  The profile itself is immutable and contains no runtime counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MotionState {
    /// Current action/phase frame used by track indexing and deadlines.
    pub phase_frame: f32,
    /// Remaining delayed-gravity frames.  This is also the timer that a
    /// grounded tick may decrement before a later ground-to-air transfer.
    pub gravity_delay: f32,
}

/// Number of command-variable slots exposed by the native action-state ABI.
pub const COMMAND_SLOTS: usize = 4;
/// Keep declarative command comparisons bounded at resource-load time.
pub const MAX_COMMAND_VALUE: u32 = 1_000_000;

/// Small per-fighter values supplied by the policy that selected a cached
/// profile.  Facing and launch angle are runtime choices, so they are kept
/// out of immutable resource tracks and out of the profile descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionBinding {
    /// Facing multiplier used by the horizontal component of a directional
    /// acceleration operation.
    pub facing: f32,
    /// Cosine and sine components selected by the policy.  Keeping these
    /// factors separate preserves the source grouping `facing * (magnitude *
    /// cosine)` instead of precomputing a direction and changing rounding.
    pub cosine: f32,
    pub sine: f32,
    /// Scalar used by tracks whose native values are in local-forward space.
    /// A typical policy binding supplies `+1` or `-1` here.
    pub ground_scale: f32,
}

impl Default for MotionBinding {
    fn default() -> Self {
        Self {
            facing: 1.0,
            cosine: 1.0,
            sine: 0.0,
            ground_scale: 1.0,
        }
    }
}

impl MotionBinding {
    pub const fn new(facing: f32, cosine: f32, sine: f32, ground_scale: f32) -> Self {
        Self {
            facing,
            cosine,
            sine,
            ground_scale,
        }
    }

    pub fn validate(&self) -> Result<(), MotionProfileError> {
        for component in [self.facing, self.cosine, self.sine] {
            finite(component, "binding direction")?;
        }
        finite(self.ground_scale, "binding ground scale")
    }
}

impl MotionState {
    pub const fn new(phase_frame: f32, gravity_delay: f32) -> Self {
        Self {
            phase_frame,
            gravity_delay,
        }
    }

    /// Construct the state for a newly selected phase.  The event engine may
    /// instead preserve an existing state when the source preserves timers.
    pub const fn at_phase(gravity_delay: f32) -> Self {
        Self::new(0.0, gravity_delay)
    }

    /// Advance the phase clock by one source frame.  Kept separate from
    /// [`MotionProfile::apply`] so callers can place it before or after the
    /// physics callback as required by the owning event.
    pub fn advance_phase(&mut self) {
        self.phase_frame += 1.0;
    }

    /// Reset only the phase clock, preserving delayed-gravity state.
    pub fn reset_phase(&mut self) {
        self.phase_frame = 0.0;
    }

    /// Tick the delayed-gravity timer using the source's strict `> 0` test.
    /// Returns `true` when a frame was consumed by the delay.
    pub fn tick_gravity_delay(&mut self) -> bool {
        if self.gravity_delay > 0.0 {
            self.gravity_delay -= 1.0;
            true
        } else {
            false
        }
    }

    pub fn gravity_ready(&self) -> bool {
        self.gravity_delay <= 0.0
    }
}

/// How a track behaves after its supplied samples end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackEnd {
    /// Do nothing when the requested frame is outside the track.  This is the
    /// source behavior of the optional TransN/velocity arrays.
    #[default]
    NoOp,
    /// Reuse the final sample for all later frames.
    HoldLast,
    /// Wrap at the sample count.
    Loop,
}

/// An optional pair of native self-velocity components for one animation
/// sample.  Partial samples cover tracks such as a vertical-only bound path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VelocitySample {
    pub x: Option<f32>,
    pub y: Option<f32>,
}

/// Transform applied to a native track component when it is selected.
/// Resource data remains local and immutable; the binding supplies the
/// per-fighter sign/scale at runtime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackTransform {
    #[default]
    Absolute,
    BindingScale,
}

impl VelocitySample {
    pub const fn x(value: f32) -> Self {
        Self {
            x: Some(value),
            y: None,
        }
    }

    pub const fn y(value: f32) -> Self {
        Self {
            x: None,
            y: Some(value),
        }
    }

    pub const fn xy(x: f32, y: f32) -> Self {
        Self {
            x: Some(x),
            y: Some(y),
        }
    }
}

/// Native animation-resource velocity samples.  The owning resource cache
/// stores this once; profiles refer to that immutable data when applying a
/// phase rather than cloning a track into fighter/checkpoint state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VelocityTrack {
    pub samples: Vec<VelocitySample>,
    pub end: TrackEnd,
    pub x_transform: TrackTransform,
    pub y_transform: TrackTransform,
}

impl VelocityTrack {
    pub fn new(samples: Vec<VelocitySample>) -> Self {
        Self {
            samples,
            end: TrackEnd::NoOp,
            x_transform: TrackTransform::Absolute,
            y_transform: TrackTransform::Absolute,
        }
    }

    pub fn with_end(samples: Vec<VelocitySample>, end: TrackEnd) -> Self {
        Self {
            samples,
            end,
            ..Self::default()
        }
    }

    pub fn with_transforms(
        samples: Vec<VelocitySample>,
        end: TrackEnd,
        x_transform: TrackTransform,
        y_transform: TrackTransform,
    ) -> Self {
        Self {
            samples,
            end,
            x_transform,
            y_transform,
        }
    }

    fn sample(&self, frame: f32) -> Option<VelocitySample> {
        sample_at(&self.samples, self.end, frame)
    }
}

/// A scalar native animation-resource track used for ground target velocity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScalarTrack {
    pub samples: Vec<Option<f32>>,
    pub end: TrackEnd,
    pub transform: TrackTransform,
}

impl ScalarTrack {
    pub fn new(samples: Vec<Option<f32>>) -> Self {
        Self {
            samples,
            end: TrackEnd::NoOp,
            transform: TrackTransform::Absolute,
        }
    }

    pub fn with_end(samples: Vec<Option<f32>>, end: TrackEnd) -> Self {
        Self {
            samples,
            end,
            ..Self::default()
        }
    }

    pub fn with_transform(
        samples: Vec<Option<f32>>,
        end: TrackEnd,
        transform: TrackTransform,
    ) -> Self {
        Self {
            samples,
            end,
            transform,
        }
    }

    pub(crate) fn sample(&self, frame: f32) -> Option<f32> {
        sample_at(&self.samples, self.end, frame).flatten()
    }
}

fn sample_at<T: Copy>(samples: &[T], end: TrackEnd, frame: f32) -> Option<T> {
    if samples.is_empty() || !frame.is_finite() || frame < 0.0 {
        return None;
    }
    // Native track callers use an integer action frame.  Truncation here is
    // the same observable operation as the source's `frame as usize` if a
    // future event supplies a fractional phase clock.
    let frame = frame as usize;
    let index = match end {
        TrackEnd::NoOp => (frame < samples.len()).then_some(frame)?,
        TrackEnd::HoldLast => frame.min(samples.len() - 1),
        TrackEnd::Loop => frame % samples.len(),
    };
    samples.get(index).copied()
}

/// Delayed or immediate vertical acceleration.  `delay` is the value used to
/// initialize [`MotionState::gravity_delay`] when the event selects the
/// profile; the mutable countdown remains in `MotionState`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gravity {
    pub acceleration: f32,
    pub terminal_velocity: f32,
    pub delay: f32,
}

impl Gravity {
    pub const fn new(acceleration: f32, terminal_velocity: f32, delay: f32) -> Self {
        Self {
            acceleration,
            terminal_velocity,
            delay,
        }
    }
}

/// Aerial operation, applied in vector order.  Operation order is part of the
/// descriptor so a track can precede a drift clamp exactly as in native code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AirOperation {
    Gravity(Gravity),
    Friction {
        amount: f32,
    },
    VelocityTrack(VelocityTrack),
    /// After `starts_at`, write the source's direct reverse-direction delta:
    /// `-(binding.facing * (magnitude * binding.cosine) - current_velocity)`
    /// and `-(magnitude * binding.sine - current_velocity)`.  The explicit grouping
    /// is intentional; algebraically equivalent rewrites can change f32
    /// rounding and signed zero.
    DirectionalAcceleration {
        starts_at: f32,
        magnitude: f32,
    },
    DriftClamp {
        maximum: f32,
        acceleration: f32,
    },
    DriftOrFriction {
        recovery_step: f32,
    },
    /// Multiply both self-velocity components only when the selected
    /// action-state command slot contains the authored value.  A conditional
    /// operation is a no-op when its command does not match, allowing the
    /// native physics fallback to continue.
    CommandVelocityScale {
        index: usize,
        value: u32,
        multiplier: f32,
    },
}

impl AirOperation {
    pub const fn gravity(acceleration: f32, terminal_velocity: f32, delay: f32) -> Self {
        Self::Gravity(Gravity::new(acceleration, terminal_velocity, delay))
    }

    pub const fn friction(amount: f32) -> Self {
        Self::Friction { amount }
    }

    pub fn velocity_track(track: VelocityTrack) -> Self {
        Self::VelocityTrack(track)
    }

    pub const fn directional_acceleration(starts_at: f32, magnitude: f32) -> Self {
        Self::DirectionalAcceleration {
            starts_at,
            magnitude,
        }
    }

    pub const fn drift_clamp(maximum: f32, acceleration: f32) -> Self {
        Self::DriftClamp {
            maximum,
            acceleration,
        }
    }

    pub const fn drift_or_friction(recovery_step: f32) -> Self {
        Self::DriftOrFriction { recovery_step }
    }

    pub const fn command_velocity_scale(index: usize, value: u32, multiplier: f32) -> Self {
        Self::CommandVelocityScale {
            index,
            value,
            multiplier,
        }
    }
}

/// Ground operation, applied in descriptor order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GroundOperation {
    Friction {
        amount: f32,
    },
    FrictionAfter {
        starts_at: f32,
        before: f32,
        after: f32,
    },
    TargetTrack(ScalarTrack),
}

impl GroundOperation {
    pub const fn friction(amount: f32) -> Self {
        Self::Friction { amount }
    }

    pub const fn friction_after(starts_at: f32, before: f32, after: f32) -> Self {
        Self::FrictionAfter {
            starts_at,
            before,
            after,
        }
    }

    pub fn target_track(track: ScalarTrack) -> Self {
        Self::TargetTrack(track)
    }
}

/// Generic immutable movement descriptor.  Character policy lowers resource
/// values and direction choices into this form; the native engine only
/// executes the resulting operations.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MotionProfile {
    pub air: Vec<AirOperation>,
    pub ground: Vec<GroundOperation>,
}

impl MotionProfile {
    pub fn new(air: Vec<AirOperation>, ground: Vec<GroundOperation>) -> Self {
        Self { air, ground }
    }

    /// Construct a fresh runtime clock using the profile's declared initial
    /// gravity delay.  Preserving transitions should retain the old state
    /// instead of calling this constructor.
    pub fn initial_state(&self) -> MotionState {
        MotionState::at_phase(self.initial_gravity_delay().unwrap_or(0.0))
    }

    /// Return the first delayed-gravity value declared by this profile.  The
    /// event engine uses this when constructing a fresh `MotionState`; a
    /// transition that preserves state can skip this and retain its timer.
    pub fn initial_gravity_delay(&self) -> Option<f32> {
        self.air.iter().find_map(|operation| match operation {
            AirOperation::Gravity(gravity) => Some(gravity.delay),
            _ => None,
        })
    }

    /// Apply one native physics callback.  The clock itself is not advanced.
    /// Returns whether this profile supplied at least one operation for the
    /// selected surface.
    pub fn apply(&self, state: &mut MotionState, movement: &mut Movement, grounded: bool) -> bool {
        self.apply_with_binding_and_command(
            state,
            movement,
            grounded,
            &MotionBinding::default(),
            &[],
        )
    }

    /// Apply with the small runtime binding chosen by the script/event
    /// boundary.  Cached profile data and tracks remain immutable.
    pub fn apply_with_binding(
        &self,
        state: &mut MotionState,
        movement: &mut Movement,
        grounded: bool,
        binding: &MotionBinding,
    ) -> bool {
        self.apply_with_binding_and_command(state, movement, grounded, binding, &[])
    }

    /// Apply with a runtime command-variable tuple supplied by the current
    /// fighter action state.  Command values are deliberately passed at the
    /// call site instead of being folded into [`MotionBinding`]: the binding
    /// describes per-fighter direction, while commands change during an
    /// action's animation trace.
    pub fn apply_with_binding_and_command(
        &self,
        state: &mut MotionState,
        movement: &mut Movement,
        grounded: bool,
        binding: &MotionBinding,
        command: &[i64],
    ) -> bool {
        if grounded {
            let mut applied = false;
            for operation in &self.ground {
                applied |= apply_ground(operation, state.phase_frame, movement, binding);
            }
            applied
        } else {
            let mut applied = false;
            for operation in &self.air {
                applied |= apply_air(operation, state, movement, binding, command);
            }
            applied
        }
    }

    /// Return the authored ground target at a phase without applying any
    /// operation. This is used only by compatibility query seams; the
    /// simulation's production path calls `apply_with_binding` once so target
    /// and friction operations retain their authored order.
    pub fn ground_target_velocity(&self, phase_frame: f32, binding: &MotionBinding) -> Option<f32> {
        self.ground
            .iter()
            .rev()
            .find_map(|operation| match operation {
                GroundOperation::TargetTrack(track) => track
                    .sample(phase_frame)
                    .map(|value| transform(value, track.transform, binding)),
                _ => None,
            })
    }

    /// Return the effective authored friction for a ground phase. A profile
    /// may still contain a target track; callers that need both must apply the
    /// complete profile through `apply_with_binding`.
    pub fn ground_friction(&self, phase_frame: f32) -> Option<f32> {
        self.ground
            .iter()
            .rev()
            .find_map(|operation| match operation {
                GroundOperation::Friction { amount } => Some(*amount),
                GroundOperation::FrictionAfter {
                    starts_at,
                    before,
                    after,
                } => Some(if phase_frame >= *starts_at {
                    *after
                } else {
                    *before
                }),
                GroundOperation::TargetTrack(_) => None,
            })
    }

    /// Validate a descriptor at resource-load time.  Applying a validated
    /// profile then has no runtime data-error path or script callback.
    pub fn validate(&self) -> Result<(), MotionProfileError> {
        for operation in &self.air {
            match operation {
                AirOperation::Gravity(value) => {
                    finite_nonnegative(value.acceleration, "gravity acceleration")?;
                    finite_nonnegative(value.terminal_velocity, "terminal velocity")?;
                    finite_nonnegative(value.delay, "gravity delay")?;
                }
                AirOperation::Friction { amount } => {
                    finite_nonnegative(*amount, "air friction")?;
                }
                AirOperation::VelocityTrack(track) => validate_velocity_track(track)?,
                AirOperation::DirectionalAcceleration {
                    starts_at,
                    magnitude,
                } => {
                    finite_nonnegative(*starts_at, "directional deadline")?;
                    finite(*magnitude, "directional magnitude")?;
                }
                AirOperation::DriftClamp {
                    maximum,
                    acceleration,
                } => {
                    finite_nonnegative(*maximum, "drift maximum")?;
                    finite_nonnegative(*acceleration, "drift clamp acceleration")?;
                }
                AirOperation::DriftOrFriction { recovery_step } => {
                    finite_nonnegative(*recovery_step, "drift recovery step")?;
                }
                AirOperation::CommandVelocityScale {
                    index,
                    value,
                    multiplier,
                } => {
                    if *index >= COMMAND_SLOTS {
                        return Err(MotionProfileError::Invalid("command index"));
                    }
                    if *value > MAX_COMMAND_VALUE {
                        return Err(MotionProfileError::Invalid("command value"));
                    }
                    finite(*multiplier, "command velocity multiplier")?;
                }
            }
        }
        for operation in &self.ground {
            match operation {
                GroundOperation::Friction { amount } => {
                    finite_nonnegative(*amount, "ground friction")?;
                }
                GroundOperation::FrictionAfter {
                    starts_at,
                    before,
                    after,
                } => {
                    finite_nonnegative(*starts_at, "ground friction deadline")?;
                    finite_nonnegative(*before, "ground friction before")?;
                    finite_nonnegative(*after, "ground friction after")?;
                }
                GroundOperation::TargetTrack(track) => validate_scalar_track(track)?,
            }
        }
        Ok(())
    }
}

fn apply_air(
    operation: &AirOperation,
    state: &mut MotionState,
    movement: &mut Movement,
    binding: &MotionBinding,
    command: &[i64],
) -> bool {
    match operation {
        AirOperation::Gravity(gravity) => {
            if !state.tick_gravity_delay() {
                movement.fall(gravity.acceleration, gravity.terminal_velocity);
            }
            true
        }
        AirOperation::Friction { amount } => {
            movement.friction_air(*amount);
            true
        }
        AirOperation::VelocityTrack(track) => {
            if let Some(sample) = track.sample(state.phase_frame) {
                if let Some(x) = sample.x {
                    movement.self_velocity[0] = transform(x, track.x_transform, binding);
                }
                if let Some(y) = sample.y {
                    movement.self_velocity[1] = transform(y, track.y_transform, binding);
                }
                true
            } else {
                false
            }
        }
        AirOperation::DirectionalAcceleration {
            starts_at,
            magnitude,
        } if state.phase_frame >= *starts_at => {
            let target_x = binding.facing * (*magnitude * binding.cosine);
            let target_y = *magnitude * binding.sine;
            movement.animation_velocity[0] = -(target_x - movement.self_velocity[0]);
            movement.animation_velocity[1] = -(target_y - movement.self_velocity[1]);
            true
        }
        AirOperation::DirectionalAcceleration { .. } => false,
        AirOperation::DriftClamp {
            maximum,
            acceleration,
        } => {
            movement.drift_clamp(*maximum, *acceleration);
            true
        }
        AirOperation::DriftOrFriction { recovery_step } => {
            helpers::drift_or_friction_air(movement, *recovery_step);
            true
        }
        AirOperation::CommandVelocityScale {
            index,
            value,
            multiplier,
        } => {
            if command.get(*index).copied() != Some(i64::from(*value)) {
                return false;
            }
            movement.self_velocity[0] *= *multiplier;
            movement.self_velocity[1] *= *multiplier;
            true
        }
    }
}

fn apply_ground(
    operation: &GroundOperation,
    phase_frame: f32,
    movement: &mut Movement,
    binding: &MotionBinding,
) -> bool {
    match operation {
        GroundOperation::Friction { amount } => {
            movement.friction_ground(*amount);
            movement.project_ground();
            true
        }
        GroundOperation::FrictionAfter {
            starts_at,
            before,
            after,
        } => {
            let amount = if phase_frame >= *starts_at {
                *after
            } else {
                *before
            };
            movement.friction_ground(amount);
            movement.project_ground();
            true
        }
        GroundOperation::TargetTrack(track) => {
            if let Some(target) = track.sample(phase_frame) {
                let target = transform(target, track.transform, binding);
                movement.ground_acceleration = target - movement.ground_velocity;
                movement.project_ground();
                true
            } else {
                false
            }
        }
    }
}

fn transform(value: f32, transform: TrackTransform, binding: &MotionBinding) -> f32 {
    match transform {
        TrackTransform::Absolute => value,
        TrackTransform::BindingScale => value * binding.ground_scale,
    }
}

fn validate_velocity_track(track: &VelocityTrack) -> Result<(), MotionProfileError> {
    for sample in &track.samples {
        if let Some(value) = sample.x {
            finite(value, "velocity track x")?;
        }
        if let Some(value) = sample.y {
            finite(value, "velocity track y")?;
        }
    }
    Ok(())
}

fn validate_scalar_track(track: &ScalarTrack) -> Result<(), MotionProfileError> {
    for value in track.samples.iter().flatten() {
        finite(*value, "scalar track")?;
    }
    Ok(())
}

fn finite(value: f32, field: &'static str) -> Result<(), MotionProfileError> {
    if value.is_finite() && value.abs() <= 1_000_000.0 {
        Ok(())
    } else {
        Err(MotionProfileError::Invalid(field))
    }
}

fn finite_nonnegative(value: f32, field: &'static str) -> Result<(), MotionProfileError> {
    finite(value, field)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(MotionProfileError::Invalid(field))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionProfileError {
    Invalid(&'static str),
}

impl core::fmt::Display for MotionProfileError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Invalid(field) => write!(formatter, "invalid motion profile field: {field}"),
        }
    }
}

impl std::error::Error for MotionProfileError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::Attributes;

    fn movement() -> Movement {
        Movement {
            attributes: Attributes {
                air_max_horizontal_velocity: 10.0,
                aerial_friction: 0.2,
                terminal_velocity: 9.0,
                ..Attributes::default()
            },
            self_velocity: [1.0, 2.0, 0.0],
            floor_normal: [0.0, 1.0, 0.0],
            ..Movement::default()
        }
    }

    #[test]
    fn delayed_gravity_consumes_only_the_delay_frames() {
        let profile = MotionProfile {
            air: vec![AirOperation::Gravity(Gravity {
                acceleration: 0.25,
                terminal_velocity: 9.0,
                delay: 2.0,
            })],
            ground: Vec::new(),
        };
        let mut state = MotionState::at_phase(profile.initial_gravity_delay().unwrap());
        let mut value = movement();

        profile.apply(&mut state, &mut value, false);
        assert_eq!(state.gravity_delay.to_bits(), 1.0_f32.to_bits());
        assert_eq!(value.self_velocity[1].to_bits(), 2.0_f32.to_bits());
        profile.apply(&mut state, &mut value, false);
        assert_eq!(state.gravity_delay.to_bits(), 0.0_f32.to_bits());
        assert_eq!(value.self_velocity[1].to_bits(), 2.0_f32.to_bits());
        profile.apply(&mut state, &mut value, false);
        assert_eq!(value.self_velocity[1].to_bits(), 1.75_f32.to_bits());
    }

    #[test]
    fn partial_track_and_noop_end_match_optional_native_samples() {
        let profile = MotionProfile {
            air: vec![AirOperation::VelocityTrack(VelocityTrack::new(vec![
                VelocitySample::y(-3.0),
            ]))],
            ground: Vec::new(),
        };
        let mut state = MotionState::new(0.0, 0.0);
        let mut value = movement();
        profile.apply(&mut state, &mut value, false);
        assert_eq!(value.self_velocity, [1.0, -3.0, 0.0]);
        state.phase_frame = 1.0;
        profile.apply(&mut state, &mut value, false);
        assert_eq!(value.self_velocity, [1.0, -3.0, 0.0]);
    }

    #[test]
    fn track_uses_native_frame_truncation_and_runtime_forward_scale() {
        let track = VelocityTrack::with_transforms(
            vec![VelocitySample::x(1.0), VelocitySample::x(2.0)],
            TrackEnd::NoOp,
            TrackTransform::BindingScale,
            TrackTransform::Absolute,
        );
        let profile = MotionProfile {
            air: vec![AirOperation::VelocityTrack(track)],
            ground: Vec::new(),
        };
        let mut state = MotionState::new(1.9, 0.0);
        let mut value = movement();
        let binding = MotionBinding {
            ground_scale: -1.0,
            ..MotionBinding::default()
        };
        profile.apply_with_binding(&mut state, &mut value, false, &binding);
        // Native action-frame indexing truncates to sample 1; the local
        // forward sample is then multiplied by the selected facing sign.
        assert_eq!(value.self_velocity[0].to_bits(), (-2.0_f32).to_bits());
    }

    #[test]
    fn reverse_direction_keeps_source_negation_and_f32_grouping() {
        let current_x = f32::from_bits(0x3f99999a);
        let current_y = f32::from_bits(0xbf4ccccd);
        let facing = -1.0_f32;
        let cosine = f32::from_bits(0x3eaaaaab);
        let sine = f32::from_bits(0x3f2aaaab);
        let magnitude = f32::from_bits(0x400ccccd);
        let profile = MotionProfile {
            air: vec![AirOperation::DirectionalAcceleration {
                starts_at: 3.0,
                magnitude,
            }],
            ground: Vec::new(),
        };
        let mut state = MotionState::new(3.0, 0.0);
        let mut value = movement();
        value.self_velocity = [current_x, current_y, 0.0];
        let binding = MotionBinding {
            facing,
            cosine,
            sine,
            ground_scale: 1.0,
        };
        profile.apply_with_binding(&mut state, &mut value, false, &binding);

        let expected_x = -((facing * (magnitude * cosine)) - current_x);
        let expected_y = -((magnitude * sine) - current_y);
        assert_eq!(value.animation_velocity[0].to_bits(), expected_x.to_bits());
        assert_eq!(value.animation_velocity[1].to_bits(), expected_y.to_bits());
    }

    #[test]
    fn ground_target_projects_the_track_delta_on_the_floor() {
        let profile = MotionProfile {
            air: Vec::new(),
            ground: vec![GroundOperation::TargetTrack(ScalarTrack::new(vec![Some(
                3.0,
            )]))],
        };
        let mut state = MotionState::default();
        let mut value = Movement {
            ground_velocity: 1.5,
            floor_normal: [-0.6, 0.8, 0.0],
            ..movement()
        };
        profile.apply(&mut state, &mut value, true);
        assert_eq!(value.ground_acceleration.to_bits(), 1.5_f32.to_bits());
        assert_eq!(value.self_velocity, [1.2, 0.90000004, 0.0]);
        assert_eq!(value.animation_velocity, [1.2, 0.90000004, 0.0]);
    }

    #[test]
    fn ground_profile_runs_all_operations_and_reports_null_track_as_noop() {
        let profile = MotionProfile {
            air: Vec::new(),
            ground: vec![
                GroundOperation::Friction { amount: 0.2 },
                GroundOperation::TargetTrack(ScalarTrack::new(vec![Some(3.0)])),
            ],
        };
        let mut state = MotionState::default();
        let mut value = Movement {
            ground_velocity: 1.5,
            ..movement()
        };
        assert!(profile.apply(&mut state, &mut value, true));
        // The target operation must still run after the earlier friction
        // operation; it overwrites the friction acceleration with its own
        // target delta instead of leaving the earlier -0.2 value in place.
        assert_eq!(value.ground_velocity.to_bits(), 1.5_f32.to_bits());
        assert_eq!(value.ground_acceleration.to_bits(), 1.5_f32.to_bits());

        let noop = MotionProfile {
            air: Vec::new(),
            ground: vec![GroundOperation::TargetTrack(ScalarTrack::new(vec![None]))],
        };
        let mut value = movement();
        assert!(!noop.apply(&mut state, &mut value, true));
    }

    #[test]
    fn validation_rejects_nonfinite_and_negative_profile_values() {
        let invalid = MotionProfile {
            air: vec![AirOperation::Gravity(Gravity {
                acceleration: f32::NAN,
                terminal_velocity: 1.0,
                delay: 0.0,
            })],
            ground: Vec::new(),
        };
        assert_eq!(
            invalid.validate(),
            Err(MotionProfileError::Invalid("gravity acceleration"))
        );

        let invalid = MotionProfile {
            air: Vec::new(),
            ground: vec![GroundOperation::Friction { amount: -1.0 }],
        };
        assert_eq!(
            invalid.validate(),
            Err(MotionProfileError::Invalid("ground friction"))
        );
    }

    #[test]
    fn command_velocity_scale_only_applies_for_the_matching_current_command() {
        let profile = MotionProfile {
            air: vec![AirOperation::command_velocity_scale(1, 1, 0.5)],
            ground: Vec::new(),
        };
        let mut state = MotionState::default();
        let mut value = movement();
        assert!(profile.apply_with_binding_and_command(
            &mut state,
            &mut value,
            false,
            &MotionBinding::default(),
            &[0, 1, 0, 0],
        ));
        assert_eq!(value.self_velocity[0].to_bits(), 0.5_f32.to_bits());
        assert_eq!(value.self_velocity[1].to_bits(), 1.0_f32.to_bits());

        let before = value.self_velocity;
        assert!(!profile.apply_with_binding_and_command(
            &mut state,
            &mut value,
            false,
            &MotionBinding::default(),
            &[0, 0, 0, 0],
        ));
        assert_eq!(value.self_velocity, before);
    }

    #[test]
    fn command_velocity_scale_validates_index_value_and_multiplier() {
        for (operation, error) in [
            (
                AirOperation::command_velocity_scale(COMMAND_SLOTS, 1, 1.0),
                "command index",
            ),
            (
                AirOperation::command_velocity_scale(0, MAX_COMMAND_VALUE + 1, 1.0),
                "command value",
            ),
            (
                AirOperation::command_velocity_scale(0, 1, f32::NAN),
                "command velocity multiplier",
            ),
        ] {
            let profile = MotionProfile {
                air: vec![operation],
                ground: Vec::new(),
            };
            assert_eq!(profile.validate(), Err(MotionProfileError::Invalid(error)));
        }
    }
}
