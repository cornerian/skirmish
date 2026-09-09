//! Directed stage queries translated from `melee/mp/mplib.c`.
//!
//! Caller-owned lines and joints replace the global collision arrays/list.
//! Queries retain endpoint direction, tolerances, neighbor extension, flags,
//! traversal order and strict nearest-contact selection. Joint ranges include
//! currently sampled dynamic lines, but previous-frame remapping, moving-joint
//! callbacks, stage transforms and the fighter ECB response pipeline are separate.
//!
//! Sloped normals use Dolphin's scalar C_VECNormalize arithmetic with libm sqrt.
//! This does not reproduce PSVECNormalize's PowerPC reciprocal-root estimate.
use core::ops::Range;

pub type Point = [f32; 2];
pub const FLOOR: u32 = 1;
pub const CEILING: u32 = 2;
pub const RIGHT_WALL: u32 = 4;
pub const LEFT_WALL: u32 = 8;
pub const EMPTY: u32 = 1 << 7;
pub const PLATFORM: u32 = 1 << 8;
pub const LEDGE: u32 = 1 << 9;
pub const ENABLED: u32 = 1 << 16;
pub const HIDDEN: u32 = 1 << 18;
pub const JOINT_ALWAYS_CHECK: u32 = 1 << 10;
pub const JOINT_TOO_FAR: u32 = 1 << 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Floor,
    Ceiling,
    LeftWall,
    RightWall,
}

impl Surface {
    pub fn flag(self) -> u32 {
        match self {
            Self::Floor => FLOOR,
            Self::Ceiling => CEILING,
            Self::LeftWall => LEFT_WALL,
            Self::RightWall => RIGHT_WALL,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    pub start: Point,
    pub end: Point,
    /// CollLine flags, including the surface-kind and enabled/empty bits.
    pub flags: u32,
    /// MapLine.lo_flags, returned as contact flags (platform/ledge/material).
    pub material_flags: u16,
    /// MapLine.prev_id0/id1: fallback and proximity-checked alternate.
    pub previous: [Option<usize>; 2],
    /// MapLine.next_id0/id1: fallback and proximity-checked alternate.
    pub next: [Option<usize>; 2],
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Joint {
    /// Stable original array index; joints are supplied in linked-list order.
    pub id: usize,
    pub flags: u32,
    pub bounds_min: Point,
    pub bounds_max: Point,
    pub floor: Range<usize>,
    pub ceiling: Range<usize>,
    pub left_wall: Range<usize>,
    pub right_wall: Range<usize>,
    pub dynamic: Range<usize>,
}

impl Joint {
    fn range(&self, kind: Surface) -> Range<usize> {
        match kind {
            Surface::Floor => self.floor.clone(),
            Surface::Ceiling => self.ceiling.clone(),
            Surface::LeftWall => self.left_wall.clone(),
            Surface::RightWall => self.right_wall.clone(),
        }
    }

    fn in_range(&self, query: Query) -> bool {
        if query.bounding == Bounding::Prechecked {
            return self.flags & JOINT_TOO_FAR == 0;
        }
        if self.flags & ENABLED == 0 || self.flags & HIDDEN != 0 {
            return false;
        }
        if self.flags & JOINT_ALWAYS_CHECK != 0 {
            return true;
        }
        // mpBoundingCheck2's ordered endpoint choices, followed by mpBoundingCheck.
        let [a, b] = [query.from, query.to];
        let (left, right) = if a[0] > b[0] {
            (b[0], a[0])
        } else {
            (a[0], b[0])
        };
        let (bottom, top) = if a[1] > b[1] {
            (b[1], a[1])
        } else {
            (a[1], b[1])
        };
        !(left > self.bounds_max[0]
            || right < self.bounds_min[0]
            || bottom > self.bounds_max[1]
            || top < self.bounds_min[1])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bounding {
    /// Compute joint eligibility as mpBoundingCheck2 does for this sweep.
    #[default]
    Compute,
    /// Honor supplied JOINT_TOO_FAR bits, as an already checked bounding pass.
    Prechecked,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Query {
    pub from: Point,
    pub to: Point,
    /// Only floor queries offset the extended line endpoints vertically.
    pub floor_y_offset: f32,
    /// Only mpCheckFloor has a line-skip parameter.
    pub skip_line: Option<usize>,
    pub skip_joint: Option<usize>,
    pub only_joint: Option<usize>,
    pub bounding: Bounding,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub line_id: usize,
    pub joint_id: usize,
    pub flags: u32,
    pub distance_squared: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    pub line_id: usize,
    pub flags: u32,
    pub normal: [f32; 3],
    /// Displacement relative to the requested Y, including source's +0.0001 bias.
    pub vertical_delta: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceProjection {
    pub line_id: usize,
    pub flags: u32,
    pub normal: [f32; 3],
    /// Y displacement for floor/ceiling, X displacement for either wall.
    /// Floor/ceiling include the source's +0.0001/-0.0001 separation bias.
    pub delta: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageError {
    NonFinite,
    LineIndex(usize),
    JointRange(usize),
    JointBounds(usize),
    DuplicateJoint(usize),
    LineCycle(usize),
}

impl core::fmt::Display for StageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "stage collision error: {self:?}")
    }
}
impl core::error::Error for StageError {}

/// Immutable validated view; no global bounding cache or callback state.
#[derive(Clone, Copy, Debug)]
pub struct Stage<'a> {
    lines: &'a [Line],
    joints: &'a [Joint],
}

impl<'a> Stage<'a> {
    pub fn new(lines: &'a [Line], joints: &'a [Joint]) -> Result<Self, StageError> {
        for line in lines {
            if !finite(line.start.into_iter().chain(line.end)) {
                return Err(StageError::NonFinite);
            }
            for id in line.previous.into_iter().chain(line.next).flatten() {
                if id >= lines.len() {
                    return Err(StageError::LineIndex(id));
                }
            }
        }
        for (i, joint) in joints.iter().enumerate() {
            if !finite(joint.bounds_min.into_iter().chain(joint.bounds_max)) {
                return Err(StageError::NonFinite);
            }
            if (0..2).any(|axis| joint.bounds_min[axis] > joint.bounds_max[axis]) {
                return Err(StageError::JointBounds(joint.id));
            }
            if joints[..i].iter().any(|other| other.id == joint.id) {
                return Err(StageError::DuplicateJoint(joint.id));
            }
            for range in [
                &joint.floor,
                &joint.ceiling,
                &joint.left_wall,
                &joint.right_wall,
                &joint.dynamic,
            ] {
                if range.start > range.end || range.end > lines.len() {
                    return Err(StageError::JointRange(joint.id));
                }
            }
        }
        Ok(Self { lines, joints })
    }

    /// mpLineGetPrev/Next, including fallback links without flag checks.
    pub fn neighbor(&self, line_id: usize, next: bool) -> Result<Option<usize>, StageError> {
        let line = self
            .lines
            .get(line_id)
            .ok_or(StageError::LineIndex(line_id))?;
        let [fallback, alternate] = if next { line.next } else { line.previous };
        if let Some(id) = alternate {
            let adjacent = &self.lines[id];
            let (a, b) = if next {
                (line.end, adjacent.start)
            } else {
                (line.start, adjacent.end)
            };
            if adjacent.flags & ENABLED != 0
                && adjacent.flags & HIDDEN == 0
                && f64::from(distance_squared(a, b)) < 4.0
            {
                return Ok(Some(id));
            }
        }
        Ok(fallback)
    }

    /// mpLib_8004ED5C. A connected endpoint extends one unit. If both ends
    /// extend, the second uses the already modified first endpoint and the
    /// original distance; preserving this asymmetry matters at short seams.
    pub fn extended_endpoints(&self, line_id: usize) -> Result<[Point; 2], StageError> {
        let line = self
            .lines
            .get(line_id)
            .ok_or(StageError::LineIndex(line_id))?;
        let [mut a, mut b] = [line.start, line.end];
        let mut distance = None;
        if self.neighbor(line_id, false)?.is_some() {
            let d = libm::sqrtf(distance_squared(a, b));
            if d > 0.001 {
                a[0] += (a[0] - b[0]) / d;
                a[1] += (a[1] - b[1]) / d;
            }
            distance = Some(d);
        }
        if self.neighbor(line_id, true)?.is_some() {
            let d = distance.unwrap_or_else(|| libm::sqrtf(distance_squared(a, b)));
            if d > 0.001 {
                b[0] += (b[0] - a[0]) / d;
                b[1] += (b[1] - a[1]) / d;
            }
        }
        Ok([a, b])
    }

    /// mpLib_8004DD90_Floor: follow the current floor across adjacent segments,
    /// project the requested X, and return the Y correction and new floor ID.
    /// Disconnected misses beyond 0.1 return None. Invalid cycles/nonfinite
    /// results return errors instead of hanging or publishing unusable geometry.
    pub fn project_floor(&self, id: usize, point: Point) -> Result<Option<Projection>, StageError> {
        Ok(self
            .project(Surface::Floor, id, point)?
            .map(|hit| Projection {
                line_id: hit.line_id,
                flags: hit.flags,
                normal: hit.normal,
                vertical_delta: hit.delta,
            }))
    }

    /// mpLib_8004DD90_Floor / 8004E090_Ceiling / 8004E398_LeftWall /
    /// 8004E684_RightWall. Project the final point onto its directed surface,
    /// walking matching adjacent segments as necessary. Apply delta on the
    /// surface's normal axis; the requested tangent coordinate is unchanged.
    pub fn project(
        &self,
        surface: Surface,
        mut id: usize,
        point: Point,
    ) -> Result<Option<SurfaceProjection>, StageError> {
        if !finite(point) {
            return Err(StageError::NonFinite);
        }
        let vertical = matches!(surface, Surface::LeftWall | Surface::RightWall);
        let along = usize::from(vertical);
        let increasing = matches!(surface, Surface::Floor | Surface::LeftWall);
        let mut direction = 0;
        for _ in 0..=self.lines.len() {
            let line = self.lines.get(id).ok_or(StageError::LineIndex(id))?;
            let [x0, y0] = line.start;
            let [x1, y1] = line.end;
            let mut coordinate = point[along];
            let (low, high) = if increasing {
                (line.start[along], line.end[along])
            } else {
                (line.end[along], line.start[along])
            };
            // RightWall tests its high end first; retain this order even for
            // malformed reversed lines accepted by the low-level geometry view.
            let excursion = if surface == Surface::RightWall && coordinate > high {
                Some((high, 1))
            } else if coordinate < low {
                Some((low, -1))
            } else if coordinate > high {
                Some((high, 1))
            } else {
                None
            };
            if let Some((endpoint, travel)) = excursion {
                if direction != -travel {
                    let next = (travel == 1) == increasing;
                    match self.neighbor(id, next)? {
                        Some(neighbor) if self.lines[neighbor].flags & surface.flag() != 0 => {
                            id = neighbor;
                            // Floor alone omits direction=1 on forward travel.
                            if surface != Surface::Floor || travel != 1 {
                                direction = travel;
                            }
                            continue;
                        }
                        _ => {
                            let difference = f64::from(coordinate - endpoint);
                            if (travel == -1 && difference < -0.1)
                                || (travel == 1 && difference > 0.1)
                            {
                                return Ok(None);
                            }
                            coordinate = endpoint;
                        }
                    }
                } else if surface != Surface::LeftWall {
                    // LeftWall deliberately extrapolates on a direction reversal;
                    // the other source routines clamp to the current endpoint.
                    coordinate = endpoint;
                }
            }
            let delta = if vertical {
                x0 + (x1 - x0) * (coordinate - y0) / (y1 - y0) - point[0]
            } else {
                let value = (y1 - y0) * (coordinate - x0) / (x1 - x0) + y0 - point[1];
                if surface == Surface::Floor {
                    (f64::from(value) + 0.0001) as f32
                } else {
                    (f64::from(value) - 0.0001) as f32
                }
            };
            let normal = scalar_normal(line.start, line.end);
            if !finite(normal.into_iter().chain([delta])) {
                return Err(StageError::NonFinite);
            }
            return Ok(Some(SurfaceProjection {
                line_id: id,
                flags: u32::from(line.material_flags),
                normal,
                delta,
            }));
        }
        Err(StageError::LineCycle(id))
    }

    pub fn sweep(&self, kind: Surface, query: Query) -> Result<Option<Contact>, StageError> {
        self.sweep_filtered(kind, query, |_| true)
    }

    /// The acceptance closure is mpCheckFloor's callback: called for each
    /// candidate floor-range line before line skip/flag checks. Other surfaces
    /// do not call it. Platform dropping and fighter policy belong in this hook.
    pub fn sweep_filtered(
        &self,
        kind: Surface,
        query: Query,
        mut accept_floor: impl FnMut(usize) -> bool,
    ) -> Result<Option<Contact>, StageError> {
        if !finite(
            query
                .from
                .into_iter()
                .chain(query.to)
                .chain([query.floor_y_offset]),
        ) {
            return Err(StageError::NonFinite);
        }
        let mut nearest = None;
        let mut min_distance = f32::MAX;
        for joint in self.joints {
            if !joint.in_range(query)
                || query.skip_joint == Some(joint.id)
                || query.only_joint.is_some_and(|id| id != joint.id)
            {
                continue;
            }
            for id in joint.range(kind).chain(joint.dynamic.clone()) {
                if kind == Surface::Floor && (!accept_floor(id) || query.skip_line == Some(id)) {
                    continue;
                }
                let line = &self.lines[id];
                if line.flags & kind.flag() == 0
                    || line.flags & ENABLED == 0
                    || line.flags & EMPTY != 0
                {
                    continue;
                }
                let [mut a, mut b] = if matches!(kind, Surface::Floor | Surface::Ceiling) {
                    self.extended_endpoints(id)?
                } else {
                    [line.start, line.end]
                };
                if kind == Surface::Floor {
                    a[1] += query.floor_y_offset;
                    b[1] += query.floor_y_offset;
                }
                let vertical = matches!(kind, Surface::LeftWall | Surface::RightWall);
                let axis = usize::from(!vertical);
                let slope = f64::from((a[axis] - b[axis]).abs()) > 0.0001;
                let position = if slope {
                    line_intersection(a, b, query.from, query.to)
                } else {
                    let [from, to] = [query.from[axis], query.to[axis]];
                    let toward = match kind {
                        Surface::Floor | Surface::RightWall => from >= to,
                        Surface::Ceiling | Surface::LeftWall => from <= to,
                    };
                    if !toward {
                        continue;
                    }
                    if vertical {
                        line_intersection_v(a, b[1], query.from, query.to)
                    } else {
                        line_intersection_h(a, b[0], query.from, query.to)
                    }
                };
                if let Some(position) = position {
                    let distance = distance_squared(position, query.from);
                    // Strict > preserves the first contact when distances tie.
                    if min_distance > distance {
                        let normal = if slope {
                            scalar_normal(a, b)
                        } else {
                            match kind {
                                Surface::Floor => [0.0, 1.0, 0.0],
                                Surface::Ceiling => [0.0, -1.0, 0.0],
                                Surface::LeftWall => [-1.0, 0.0, 0.0],
                                Surface::RightWall => [1.0, 0.0, 0.0],
                            }
                        };
                        if !finite(position.into_iter().chain(normal).chain([distance])) {
                            return Err(StageError::NonFinite);
                        }
                        min_distance = distance;
                        nearest = Some(Contact {
                            position: [position[0], position[1], 0.0],
                            normal,
                            line_id: id,
                            joint_id: joint.id,
                            flags: u32::from(line.material_flags),
                            distance_squared: distance,
                        });
                    }
                }
            }
        }
        Ok(nearest)
    }
}

/// mpLineIntersection: directed crossing from a line's left half-plane to its
/// right half-plane. Returns None without an output point on every miss.
/// Differences round as f32 before the original double-precision determinants.
pub fn line_intersection(a0: Point, a1: Point, b0: Point, b1: Point) -> Option<Point> {
    for axis in 0..2 {
        let (min, max) = if a0[axis] <= a1[axis] {
            (a0[axis], a1[axis])
        } else {
            (a1[axis], a0[axis])
        };
        if (b0[axis] < min && b1[axis] < min) || (max < b0[axis] && max < b1[axis]) {
            return None;
        }
    }
    let [ah, d0x, aw, d0y] =
        [a1[1] - a0[1], b0[0] - a0[0], a1[0] - a0[0], b0[1] - a0[1]].map(f64::from);
    let h0 = aw * d0y - ah * d0x;
    let below = h0 < 0.0;
    if below && h0 < -0.1 {
        return None;
    }
    let [d1x, d1y] = [b1[0] - a1[0], b1[1] - a1[1]].map(f64::from);
    let h1 = aw * d1y - ah * d1x;
    let above = h1 > 0.0;
    if above && h1 > 0.1 {
        return None;
    }
    if h0 == 0.0 && h1 == 0.0 {
        return None;
    }
    let det = d0x * d1y - d0y * d1x;
    if (det < h0 && det < h1) || (det > h0 && det > h1) {
        return None;
    }
    let [bw, bh] = [b1[0] - b0[0], b1[1] - b0[1]].map(f64::from);
    if (bw == 0.0 && bh == 0.0) || (below && above) || (h0 >= 0.0 && above) {
        return None;
    }
    let area = bw * ah - bh * aw;
    if area.abs() > f64::from(0.0001_f32) {
        let t = (bw * d0y - bh * d0x) / area;
        Some(if t > 0.0 {
            if t < 1.0 {
                [
                    (aw * t + f64::from(a0[0])) as f32,
                    (ah * t + f64::from(a0[1])) as f32,
                ]
            } else {
                a1
            }
        } else {
            a0
        })
    } else {
        None
    }
}

/// mpLineIntersectionH. The second endpoint's Y equals a0's Y.
pub fn line_intersection_h(a0: Point, a1x: f32, b0: Point, b1: Point) -> Option<Point> {
    axis_intersection(a0, a1x, b0, b1, false)
}

/// mpLineIntersectionV. The second endpoint's X equals a0's X.
pub fn line_intersection_v(a0: Point, a1y: f32, b0: Point, b1: Point) -> Option<Point> {
    axis_intersection(a0, a1y, b0, b1, true)
}

fn axis_intersection(a0: Point, end: f32, b0: Point, b1: Point, vertical: bool) -> Option<Point> {
    let along = usize::from(vertical);
    let cross = 1 - along;
    let forward = a0[along] < end;
    let (min, max) = if forward {
        (a0[along], end)
    } else {
        (end, a0[along])
    };
    if (b0[along] < min && b1[along] < min) || (max < b0[along] && max < b1[along]) {
        return None;
    }
    let (first, second) = if forward != vertical {
        (b0, b1)
    } else {
        (b1, b0)
    };
    if f64::from(first[cross] - a0[cross]) < -0.0001
        || f64::from(second[cross] - a0[cross]) > 0.0001
    {
        return None;
    }
    let dc = f64::from(b1[cross] - b0[cross]);
    let da = f64::from(b1[along] - b0[along]);
    if dc.abs() < 0.0001 {
        return None;
    }
    let mut value = da / dc * f64::from(a0[cross] - b0[cross]) + f64::from(b0[along]);
    let low_delta = value - f64::from(min);
    if low_delta < 0.0 {
        if low_delta < -0.1 {
            return None;
        }
        value = f64::from(min);
    }
    let high_delta = value - f64::from(max);
    if high_delta > 0.0 {
        if high_delta > 0.1 {
            return None;
        }
        value = f64::from(max);
    }
    let mut result = a0;
    result[along] = value as f32;
    Some(result)
}

fn distance_squared(a: Point, b: Point) -> f32 {
    let [x, y] = [a[0] - b[0], a[1] - b[1]];
    x * x + y * y
}

fn scalar_normal(a: Point, b: Point) -> [f32; 3] {
    let [x, y] = [-(b[1] - a[1]), b[0] - a[0]];
    let inverse = 1.0 / libm::sqrtf(0.0 + (x * x + y * y));
    [x * inverse, y * inverse, 0.0 * inverse]
}

fn finite(values: impl IntoIterator<Item = f32>) -> bool {
    values.into_iter().all(f32::is_finite)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_slope_contact_and_projection_retain_floor_direction() {
        let lines = [Line {
            start: [-2.0, 0.0],
            end: [2.0, 2.0],
            flags: FLOOR | ENABLED,
            material_flags: PLATFORM as u16,
            ..Line::default()
        }];
        let joints = [Joint {
            flags: ENABLED,
            bounds_min: [-2.0, 0.0],
            bounds_max: [2.0, 2.0],
            floor: 0..1,
            ..Joint::default()
        }];
        let stage = Stage::new(&lines, &joints).unwrap();
        let query = Query {
            from: [0.0, 3.0],
            to: [0.0, -1.0],
            ..Query::default()
        };
        let contact = stage.sweep(Surface::Floor, query).unwrap().unwrap();
        assert_eq!(
            (contact.position, contact.flags),
            ([0.0, 1.0, 0.0], PLATFORM)
        );
        assert!(
            stage
                .sweep(
                    Surface::Floor,
                    Query {
                        from: query.to,
                        to: query.from,
                        ..query
                    }
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(
            stage
                .project_floor(0, [0.0, 1.0])
                .unwrap()
                .unwrap()
                .vertical_delta
                .to_bits(),
            0.0001_f32.to_bits()
        );
    }

    #[test]
    fn malformed_geometry_and_nonfinite_queries_are_errors() {
        let mut line = Line {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
            flags: FLOOR | ENABLED,
            ..Line::default()
        };
        line.next[0] = Some(1);
        assert!(matches!(
            Stage::new(&[line], &[]),
            Err(StageError::LineIndex(1))
        ));
        line.next[0] = Some(0);
        let lines = [line];
        let stage = Stage::new(&lines, &[]).unwrap();
        assert!(matches!(
            stage.project_floor(0, [2.0, 0.0]),
            Err(StageError::LineCycle(0))
        ));
        assert!(matches!(
            stage.sweep(
                Surface::Floor,
                Query {
                    from: [f32::NAN, 0.0],
                    ..Query::default()
                }
            ),
            Err(StageError::NonFinite)
        ));
        assert!(matches!(
            Stage::new(
                &lines,
                &[Joint {
                    floor: 0..2,
                    ..Joint::default()
                }]
            ),
            Err(StageError::JointRange(0))
        ));
    }
}
