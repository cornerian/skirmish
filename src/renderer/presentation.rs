//! Exact joins between immutable native presentation data and visual exports.
//!
//! Native descriptor offsets are meaningful only inside the exact archive that
//! supplied them. This adapter verifies resource provenance, normalizes the
//! visual export's declared offset spaces, and retains complete occurrence
//! identity before any sampled animation is allowed to reach the renderer.

use std::{collections::HashMap, sync::Arc};

use thiserror::Error;

use crate::{
    animation::DataOffset,
    menu::{AnimationCue, AnimationId},
    presentation::{
        AnimationPlayback, PlaybackError,
        instance::{
            InstanceError, InstanceId, SceneInstance, SourceJointId, SourceMaterialId,
            SourceTarget, SourceTextureId,
        },
        manifest::{
            BatchApplyError, BindError, BoundClip, BoundHierarchy, BoundPresentation,
            PresentationUpdate, SampleError, SourceBindingIdentity, SourceObjectKind,
            VisualOffsetSpaces,
        },
    },
};

use super::{
    gpu::{
        DrawUpdate, ExportDrawSelector, ExportMaterialSelector, RuntimeDrawBatchError,
        RuntimeDrawUpdate, WindowRenderer,
    },
    scene::{
        GeometrySpace, Scene, VisualJointOccurrence, VisualMaterialOccurrence,
        VisualTextureOccurrence,
    },
};

/// A provenance-checked correspondence between one native hierarchy and its
/// exact visual occurrences.
#[derive(Clone, Debug)]
pub struct VisualPresentationBinding {
    presentation: Arc<BoundPresentation>,
    hierarchy: String,
    joints: HashMap<SourceJointId, VisualJointOccurrence>,
    /// Declared vertex space shared by every draw owned by a bound joint.
    joint_geometry: HashMap<SourceJointId, GeometrySpace>,
    materials: HashMap<SourceMaterialId, VisualMaterialOccurrence>,
    textures: HashMap<SourceTextureId, VisualTextureOccurrence>,
}

impl VisualPresentationBinding {
    /// Bind one hierarchy without using filenames, bare offsets, or exporter
    /// instance strings as identity fallbacks.
    pub fn bind(
        scene: &Scene,
        presentation: Arc<BoundPresentation>,
        hierarchy: &str,
    ) -> Result<Self, VisualPresentationBindError> {
        let bound_hierarchy = presentation.hierarchy(hierarchy).ok_or_else(|| {
            VisualPresentationBindError::MissingHierarchy {
                hierarchy: hierarchy.to_owned(),
            }
        })?;
        validate_resource(scene, presentation.as_ref())?;
        let maps = bind_exact_occurrences(scene, presentation.as_ref(), bound_hierarchy)?;
        if maps.joints.is_empty() && maps.materials.is_empty() && maps.textures.is_empty() {
            return Err(VisualPresentationBindError::NoExactOccurrences {
                resource_id: presentation.resource().id.clone(),
                hierarchy: hierarchy.to_owned(),
            });
        }
        Ok(Self {
            presentation,
            hierarchy: hierarchy.to_owned(),
            joints: maps.joints,
            joint_geometry: maps.joint_geometry,
            materials: maps.materials,
            textures: maps.textures,
        })
    }

    pub fn presentation(&self) -> &BoundPresentation {
        self.presentation.as_ref()
    }

    pub fn hierarchy(&self) -> &str {
        &self.hierarchy
    }

    pub fn joint_occurrence(&self, source: &SourceJointId) -> Option<&VisualJointOccurrence> {
        self.joints.get(source)
    }

    /// Vertex space declared by the draws owned by one bound joint, when any
    /// exported draw is attached to it.
    pub fn joint_geometry_space(&self, source: &SourceJointId) -> Option<GeometrySpace> {
        self.joint_geometry.get(source).copied()
    }

    pub fn material_occurrence(
        &self,
        source: &SourceMaterialId,
    ) -> Option<&VisualMaterialOccurrence> {
        self.materials.get(source)
    }

    pub fn texture_occurrence(&self, source: &SourceTextureId) -> Option<&VisualTextureOccurrence> {
        self.textures.get(source)
    }
}

/// One independently mutable native presentation instance routed to exact
/// visual occurrences.
///
/// Playback and scene state remain private so every source sample is applied as
/// one transaction. GPU application is deliberately separate: a caller may
/// retry the returned tick after a surface or renderer error without sampling
/// another authored frame.
#[derive(Debug)]
pub struct RenderedPresentationInstance {
    binding: Arc<VisualPresentationBinding>,
    instance: SceneInstance,
    playback: AnimationPlayback,
}

impl RenderedPresentationInstance {
    pub fn instantiate(
        binding: Arc<VisualPresentationBinding>,
        instance_id: InstanceId,
        cue: AnimationCue,
    ) -> Result<Self, PresentationDriverError> {
        let playback = validated_playback(binding.as_ref(), cue)?;
        let hierarchy = binding
            .presentation()
            .hierarchy(binding.hierarchy())
            .expect("a visual binding retains its validated native hierarchy");
        let instance = hierarchy.instantiate(instance_id)?;
        Ok(Self {
            binding,
            instance,
            playback,
        })
    }

    pub fn binding(&self) -> &VisualPresentationBinding {
        self.binding.as_ref()
    }

    pub fn scene_instance(&self) -> &SceneInstance {
        &self.instance
    }

    pub fn playback(&self) -> &AnimationPlayback {
        &self.playback
    }

    /// Replace the current cue only after its generic range and bound-HSD
    /// sampling contract have both been validated.
    pub fn restart(&mut self, cue: AnimationCue) -> Result<(), PresentationDriverError> {
        let playback = validated_playback(self.binding.as_ref(), cue)?;
        self.playback = playback;
        Ok(())
    }

    /// Return every current native state field without consuming a playback
    /// tick.
    ///
    /// This is the explicit initial/recovery synchronization path. Joint-local
    /// and texture state remain retained as unsupported instead of assuming the
    /// visual export already agrees with the native resource.
    pub fn state_snapshot(&self) -> RenderedPresentationTick {
        RenderedPresentationTick {
            frame: self.playback.frame(),
            updates: route_updates(self.binding.as_ref(), current_state_updates(&self.instance)),
        }
    }

    /// Sample and atomically commit one authored presentation tick.
    ///
    /// Playback is cloned and committed last.
    /// [`SampleBatch::apply`](crate::presentation::manifest::SampleBatch::apply)
    /// stages the scene mutation, so either sampling/apply failure leaves both
    /// retained states unchanged.
    pub fn tick(&mut self) -> Result<RenderedPresentationTick, PresentationDriverError> {
        let mut playback = self.playback.clone();
        let frame = playback.tick();
        let clip = resolve_clip(self.binding.as_ref(), playback.animation_id())?;
        let batch = clip.sample(frame)?;
        let source_updates = batch.apply(&mut self.instance)?;
        let updates = route_updates(self.binding.as_ref(), source_updates);
        self.playback = playback;
        Ok(RenderedPresentationTick { frame, updates })
    }
}

/// One sampled authored frame plus lossless renderer-routing decisions.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderedPresentationTick {
    frame: f32,
    updates: Vec<RoutedPresentationUpdate>,
}

impl RenderedPresentationTick {
    pub const fn frame(&self) -> f32 {
        self.frame
    }

    pub fn updates(&self) -> &[RoutedPresentationUpdate] {
        &self.updates
    }

    /// Apply all renderable updates as one prevalidated batch.
    ///
    /// The returned vector is one-to-one with [`Self::updates`]. A mapped
    /// update that reaches no retained draw is reported as `MatchedDraws(0)`;
    /// unsupported and unmapped source deltas remain explicit. This result
    /// describes only this tick and is not a claim of whole-presentation parity.
    pub fn apply_to(
        &self,
        renderer: &mut WindowRenderer,
    ) -> Result<Vec<PresentationApplyOutcome>, PresentationRendererError> {
        self.apply_with(|updates| renderer.update_instance_draws_batch(updates))
    }

    fn apply_with(
        &self,
        submit: impl FnOnce(&[RuntimeDrawUpdate<'_>]) -> Result<Vec<usize>, RuntimeDrawBatchError>,
    ) -> Result<Vec<PresentationApplyOutcome>, PresentationRendererError> {
        let mut source_indices = Vec::new();
        let mut draw_updates = Vec::new();
        for (index, routed) in self.updates.iter().enumerate() {
            if let Some(update) = routed.runtime_draw_update() {
                source_indices.push(index);
                draw_updates.push(update);
            }
        }
        let match_counts = submit(&draw_updates).map_err(|source| PresentationRendererError {
            update_index: source_indices[source.index()],
            source,
        })?;
        assert_eq!(
            match_counts.len(),
            source_indices.len(),
            "renderer batch result must preserve one count per update"
        );
        let mut match_counts = match_counts.into_iter();
        Ok(self
            .updates
            .iter()
            .map(|update| match &update.route {
                PresentationUpdateRoute::JointVisibility(_)
                | PresentationUpdateRoute::Material(_) => PresentationApplyOutcome::MatchedDraws(
                    match_counts
                        .next()
                        .expect("every routed draw update has one match count"),
                ),
                PresentationUpdateRoute::Retained(reason) => {
                    PresentationApplyOutcome::Retained(*reason)
                }
            })
            .collect())
    }
}

/// One source delta paired with an immutable routing decision.
///
/// Fields stay private so callers cannot associate a source update with the
/// wrong visual occurrence.
#[derive(Clone, Debug, PartialEq)]
pub struct RoutedPresentationUpdate {
    update: PresentationUpdate,
    route: PresentationUpdateRoute,
}

impl RoutedPresentationUpdate {
    pub fn update(&self) -> &PresentationUpdate {
        &self.update
    }

    pub fn route(&self) -> &PresentationUpdateRoute {
        &self.route
    }

    fn runtime_draw_update(&self) -> Option<RuntimeDrawUpdate<'_>> {
        match (&self.update, &self.route) {
            (
                PresentationUpdate::JointVisibility {
                    instance_id,
                    visible,
                    ..
                },
                PresentationUpdateRoute::JointVisibility(occurrence),
            ) => Some(RuntimeDrawUpdate {
                instance_id: *instance_id,
                update: DrawUpdate::Visibility {
                    target: ExportDrawSelector::Exact(occurrence),
                    visible: *visible,
                },
            }),
            (
                PresentationUpdate::Material {
                    instance_id,
                    diffuse,
                    alpha,
                    ..
                },
                PresentationUpdateRoute::Material(occurrence),
            ) => Some(RuntimeDrawUpdate {
                instance_id: *instance_id,
                update: DrawUpdate::MaterialColor {
                    target: ExportMaterialSelector::Exact(occurrence),
                    color: [
                        f32::from(diffuse[0]) / 255.0,
                        f32::from(diffuse[1]) / 255.0,
                        f32::from(diffuse[2]) / 255.0,
                        *alpha,
                    ],
                },
            }),
            (_, PresentationUpdateRoute::Retained(_)) => None,
            _ => unreachable!("routing is constructed from the same source update"),
        }
    }
}

/// How one native source delta can reach the current renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationUpdateRoute {
    JointVisibility(VisualJointOccurrence),
    Material(VisualMaterialOccurrence),
    Retained(RetainedPresentationReason),
}

/// Why a lossless native source delta was retained instead of rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetainedPresentationReason {
    /// The joint owns joint-local draws, but the renderer has no per-draw
    /// joint transform yet.
    UnsupportedJointLocal,
    /// The joint's exported draws were baked into world space, so a native
    /// local transform cannot move them without double-transforming.
    BakedWorldGeometry,
    UnsupportedTexture,
    /// No exported draw is attached directly to this joint. Descendant draws
    /// are not repositioned until hierarchy composition exists.
    UnmappedJointLocal,
    UnmappedJointVisibility,
    UnmappedMaterial,
}

/// Renderer result corresponding to exactly one routed source delta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationApplyOutcome {
    MatchedDraws(usize),
    Retained(RetainedPresentationReason),
}

#[derive(Debug, Error)]
pub enum PresentationDriverError {
    #[error("presentation clip {clip:?} does not exist")]
    MissingClip { clip: AnimationId },
    #[error(
        "presentation clip {clip:?} belongs to hierarchy {actual:?}, not bound hierarchy {expected:?}"
    )]
    ClipHierarchyMismatch {
        clip: AnimationId,
        expected: String,
        actual: String,
    },
    #[error(
        "presentation clip {clip:?} has non-integral HSD-v1 {field} frame bits 0x{frame_bits:08x}"
    )]
    NonIntegralCueRange {
        clip: AnimationId,
        field: &'static str,
        frame_bits: u32,
    },
    #[error(transparent)]
    Playback(#[from] PlaybackError),
    #[error(transparent)]
    Instance(#[from] InstanceError),
    #[error(transparent)]
    Sample(#[from] SampleError),
    #[error(transparent)]
    Apply(#[from] BatchApplyError),
}

#[derive(Debug, Error)]
#[error("apply presentation update {update_index}: {source}")]
pub struct PresentationRendererError {
    update_index: usize,
    #[source]
    source: RuntimeDrawBatchError,
}

impl PresentationRendererError {
    pub const fn update_index(&self) -> usize {
        self.update_index
    }
}

fn validated_playback(
    binding: &VisualPresentationBinding,
    cue: AnimationCue,
) -> Result<AnimationPlayback, PresentationDriverError> {
    let playback = AnimationPlayback::new(cue)?;
    resolve_clip(binding, playback.animation_id())?;
    let range = playback.frame_range();
    for (field, frame) in [
        ("start", Some(range.start)),
        ("end", Some(range.end)),
        ("loop_start", range.loop_start),
    ] {
        if let Some(frame) = frame
            && frame.fract() != 0.0
        {
            return Err(PresentationDriverError::NonIntegralCueRange {
                clip: playback.animation_id().clone(),
                field,
                frame_bits: frame.to_bits(),
            });
        }
    }
    Ok(playback)
}

fn resolve_clip<'a>(
    binding: &'a VisualPresentationBinding,
    animation: &AnimationId,
) -> Result<&'a BoundClip, PresentationDriverError> {
    let clip = binding.presentation().clip(animation).ok_or_else(|| {
        PresentationDriverError::MissingClip {
            clip: animation.clone(),
        }
    })?;
    if clip.hierarchy() != binding.hierarchy() {
        return Err(PresentationDriverError::ClipHierarchyMismatch {
            clip: animation.clone(),
            expected: binding.hierarchy().to_owned(),
            actual: clip.hierarchy().to_owned(),
        });
    }
    Ok(clip)
}

fn current_state_updates(instance: &SceneInstance) -> Vec<PresentationUpdate> {
    let instance_id = instance.id();
    let mut updates = Vec::with_capacity(
        instance.joints().len() * 2 + instance.materials().len() + instance.textures().len(),
    );
    for joint in instance.joints() {
        updates.push(PresentationUpdate::JointVisibility {
            instance_id,
            source_id: joint.source_id().clone(),
            visible: joint.visible(),
        });
        updates.push(PresentationUpdate::JointLocal {
            instance_id,
            source_id: joint.source_id().clone(),
            local: joint.local(),
        });
    }
    for material in instance.materials() {
        updates.push(PresentationUpdate::Material {
            instance_id,
            source_id: material.source_id().clone(),
            diffuse: material.diffuse(),
            alpha: material.alpha(),
        });
    }
    for texture in instance.textures() {
        updates.push(PresentationUpdate::Texture {
            instance_id,
            source_id: texture.source_id().clone(),
            current_image: texture.current_image().cloned(),
            translation: texture.translation(),
            scale: texture.scale(),
            blend: texture.blend(),
            konst: texture.konst(),
            tev0: texture.tev0(),
        });
    }
    updates
}

fn route_updates(
    binding: &VisualPresentationBinding,
    updates: Vec<PresentationUpdate>,
) -> Vec<RoutedPresentationUpdate> {
    updates
        .into_iter()
        .map(|update| route_update(binding, update))
        .collect()
}

fn route_update(
    binding: &VisualPresentationBinding,
    update: PresentationUpdate,
) -> RoutedPresentationUpdate {
    let route = match &update {
        PresentationUpdate::JointVisibility { source_id, .. } => binding
            .joint_occurrence(source_id)
            .cloned()
            .map(PresentationUpdateRoute::JointVisibility)
            .unwrap_or(PresentationUpdateRoute::Retained(
                RetainedPresentationReason::UnmappedJointVisibility,
            )),
        PresentationUpdate::JointLocal { source_id, .. } => {
            PresentationUpdateRoute::Retained(match binding.joint_geometry_space(source_id) {
                Some(GeometrySpace::JointLocal) => {
                    RetainedPresentationReason::UnsupportedJointLocal
                }
                Some(GeometrySpace::World) => RetainedPresentationReason::BakedWorldGeometry,
                None => RetainedPresentationReason::UnmappedJointLocal,
            })
        }
        PresentationUpdate::Material { source_id, .. } => binding
            .material_occurrence(source_id)
            .cloned()
            .map(PresentationUpdateRoute::Material)
            .unwrap_or(PresentationUpdateRoute::Retained(
                RetainedPresentationReason::UnmappedMaterial,
            )),
        PresentationUpdate::Texture { .. } => {
            PresentationUpdateRoute::Retained(RetainedPresentationReason::UnsupportedTexture)
        }
    };
    RoutedPresentationUpdate { update, route }
}

#[derive(Debug, Error)]
pub enum VisualPresentationBindError {
    #[error("presentation hierarchy {hierarchy:?} does not exist")]
    MissingHierarchy { hierarchy: String },
    #[error("visual scene does not declare presentation resource {resource_id:?}")]
    MissingVisualResource { resource_id: String },
    #[error("visual scene declares presentation resource {resource_id:?} more than once")]
    AmbiguousVisualResource { resource_id: String },
    #[error("visual resource {resource_id:?} hash mismatch: expected {expected}, found {actual}")]
    HashMismatch {
        resource_id: String,
        expected: String,
        actual: String,
    },
    #[error(
        "visual resource {resource_id:?} declares offset spaces {declared:?}, but the presentation manifest expects {expected:?}"
    )]
    OffsetSpaceMismatch {
        resource_id: String,
        declared: VisualOffsetSpaces,
        expected: VisualOffsetSpaces,
    },
    #[error("source joint {target:?} owns draws in both world and joint-local space")]
    MixedGeometrySpace { target: SourceTarget },
    #[error("visual {kind:?} offset {visual_offset:#x} cannot be normalized: {source}")]
    InvalidVisualOffset {
        kind: SourceObjectKind,
        visual_offset: u32,
        #[source]
        source: BindError,
    },
    #[error("visual resource {resource_id:?} has no exact occurrences in hierarchy {hierarchy:?}")]
    NoExactOccurrences {
        resource_id: String,
        hierarchy: String,
    },
    #[error("source target {target:?} maps to conflicting visual occurrences")]
    ConflictingOccurrence { target: SourceTarget },
    #[error(
        "texture source {texture} owner material mismatch: expected {expected:#x}, found {actual:#x}"
    )]
    TextureOwnerMaterialMismatch {
        texture: SourceTextureId,
        expected: u32,
        actual: u32,
    },
}

#[derive(Default)]
struct ExactOccurrenceMaps {
    joints: HashMap<SourceJointId, VisualJointOccurrence>,
    joint_geometry: HashMap<SourceJointId, GeometrySpace>,
    materials: HashMap<SourceMaterialId, VisualMaterialOccurrence>,
    textures: HashMap<SourceTextureId, VisualTextureOccurrence>,
}

fn validate_resource(
    scene: &Scene,
    presentation: &BoundPresentation,
) -> Result<(), VisualPresentationBindError> {
    let expected = presentation.resource();
    let mut matches = scene
        .resources
        .iter()
        .filter(|resource| resource.id().as_str() == expected.id);
    let Some(resource) = matches.next() else {
        return Err(VisualPresentationBindError::MissingVisualResource {
            resource_id: expected.id.clone(),
        });
    };
    if matches.next().is_some() {
        return Err(VisualPresentationBindError::AmbiguousVisualResource {
            resource_id: expected.id.clone(),
        });
    }
    if resource.sha256() != expected.sha256 {
        return Err(VisualPresentationBindError::HashMismatch {
            resource_id: expected.id.clone(),
            expected: expected.sha256.clone(),
            actual: resource.sha256().to_owned(),
        });
    }
    if resource.offset_spaces() != presentation.visual_offsets() {
        return Err(VisualPresentationBindError::OffsetSpaceMismatch {
            resource_id: expected.id.clone(),
            declared: resource.offset_spaces(),
            expected: presentation.visual_offsets(),
        });
    }
    Ok(())
}

fn bind_exact_occurrences(
    scene: &Scene,
    presentation: &BoundPresentation,
    hierarchy: &BoundHierarchy,
) -> Result<ExactOccurrenceMaps, VisualPresentationBindError> {
    let resource_id = presentation.resource().id.as_str();
    let mut maps = ExactOccurrenceMaps::default();
    for mesh in &scene.meshes {
        if let Some(occurrence) = mesh.source_occurrence.as_ref()
            && occurrence.owner_joint.resource_id.as_str() == resource_id
        {
            let identity = joint_identity(presentation, &occurrence.owner_joint)?;
            if let Some(target) = hierarchy.target(identity) {
                let SourceTarget::Joint(source) = &target else {
                    unreachable!("a bound joint identity resolves only to a joint target");
                };
                insert_unique(
                    &mut maps.joints,
                    source.clone(),
                    occurrence.owner_joint.clone(),
                    target.clone(),
                )?;
                if let Some(previous) = maps
                    .joint_geometry
                    .insert(source.clone(), mesh.geometry_space)
                    && previous != mesh.geometry_space
                {
                    return Err(VisualPresentationBindError::MixedGeometrySpace { target });
                }
            }
        }

        if let Some(occurrence) = mesh.material.source_occurrence.as_ref()
            && occurrence.owner_dobj.owner_joint.resource_id.as_str() == resource_id
        {
            let identity = material_identity(presentation, occurrence)?;
            if let Some(target) = hierarchy.target(identity) {
                let SourceTarget::Material(source) = &target else {
                    unreachable!("a bound material identity resolves only to a material target");
                };
                insert_unique(
                    &mut maps.materials,
                    source.clone(),
                    occurrence.clone(),
                    target,
                )?;
            }
        }

        for stage in &mesh.material.texture_sources {
            let Some(occurrence) = stage.occurrence.as_ref() else {
                continue;
            };
            if occurrence
                .owner_material
                .owner_dobj
                .owner_joint
                .resource_id
                .as_str()
                != resource_id
            {
                continue;
            }
            let (identity, owner_material_offset) = texture_identity(presentation, occurrence)?;
            if let Some(target) = hierarchy.target(identity) {
                let SourceTarget::Texture(source) = &target else {
                    unreachable!("a bound texture identity resolves only to a texture target");
                };
                let expected_owner = hierarchy
                    .textures()
                    .iter()
                    .find(|bound| bound.source_id == *source)
                    .expect("a resolved texture target belongs to this hierarchy")
                    .owner_material_offset;
                if owner_material_offset != expected_owner {
                    return Err(VisualPresentationBindError::TextureOwnerMaterialMismatch {
                        texture: source.clone(),
                        expected: expected_owner.get(),
                        actual: owner_material_offset.get(),
                    });
                }
                insert_unique(
                    &mut maps.textures,
                    source.clone(),
                    occurrence.clone(),
                    target,
                )?;
            }
        }
    }
    Ok(maps)
}

fn insert_unique<K, V>(
    map: &mut HashMap<K, V>,
    key: K,
    occurrence: V,
    target: SourceTarget,
) -> Result<(), VisualPresentationBindError>
where
    K: std::hash::Hash + Eq,
    V: PartialEq,
{
    if let Some(previous) = map.get(&key)
        && previous != &occurrence
    {
        return Err(VisualPresentationBindError::ConflictingOccurrence { target });
    }
    map.insert(key, occurrence);
    Ok(())
}

fn normalize(
    presentation: &BoundPresentation,
    kind: SourceObjectKind,
    visual_offset: u32,
) -> Result<DataOffset, VisualPresentationBindError> {
    presentation
        .normalize_visual_offset(kind, visual_offset)
        .map_err(|source| VisualPresentationBindError::InvalidVisualOffset {
            kind,
            visual_offset,
            source,
        })
}

fn joint_identity(
    presentation: &BoundPresentation,
    occurrence: &VisualJointOccurrence,
) -> Result<SourceBindingIdentity, VisualPresentationBindError> {
    let offset = normalize(
        presentation,
        SourceObjectKind::Joint,
        occurrence.visual_offset,
    )?;
    Ok(SourceBindingIdentity {
        kind: SourceObjectKind::Joint,
        descriptor_offset: offset,
        owner_joint_offset: offset,
        dobj_index: None,
        texture_index: None,
    })
}

fn material_identity(
    presentation: &BoundPresentation,
    occurrence: &VisualMaterialOccurrence,
) -> Result<SourceBindingIdentity, VisualPresentationBindError> {
    Ok(SourceBindingIdentity {
        kind: SourceObjectKind::Material,
        descriptor_offset: normalize(
            presentation,
            SourceObjectKind::Material,
            occurrence.visual_offset.get(),
        )?,
        owner_joint_offset: normalize(
            presentation,
            SourceObjectKind::Joint,
            occurrence.owner_dobj.owner_joint.visual_offset,
        )?,
        dobj_index: Some(occurrence.owner_dobj.dobj_index),
        texture_index: None,
    })
}

fn texture_identity(
    presentation: &BoundPresentation,
    occurrence: &VisualTextureOccurrence,
) -> Result<(SourceBindingIdentity, DataOffset), VisualPresentationBindError> {
    let owner_material_offset = normalize(
        presentation,
        SourceObjectKind::Material,
        occurrence.owner_material.visual_offset.get(),
    )?;
    Ok((
        SourceBindingIdentity {
            kind: SourceObjectKind::Texture,
            descriptor_offset: normalize(
                presentation,
                SourceObjectKind::Texture,
                occurrence.visual_offset.get(),
            )?,
            owner_joint_offset: normalize(
                presentation,
                SourceObjectKind::Joint,
                occurrence
                    .owner_material
                    .owner_dobj
                    .owner_joint
                    .visual_offset,
            )?,
            dobj_index: Some(occurrence.owner_material.owner_dobj.dobj_index),
            texture_index: Some(occurrence.tobj_index),
        },
        owner_material_offset,
    ))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use serde::Deserialize;
    use serde_json::json;
    use sha2::{Digest, Sha256};

    use crate::{
        menu::{AnimationCue, FrameRange},
        presentation::manifest::{
            ClipScope, ClipSpec, HierarchySpec, OffsetSpace, PRESENTATION_MANIFEST_SCHEMA,
            PresentationManifest, ResourceSpec, VisualOffsetSpaces,
        },
        renderer::scene::{VisualDObjOccurrence, VisualResourceId},
    };

    use super::*;

    const RESOURCE_ID: &str = "menu.dat";
    const MODEL_ROOT: u32 = 0x20;
    const ANIMATED_MODEL: u32 = 0x100;
    const MAT_ANIM_JOINT: u32 = 0x300;
    const MAT_ANIM: u32 = 0x340;
    const DOBJ: u32 = 0x400;
    const MOBJ: u32 = 0x440;
    const MATERIAL: u32 = 0x460;
    const TOBJ: u32 = 0x480;
    const TEV: u32 = 0x4e0;
    const TEX_ANIM: u32 = 0x520;
    const IMAGE_TABLE: u32 = 0x540;
    const IMAGE_0: u32 = 0x580;
    const IMAGE_2: u32 = 0x5a0;
    const SECOND_DOBJ: u32 = 0x680;
    const SECOND_MAT_ANIM: u32 = 0x6a0;
    const TEXTURE_AOBJ: u32 = 352_476;
    const MATERIAL_AOBJ: u32 = 352_732;
    const COVERAGE_FIXTURE: &str =
        include_str!("../../tests/fixtures/melee-ui/back-panel-animation.json");

    #[derive(Deserialize)]
    struct Fixture {
        source: FixtureSource,
        data_ranges: Vec<ByteRange>,
    }

    #[derive(Deserialize)]
    struct FixtureSource {
        data_section_size: u32,
    }

    #[derive(Deserialize)]
    struct ByteRange {
        offset: u32,
        bytes_hex: String,
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn put_data_u32(bytes: &mut [u8], offset: u32, value: u32) {
        put_u32(bytes, offset as usize, value);
    }

    fn put_data_u16(bytes: &mut [u8], offset: u32, value: u16) {
        let start = offset as usize;
        bytes[start..start + 2].copy_from_slice(&value.to_be_bytes());
    }

    fn put_data_f32(bytes: &mut [u8], offset: u32, value: f32) {
        put_data_u32(bytes, offset, value.to_bits());
    }

    fn hex_bytes(value: &str) -> Vec<u8> {
        let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
        assert!(remainder.is_empty());
        pairs
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn one_joint_presentation(offset_space: OffsetSpace) -> Arc<BoundPresentation> {
        let mut data = vec![0_u8; 0x80];
        for field in [0x20, 0x24, 0x28] {
            put_u32(&mut data, MODEL_ROOT as usize + field, 1.0_f32.to_bits());
        }
        let mut archive = vec![0_u8; 32];
        archive.extend_from_slice(&data);
        let archive_len = archive.len() as u32;
        put_u32(&mut archive, 0, archive_len);
        put_u32(&mut archive, 4, data.len() as u32);
        let manifest = PresentationManifest {
            schema: PRESENTATION_MANIFEST_SCHEMA.into(),
            resource: ResourceSpec {
                id: RESOURCE_ID.into(),
                sha256: format!("{:x}", Sha256::digest(&archive)),
                data_section_file_offset: 32,
                data_section_size: data.len() as u32,
            },
            visual_offsets: VisualOffsetSpaces {
                joints: offset_space,
                materials: offset_space,
                textures: offset_space,
            },
            hierarchies: ["root", "other"]
                .into_iter()
                .map(|id| HierarchySpec {
                    id: id.into(),
                    model_root: MODEL_ROOT,
                    joint_animation_root: None,
                    material_animation_root: None,
                    shape_animation_root: None,
                })
                .collect(),
            clips: ["root", "other"]
                .into_iter()
                .map(|hierarchy| ClipSpec {
                    id: AnimationId::from(format!("{hierarchy}.empty")),
                    hierarchy: hierarchy.into(),
                    scope: ClipScope::Subtree { joint: MODEL_ROOT },
                })
                .collect(),
        };
        Arc::new(manifest.bind_hsd_dat(&archive).unwrap())
    }

    fn repeated_material_presentation() -> Arc<BoundPresentation> {
        let fixture: Fixture = serde_json::from_str(COVERAGE_FIXTURE).unwrap();
        let mut data = vec![0; fixture.source.data_section_size as usize];
        for range in fixture.data_ranges {
            let bytes = hex_bytes(&range.bytes_hex);
            let start = range.offset as usize;
            data[start..start + bytes.len()].copy_from_slice(&bytes);
        }

        put_data_u32(&mut data, ANIMATED_MODEL + 0x10, DOBJ);
        for field in [0x20, 0x24, 0x28] {
            put_data_f32(&mut data, ANIMATED_MODEL + field, 1.0);
        }
        put_data_u32(&mut data, MAT_ANIM_JOINT + 8, MAT_ANIM);
        put_data_u32(&mut data, MAT_ANIM, SECOND_MAT_ANIM);
        put_data_u32(&mut data, MAT_ANIM + 4, MATERIAL_AOBJ);
        put_data_u32(&mut data, MAT_ANIM + 8, TEX_ANIM);
        put_data_u32(&mut data, SECOND_MAT_ANIM + 4, MATERIAL_AOBJ);
        put_data_u32(&mut data, SECOND_MAT_ANIM + 8, TEX_ANIM);
        put_data_u32(&mut data, DOBJ + 4, SECOND_DOBJ);
        put_data_u32(&mut data, DOBJ + 8, MOBJ);
        put_data_u32(&mut data, SECOND_DOBJ + 8, MOBJ);
        put_data_u32(&mut data, MOBJ + 8, TOBJ);
        put_data_u32(&mut data, MOBJ + 0x0c, MATERIAL);
        data[MATERIAL as usize + 4..MATERIAL as usize + 7].copy_from_slice(&[10, 20, 30]);
        put_data_f32(&mut data, MATERIAL + 0x0c, 0.75);
        put_data_u32(&mut data, TOBJ + 8, 7);
        put_data_f32(&mut data, TOBJ + 0x1c, 1.0);
        put_data_f32(&mut data, TOBJ + 0x20, 1.0);
        put_data_u32(&mut data, TOBJ + 0x4c, IMAGE_0);
        put_data_u32(&mut data, TOBJ + 0x58, TEV);
        data[TEV as usize + 16..TEV as usize + 24].copy_from_slice(&[1; 8]);
        put_data_u32(&mut data, TEX_ANIM + 4, 7);
        put_data_u32(&mut data, TEX_ANIM + 8, TEXTURE_AOBJ);
        put_data_u32(&mut data, TEX_ANIM + 0x0c, IMAGE_TABLE);
        put_data_u16(&mut data, TEX_ANIM + 0x14, 3);
        put_data_u32(&mut data, IMAGE_TABLE, IMAGE_0);
        put_data_u32(&mut data, IMAGE_TABLE + 8, IMAGE_2);

        let mut archive = vec![0; 32];
        archive.extend_from_slice(&data);
        let archive_len = archive.len() as u32;
        put_u32(&mut archive, 0, archive_len);
        put_u32(&mut archive, 4, data.len() as u32);
        let manifest = PresentationManifest {
            schema: PRESENTATION_MANIFEST_SCHEMA.into(),
            resource: ResourceSpec {
                id: RESOURCE_ID.into(),
                sha256: format!("{:x}", Sha256::digest(&archive)),
                data_section_file_offset: 32,
                data_section_size: data.len() as u32,
            },
            visual_offsets: VisualOffsetSpaces {
                joints: OffsetSpace::DataSection,
                materials: OffsetSpace::File,
                textures: OffsetSpace::File,
            },
            hierarchies: vec![HierarchySpec {
                id: "animated".into(),
                model_root: ANIMATED_MODEL,
                joint_animation_root: None,
                material_animation_root: Some(MAT_ANIM_JOINT),
                shape_animation_root: None,
            }],
            clips: vec![ClipSpec {
                id: AnimationId::from("animated.full"),
                hierarchy: "animated".into(),
                scope: ClipScope::Subtree {
                    joint: ANIMATED_MODEL,
                },
            }],
        };
        Arc::new(manifest.bind_hsd_dat(&archive).unwrap())
    }

    fn resource_entry(id: &str, sha256: &str, spaces: VisualOffsetSpaces) -> serde_json::Value {
        json!({
            "id": id,
            "sha256": sha256,
            "offset_spaces": serde_json::to_value(spaces).unwrap(),
        })
    }

    fn scene_with_resources(path: &Path, resources: serde_json::Value) -> Scene {
        fs::write(
            path,
            serde_json::to_vec(&json!({
                "schema": "skirmish-visual-v1",
                "resources": resources,
                "meshes": []
            }))
            .unwrap(),
        )
        .unwrap();
        Scene::load(path).unwrap()
    }

    fn repeated_material_scene(path: &Path, hash: &str, material_offset: u32) -> Scene {
        let mesh = |name: &str, dobj_index: u16| {
            json!({
                "name": name,
                "joint": ANIMATED_MODEL,
                "resource_id": RESOURCE_ID,
                "dobj_index": dobj_index,
                "geometry_space": "world",
                "positions": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                "indices": [0, 1, 2],
                "material": {
                    "material_offset": material_offset + 32,
                    "textures": [{"tobj_offset": TOBJ + 32, "tobj_index": 0}]
                }
            })
        };
        fs::write(
            path,
            serde_json::to_vec(&json!({
                "schema": "skirmish-visual-v1",
                "resources": [resource_entry(
                    RESOURCE_ID,
                    hash,
                    VisualOffsetSpaces {
                        joints: OffsetSpace::DataSection,
                        materials: OffsetSpace::File,
                        textures: OffsetSpace::File,
                    },
                )],
                "joints": [{"name": "root", "offset": ANIMATED_MODEL}],
                "meshes": [mesh("first", 0), mesh("second", 1)]
            }))
            .unwrap(),
        )
        .unwrap();
        Scene::load(path).unwrap()
    }

    fn exact_joint_mesh(
        resource_id: VisualResourceId,
        visual_offset: u32,
    ) -> super::super::scene::Mesh {
        exact_joint_mesh_in(resource_id, visual_offset, 0, GeometrySpace::World)
    }

    fn exact_joint_mesh_in(
        resource_id: VisualResourceId,
        visual_offset: u32,
        dobj_index: u16,
        geometry_space: GeometrySpace,
    ) -> super::super::scene::Mesh {
        let mut mesh = Scene::demo().meshes.remove(0);
        mesh.joint = Some(visual_offset);
        mesh.source_occurrence = Some(VisualDObjOccurrence {
            owner_joint: VisualJointOccurrence {
                resource_id,
                visual_offset,
            },
            dobj_index,
        });
        mesh.geometry_space = geometry_space;
        mesh
    }

    fn cue(id: &str, start: f32, end: f32, loop_start: Option<f32>) -> AnimationCue {
        AnimationCue {
            id: AnimationId::from(id),
            frames: FrameRange {
                start,
                end,
                loop_start,
            },
        }
    }

    fn exact_joint_binding(
        directory: &Path,
        presentation: Arc<BoundPresentation>,
    ) -> Arc<VisualPresentationBinding> {
        let mut scene = scene_with_resources(
            &directory.join("joint.json"),
            json!([resource_entry(
                RESOURCE_ID,
                &presentation.resource().sha256,
                presentation.visual_offsets(),
            )]),
        );
        scene.meshes.push(exact_joint_mesh(
            scene.resources[0].id().clone(),
            MODEL_ROOT,
        ));
        Arc::new(VisualPresentationBinding::bind(&scene, presentation, "root").unwrap())
    }

    fn repeated_material_binding(
        directory: &Path,
        presentation: Arc<BoundPresentation>,
    ) -> Arc<VisualPresentationBinding> {
        let scene = repeated_material_scene(
            &directory.join("material.json"),
            &presentation.resource().sha256,
            MOBJ,
        );
        Arc::new(VisualPresentationBinding::bind(&scene, presentation, "animated").unwrap())
    }

    #[test]
    fn binds_only_the_exact_resource_and_normalizes_file_offsets() {
        let presentation = one_joint_presentation(OffsetSpace::File);
        let directory = tempfile::tempdir().unwrap();
        let hash = presentation.resource().sha256.clone();
        let mut scene = scene_with_resources(
            &directory.path().join("scene.json"),
            json!([
                resource_entry(RESOURCE_ID, &hash, presentation.visual_offsets()),
                resource_entry("other.dat", &"1".repeat(64), presentation.visual_offsets()),
            ]),
        );
        let target_resource = scene.resources[0].id().clone();
        let other_resource = scene.resources[1].id().clone();
        scene.meshes.push(exact_joint_mesh(
            other_resource,
            MODEL_ROOT + presentation.resource().data_section_file_offset,
        ));
        scene.meshes.push(exact_joint_mesh(
            target_resource.clone(),
            MODEL_ROOT + presentation.resource().data_section_file_offset,
        ));

        let binding =
            VisualPresentationBinding::bind(&scene, presentation.clone(), "root").unwrap();
        let source = &presentation.hierarchy("root").unwrap().joints()[0].source_id;
        assert_eq!(binding.hierarchy(), "root");
        assert_eq!(binding.presentation().resource(), presentation.resource());
        assert_eq!(
            binding.joint_occurrence(source),
            Some(&VisualJointOccurrence {
                resource_id: target_resource,
                visual_offset: MODEL_ROOT + 32,
            })
        );
    }

    #[test]
    fn binds_reused_materials_and_textures_by_full_occurrence_and_rejects_wrong_owner() {
        let presentation = repeated_material_presentation();
        let directory = tempfile::tempdir().unwrap();
        let scene = repeated_material_scene(
            &directory.path().join("exact.json"),
            &presentation.resource().sha256,
            MOBJ,
        );
        let binding =
            VisualPresentationBinding::bind(&scene, presentation.clone(), "animated").unwrap();
        let hierarchy = presentation.hierarchy("animated").unwrap();
        assert_eq!(hierarchy.materials().len(), 2);
        assert_eq!(hierarchy.textures().len(), 2);

        for material in hierarchy.materials() {
            let occurrence = binding
                .material_occurrence(&material.source_id)
                .expect("every animated MObj occurrence must bind");
            assert_eq!(occurrence.visual_offset.get(), MOBJ + 32);
            assert_eq!(
                occurrence.owner_dobj.dobj_index,
                material.identity.dobj_index.unwrap()
            );
        }
        for texture in hierarchy.textures() {
            let occurrence = binding
                .texture_occurrence(&texture.source_id)
                .expect("every animated TObj occurrence must bind");
            assert_eq!(occurrence.visual_offset.get(), TOBJ + 32);
            assert_eq!(
                occurrence.owner_material.owner_dobj.dobj_index,
                texture.identity.dobj_index.unwrap()
            );
            assert_eq!(
                occurrence.tobj_index,
                texture.identity.texture_index.unwrap()
            );
        }

        let wrong_owner = repeated_material_scene(
            &directory.path().join("wrong-owner.json"),
            &presentation.resource().sha256,
            MOBJ + 4,
        );
        assert!(matches!(
            VisualPresentationBinding::bind(&wrong_owner, presentation, "animated"),
            Err(VisualPresentationBindError::TextureOwnerMaterialMismatch {
                expected: MOBJ,
                actual,
                ..
            }) if actual == MOBJ + 4
        ));
    }

    #[test]
    fn presentation_driver_validates_and_restarts_without_partial_state() {
        let directory = tempfile::tempdir().unwrap();
        let binding = exact_joint_binding(
            directory.path(),
            one_joint_presentation(OffsetSpace::DataSection),
        );
        let mut driver = RenderedPresentationInstance::instantiate(
            binding,
            InstanceId::new(77),
            cue("root.empty", 0.0, 2.0, None),
        )
        .unwrap();
        assert_eq!(driver.scene_instance().id(), InstanceId::new(77));
        assert_eq!(driver.playback().frame(), 0.0);

        let snapshot = driver.state_snapshot();
        assert_eq!(snapshot.frame(), 0.0);
        assert!(matches!(
            snapshot.updates(),
            [
                RoutedPresentationUpdate {
                    route: PresentationUpdateRoute::JointVisibility(_),
                    ..
                },
                RoutedPresentationUpdate {
                    route: PresentationUpdateRoute::Retained(
                        RetainedPresentationReason::BakedWorldGeometry
                    ),
                    ..
                },
            ]
        ));

        assert_eq!(driver.tick().unwrap().frame(), 0.0);
        assert_eq!(driver.tick().unwrap().frame(), 1.0);
        let before_playback = driver.playback().clone();
        let before_joints = driver.scene_instance().joints().to_vec();

        assert!(matches!(
            driver.restart(cue("missing", 0.0, 2.0, None)),
            Err(PresentationDriverError::MissingClip { .. })
        ));
        assert!(matches!(
            driver.restart(cue("other.empty", 0.0, 2.0, None)),
            Err(PresentationDriverError::ClipHierarchyMismatch { .. })
        ));
        assert!(matches!(
            driver.restart(cue("root.empty", 0.5, 2.0, None)),
            Err(PresentationDriverError::NonIntegralCueRange { field: "start", .. })
        ));
        assert!(matches!(
            driver.restart(cue("root.empty", 2.0, 1.0, None)),
            Err(PresentationDriverError::Playback(_))
        ));
        assert_eq!(driver.playback(), &before_playback);
        assert_eq!(driver.scene_instance().joints(), before_joints);

        driver.restart(cue("root.empty", 0.0, 2.0, None)).unwrap();
        assert_eq!(driver.tick().unwrap().frame(), 0.0);
    }

    #[test]
    fn presentation_routes_are_lossless_and_apply_counts_stay_aligned() {
        let directory = tempfile::tempdir().unwrap();
        let presentation = repeated_material_presentation();
        let hierarchy = presentation.hierarchy("animated").unwrap();
        let joint = hierarchy.joints()[0].source_id.clone();
        let local = hierarchy.joints()[0].local.initial_runtime_local();
        let material = hierarchy.materials()[0].source_id.clone();
        let texture = hierarchy.textures()[0].source_id.clone();
        let binding = repeated_material_binding(directory.path(), presentation);
        let instance_id = InstanceId::new(91);
        let source_updates = vec![
            PresentationUpdate::JointVisibility {
                instance_id,
                source_id: joint.clone(),
                visible: false,
            },
            PresentationUpdate::JointVisibility {
                instance_id,
                source_id: SourceJointId::from("missing-joint"),
                visible: true,
            },
            PresentationUpdate::JointLocal {
                instance_id,
                source_id: joint,
                local,
            },
            PresentationUpdate::Material {
                instance_id,
                source_id: material,
                diffuse: [1, 128, 255],
                alpha: 0.25,
            },
            PresentationUpdate::Material {
                instance_id,
                source_id: SourceMaterialId::from("missing-material"),
                diffuse: [2, 3, 4],
                alpha: 0.5,
            },
            PresentationUpdate::Texture {
                instance_id,
                source_id: texture,
                current_image: None,
                translation: [1.0, 2.0],
                scale: [3.0, 4.0],
                blend: 0.75,
                konst: Some([1, 2, 3, 4]),
                tev0: Some([5, 6, 7, 8]),
            },
        ];
        let tick = RenderedPresentationTick {
            frame: 3.0,
            updates: route_updates(binding.as_ref(), source_updates.clone()),
        };

        assert_eq!(
            tick.updates()
                .iter()
                .map(RoutedPresentationUpdate::update)
                .cloned()
                .collect::<Vec<_>>(),
            source_updates
        );
        assert!(matches!(
            tick.updates()[0].route(),
            PresentationUpdateRoute::JointVisibility(_)
        ));
        assert_eq!(
            tick.updates()[1].route(),
            &PresentationUpdateRoute::Retained(RetainedPresentationReason::UnmappedJointVisibility)
        );
        assert_eq!(
            tick.updates()[2].route(),
            &PresentationUpdateRoute::Retained(RetainedPresentationReason::BakedWorldGeometry)
        );
        assert!(matches!(
            tick.updates()[3].route(),
            PresentationUpdateRoute::Material(_)
        ));
        assert_eq!(
            tick.updates()[4].route(),
            &PresentationUpdateRoute::Retained(RetainedPresentationReason::UnmappedMaterial)
        );
        assert_eq!(
            tick.updates()[5].route(),
            &PresentationUpdateRoute::Retained(RetainedPresentationReason::UnsupportedTexture)
        );

        let outcomes = tick
            .apply_with(|updates| {
                assert_eq!(updates.len(), 2);
                assert_eq!(updates[0].instance_id, instance_id);
                assert!(matches!(
                    updates[0].update,
                    DrawUpdate::Visibility {
                        target: ExportDrawSelector::Exact(_),
                        visible: false,
                    }
                ));
                assert!(matches!(
                    updates[1].update,
                    DrawUpdate::MaterialColor {
                        target: ExportMaterialSelector::Exact(_),
                        color,
                    } if color == [1.0 / 255.0, 128.0 / 255.0, 1.0, 0.25]
                ));
                Ok(vec![0, 2])
            })
            .unwrap();
        assert_eq!(
            outcomes,
            [
                PresentationApplyOutcome::MatchedDraws(0),
                PresentationApplyOutcome::Retained(
                    RetainedPresentationReason::UnmappedJointVisibility
                ),
                PresentationApplyOutcome::Retained(RetainedPresentationReason::BakedWorldGeometry),
                PresentationApplyOutcome::MatchedDraws(2),
                PresentationApplyOutcome::Retained(RetainedPresentationReason::UnmappedMaterial),
                PresentationApplyOutcome::Retained(RetainedPresentationReason::UnsupportedTexture),
            ]
        );

        let error = tick
            .apply_with(|_| Err(RuntimeDrawBatchError::new(1, anyhow::anyhow!("rejected"))))
            .unwrap_err();
        assert_eq!(error.update_index(), 3);
    }

    #[test]
    fn sampled_driver_ticks_keep_instances_isolated_and_fail_transactionally() {
        let directory = tempfile::tempdir().unwrap();
        let binding = repeated_material_binding(directory.path(), repeated_material_presentation());
        let mut first = RenderedPresentationInstance::instantiate(
            binding.clone(),
            InstanceId::new(101),
            cue("animated.full", 0.0, 4.0, None),
        )
        .unwrap();
        let mut second = RenderedPresentationInstance::instantiate(
            binding,
            InstanceId::new(102),
            cue("animated.full", 0.0, 4.0, None),
        )
        .unwrap();

        let initial = first.state_snapshot();
        assert_eq!(
            initial
                .updates()
                .iter()
                .filter(|update| matches!(update.route(), PresentationUpdateRoute::Material(_)))
                .count(),
            2
        );
        assert_eq!(
            initial
                .updates()
                .iter()
                .filter(|update| {
                    update.route()
                        == &PresentationUpdateRoute::Retained(
                            RetainedPresentationReason::UnsupportedTexture,
                        )
                })
                .count(),
            2
        );

        let first_tick = first.tick().unwrap();
        let second_tick = second.tick().unwrap();
        assert_eq!(first_tick.frame(), 0.0);
        assert_eq!(second_tick.frame(), 0.0);
        assert!(
            first_tick
                .updates()
                .iter()
                .any(|update| matches!(update.route(), PresentationUpdateRoute::Material(_)))
        );
        assert!(
            first_tick
                .updates()
                .iter()
                .all(|update| match update.update() {
                    PresentationUpdate::JointVisibility { instance_id, .. }
                    | PresentationUpdate::JointLocal { instance_id, .. }
                    | PresentationUpdate::Material { instance_id, .. }
                    | PresentationUpdate::Texture { instance_id, .. } => {
                        *instance_id == InstanceId::new(101)
                    }
                })
        );
        assert!(
            second_tick
                .updates()
                .iter()
                .all(|update| match update.update() {
                    PresentationUpdate::JointVisibility { instance_id, .. }
                    | PresentationUpdate::JointLocal { instance_id, .. }
                    | PresentationUpdate::Material { instance_id, .. }
                    | PresentationUpdate::Texture { instance_id, .. } => {
                        *instance_id == InstanceId::new(102)
                    }
                })
        );

        first.playback = AnimationPlayback::new(cue("animated.full", 0.5, 4.5, None)).unwrap();
        let before_playback = first.playback.clone();
        let before_joints = first.instance.joints().to_vec();
        let before_materials = first.instance.materials().to_vec();
        let before_textures = first.instance.textures().to_vec();
        assert!(matches!(
            first.tick(),
            Err(PresentationDriverError::Sample(
                SampleError::FractionalFrame { .. }
            ))
        ));
        assert_eq!(first.playback, before_playback);
        assert_eq!(first.instance.joints(), before_joints);
        assert_eq!(first.instance.materials(), before_materials);
        assert_eq!(first.instance.textures(), before_textures);
    }

    #[test]
    fn rejects_missing_ambiguous_wrong_and_legacy_resource_inputs() {
        let presentation = one_joint_presentation(OffsetSpace::DataSection);
        let directory = tempfile::tempdir().unwrap();
        let missing = Scene::demo();
        assert!(matches!(
            VisualPresentationBinding::bind(&missing, presentation.clone(), "root"),
            Err(VisualPresentationBindError::MissingVisualResource { .. })
        ));

        let mut wrong_hash = scene_with_resources(
            &directory.path().join("wrong.json"),
            json!([resource_entry(
                RESOURCE_ID,
                &"0".repeat(64),
                presentation.visual_offsets()
            )]),
        );
        wrong_hash.meshes.push(exact_joint_mesh(
            wrong_hash.resources[0].id().clone(),
            MODEL_ROOT,
        ));
        assert!(matches!(
            VisualPresentationBinding::bind(&wrong_hash, presentation.clone(), "root"),
            Err(VisualPresentationBindError::HashMismatch { .. })
        ));

        let mut ambiguous = scene_with_resources(
            &directory.path().join("ambiguous.json"),
            json!([resource_entry(
                RESOURCE_ID,
                &presentation.resource().sha256,
                presentation.visual_offsets()
            )]),
        );
        ambiguous.resources.push(ambiguous.resources[0].clone());
        assert!(matches!(
            VisualPresentationBinding::bind(&ambiguous, presentation.clone(), "root"),
            Err(VisualPresentationBindError::AmbiguousVisualResource { .. })
        ));

        let legacy = scene_with_resources(
            &directory.path().join("legacy.json"),
            json!([resource_entry(
                RESOURCE_ID,
                &presentation.resource().sha256,
                presentation.visual_offsets()
            )]),
        );
        assert!(matches!(
            VisualPresentationBinding::bind(&legacy, presentation.clone(), "root"),
            Err(VisualPresentationBindError::NoExactOccurrences { .. })
        ));
        assert!(matches!(
            VisualPresentationBinding::bind(&legacy, presentation.clone(), "missing"),
            Err(VisualPresentationBindError::MissingHierarchy { .. })
        ));

        let file_spaces = VisualOffsetSpaces {
            joints: OffsetSpace::File,
            materials: OffsetSpace::File,
            textures: OffsetSpace::File,
        };
        let mut mismatched = scene_with_resources(
            &directory.path().join("mismatched.json"),
            json!([resource_entry(
                RESOURCE_ID,
                &presentation.resource().sha256,
                file_spaces
            )]),
        );
        mismatched.meshes.push(exact_joint_mesh(
            mismatched.resources[0].id().clone(),
            MODEL_ROOT,
        ));
        assert!(matches!(
            VisualPresentationBinding::bind(&mismatched, presentation, "root"),
            Err(VisualPresentationBindError::OffsetSpaceMismatch { declared, expected, .. })
                if declared == file_spaces && expected.joints == OffsetSpace::DataSection
        ));
    }

    #[test]
    fn joint_local_geometry_is_retained_explicitly_until_transforms_exist() {
        let presentation = one_joint_presentation(OffsetSpace::DataSection);
        let directory = tempfile::tempdir().unwrap();
        let joint = presentation.hierarchy("root").unwrap().joints()[0]
            .source_id
            .clone();
        let local_update = |source_id: SourceJointId| PresentationUpdate::JointLocal {
            instance_id: InstanceId::new(5),
            source_id,
            local: presentation.hierarchy("root").unwrap().joints()[0]
                .local
                .initial_runtime_local(),
        };

        let mut scene = scene_with_resources(
            &directory.path().join("local.json"),
            json!([resource_entry(
                RESOURCE_ID,
                &presentation.resource().sha256,
                presentation.visual_offsets()
            )]),
        );
        let resource = scene.resources[0].id().clone();
        scene.meshes.push(exact_joint_mesh_in(
            resource.clone(),
            MODEL_ROOT,
            0,
            GeometrySpace::JointLocal,
        ));
        scene.meshes.push(exact_joint_mesh_in(
            resource.clone(),
            MODEL_ROOT,
            1,
            GeometrySpace::JointLocal,
        ));
        let binding =
            VisualPresentationBinding::bind(&scene, presentation.clone(), "root").unwrap();
        assert_eq!(
            binding.joint_geometry_space(&joint),
            Some(GeometrySpace::JointLocal)
        );
        assert_eq!(
            route_update(&binding, local_update(joint.clone())).route(),
            &PresentationUpdateRoute::Retained(RetainedPresentationReason::UnsupportedJointLocal)
        );
        assert_eq!(
            binding.joint_geometry_space(&SourceJointId::from("absent")),
            None
        );
        assert_eq!(
            route_update(&binding, local_update(SourceJointId::from("absent"))).route(),
            &PresentationUpdateRoute::Retained(RetainedPresentationReason::UnmappedJointLocal)
        );

        scene.meshes[1].geometry_space = GeometrySpace::World;
        assert!(matches!(
            VisualPresentationBinding::bind(&scene, presentation, "root"),
            Err(VisualPresentationBindError::MixedGeometrySpace {
                target: SourceTarget::Joint(source)
            }) if source == joint
        ));
    }

    #[test]
    fn malformed_visual_offset_and_conflicting_occurrences_are_explicit() {
        let presentation = one_joint_presentation(OffsetSpace::File);
        let directory = tempfile::tempdir().unwrap();
        let mut scene = scene_with_resources(
            &directory.path().join("scene.json"),
            json!([resource_entry(
                RESOURCE_ID,
                &presentation.resource().sha256,
                presentation.visual_offsets()
            )]),
        );
        scene
            .meshes
            .push(exact_joint_mesh(scene.resources[0].id().clone(), 4));
        assert!(matches!(
            VisualPresentationBinding::bind(&scene, presentation, "root"),
            Err(VisualPresentationBindError::InvalidVisualOffset {
                kind: SourceObjectKind::Joint,
                visual_offset: 4,
                ..
            })
        ));

        let target = SourceTarget::Joint("joint".into());
        let mut map = HashMap::new();
        let first = VisualJointOccurrence {
            resource_id: VisualResourceId::for_test(RESOURCE_ID),
            visual_offset: 32,
        };
        let second = VisualJointOccurrence {
            resource_id: VisualResourceId::for_test(RESOURCE_ID),
            visual_offset: 36,
        };
        insert_unique(
            &mut map,
            SourceJointId::from("joint"),
            first,
            target.clone(),
        )
        .unwrap();
        assert!(matches!(
            insert_unique(&mut map, SourceJointId::from("joint"), second, target,),
            Err(VisualPresentationBindError::ConflictingOccurrence { .. })
        ));
    }

    #[test]
    fn occurrence_identity_keeps_owner_and_ordinals_for_each_kind() {
        let presentation = one_joint_presentation(OffsetSpace::DataSection);
        let joint = VisualJointOccurrence {
            resource_id: VisualResourceId::for_test(RESOURCE_ID),
            visual_offset: MODEL_ROOT,
        };
        let dobj = VisualDObjOccurrence {
            owner_joint: joint,
            dobj_index: 7,
        };
        let material = VisualMaterialOccurrence {
            owner_dobj: dobj,
            visual_offset: super::super::scene::MaterialSourceId::new(0x64),
        };
        let texture = VisualTextureOccurrence {
            owner_material: material.clone(),
            tobj_index: 3,
            visual_offset: super::super::scene::TextureSourceId::new(0x70),
        };
        assert_eq!(
            material_identity(presentation.as_ref(), &material).unwrap(),
            SourceBindingIdentity {
                kind: SourceObjectKind::Material,
                descriptor_offset: DataOffset::new(0x64),
                owner_joint_offset: DataOffset::new(MODEL_ROOT),
                dobj_index: Some(7),
                texture_index: None,
            }
        );
        assert_eq!(
            texture_identity(presentation.as_ref(), &texture).unwrap().0,
            SourceBindingIdentity {
                kind: SourceObjectKind::Texture,
                descriptor_offset: DataOffset::new(0x70),
                owner_joint_offset: DataOffset::new(MODEL_ROOT),
                dobj_index: Some(7),
                texture_index: Some(3),
            }
        );
    }
}
