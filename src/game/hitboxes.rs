//! Hitbox motion history from `ftColl_8007AD18` (`src/melee/ft/ftcoll.c`).
//! `ftAction_8007121C` re-enables a slot when its hit group changes or the slot
//! was disabled. Native frame hitbox indices are slots; absent slots are disabled.
//! The original Enabled state initializes both centers; subsequent Unk2/Unk3
//! updates shift the old current center to previous before evaluating the new one.

use super::{Error, data::AttackFrame};
use crate::{
    collision::bones::{BoneCapsule, Pose},
    fighter::combat::Capsule,
};
use serde::Serialize;

/// World-space history included in match checkpoints and serialized traces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Track {
    pub group: Option<u8>,
    pub previous: [f32; 3],
    pub current: [f32; 3],
    pub radius: f32,
}

/// Update every frame, including hitlag frames: a frozen world pose collapses
/// the previous sweep to a stationary sphere. `None` disables every slot.
/// Radius uses the current sample; nonuniform transformed radii are unsupported.
/// Errors preserve all tracks.
pub fn update_tracks(
    tracks: &mut [Track; 4],
    frame: Option<&AttackFrame>,
    pose: &Pose,
) -> Result<[Option<Capsule>; 4], Error> {
    let mut next = [Track::default(); 4];
    let mut capsules = [None; 4];
    if let Some(frame) = frame {
        if frame.hitboxes.len() > tracks.len() {
            return Err(Error::Data("at most four hitbox slots per frame".into()));
        }
        for (slot, hit) in frame.hitboxes.iter().enumerate() {
            let world = BoneCapsule::sphere(hit.bone, hit.center, hit.radius)
                .transform(pose, 1.0)
                .map_err(|error| Error::Physics(error.to_string()))?;
            let previous = if tracks[slot].group == Some(hit.group) {
                tracks[slot].current
            } else {
                world.start
            };
            next[slot] = Track {
                group: Some(hit.group),
                previous,
                current: world.start,
                radius: world.radius,
            };
            capsules[slot] = Some(Capsule {
                start: previous,
                end: world.start,
                radius: world.radius,
            });
        }
    }
    *tracks = next;
    Ok(capsules)
}
