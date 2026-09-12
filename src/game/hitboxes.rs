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

/// Per-attacker `hit_groups` bits (`src/game/simulation.rs`) that a script
/// (re-)creates this frame after being absent from every one of the four
/// slots on the previous frame, so the caller can clear the matching
/// victim-record bit before this frame's hit check -- the generic
/// counterpart of `ftAction_8007121C`'s own re-enable gate.
///
/// `ftAction_8007121C` (`ftaction.c:284-359`, the CreateHitbox opcode) only
/// treats a hitbox as freshly spawned when `hitbox->state ==
/// HitCapsule_Disabled || hitbox->x4 != hit_group` (`x4` is the capsule's
/// own hit-group id, i.e. this crate's `Hitbox::group`); a same-group
/// re-issue of an already-`Enabled` capsule is a no-op. On a fresh spawn it
/// calls `ftColl_800768A0` (`ftcoll.c:301-314`), which searches the
/// attacker's other three capsules (`fp->x914[4]`, `ftFox/types.h:1309`) for
/// one still `!= HitCapsule_Disabled` sharing the same group: if found, it
/// copies that capsule's victim lists (`lbColl_CopyHitCapsule`,
/// `lbcollision.c:1809-1822`) so the group's hit record survives the
/// hand-off; otherwise it clears them (`lbColl_80008440`,
/// `lbcollision.c:1796-1807`, zeroing `victims_1`/`victims_2` and their
/// counts `x44`/`x45`), letting the group hit the same victim again. The
/// explicit ClearHitbox/ClearAllHitboxes opcodes
/// (`ftAction_80071784`/`ftAction_800717D8`, `ftaction.c:426-444`, via
/// `ftColl_8007AFC8`/`ftColl_8007AFF8` -> `lbColl_80008428`) only flip
/// `state` to `Disabled`; they never touch the victim lists themselves --
/// the clear happens lazily, on the next creation.
///
/// This crate has no per-opcode timeline, only one exported `AttackFrame`
/// per simulation frame, so "disabled" is read back as "absent from
/// `frame.hitboxes`" (`update_tracks`'s own contract) and "was there
/// already-enabled capsule sharing this group" as "did any of the four
/// `tracks` slots already carry `Some(hit.group)` on the previous frame" --
/// the same per-slot record `update_tracks` already keeps, checked across
/// every slot (not just this one) to match `ftColl_800768A0`'s own
/// all-capsules search. Call this before [`update_tracks`] overwrites
/// `tracks` for the frame, since it reads the *previous* frame's state.
///
/// A hitbox present on every consecutive frame from its own action entry
/// (jab, Travel's continuous hit) is never absent, so this never fires for
/// it past its own first frame, matching the pinned source's own
/// same-group-reissue no-op. A script that clears a hitbox and re-creates
/// it again on the very same exported frame (jab's own `clear_hits` flag,
/// the rapid-jab loop restart) is invisible to this per-frame presence
/// check, since the capsule never leaves `frame.hitboxes` at that
/// granularity -- those two moves keep their own explicit, already-working
/// reset (`jab.rs`'s `FrameFlags::clear_hits` and the loop's own frame-zero
/// `hit_groups = 0`) rather than relying on this generic mechanism.
pub fn refreshed_groups(tracks: &[Track; 4], frame: Option<&AttackFrame>) -> u16 {
    let mut refreshed = 0u16;
    if let Some(frame) = frame {
        for hit in &frame.hitboxes {
            if !tracks.iter().any(|track| track.group == Some(hit.group)) {
                refreshed |= 1 << hit.group;
            }
        }
    }
    refreshed
}
