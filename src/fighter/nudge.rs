//! Fixed horizontal/depth nudges from ftCommon_8007E0E4, 8007DD7C and 8007DFD0.
//! These are per-frame X/Z velocities, not penetration correction or a guarantee
//! against crossing. Inputs retain entity-list order and pre-physics positions.
//! The native match scheduler does not yet call this helper.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// Common data x450, x454, x458, x45C and x460, respectively.
    pub horizontal_step: f32,
    pub depth_step: f32,
    pub depth_limit: f32,
    pub follower_depth_step: f32,
    pub follower_depth_limit: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub position: [f32; 3],
    /// xD4_unk_vel: accumulated position offset used by ftCommon_8007F8B4.
    pub deferred_position: [f32; 3],
    pub facing: f32,
    /// The already scaled ft_data->x50 Vec2: facing-relative X offset and width.
    pub center_offset: f32,
    pub half_width: f32,
    pub player_id: u8,
    /// None means airborne. Grounded subjects require a valid floor index.
    pub floor: Option<usize>,
    /// Player_GetEntity(player_id) for a follower; None means a leader.
    pub follower_of: Option<usize>,
    /// x221F_b3 excludes an ordinary target. The outer animation callback also
    /// skips inactive subjects; E0E4 itself does not, and neither does velocity().
    pub inactive: bool,
    /// victim_gobj != NULL excludes an ordinary target, but not a follower's owner.
    pub holds_victim: bool,
    /// x2219_b1 suppresses this subject's entire nudge calculation.
    pub nudge_disabled: bool,
    /// x2219_b5 suppresses this subject, but does not exclude it as a target.
    pub hitlag: bool,
    /// x221D_b5 skips overlap nudges but still allows depth recentering.
    pub overlap_disabled: bool,
}

impl Body {
    /// ftCommon_8007F8B4, retaining the source's three independent additions.
    pub fn effective_position(&self) -> [f32; 3] {
        core::array::from_fn(|axis| self.position[axis] + self.deferred_position[axis])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Neighbors {
    /// Results of mpLineGetPrev/Next, including their alternate-link checks.
    /// collision::stage::Stage::neighbor supplies these for native geometry.
    pub previous: Option<usize>,
    pub next: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error(
        "nudge inputs require finite positions/offsets, unit facing and nonnegative widths/rules"
    )]
    Input,
    #[error("nudge body, owner or floor reference is invalid")]
    Reference,
    #[error("nudge arithmetic produced a nonfinite result")]
    NonFinite,
}

/// Calculate one subject's X/Z nudge without changing any body. Callers preserve
/// source Anim→nudge entity order and defer position integration until physics.
/// `neighbors` contains resolved links, not unchecked stage adjacency IDs.
/// Finite source results retain their bits, including signed zero. Invalid
/// physical inputs/references and nonfinite effective positions/results error.
pub fn velocity(
    subject: usize,
    bodies: &[Body],
    neighbors: &[Neighbors],
    rules: &Rules,
) -> Result<[f32; 2], Error> {
    validate(subject, bodies, neighbors, rules)?;
    let body = &bodies[subject];
    let mut nudge = [0.0; 2];
    let Some(floor) = body.floor.filter(|_| !body.nudge_disabled && !body.hitlag) else {
        return Ok(nudge);
    };
    let position = body.effective_position();
    let (step, limit) = if body.follower_of.is_some() {
        (rules.follower_depth_step, rules.follower_depth_limit)
    } else {
        (rules.depth_step, rules.depth_limit)
    };
    let shares_floor = |target: &Body| {
        target.floor.is_some_and(|other| {
            other == floor
                || Some(other) == neighbors[floor].next
                || Some(other) == neighbors[floor].previous
        })
    };
    let difference = |target: &Body| {
        (body.center_offset * body.facing + position[0])
            - (target.facing * target.center_offset + target.effective_position()[0])
    };
    if !body.overlap_disabled {
        // DFD0 adds the follower's owner bias before the ordinary fighter scan.
        if let Some(owner) = body.follower_of.map(|index| &bodies[index])
            && !owner.inactive
            && shares_floor(owner)
            && difference(owner).abs() < body.half_width + owner.half_width
        {
            nudge[1] -= rules.follower_depth_step;
        }
        let mut seen_same_player = false;
        for (index, target) in bodies.iter().enumerate() {
            if index == subject || target.player_id == body.player_id {
                seen_same_player = true;
                continue;
            }
            if target.inactive
                || target.holds_victim
                || target.follower_of.is_some()
                || !shares_floor(target)
            {
                continue;
            }
            let dx = difference(target);
            if dx.abs() < body.half_width + target.half_width {
                let left = if dx != 0.0 {
                    dx < 0.0
                } else {
                    seen_same_player
                };
                nudge[0] += if left {
                    -rules.horizontal_step
                } else {
                    rules.horizontal_step
                };
                let dz = position[2] - target.effective_position()[2];
                let behind = if dz != 0.0 { dz < 0.0 } else { left };
                nudge[1] += if behind {
                    -rules.depth_step
                } else {
                    rules.depth_step
                };
            }
        }
    }
    let mut depth = position[2];
    if nudge[1] == 0.0 && depth != 0.0 {
        nudge[1] = if depth < 0.0 { step } else { -step };
    }
    if (nudge[1] > 0.0 && depth < 0.0 && depth + nudge[1] >= 0.0)
        || (nudge[1] < 0.0 && depth > 0.0 && depth + nudge[1] <= 0.0)
    {
        nudge[1] = -depth;
        // Source changes this temporary before its cap checks. Replacing the
        // sequence with clamp(depth+nudge)-depth changes overshoot behavior.
        depth = 0.0;
    }
    if depth + nudge[1] > limit {
        nudge[1] = limit - depth;
    } else if depth + nudge[1] < -limit {
        nudge[1] = -limit - depth;
    }
    if body.follower_of.is_some() {
        nudge[0] = 0.0;
    }
    if nudge.into_iter().all(f32::is_finite) {
        Ok(nudge)
    } else {
        Err(Error::NonFinite)
    }
}

fn validate(
    subject: usize,
    bodies: &[Body],
    neighbors: &[Neighbors],
    rules: &Rules,
) -> Result<(), Error> {
    if subject >= bodies.len()
        || neighbors.iter().any(|n| {
            n.previous
                .into_iter()
                .chain(n.next)
                .any(|i| i >= neighbors.len())
        })
    {
        return Err(Error::Reference);
    }
    if ![
        rules.horizontal_step,
        rules.depth_step,
        rules.depth_limit,
        rules.follower_depth_step,
        rules.follower_depth_limit,
    ]
    .into_iter()
    .all(|x| x.is_finite() && x >= 0.0)
    {
        return Err(Error::Input);
    }
    for (index, body) in bodies.iter().enumerate() {
        if body.floor.is_some_and(|i| i >= neighbors.len())
            || body.follower_of.is_some_and(|owner| {
                owner == index
                    || bodies.get(owner).is_none_or(|leader| {
                        leader.player_id != body.player_id || leader.follower_of.is_some()
                    })
                    || bodies.iter().any(|other| {
                        other.player_id == body.player_id
                            && other.follower_of.is_some_and(|index| index != owner)
                    })
            })
        {
            return Err(Error::Reference);
        }
        if !body
            .position
            .into_iter()
            .chain(body.deferred_position)
            .chain([body.center_offset, body.half_width])
            .all(f32::is_finite)
            || body.half_width < 0.0
            || ![-1.0, 1.0].contains(&body.facing)
        {
            return Err(Error::Input);
        }
        if !body.effective_position().into_iter().all(f32::is_finite) {
            return Err(Error::NonFinite);
        }
    }
    Ok(())
}
