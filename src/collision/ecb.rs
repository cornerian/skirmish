//! Selected mpcoll.c environment-collision-box geometry and state transitions.
//! Joint inputs are already evaluated world positions. No stage callback graph,
//! ledge decision, JObj traversal, or collision scheduler is supplied here.

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Shape {
    pub top: [f32; 2],
    pub bottom: [f32; 2],
    pub left: [f32; 2],
    pub right: [f32; 2],
}

impl Shape {
    /// mpColl_80042384. Preserve its asymmetric validation and threshold order.
    pub fn normalize(&mut self) {
        if (self.top[1] - self.bottom[1]).abs() < 1.0 {
            self.top[1] += 1.0;
            let mid = 0.5 * (self.top[1] + self.bottom[1]);
            self.left[1] = mid;
            self.right[1] = mid;
        }
        above(&mut self.top[1], 1.0);
        below(&mut self.left[0], -1.0);
        above(&mut self.right[0], 1.0);
        if self.top[1] < self.bottom[1] {
            self.top[1] = 1.0 + self.bottom[1];
        }
        if self.right[1] > self.top[1] || self.right[1] < self.bottom[1] {
            let mid = 0.5 * (self.top[1] + self.bottom[1]);
            self.left[1] = mid;
            self.right[1] = mid;
        }
        for point in [&mut self.right, &mut self.left] {
            if self.top[1] - point[1] < 0.001 || point[1] - self.bottom[1] < 0.001 {
                point[1] = 0.5 * (self.top[1] + self.bottom[1]);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FixedSource {
    pub up: f32,
    pub down: f32,
    pub front: f32,
    pub back: f32,
    pub angle: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JointParameters {
    pub side_y_offset: f32,
    pub height_threshold: f32,
    pub width_threshold: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct State {
    pub current: Shape,
    pub previous: Shape,
    pub desired: Shape,
    /// xE4_ecb: current geometry immediately after applying Clear on a load.
    pub before_load: Shape,
    /// x64_ecb: geometry saved before the first squeeze.
    pub unsqueezed: Shape,
    pub clear_on_load: bool,
    pub bottom_locked: bool,
    pub restore_unsqueezed: bool,
    pub uninitialized: bool,
    /// x34_flags.b5; squeezes clear the caller's stop request.
    pub stop: bool,
}

impl State {
    pub fn new(shape: Shape) -> Self {
        Self {
            current: shape,
            previous: shape,
            desired: shape,
            before_load: shape,
            unsqueezed: shape,
            ..Self::default()
        }
    }

    fn begin_load(&mut self) {
        if self.clear_on_load {
            self.current = Shape::default();
            self.clear_on_load = false;
        }
        self.before_load = self.current;
        self.uninitialized = false;
    }

    fn finish_load(&mut self, saved_bottom: [f32; 2]) {
        if self.bottom_locked {
            self.desired.bottom = saved_bottom;
        }
        self.desired.normalize();
    }

    /// mpColl_LoadECB_Fixed plus LoadECB's locked-bottom restoration/normalizer.
    /// Facing uses the original exact equality to +1; every other value flips.
    pub fn load_fixed(&mut self, source: &FixedSource, facing: i32) {
        let saved_bottom = self.desired.bottom;
        self.begin_load();
        let (mut bottom, mut top) = (-source.down, source.up);
        let (mut right, mut left) = if facing == 1 {
            (source.front, -source.back)
        } else {
            (source.back, -source.front)
        };
        if source.angle != 0.0 {
            let (sin, cos) = (
                crate::math::sinf(source.angle),
                crate::math::cosf(source.angle),
            );
            let (old_top, old_bottom, old_right, old_left) = (top, bottom, right, left);
            // The source uses horizontal midpoint as the side points' y input.
            let middle = 0.5 * (old_right + old_left);
            (top, right, bottom, left) = (0.0, 0.0, 0.0, 0.0);
            for [x, y] in [
                [-old_top * sin, old_top * cos],
                [-old_bottom * sin, old_bottom * cos],
                [
                    old_right * cos - middle * sin,
                    old_right * sin + middle * cos,
                ],
                [old_left * cos - middle * sin, old_left * sin + middle * cos],
            ] {
                expand_max_first(&mut left, &mut right, x);
                expand_max_first(&mut bottom, &mut top, y);
            }
        }
        above(&mut top, 0.0);
        below(&mut bottom, -0.0);
        above(&mut right, 0.0);
        below(&mut left, -0.0);
        if top - bottom < 3.0 {
            top = 1.5;
            bottom = -top;
        }
        if right - left < 3.0 {
            right = 1.5;
            left = -right;
        }
        let mid = 0.5 * (top + bottom);
        self.desired = Shape {
            top: [0.0, top],
            bottom: [0.0, bottom],
            left: [left, mid],
            right: [right, mid],
        };
        self.finish_load(saved_bottom);
    }

    /// mpColl_LoadECB_JObj with six supplied world samples, then LoadECB's
    /// lock/normalize behavior. Flags retain raw source bits: 4 omits padding,
    /// 8 narrows width, 1 anchors bottom, and 16 requests a two-unit height.
    /// The caller picks the value per collision entry point, the same as the
    /// source (`docs/ecb-load-flags.md`; `game::collision::sample`'s
    /// `load_flags`); this function has no notion of a single "the" flags
    /// value for a resource pack.
    pub fn load_joints(
        &mut self,
        world: [[f32; 2]; 6],
        position: [f32; 2],
        parameters: &JointParameters,
        flags: u32,
    ) {
        let saved_bottom = self.desired.bottom;
        self.begin_load();
        let (mut left, mut right) = (world[0][0] - position[0], world[0][0] - position[0]);
        let (mut bottom, mut top) = (world[0][1] - position[1], world[0][1] - position[1]);
        for point in &world[1..] {
            expand_min_first(&mut left, &mut right, point[0] - position[0]);
            expand_min_first(&mut bottom, &mut top, point[1] - position[1]);
        }
        if flags & 4 == 0 {
            left -= 2.0;
            right += 2.0;
            bottom -= 2.0;
            top += 2.0;
        }
        let threshold = if 4.0 > parameters.width_threshold {
            4.0
        } else {
            parameters.width_threshold
        };
        let width = (right - left).abs();
        if width < threshold {
            right = 0.5 * width;
            left = -right;
        }
        let threshold = if 4.0 > parameters.height_threshold {
            4.0
        } else {
            parameters.height_threshold
        };
        let height = (top - bottom).abs();
        if height < threshold {
            let half = 0.5 * height;
            let mid = 0.5 * (top + bottom);
            top = mid + half;
            bottom = mid - half;
        }
        if flags & 8 != 0 {
            left = -1.0;
            right = 1.0;
        } else {
            if right < 2.0 {
                right = 2.0;
            }
            if left > -2.0 {
                left = -2.0;
            }
        }
        if flags & 1 != 0 {
            bottom = 0.0;
            if flags & 16 != 0 {
                top = 2.0;
            }
        } else {
            if bottom < 0.0 {
                bottom = 0.0;
            }
            if flags & 16 != 0 {
                let mid = 0.5 * (bottom + top);
                bottom = mid - 1.0;
                top = mid + 1.0;
                if bottom < 0.0 {
                    bottom = 0.0;
                    top = 2.0;
                }
            }
        }
        let mid = parameters.side_y_offset + 0.5 * (bottom + top);
        self.desired = Shape {
            top: [0.0, top],
            bottom: [0.0, bottom],
            left: [left, mid],
            right: [right, mid],
        };
        self.finish_load(saved_bottom);
    }

    /// mpColl_80042C58. This external-box path bypasses normalization and locking.
    pub fn load_external(&mut self, shape: Shape) {
        self.begin_load();
        self.desired = Shape {
            top: [0.0, shape.top[1]],
            bottom: [0.0, shape.bottom[1]],
            ..shape
        };
    }

    /// mpCollInterpolateECB. NaN becomes an error in place of the source assert;
    /// mutation before the error is retained. No extrapolation clamp is added.
    pub fn interpolate(&mut self, fraction: f32) -> Result<(), EcbError> {
        self.previous = self.current;
        if self.restore_unsqueezed {
            self.current = self.unsqueezed;
            self.restore_unsqueezed = false;
        }
        let destinations = [
            &mut self.current.top,
            &mut self.current.bottom,
            &mut self.current.left,
            &mut self.current.right,
        ];
        let sources = [
            self.desired.top,
            self.desired.bottom,
            self.desired.left,
            self.desired.right,
        ];
        let mut nan = false;
        for (destination, source) in destinations.into_iter().zip(sources) {
            for axis in 0..2 {
                destination[axis] += fraction * (source[axis] - destination[axis]);
                nan |= destination[axis].is_nan();
            }
        }
        if nan { Err(EcbError::NanShape) } else { Ok(()) }
    }

    pub fn squeeze_horizontal(&mut self, position: &mut [f32; 2], left: f32, right: f32) {
        let half = 0.5 * (right - left + self.current.right[0] - self.current.left[0]);
        self.save_unsqueezed();
        position[0] = (right + self.current.right[0]) - half;
        self.current.right[0] = half;
        self.current.left[0] = -half;
        self.desired.right[0] = self.current.right[0];
        self.desired.left[0] = self.current.left[0];
        self.stop = false;
    }

    pub fn squeeze_vertical(
        &mut self,
        position: &mut [f32; 2],
        airborne: bool,
        top: f32,
        bottom: f32,
    ) {
        let height = top - bottom + self.current.top[1] - self.current.bottom[1];
        self.save_unsqueezed();
        if height < 3.0 {
            let old = self.current.top[1] - self.current.bottom[1];
            let new = self.current.top[1] + top - bottom;
            self.current.top[1] = if old < new { old } else { new };
            self.current.bottom[1] = 0.0;
            position[1] = bottom;
        } else if !airborne {
            position[1] = bottom;
            self.current.top[1] = height + self.current.bottom[1];
        } else {
            position[1] = 0.5 * (top + bottom);
            self.current.top[1] = 0.5 * (self.current.top[1] + self.current.bottom[1] + height);
            self.current.bottom[1] = self.current.top[1] - height;
        }
        let mid = 0.5 * (self.current.top[1] + self.current.bottom[1]);
        self.current.right[1] = mid;
        self.current.left[1] = mid;
        self.desired.top[1] = self.current.top[1];
        self.desired.bottom[1] = self.current.bottom[1];
        self.desired.left[1] = self.current.left[1];
        self.desired.right[1] = self.current.right[1];
        self.stop = false;
    }

    fn save_unsqueezed(&mut self) {
        if !self.restore_unsqueezed {
            self.unsqueezed = self.current;
        }
        self.restore_unsqueezed = true;
    }

    /// mpCollCheckBounding: union of previous/current extrema with optional ledge
    /// query expansion. This produces a broad-phase rectangle, not a collision.
    pub fn bounds(
        &self,
        position: [f32; 2],
        previous_position: [f32; 2],
        ledge: Option<LedgeSnap>,
    ) -> Bounds {
        let mut bounds = Bounds {
            left: self.current.left[0] + position[0],
            right: self.current.right[0] + position[0],
            bottom: self.current.bottom[1] + position[1],
            top: self.current.top[1] + position[1],
        };
        below(
            &mut bounds.left,
            self.previous.left[0] + previous_position[0],
        );
        above(
            &mut bounds.right,
            self.previous.right[0] + previous_position[0],
        );
        below(
            &mut bounds.bottom,
            self.previous.bottom[1] + previous_position[1],
        );
        above(&mut bounds.top, self.previous.top[1] + previous_position[1]);
        if let Some(ledge) = ledge {
            bounds.right += ledge.horizontal;
            bounds.left -= ledge.horizontal;
            let half = 0.5 * ledge.height;
            below(&mut bounds.bottom, ledge.vertical - half + position[1]);
            below(
                &mut bounds.bottom,
                ledge.vertical - half + previous_position[1],
            );
            let offset = ledge.vertical + half;
            above(&mut bounds.top, position[1] + offset);
            above(&mut bounds.top, previous_position[1] + offset);
        }
        bounds
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LedgeSnap {
    pub horizontal: f32,
    pub vertical: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Bounds {
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SubstepPlan {
    pub steps: u32,
    pub velocity: [f32; 3],
}

impl SubstepPlan {
    /// Count/delta selection from mpColl_80043754. The source tests movement,
    /// left/right x growth and top/right y growth, excluding bottom growth.
    /// Six units uses one step; exactly twelve uses three, not two.
    pub fn new(
        last: [f32; 3],
        current: [f32; 3],
        shape: Shape,
        desired: Shape,
    ) -> Result<Self, EcbError> {
        let mut velocity = core::array::from_fn(|axis| current[axis] - last[axis]);
        let dx = max_source(
            (desired.left[0] - shape.left[0]).abs(),
            (desired.right[0] - shape.right[0]).abs(),
        );
        let dy = max_source(
            (desired.top[1] - shape.top[1]).abs(),
            (desired.right[1] - shape.right[1]).abs(),
        );
        let extent = max_source(
            max_source(velocity[0].abs(), dx),
            max_source(velocity[1].abs(), dy),
        );
        if !extent.is_finite() || velocity.iter().any(|value| !value.is_finite()) {
            return Err(EcbError::InvalidSubsteps);
        }
        let steps = if extent > 6.0 {
            let quotient = extent / 6.0;
            if quotient >= 2_147_483_648.0 {
                return Err(EcbError::InvalidSubsteps);
            }
            let steps = (quotient as i32)
                .checked_add(1)
                .ok_or(EcbError::InvalidSubsteps)? as u32;
            for value in &mut velocity {
                *value /= steps as f32;
            }
            steps
        } else {
            1
        };
        Ok(Self { steps, velocity })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EcbError {
    NanShape,
    InvalidSubsteps,
}

impl core::fmt::Display for EcbError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ECB error: {self:?}")
    }
}
impl core::error::Error for EcbError {}

fn above(value: &mut f32, minimum: f32) {
    if *value < minimum {
        *value = minimum;
    }
}
fn below(value: &mut f32, maximum: f32) {
    if *value > maximum {
        *value = maximum;
    }
}
fn expand_min_first(minimum: &mut f32, maximum: &mut f32, value: f32) {
    if *minimum > value {
        *minimum = value;
    } else if *maximum < value {
        *maximum = value;
    }
}
fn expand_max_first(minimum: &mut f32, maximum: &mut f32, value: f32) {
    if *maximum < value {
        *maximum = value;
    } else if *minimum > value {
        *minimum = value;
    }
}
fn max_source(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diamond() -> Shape {
        Shape {
            top: [0.0, 8.0],
            bottom: [0.0, 0.0],
            left: [-4.0, 4.0],
            right: [4.0, 4.0],
        }
    }

    #[test]
    fn multiple_squeezes_restore_original_before_interpolating() {
        let mut state = State::new(diamond());
        let mut position = [0.0; 2];
        state.stop = true;
        state.squeeze_horizontal(&mut position, 2.0, -2.0);
        assert_eq!(state.current.right[0], 2.0);
        assert!(!state.stop);
        state.squeeze_vertical(&mut position, true, 1.0, 4.0);
        assert_eq!(state.unsqueezed, diamond());
        assert_eq!(position, [0.0, 2.5]);
        let squeezed = state.current;
        state.interpolate(0.5).unwrap();
        assert_eq!(state.previous, squeezed);
        assert_eq!(state.current.right[0], 3.0);
        assert_eq!(state.current.top[1], 7.25);
        assert_eq!(state.current.bottom[1], 0.75);
        assert!(!state.restore_unsqueezed);
        state.interpolate(1.0).unwrap();
        assert_eq!(state.current, state.desired);
    }

    #[test]
    fn locked_bottom_clear_and_external_bypass_are_distinct() {
        let mut state = State::new(diamond());
        state.desired.bottom = [0.0, 2.0];
        state.bottom_locked = true;
        state.clear_on_load = true;
        state.load_fixed(
            &FixedSource {
                up: 8.0,
                down: 2.0,
                front: 4.0,
                back: 4.0,
                angle: 0.0,
            },
            1,
        );
        assert_eq!(state.before_load, Shape::default());
        assert_eq!(state.desired.bottom, [0.0, 2.0]);
        assert!(!state.clear_on_load);
        let external = Shape {
            top: [99.0, -0.5],
            bottom: [99.0, -7.0],
            left: [0.25, -8.0],
            right: [0.5, 9.0],
        };
        state.load_external(external);
        assert_eq!(state.desired.top, [0.0, -0.5]);
        assert_eq!(state.desired.bottom, [0.0, -7.0]);
        assert_eq!(state.desired.left, external.left);
        assert_eq!(state.desired.right, external.right);
        state.desired.top[1] = f32::NAN;
        assert_eq!(state.interpolate(1.0), Err(EcbError::NanShape));
    }
}
