//! Per-fighter joint-pose blending across a motion-state entry.
//!
//! Ports the blend recursion `Fighter_ChangeMotionState` arms on every
//! motion change (`ft/fighter.c:1236-1300`, `ft/ftanim.c:388-428`): each
//! anim-phase frame, `ftAnim_8006E9B4` (`ft/ftanim.c:314-376`) advances
//! `x8A4_animBlendFrames`/`x8A8_anim_frame` and calls `ftAnim_8006FE9C`
//! (`ft/ftanim.c:925-940`), which blends every joint from `FtPart_TransN`
//! down (bone 0, TopN, is never blended) through `lb_8000C490`
//! (`melee/lb/lb_00B0.c:469-550`). With `blend_frames == 0` the figatree
//! drives the model directly (`JOBJ_USE_QUATERNION` cleared,
//! `ft/ftanim.c:330-339`), matching this codebase's pre-batch behavior
//! exactly.
//!
//! # Scope of this batch
//!
//! `Fighter_ChangeMotionState`'s own `blend` argument is `(anim_blend ==
//! -1) ? 0 : anim_blend ? anim_blend : (*unk_byte_ptr)[0]` -- an explicit
//! per-call-site override, an explicit force-zero, or the destination
//! subaction's own default byte. A decomp grep for every direct call site
//! passing a literal `-1` or a nonzero literal blend (see
//! `docs/pose-blend.md`) found none in Fox's or Falco's own code, and only
//! three in shared common code, all in mechanics this codebase does not
//! model yet (a death/rebirth respawn transition, a Yoshi-egg fall entry,
//! and a duplicate item-parasol path). So for the fighters this codebase
//! actually simulates, the full rule collapses to just the default-byte
//! case, and `simulation::enter` (this module's only caller) always arms
//! with `explicit: None`, deferring to the pack's own default byte. A
//! future batch should thread real explicit values through if a modeled
//! action ever needs one; `arm`'s `explicit` parameter already carries that
//! contract.
//!
//! The pack schema carries one `blend_frames` byte per pose-source table
//! (currently only `data::MovementPoses`, `docs/pose-blend.md`) rather than
//! Melee's real per-subaction array; every pack through v13 has none, so
//! `resolve_pending` always resolves to `0` for them and blending never
//! engages -- byte-for-byte identical to this codebase's pre-batch pose
//! sampling.

use crate::collision::bones::{Bone, LocalTransform};
use crate::compat::math::quaternion;
use serde::Serialize;

/// Per-fighter pose-blend state, checkpointed as part of `game::Fighter`
/// like every other physics state.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PoseBlend {
    /// This frame's local transform per joint (index-aligned with
    /// `FighterData.bones`/the pose source's own bone list), after
    /// blending -- the model's own currently-displayed pose, carried
    /// across motion-state entries so a fresh blend has a real
    /// `model_joint_prev` to interpolate away from (`arm` never clears
    /// this). `None` only before this fighter's very first anim-phase
    /// update; `simulation::pose` falls back to the unblended sample in
    /// that case, which is also exactly correct once blending is inactive
    /// (`blend_frames == Some(0)`), since `advance` always fills `joints`
    /// with the raw target then.
    pub joints: Option<Vec<LocalTransform>>,
    /// `x8A4_animBlendFrames`. `None` means "not yet resolved for the
    /// current action" -- `simulation::enter` cannot look up the pack's
    /// default byte itself (it has no `&FighterData`), so resolution is
    /// deferred to the first `update` call this action, which does.
    pub blend_frames: Option<u8>,
    /// `x8A8_anim_frame`.
    pub blend_elapsed: f32,
}

impl Default for PoseBlend {
    fn default() -> Self {
        Self {
            joints: None,
            blend_frames: None,
            blend_elapsed: 0.0,
        }
    }
}

/// `ftAnim_8006EBE8` (`ft/ftanim.c:388-428`), called from every
/// `Fighter_ChangeMotionState`: arms the blend for the just-entered action.
/// `explicit` is the decomp's own `anim_blend` parameter when a call site
/// supplies `-1` (force `0`, i.e. instantaneous) or a nonzero literal;
/// `None` defers to the pack's per-profile default byte (see the module
/// doc comment for why every call site this codebase currently reaches
/// passes `None`).
pub fn arm(blend: &mut PoseBlend, explicit: Option<i32>) {
    // `joints` is deliberately left untouched: it holds the model's own
    // currently-displayed pose (last frame's blend result, or the previous
    // action's fully-blended pose), exactly the `model_joint_prev` the new
    // blend recursion needs to interpolate away from on its own first
    // frame (`lb_8000C490`). Clearing it here would make every blend start
    // from the *new* target instead of the old displayed pose.
    blend.blend_elapsed = 0.0;
    blend.blend_frames = match explicit {
        Some(-1) => Some(0),
        Some(n) if n > 0 => Some(n.min(i32::from(u8::MAX)) as u8),
        Some(_) => Some(0),
        None => None,
    };
}

/// Resolves a still-pending (`None`) `blend_frames` using the pack's
/// default byte for the profile the current action samples. Idempotent
/// once resolved, so calling this every frame (cheap: one comparison) is
/// safe.
pub fn resolve_pending(blend: &mut PoseBlend, default_byte: u8) {
    if blend.blend_frames.is_none() {
        blend.blend_frames = Some(default_byte);
    }
}

/// `ftAnim_8006E9B4`/`ftAnim_8006FE9C`: recomputes this frame's blended
/// local pose from `target` (this frame's raw figatree sample -- the same
/// bones `simulation::pose` would use unblended) and advances
/// `blend_elapsed`. Bone 0 (TopN) is excluded, matching `FtPart_TransN`
/// being the blend recursion's own starting point, and copied from
/// `target` unchanged (`simulation::pose` already sets its rotation from
/// facing separately). `framerate` is fixed at `1.0`, matching every pack
/// animation this codebase samples at native frame rate.
///
/// Must be called after `resolve_pending` this frame; if `blend_frames` is
/// still `None` (caller error), this falls back to the raw target so
/// behavior stays defined rather than panicking.
pub fn advance(blend: &mut PoseBlend, target: &[Bone]) {
    let Some(frames) = blend.blend_frames else {
        blend.joints = Some(target.iter().map(|bone| bone.local).collect());
        return;
    };
    if frames == 0 {
        blend.joints = Some(target.iter().map(|bone| bone.local).collect());
        return;
    }
    let previous = blend
        .joints
        .take()
        .unwrap_or_else(|| target.iter().map(|bone| bone.local).collect());
    blend.blend_elapsed += 1.0;
    let t = if f32::from(frames) <= blend.blend_elapsed {
        1.0
    } else {
        1.0 / (1.0 + (f32::from(frames) - blend.blend_elapsed))
    };
    let t_inv = 1.0 - t;
    let mut joints = Vec::with_capacity(target.len());
    for (index, bone) in target.iter().enumerate() {
        if index == 0 {
            joints.push(bone.local);
            continue;
        }
        let model_prev = previous.get(index).copied().unwrap_or(bone.local);
        joints.push(blend_joint(bone.local, model_prev, t, t_inv));
    }
    blend.joints = Some(joints);
}

/// `lb_8000C490` (`melee/lb/lb_00B0.c:469-550`): translation/scale linear
/// blend (`a*t + b*t_inv`); rotation copies the anim (figatree) value
/// directly when both sides are already plain Euler and every component
/// differs by no more than `1e-4`, else converts both to quaternions
/// (`EulerToQuat`), negates the model's previous-frame quaternion if its
/// squared-difference sum from the anim quaternion exceeds its
/// squared-sum sum, and calls `HSD_QuatLib_8037EF28(anim, model_prev,
/// out, t_inv)`.
///
/// Every pack bone this codebase loads has `flags_b0`/`flags_b4`/`flags_b5`
/// all false (the schema has no such flags -- `docs/pose-blend.md`), so the
/// "not flags_b0/flags_b5" gate and `flags_b4`'s "copy instead" branch are
/// not modeled: every non-root bone always takes this ordinary blend path.
fn blend_joint(
    anim: LocalTransform,
    model_prev: LocalTransform,
    t: f32,
    t_inv: f32,
) -> LocalTransform {
    let lerp = |a: f32, b: f32| a * t + b * t_inv;
    let translation =
        core::array::from_fn(|i| lerp(anim.translation[i], model_prev.translation[i]));
    let scale = core::array::from_fn(|i| lerp(anim.scale[i], model_prev.scale[i]));

    let euler_close = anim.rotation_quaternion.is_none()
        && model_prev.rotation_quaternion.is_none()
        && anim
            .rotation
            .iter()
            .zip(model_prev.rotation)
            .all(|(a, b)| (a - b).abs() <= 1e-4);
    if euler_close {
        return LocalTransform {
            translation,
            rotation: anim.rotation,
            rotation_quaternion: None,
            scale,
        };
    }
    let q1 = anim
        .rotation_quaternion
        .unwrap_or_else(|| quaternion::from_euler(anim.rotation));
    let mut q2 = model_prev
        .rotation_quaternion
        .unwrap_or_else(|| quaternion::from_euler(model_prev.rotation));
    let diff: f32 = q1.iter().zip(q2).map(|(a, b)| (a - b) * (a - b)).sum();
    let add: f32 = q1.iter().zip(q2).map(|(a, b)| (a + b) * (a + b)).sum();
    if diff > add {
        q2 = q2.map(|x| -x);
    }
    let out = quaternion::interpolate(q1, q2, t_inv);
    LocalTransform {
        translation,
        rotation: [0.0; 3],
        rotation_quaternion: Some(out),
        scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn euler(translation: [f32; 3], rotation: [f32; 3]) -> LocalTransform {
        LocalTransform {
            translation,
            rotation,
            rotation_quaternion: None,
            scale: [1.0; 3],
        }
    }

    fn bones(local: LocalTransform) -> Vec<Bone> {
        vec![
            Bone::default(), // bone 0 (TopN), never blended
            Bone {
                parent: Some(0),
                local,
                classical_scale: false,
            },
        ]
    }

    /// Blend 0 must be byte-for-byte identical to the pre-batch unblended
    /// sample, on the very first frame and every frame after (the pack
    /// default byte resolves to 0 for every pack through v13).
    #[test]
    fn blend_zero_matches_the_raw_target_exactly() {
        let mut blend = PoseBlend::default();
        resolve_pending(&mut blend, 0);
        let target = bones(euler([1.0, 2.0, 3.0], [0.1, 0.2, 0.3]));
        advance(&mut blend, &target);
        assert_eq!(blend.joints.as_ref().unwrap()[1], target[1].local);
        // A second frame, with a different target, still snaps exactly.
        let target2 = bones(euler([4.0, 5.0, 6.0], [0.4, 0.5, 0.6]));
        advance(&mut blend, &target2);
        assert_eq!(blend.joints.as_ref().unwrap()[1], target2[1].local);
    }

    /// A synthetic two-frame blend's own `t` sequence must be exactly
    /// `1/N, 1/(N-1), ..., 1` (`t = (blend <= x8A8) ? 1 : framerate /
    /// (framerate + (blend - x8A8))`, `x8A8` incrementing by `framerate ==
    /// 1.0` every call). `advance` feeds each frame's own blended result
    /// back in as the next frame's `model_joint_prev`
    /// (`lb_8000C490`/`ftAnim_8006FE9C` read the model's own live joint,
    /// which this recursion's own last write already updated) rather than
    /// re-blending from a fixed starting pose, so this checks the `t`
    /// sequence through its exact closed form under that compounding:
    /// blending translation `0 -> 1` this way lands exactly on `k/N` after
    /// the `k`-th of `N` frames, for every `N` (verified here for `N` in
    /// `1..=5`, not just the minimal two-frame case).
    #[test]
    fn blend_t_sequence_is_exactly_1_over_n_counting_down() {
        for frames in 1_u8..=5 {
            let mut blend = PoseBlend::default();
            // Seed a resting displayed pose (translation 0) as if left
            // behind by a previous, already-settled action, with no blend
            // active -- the real `model_joint_prev` a fresh blend
            // interpolates away from.
            resolve_pending(&mut blend, 0);
            advance(&mut blend, &bones(euler([0.0, 0.0, 0.0], [0.0; 3])));
            // Arm a fresh blend without disturbing the carried-over
            // `joints` (`arm`'s own contract), then resolve its default
            // byte to `frames`.
            arm(&mut blend, None);
            resolve_pending(&mut blend, frames);
            let target = bones(euler([1.0, 0.0, 0.0], [0.0; 3]));
            for k in 1..=frames {
                advance(&mut blend, &target);
                let observed = blend.joints.as_ref().unwrap()[1].translation[0];
                let expected = f32::from(k) / f32::from(frames);
                assert!(
                    (observed - expected).abs() < 1e-6,
                    "frames={frames} k={k}: {observed} != {expected}"
                );
            }
            // t stays pinned at 1 (fully blended) on every subsequent frame.
            advance(&mut blend, &target);
            assert!((blend.joints.as_ref().unwrap()[1].translation[0] - 1.0).abs() < 1e-6);
        }
    }

    /// A rotation difference beyond the `1e-4` Euler-copy threshold takes
    /// the quaternion path and sets `rotation_quaternion`.
    #[test]
    fn a_large_rotation_gap_takes_the_quaternion_path() {
        let mut blend = PoseBlend::default();
        resolve_pending(&mut blend, 2);
        let target = bones(euler([0.0; 3], [1.0, 0.0, 0.0]));
        advance(&mut blend, &target); // model_prev defaults to target on frame 1: still Euler
        let target2 = bones(euler([0.0; 3], [-1.0, 0.0, 0.0]));
        advance(&mut blend, &target2);
        let blended = blend.joints.as_ref().unwrap()[1];
        assert!(
            blended.rotation_quaternion.is_some(),
            "a >1e-4 rotation gap must set JOBJ_USE_QUATERNION"
        );
        // The quaternion result must itself be finite and normalized-scale
        // (interpolate does not normalize, but for two valid unit inputs at
        // an interior t the result should not blow up).
        let q = blended.rotation_quaternion.unwrap();
        assert!(q.iter().all(|x| x.is_finite()));
    }

    /// A rotation difference within the `1e-4` threshold copies the anim
    /// (figatree) rotation directly and clears the quaternion flag, exactly
    /// like Melee's own "close enough" fast path.
    #[test]
    fn a_small_rotation_gap_copies_the_anim_euler_value() {
        let mut blend = PoseBlend::default();
        resolve_pending(&mut blend, 2);
        let target = bones(euler([0.0; 3], [0.5, 0.0, 0.0]));
        advance(&mut blend, &target);
        let target2 = bones(euler([0.0; 3], [0.500_005, 0.0, 0.0]));
        advance(&mut blend, &target2);
        let blended = blend.joints.as_ref().unwrap()[1];
        assert_eq!(blended.rotation_quaternion, None);
        assert_eq!(blended.rotation, target2[1].local.rotation);
    }

    /// Bone 0 (TopN) is never blended, even mid-blend: `simulation::pose`
    /// sets its rotation from facing separately, and the recursion must not
    /// interfere.
    #[test]
    fn bone_zero_is_never_blended() {
        let mut blend = PoseBlend::default();
        resolve_pending(&mut blend, 4);
        let mut target = bones(euler([0.0; 3], [0.0; 3]));
        target[0].local.rotation = [0.0, 1.5, 0.0];
        advance(&mut blend, &target);
        let mut target2 = target.clone();
        target2[0].local.rotation = [0.0, -1.5, 0.0];
        advance(&mut blend, &target2);
        assert_eq!(blend.joints.as_ref().unwrap()[0], target2[0].local);
    }
}
