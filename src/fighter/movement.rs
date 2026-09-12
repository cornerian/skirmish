//! Selected native Melee fighter movement arithmetic.
//!
//! Caller-supplied attributes and floor normals retain the upstream data model.
//! This crate neither schedules a frame nor supplies character/stage resources.
//! These scalar routines translate `ftcommon.c`.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Attributes {
    pub ground_max_horizontal_velocity: f32,
    pub air_max_horizontal_velocity: f32,
    pub air_drift_stick_mul: f32,
    pub aerial_drift_base: f32,
    pub air_drift_max: f32,
    pub aerial_friction: f32,
    pub gravity: f32,
    pub terminal_velocity: f32,
    pub fast_fall_velocity: f32,
}

/// The subset of fighter state accessed by the translated movement routines.
/// `animation_velocity` is upstream `x74_anim_vel`, not an already integrated
/// velocity. `ground_acceleration` is upstream `xE4_ground_accel_1`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Movement {
    pub self_velocity: [f32; 3],
    pub animation_velocity: [f32; 3],
    pub ground_velocity: f32,
    pub ground_acceleration: f32,
    pub ground_knockback: f32,
    pub shield_knockback: f32,
    pub stick_x: f32,
    pub floor_normal: [f32; 3],
    pub attributes: Attributes,
}

impl Movement {
    /// `ftCommon_ApplyFrictionGround`. Equality uses the original strict `>`.
    pub fn friction_ground(&mut self, mut friction: f32) {
        if friction.abs() > self.ground_velocity.abs() {
            friction = -self.ground_velocity;
        } else if self.ground_velocity > 0.0 {
            friction = -friction;
        }
        self.ground_acceleration = friction;
    }

    /// `ftCommon_8007C98C`.
    pub fn accelerate_ground(&mut self, acceleration: f32, target: f32, friction: f32) {
        if target == 0.0 {
            self.friction_ground(friction);
        } else {
            self.ground_acceleration = accelerate(
                self.ground_velocity,
                acceleration,
                target,
                friction,
                self.attributes.ground_max_horizontal_velocity,
            );
        }
    }

    /// `ftCommon_8007CA80`; its unused fourth C argument is omitted.
    pub fn accelerate_ground_direct(&mut self, acceleration: f32, target: f32) {
        self.ground_acceleration = accelerate_direct(self.ground_velocity, acceleration, target);
    }

    /// `ftCommon_8007CADC`.
    pub fn control_ground_direct(
        &mut self,
        threshold: f32,
        acceleration_max: f32,
        target_max: f32,
    ) {
        let (acceleration, target) =
            stick_control(self.stick_x, threshold, acceleration_max, target_max);
        self.accelerate_ground_direct(acceleration, target);
    }

    /// `ftCommon_ApplyGroundMovementNoSlide`. The C function also returns its
    /// unchanged object pointer. No friction multiplier or collision lookup occurs.
    pub fn project_ground(&mut self) {
        let [x, y, _] = self.floor_normal;
        self.animation_velocity = [
            y * self.ground_acceleration,
            -x * self.ground_acceleration,
            0.0,
        ];
        self.self_velocity = [y * self.ground_velocity, -x * self.ground_velocity, 0.0];
    }

    /// `ftCommon_ClampGrVel`.
    pub fn clamp_ground_velocity(&mut self, maximum: f32) {
        self.ground_velocity = clamp_signed(self.ground_velocity, maximum);
    }

    /// `ftCommon_8007CCA0`.
    pub fn decay_ground_knockback(&mut self, decrement: f32) {
        self.ground_knockback = decay_knockback(self.ground_knockback, decrement);
    }

    /// `ftCommon_8007CE4C`.
    pub fn decay_shield_knockback(&mut self, decrement: f32) {
        self.shield_knockback = decay_knockback(self.shield_knockback, decrement);
    }

    /// `ftCommon_ApplyFrictionAir`. Equality uses the original `>=`.
    pub fn friction_air(&mut self, mut friction: f32) {
        if friction.abs() >= self.self_velocity[0].abs() {
            friction = -self.self_velocity[0];
        } else if self.self_velocity[0] > 0.0 {
            friction = -friction;
        }
        self.animation_velocity[0] = friction;
    }

    /// `ftCommon_8007CEF4`.
    pub fn friction_air_basic(&mut self) {
        self.friction_air(self.attributes.aerial_friction);
    }

    /// `ftCommon_8007D140`.
    pub fn accelerate_air(&mut self, acceleration: f32, target: f32, friction: f32) {
        self.accelerate_air_from(self.self_velocity[0], acceleration, target, friction);
    }

    /// `ftCommon_8007D174`. The zero-target friction branch deliberately uses
    /// `self_velocity[0]`, even when the supplied `velocity` differs.
    pub fn accelerate_air_from(
        &mut self,
        velocity: f32,
        acceleration: f32,
        target: f32,
        friction: f32,
    ) {
        if target == 0.0 {
            self.friction_air(friction);
        } else {
            self.animation_velocity[0] = accelerate(
                velocity,
                acceleration,
                target,
                friction,
                self.attributes.air_max_horizontal_velocity,
            );
        }
    }

    /// `ftCommon_8007D268`.
    pub fn drift_air(&mut self) {
        self.drift_air_from(self.self_velocity[0]);
    }

    /// `ftCommon_8007D28C`.
    pub fn drift_air_from(&mut self, velocity: f32) {
        self.drift_air_scaled_from(velocity, self.attributes.air_drift_max);
    }

    /// `ftCommon_8007D268`/`8007D28C` generalized with an explicit drift
    /// maximum, for `ftCo_80096900` callers that scale it by a mobility
    /// multiplier (`ca->air_drift_max * mobility`, e.g. Fox/Falco's
    /// Illusion/Phantasm End, `x4C_FOX_ILLUSION_FREEFALL_MOBILITY`).
    pub fn drift_air_scaled(&mut self, drift_max: f32) {
        self.drift_air_scaled_from(self.self_velocity[0], drift_max);
    }

    fn drift_air_scaled_from(&mut self, velocity: f32, drift_max: f32) {
        let scaling = self.stick_x * self.attributes.air_drift_stick_mul;
        let flat = if self.stick_x > 0.0 {
            self.attributes.aerial_drift_base
        } else {
            -self.attributes.aerial_drift_base
        };
        self.accelerate_air_from(
            velocity,
            scaling + flat,
            self.stick_x * drift_max,
            self.attributes.aerial_friction,
        );
    }

    /// `ftCommon_8007D2E8`; its unused fourth C argument is omitted.
    pub fn accelerate_air_direct(&mut self, acceleration: f32, target: f32) {
        self.animation_velocity[0] = accelerate_direct(self.self_velocity[0], acceleration, target);
    }

    /// `ftCommon_8007D344`.
    pub fn control_air(&mut self, threshold: f32, acceleration_max: f32, target_max: f32) {
        let (acceleration, target) =
            stick_control(self.stick_x, threshold, acceleration_max, target_max);
        self.accelerate_air(acceleration, target, self.attributes.aerial_friction);
    }

    /// `ftCommon_8007D3A8`.
    pub fn control_air_direct(&mut self, threshold: f32, acceleration_max: f32, target_max: f32) {
        let (acceleration, target) =
            stick_control(self.stick_x, threshold, acceleration_max, target_max);
        self.accelerate_air_direct(acceleration, target);
    }

    /// `ftCommon_ClampSelfVelX`.
    pub fn clamp_air_velocity(&mut self, maximum: f32) {
        self.self_velocity[0] = clamp_signed(self.self_velocity[0], maximum);
    }

    /// `ftCommon_ClampAirDrift`.
    pub fn clamp_air_drift(&mut self) {
        self.clamp_air_velocity(self.attributes.air_drift_max);
    }

    /// `ftCommon_Fall`.
    pub fn fall(&mut self, gravity: f32, terminal_velocity: f32) {
        self.self_velocity[1] -= gravity;
        if self.self_velocity[1] < -terminal_velocity {
            self.self_velocity[1] = -terminal_velocity;
        }
    }

    /// `ftCommon_FallBasic`.
    pub fn fall_basic(&mut self) {
        self.fall(self.attributes.gravity, self.attributes.terminal_velocity);
    }

    /// `ftCommon_FallFast`.
    pub fn fall_fast(&mut self) {
        self.self_velocity[1] = -self.attributes.fast_fall_velocity;
    }

    /// `ftCommon_ClampFallSpeed`. This is an upper comparison, as in the source.
    pub fn clamp_fall_speed(&mut self, value: f32) {
        if self.self_velocity[1] > value {
            self.self_velocity[1] = value;
        }
    }

    /// `ftCommon_Ascend`.
    pub fn ascend(&mut self, acceleration: f32, maximum: f32) {
        self.self_velocity[1] += acceleration;
        self.clamp_fall_speed(maximum);
    }

    /// `ftCommon_8007CF58`: unlike [`Movement::clamp_air_drift`]'s immediate
    /// clamp, this eases `self_velocity[0]` toward `drift_max` by `accel`
    /// (a friction-style step written into `animation_velocity`, applied
    /// once per frame) whenever it is exceeded, else applies the fighter's
    /// ordinary aerial friction. Returns whether the over-max branch ran
    /// (the source's own boolean result, unused by every current caller but
    /// kept for parity with the translated signature).
    pub fn drift_clamp(&mut self, drift_max: f32, accel: f32) -> bool {
        let velocity = self.self_velocity[0];
        if velocity.abs() > drift_max {
            self.friction_air(accel);
            true
        } else {
            self.friction_air(self.attributes.aerial_friction);
            false
        }
    }
}

/// `ftCommon_8007CD6C`. Unlike knockback decay, zero values are unchanged even
/// for a negative decrement; retaining that branch also preserves signed zero.
pub fn decrement_toward_zero(value: f32, decrement: f32) -> f32 {
    let mut result = value;
    if value > 0.0 {
        result -= decrement;
        if result < 0.0 {
            return 0.0;
        }
    } else if value < 0.0 {
        result += decrement;
        if result > 0.0 {
            return 0.0;
        }
    }
    result
}

fn decay_knockback(mut velocity: f32, decrement: f32) -> f32 {
    if velocity < 0.0 {
        velocity += decrement;
        if velocity > 0.0 {
            velocity = 0.0;
        }
    } else {
        velocity -= decrement;
        if velocity < 0.0 {
            velocity = 0.0;
        }
    }
    velocity
}

fn clamp_signed(velocity: f32, maximum: f32) -> f32 {
    if velocity < -maximum {
        -maximum
    } else if velocity > maximum {
        maximum
    } else {
        velocity
    }
}

fn accelerate(
    velocity: f32,
    mut acceleration: f32,
    target: f32,
    friction: f32,
    maximum: f32,
) -> f32 {
    // C's negated comparison includes unordered (NaN) products, unlike >= 0.
    if (velocity * acceleration).partial_cmp(&0.0) != Some(core::cmp::Ordering::Less) {
        if acceleration > 0.0 {
            if velocity + acceleration > target {
                acceleration = -friction;
                if velocity + acceleration < target {
                    acceleration = target - velocity;
                }
                if velocity + acceleration > maximum {
                    acceleration = maximum - velocity;
                }
            }
        } else if velocity + acceleration < target {
            acceleration = friction;
            if velocity + acceleration > target {
                acceleration = target - velocity;
            }
            if velocity + acceleration < -maximum {
                acceleration = -maximum - velocity;
            }
        }
    }
    acceleration
}

fn accelerate_direct(velocity: f32, mut acceleration: f32, target: f32) -> f32 {
    if target == 0.0 {
        acceleration = -velocity;
    } else if (velocity * acceleration).partial_cmp(&0.0) != Some(core::cmp::Ordering::Less) {
        let exceeds = if acceleration > 0.0 {
            velocity + acceleration > target
        } else {
            velocity + acceleration < target
        };
        if exceeds {
            acceleration = target - velocity;
        }
    }
    acceleration
}

fn stick_control(stick: f32, threshold: f32, acceleration_max: f32, target_max: f32) -> (f32, f32) {
    if stick.abs() >= threshold {
        (stick * acceleration_max, stick * target_max)
    } else {
        (0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friction_equality_preserves_the_ground_air_difference() {
        let mut movement = Movement {
            ground_velocity: 0.0,
            ..Movement::default()
        };
        movement.friction_ground(0.0);
        movement.friction_air(0.0);
        assert_eq!(movement.ground_acceleration.to_bits(), 0.0_f32.to_bits());
        assert_eq!(
            movement.animation_velocity[0].to_bits(),
            (-0.0_f32).to_bits()
        );
    }

    #[test]
    fn fall_and_ground_projection_use_supplied_data() {
        let mut movement = Movement {
            ground_velocity: 3.0,
            ground_acceleration: 0.5,
            floor_normal: [-0.6, 0.8, 0.0],
            ..Movement::default()
        };
        movement.project_ground();
        assert_eq!(movement.self_velocity, [2.4, 1.8000001, 0.0]);
        assert_eq!(movement.animation_velocity, [0.4, 0.3, 0.0]);
        movement.self_velocity[1] = -1.9;
        movement.fall(0.2, 2.0);
        assert_eq!(movement.self_velocity[1], -2.0);
        assert_eq!(movement.ground_velocity, 3.0);
    }

    #[test]
    fn decay_preserves_zero_without_crossing_it() {
        assert_eq!(
            decrement_toward_zero(-0.0, -1.0).to_bits(),
            (-0.0_f32).to_bits()
        );
        assert_eq!(decrement_toward_zero(0.25, 1.0), 0.0);
        assert_eq!(decrement_toward_zero(-0.25, 1.0), 0.0);
        assert_eq!(decay_knockback(0.0, -1.0), 1.0);
    }

    #[test]
    fn dash_accel_reproduces_ieee754_not_the_recordings_one_ulp_lower_value() {
        // fox-fd-3.slp P2, frame -32 (docs/parity.md, docs/math.md's
        // "double-precision intermediates" section): `fighters/fox.json`'s
        // own real locomotion-pack constants (dash_initial_velocity 0x3ff33333
        // = 1.899999976, dash_accel_mul 0x3dcccccd = 0.100000001,
        // dash_accel_base 0x3ca3d70a = 0.019999999, dash_max_velocity
        // 0x400ccccd = 2.200000048, comfortably above `gr_vel + accel` so no
        // clamp branch below ever engages). Real hardware recorded
        // `velocities.self_x_air` = 0x400147ad; every rounding model tried --
        // single-precision step by step (what `game::locomotion::
        // ground_motion` and this function actually do), and fully
        // double-precision arithmetic with a single final round to f32 --
        // reproduces 0x400147ae instead, one ULP higher. `tools/
        // ppc_precision_audit.py` confirms the retail binary agrees: zero
        // double-precision arithmetic instructions anywhere in
        // `ftCo_Dash_Phys`, `ftCommon_8007C98C`, `ftCommon_ApplyGroundMovement`
        // (`NoSlide`), or `ftCommon_ApplyFrictionGround` -- the same plain,
        // separately-rounded `fmuls`/`fadds` this Rust port uses. Both a
        // fused product+sum (ruled out by `tools/ppc_fma_audit.py`, the
        // `skirmish-fma` batch) and a double-precision intermediate are
        // mathematically incapable of producing the recording's own value
        // from these exact inputs; the one-ULP gap must be upstream of this
        // frame (most likely the Dash-entry velocity itself), not in this
        // expression's evaluation order or precision. Pinned here so a
        // future change can't silently "fix" this by accident without
        // re-deriving why it would actually be correct.
        let gr_vel = f32::from_bits(0x3ff33333);
        let stick = 1.0_f32;
        let dash_accel_mul = f32::from_bits(0x3dcccccd);
        let dash_accel_base = f32::from_bits(0x3ca3d70a);
        let dash_max_velocity = f32::from_bits(0x400ccccd);

        // game::locomotion::ground_motion's own two-step accel computation.
        let mut accel = stick * dash_accel_mul;
        accel += dash_accel_base; // stick > 0.0
        let target = stick * dash_max_velocity;

        // friction/maximum are unreachable here: gr_vel + accel (2.02) never
        // exceeds target (2.2), so `accelerate`'s clamp branch never runs.
        let ground_acceleration = accelerate(gr_vel, accel, target, 1.0, f32::MAX);
        // game::simulation's own `f.ground_velocity = movement.ground_velocity
        // + movement.ground_acceleration`, the step this unit doesn't itself
        // perform.
        let new_ground_velocity = gr_vel + ground_acceleration;

        assert_eq!(new_ground_velocity.to_bits(), 0x400147ae);
        assert_ne!(new_ground_velocity.to_bits(), 0x400147ad);

        // The double-precision-throughout model gives the identical bits --
        // not merely "also passes disassembly", but arithmetically incapable
        // of reaching the recording's value from these inputs.
        let accel_f64 = (stick as f64) * (dash_accel_mul as f64) + (dash_accel_base as f64);
        let whole_f64 = (gr_vel as f64) + accel_f64;
        assert_eq!((whole_f64 as f32).to_bits(), 0x400147ae);
    }
}
