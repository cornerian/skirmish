//! Resource-driven stage collision transforms and grounded-line carry.

use super::{Error, Fighter, data};
use crate::collision::stage::{self, Line, Point};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, ops::Range};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    /// Stable collision-line range transformed by this joint animation.
    pub lines: Range<usize>,
    /// Absolute transforms sampled cyclically; element zero is the initial pose.
    pub frames: Vec<Transform>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    /// Row-major affine 2D matrix: `[m00, m01, tx]`, `[m10, m11, ty]`.
    pub matrix: [[f32; 3]; 2],
}

impl Transform {
    pub const IDENTITY: Self = Self {
        matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
    };

    fn point(self, [x, y]: Point) -> Point {
        [
            self.matrix[0][0] * x + self.matrix[0][1] * y + self.matrix[0][2],
            self.matrix[1][0] * x + self.matrix[1][1] * y + self.matrix[1][2],
        ]
    }

    fn line(self, line: Line) -> Line {
        Line {
            start: self.point(line.start),
            end: self.point(line.end),
            ..line
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct State {
    pub frame: u32,
}

pub(crate) fn validate(stage_data: &data::Stage) -> Result<(), Error> {
    let Some(rules) = &stage_data.motion else {
        return Ok(());
    };
    let geometry = stage_data
        .geometry
        .as_ref()
        .ok_or_else(|| Error::Data("stage motion requires explicit geometry".into()))?;
    if rules.tracks.is_empty()
        || rules.tracks.len() > 1024
        || rules
            .tracks
            .iter()
            .map(|track| track.frames.len())
            .sum::<usize>()
            > 1_000_000
    {
        return Err(Error::Data("invalid stage-motion tracks".into()));
    }
    let mut owned = vec![false; geometry.lines.len()];
    for track in &rules.tracks {
        if track.lines.is_empty()
            || track.lines.end > geometry.lines.len()
            || track.frames.is_empty()
            || track.frames.len() > 1_000_000
        {
            return Err(Error::Data("invalid stage-motion track range".into()));
        }
        for id in track.lines.clone() {
            if core::mem::replace(&mut owned[id], true) {
                return Err(Error::Data("stage-motion tracks overlap".into()));
            }
            for &transform in &track.frames {
                let line = transform.line(geometry.lines[id]);
                if !valid_transform(transform) || !valid_line(line) {
                    return Err(Error::Data("invalid transformed stage line".into()));
                }
            }
        }
    }
    Ok(())
}

/// Current collision geometry reconstructed from immutable resources and the
/// checkpointed stage frame.
pub(crate) fn geometry(stage_data: &data::Stage, frame: u32) -> Cow<'_, data::StageGeometry> {
    let Some(rules) = &stage_data.motion else {
        return super::collision::geometry(stage_data);
    };
    let mut geometry = stage_data
        .geometry
        .as_ref()
        .expect("validated stage motion requires geometry")
        .clone();
    apply(stage_data, rules, frame, &mut geometry);
    Cow::Owned(geometry)
}

pub(crate) fn advance(stage_data: &data::Stage, state: &mut State) -> Result<(), Error> {
    if stage_data.motion.is_none() {
        return Ok(());
    }
    state.frame = state.frame.checked_add(1).ok_or(Error::FrameOverflow)?;
    Ok(())
}

/// Apply `mpGetSpeed`'s remapping after fighter self-motion and before ECB map
/// response. Stable line IDs make the current support the remapping key.
pub(crate) fn carry(
    stage_data: &data::Stage,
    state: &State,
    fighter: &mut Fighter,
) -> Result<(), Error> {
    let Some(line_id) = fighter.grounded.then_some(fighter.ground_line).flatten() else {
        return Ok(());
    };
    let Some(rules) = &stage_data.motion else {
        return Ok(());
    };
    let Some(track) = rules
        .tracks
        .iter()
        .find(|track| track.lines.contains(&line_id))
    else {
        return Ok(());
    };
    let base = stage_data.geometry.as_ref().unwrap().lines[line_id];
    let previous = frame_transform(track, state.frame - 1).line(base);
    let current = frame_transform(track, state.frame).line(base);
    fighter.position = stage::remap_point(
        [previous.start, previous.end],
        [current.start, current.end],
        fighter.position,
    );
    if fighter.position.into_iter().any(|value| !value.is_finite()) {
        return Err(Error::NonFinite);
    }
    Ok(())
}

fn apply(stage_data: &data::Stage, rules: &Rules, frame: u32, geometry: &mut data::StageGeometry) {
    let base = stage_data.geometry.as_ref().unwrap();
    for track in &rules.tracks {
        let transform = frame_transform(track, frame);
        for id in track.lines.clone() {
            geometry.lines[id] = transform.line(base.lines[id]);
        }
    }
    for joint in &mut geometry.joints {
        let ranges = [
            joint.floor.clone(),
            joint.ceiling.clone(),
            joint.left_wall.clone(),
            joint.right_wall.clone(),
            joint.dynamic.clone(),
        ];
        let mut points = ranges
            .into_iter()
            .flatten()
            .flat_map(|id| [geometry.lines[id].start, geometry.lines[id].end]);
        if let Some(first) = points.next() {
            let (mut min, mut max) = (first, first);
            for point in points {
                for axis in 0..2 {
                    min[axis] = min[axis].min(point[axis]);
                    max[axis] = max[axis].max(point[axis]);
                }
            }
            joint.bounds_min = [min[0] - 30.0, min[1] - 30.0];
            joint.bounds_max = [max[0] + 30.0, max[1] + 30.0];
        }
    }
}

fn frame_transform(track: &Track, frame: u32) -> Transform {
    track.frames[frame as usize % track.frames.len()]
}

fn valid_transform(transform: Transform) -> bool {
    transform
        .matrix
        .into_iter()
        .flatten()
        .all(|value| value.is_finite() && value.abs() <= 1_000_000.0)
}

fn valid_line(line: Line) -> bool {
    let kind = line.flags & (stage::FLOOR | stage::CEILING | stage::LEFT_WALL | stage::RIGHT_WALL);
    let directed = match kind {
        stage::FLOOR => line.start[0] < line.end[0],
        stage::CEILING => line.start[0] > line.end[0],
        stage::LEFT_WALL => line.start[1] < line.end[1],
        stage::RIGHT_WALL => line.start[1] > line.end[1],
        _ => false,
    };
    directed
        && line
            .start
            .into_iter()
            .chain(line.end)
            .all(|value| value.is_finite() && value.abs() <= 1_000_000.0)
}
