//! Versioned presentation resources and source-animation binding.
//!
//! The manifest keeps archive-specific descriptor traversal at the adapter
//! boundary. Once bound, clips address mutable presentation instances only by
//! stable source identity plus the caller-owned runtime instance identity.

mod hsd;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    animation::{
        AObjAnimation, ChannelTarget, ChannelValue, DataOffset, DecodeError, EvaluationError,
    },
    menu::AnimationId,
    presentation::instance::{
        ApplyError, InstanceDescriptor, InstanceError, InstanceId, JointDescriptor, LocalSrt,
        MaterialDescriptor, SceneInstance, SourceImageId, SourceJointId, SourceMaterialId,
        SourceTarget, SourceTextureId, TextureDescriptor,
    },
};

/// Schema understood by [`PresentationManifest::from_json_slice`].
pub const PRESENTATION_MANIFEST_SCHEMA: &str = "skirmish-presentation-v1";

/// A versioned description of native presentation resources and logical clips.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationManifest {
    pub schema: String,
    pub resource: ResourceSpec,
    pub visual_offsets: VisualOffsetSpaces,
    pub hierarchies: Vec<HierarchySpec>,
    pub clips: Vec<ClipSpec>,
}

/// Exact provenance and data-section bounds for one HSD DAT resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceSpec {
    /// Logical namespace used in stable source identities.
    pub id: String,
    /// Lowercase SHA-256 of the complete DAT file.
    pub sha256: String,
    pub data_section_file_offset: u32,
    pub data_section_size: u32,
}

impl ResourceSpec {
    /// Collision-resistant namespace for identities derived from this exact
    /// immutable resource revision.
    pub fn source_namespace(&self) -> String {
        format!("{}/sha256/{}", self.id, self.sha256)
    }
}

impl PresentationManifest {
    /// Decode and statically validate a manifest without reading its resource.
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, BindError> {
        let manifest: Self = serde_json::from_slice(bytes)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate provenance, derive exact descriptor bindings, and decode AObjs.
    pub fn bind_hsd_dat(&self, archive: &[u8]) -> Result<BoundPresentation, BindError> {
        self.validate()?;
        if archive.len() < 32 {
            return Err(BindError::TruncatedDataSection { needed: 32 });
        }
        let declared_size = u32::from_be_bytes(archive[0..4].try_into().unwrap());
        if usize::try_from(declared_size).ok() != Some(archive.len()) {
            return Err(BindError::ArchiveSizeMismatch {
                declared: declared_size,
                actual: archive.len(),
            });
        }
        let declared_data_size = u32::from_be_bytes(archive[4..8].try_into().unwrap());
        if declared_data_size != self.resource.data_section_size {
            return Err(BindError::HeaderDataSizeMismatch {
                declared: declared_data_size,
                manifest: self.resource.data_section_size,
            });
        }
        let actual_hash = format!("{:x}", Sha256::digest(archive));
        if actual_hash != self.resource.sha256 {
            return Err(BindError::HashMismatch {
                expected: self.resource.sha256.clone(),
                actual: actual_hash,
            });
        }
        let start = self.resource.data_section_file_offset as usize;
        let needed = start
            .checked_add(self.resource.data_section_size as usize)
            .ok_or(BindError::TruncatedDataSection { needed: usize::MAX })?;
        if needed > archive.len() {
            return Err(BindError::TruncatedDataSection { needed });
        }
        self.bind_data_section(&archive[start..needed])
    }

    fn validate(&self) -> Result<(), BindError> {
        if self.schema != PRESENTATION_MANIFEST_SCHEMA {
            return Err(BindError::Schema {
                expected: PRESENTATION_MANIFEST_SCHEMA,
                found: self.schema.clone(),
            });
        }
        if self.resource.id.is_empty() {
            return Err(BindError::EmptyResourceId);
        }
        if self.resource.sha256.len() != 64
            || !self
                .resource
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(BindError::InvalidSha256);
        }
        if self.resource.data_section_file_offset != 32 {
            return Err(BindError::InvalidDataSectionFileOffset(
                self.resource.data_section_file_offset,
            ));
        }
        if self.resource.data_section_size == 0 {
            return Err(BindError::EmptyDataSection);
        }

        let mut hierarchy_ids = HashSet::new();
        for hierarchy in &self.hierarchies {
            validate_id("hierarchy", &hierarchy.id)?;
            if !hierarchy_ids.insert(hierarchy.id.clone()) {
                return Err(BindError::DuplicateId {
                    kind: "hierarchy",
                    id: hierarchy.id.clone(),
                });
            }
        }
        let mut clip_ids = HashSet::new();
        for clip in &self.clips {
            validate_id("clip", clip.id.as_str())?;
            if !clip_ids.insert(clip.id.clone()) {
                return Err(BindError::DuplicateId {
                    kind: "clip",
                    id: clip.id.as_str().to_owned(),
                });
            }
            if !hierarchy_ids.contains(&clip.hierarchy) {
                return Err(BindError::MissingHierarchy {
                    clip: clip.id.as_str().to_owned(),
                    hierarchy: clip.hierarchy.clone(),
                });
            }
        }
        Ok(())
    }

    fn bind_data_section(&self, bytes: &[u8]) -> Result<BoundPresentation, BindError> {
        let view = hsd::HsdView::new(bytes);
        let source_namespace = self.resource.source_namespace();
        let mut hierarchies = HashMap::with_capacity(self.hierarchies.len());
        let mut diagnostics = Vec::new();
        for spec in &self.hierarchies {
            let derived = hsd::derive_hierarchy(&source_namespace, spec, view)?;
            diagnostics.extend(derived.diagnostics);
            hierarchies.insert(
                spec.id.clone(),
                BoundHierarchy {
                    id: spec.id.clone(),
                    model_root: DataOffset::new(spec.model_root),
                    joints: derived.joints,
                    materials: derived.materials,
                    textures: derived.textures,
                    bindings: derived.bindings,
                    parents: derived.parents,
                },
            );
        }

        let mut clips = HashMap::with_capacity(self.clips.len());
        for spec in &self.clips {
            let hierarchy = &hierarchies[&spec.hierarchy];
            let scope_joint = match spec.scope {
                ClipScope::Node { joint } | ClipScope::Subtree { joint } => joint,
            };
            if !hierarchy.contains_joint(scope_joint) {
                return Err(BindError::MissingScopeJoint {
                    clip: spec.id.as_str().to_owned(),
                    hierarchy: spec.hierarchy.clone(),
                    joint: scope_joint,
                });
            }
            let bindings = hierarchy
                .bindings
                .iter()
                .filter(|binding| {
                    hierarchy.scope_contains(spec.scope, binding.owner_joint_offset.get())
                })
                .cloned()
                .collect();
            clips.insert(
                spec.id.clone(),
                BoundClip {
                    id: spec.id.clone(),
                    hierarchy: spec.hierarchy.clone(),
                    scope: spec.scope,
                    bindings,
                },
            );
        }
        Ok(BoundPresentation {
            resource: self.resource.clone(),
            visual_offsets: self.visual_offsets,
            hierarchies,
            clips,
            diagnostics,
        })
    }
}

fn validate_id(kind: &'static str, id: &str) -> Result<(), BindError> {
    if id.is_empty() {
        Err(BindError::EmptyId { kind })
    } else {
        Ok(())
    }
}

/// Coordinate systems used by descriptor identities in a visual export.
///
/// Native HSD pointers are relative to the DAT data section. Some visual
/// exporters instead preserve absolute file positions. The manifest makes the
/// conversion explicit so a loader never guesses from a filename or magic
/// offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OffsetSpace {
    DataSection,
    File,
}

/// Per-kind coordinate systems used by a corresponding visual resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualOffsetSpaces {
    pub joints: OffsetSpace,
    pub materials: OffsetSpace,
    pub textures: OffsetSpace,
}

/// Parallel HSD model and animation hierarchies.
///
/// Every numeric value here is a data-section-relative descriptor offset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HierarchySpec {
    pub id: String,
    pub model_root: u32,
    pub joint_animation_root: Option<u32>,
    pub material_animation_root: Option<u32>,
    #[serde(default)]
    pub shape_animation_root: Option<u32>,
}

/// A stable logical animation ID mapped to a source request scope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipSpec {
    pub id: crate::menu::AnimationId,
    pub hierarchy: String,
    pub scope: ClipScope,
}

/// The two request shapes exposed by HSD JObj animation APIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClipScope {
    /// Match animation objects owned by this JObj only.
    Node { joint: u32 },
    /// Match this JObj and its structurally owned descendants.
    Subtree { joint: u32 },
}

/// Kind-discriminated native descriptor identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceObjectKind {
    Joint,
    Material,
    Texture,
}

/// Exact source occurrence receiving one animation object.
///
/// Descriptor offsets alone are not globally unique runtime identities: an
/// archive may reuse one MObj/TObj descriptor at multiple DObj occurrences.
/// `owner_joint_offset` and the list indices preserve that distinction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceBindingIdentity {
    pub kind: SourceObjectKind,
    pub descriptor_offset: DataOffset,
    pub owner_joint_offset: DataOffset,
    pub dobj_index: Option<u16>,
    pub texture_index: Option<u16>,
}

/// Authored joint representation retained by the native adapter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AuthoredJointLocal {
    Srt(LocalSrt),
    /// HSD initializes the effective runtime matrix to identity and expects the
    /// owning class/application to update it. Descriptor SRT remains authored
    /// metadata but is not the effective local transform.
    UserDefined {
        authored_srt: LocalSrt,
        envelope_matrix: Option<[f32; 12]>,
    },
}

impl AuthoredJointLocal {
    /// Produce HSD's initial effective local state without treating the
    /// descriptor's envelope matrix as a user-defined runtime transform.
    pub const fn initial_runtime_local(self) -> crate::presentation::instance::JointLocal {
        match self {
            Self::Srt(srt) => crate::presentation::instance::JointLocal::Srt(srt),
            Self::UserDefined { .. } => crate::presentation::instance::JointLocal::Matrix([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]),
        }
    }
}

/// One source joint in model-tree preorder.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundJointSource {
    pub identity: SourceBindingIdentity,
    pub source_id: SourceJointId,
    pub parent: Option<SourceJointId>,
    pub local: AuthoredJointLocal,
    /// `JOBJ_CLASSICAL_SCALE`: this joint's scale is not compensated in its
    /// descendants' local matrices.
    pub classical_scale: bool,
    pub visible: bool,
    /// False at `JOBJ_INSTANCE`, where HSD recursive flag operations stop.
    pub branch_recurses: bool,
    /// Descriptor reference resolved by HSD's global load table for an instance
    /// boundary. The target can live outside this manifest hierarchy.
    pub instance_target: Option<DataOffset>,
}

/// Initial state for one occurrence of an animated material descriptor.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundMaterialSource {
    pub identity: SourceBindingIdentity,
    pub source_id: SourceMaterialId,
    pub diffuse: [u8; 3],
    pub alpha: f32,
}

/// One image-table position, including source-authored null entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureImageSlot {
    pub image_descriptor: Option<DataOffset>,
    pub source_id: Option<SourceImageId>,
}

/// Initial state for one occurrence of an animated texture descriptor.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundTextureSource {
    pub identity: SourceBindingIdentity,
    pub source_id: SourceTextureId,
    /// Native MObj descriptor that owns this TObj occurrence.
    ///
    /// The DObj/TObj ordinals in `identity` address runtime state, while this
    /// parent descriptor lets visual adapters verify their complete nested
    /// occurrence tuple.
    pub owner_material_offset: DataOffset,
    pub initial_image_descriptor: Option<DataOffset>,
    pub current_image: Option<SourceImageId>,
    /// Ordered table from TexAnim. Null slots intentionally remain present.
    pub image_slots: Vec<TextureImageSlot>,
    pub translation: [f32; 2],
    pub scale: [f32; 2],
    pub blend: f32,
    /// Initial TEV register colors, absent when the TObj has no TEV descriptor.
    pub konst: Option<[u8; 4]>,
    pub tev0: Option<[u8; 4]>,
}

/// One decoded AObj attached to an exact source occurrence.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundAnimation {
    pub identity: SourceBindingIdentity,
    pub target: SourceTarget,
    pub owner_joint_offset: DataOffset,
    pub aobj_offset: DataOffset,
    pub animation: AObjAnimation,
}

/// Sampled values remain renderer-independent and can be applied transactionally.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundChannelValues {
    pub identity: SourceBindingIdentity,
    pub target: SourceTarget,
    pub values: Vec<ChannelValue>,
}

/// A deliberately skipped binding whose scalar channel is not modeled yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingDiagnostic {
    pub hierarchy: String,
    pub owner_joint_offset: Option<DataOffset>,
    pub descriptor_offset: DataOffset,
    pub kind: BindingDiagnosticKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingDiagnosticKind {
    UnsupportedChannel {
        target: ChannelTarget,
        channel: u8,
        fobj_offset: DataOffset,
    },
    UnsupportedFractionEncoding {
        encoding: u8,
        fobj_offset: DataOffset,
        field: &'static str,
    },
    UnsupportedOpcode {
        opcode: u8,
        fobj_offset: DataOffset,
        stream_offset: DataOffset,
    },
    ShapeAnimationNotModeled,
    /// The joint carries HSD pose flags whose matrix construction is not
    /// modeled (billboards, quaternion rotation), so its composed world
    /// matrix follows the plain Euler SRT path.
    UnmodeledJointFlags {
        flags: u32,
    },
}

/// Manifest, provenance, and HSD graph validation failures.
#[derive(Debug, Error)]
pub enum BindError {
    #[error("decode presentation manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported presentation manifest schema {found:?}; expected {expected:?}")]
    Schema {
        expected: &'static str,
        found: String,
    },
    #[error("presentation resource ID is empty")]
    EmptyResourceId,
    #[error("resource SHA-256 must be exactly 64 lowercase hexadecimal digits")]
    InvalidSha256,
    #[error("{kind} ID is empty")]
    EmptyId { kind: &'static str },
    #[error("duplicate {kind} ID {id:?}")]
    DuplicateId { kind: &'static str, id: String },
    #[error("clip {clip:?} references missing hierarchy {hierarchy:?}")]
    MissingHierarchy { clip: String, hierarchy: String },
    #[error("HSD DAT data section must begin at file byte 32, not {0}")]
    InvalidDataSectionFileOffset(u32),
    #[error("HSD DAT data section is empty")]
    EmptyDataSection,
    #[error("resource is {actual} bytes but its HSD header declares {declared} bytes")]
    ArchiveSizeMismatch { declared: u32, actual: usize },
    #[error("HSD header declares a {declared}-byte data section, manifest declares {manifest}")]
    HeaderDataSizeMismatch { declared: u32, manifest: u32 },
    #[error("resource is too short to contain its {needed}-byte declared data section")]
    TruncatedDataSection { needed: usize },
    #[error("resource SHA-256 mismatch: expected {expected}, found {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("{context} uses the null HSD data-section offset")]
    NullOffset { context: &'static str },
    #[error(
        "{context} at HSD data-section offset {offset:#x} with length {length} exceeds the {data_length}-byte data section"
    )]
    DescriptorOutOfBounds {
        context: &'static str,
        offset: u32,
        length: usize,
        data_length: usize,
    },
    #[error("{context} at HSD data-section offset {offset:#x} is not four-byte aligned")]
    MisalignedDescriptor { context: &'static str, offset: u32 },
    #[error("{kind} descriptor graph cycles at HSD data-section offset {offset:#x}")]
    DescriptorCycle { kind: &'static str, offset: u32 },
    #[error("{field} is non-finite in descriptor at HSD data-section offset {offset:#x}")]
    NonFiniteDescriptor { field: &'static str, offset: u32 },
    #[error("hierarchy {hierarchy:?} contains more than 65536 {kind} occurrences")]
    TooManyOccurrences {
        hierarchy: String,
        kind: &'static str,
    },
    #[error("clip {clip:?} scope joint {joint:#x} is outside hierarchy {hierarchy:?}")]
    MissingScopeJoint {
        clip: String,
        hierarchy: String,
        joint: u32,
    },
    #[error("decode AObj {aobj_offset:#x} for hierarchy {hierarchy:?}: {source}")]
    AnimationDecode {
        hierarchy: String,
        aobj_offset: u32,
        #[source]
        source: DecodeError,
    },
    #[error(
        "texture AObj {aobj_offset:#x} in hierarchy {hierarchy:?} animates TObj {tobj_offset:#x} images without a non-empty image table"
    )]
    TextureImageTrackWithoutTable {
        hierarchy: String,
        tobj_offset: u32,
        aobj_offset: u32,
    },
    #[error(
        "texture AObj {aobj_offset:#x} in hierarchy {hierarchy:?} animates TObj {tobj_offset:#x} TEV colors without a TEV descriptor"
    )]
    TextureColorTrackWithoutTev {
        hierarchy: String,
        tobj_offset: u32,
        aobj_offset: u32,
    },
    #[error("visual {kind:?} offset {offset:#x} is outside its declared coordinate space")]
    InvalidVisualOffset { kind: SourceObjectKind, offset: u32 },
}

#[derive(Debug, Error)]
pub enum SampleError {
    #[error("presentation clip frame must be finite")]
    NonFiniteFrame,
    #[error(
        "presentation manifest v1 samples integral HSD ticks; fractional frame bits 0x{frame_bits:08x} are unsupported"
    )]
    FractionalFrame { frame_bits: u32 },
    #[error("sample AObj {aobj_offset:#x}: {source}")]
    Evaluation {
        aobj_offset: u32,
        #[source]
        source: EvaluationError,
    },
}

#[derive(Debug, Error)]
pub enum BatchApplyError {
    #[error("clone presentation instance for transactional validation: {0}")]
    Clone(#[from] InstanceError),
    #[error("apply sampled values to {target:?}: {source}")]
    Apply {
        target: SourceTarget,
        #[source]
        source: ApplyError,
    },
}

/// One model hierarchy with all bindings derived from its parallel HSD trees.
#[derive(Clone, Debug)]
pub struct BoundHierarchy {
    id: String,
    model_root: DataOffset,
    joints: Vec<BoundJointSource>,
    materials: Vec<BoundMaterialSource>,
    textures: Vec<BoundTextureSource>,
    bindings: Vec<BoundAnimation>,
    parents: HashMap<u32, Option<u32>>,
}

impl BoundHierarchy {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn model_root(&self) -> DataOffset {
        self.model_root
    }

    pub fn joints(&self) -> &[BoundJointSource] {
        &self.joints
    }

    pub fn materials(&self) -> &[BoundMaterialSource] {
        &self.materials
    }

    pub fn textures(&self) -> &[BoundTextureSource] {
        &self.textures
    }

    pub fn bindings(&self) -> &[BoundAnimation] {
        &self.bindings
    }

    /// Resolve one exact source occurrence to its typed runtime target.
    ///
    /// Every [`SourceBindingIdentity`] field participates in equality. Callers
    /// that already have an occurrence identity should prefer this over an
    /// offset-only query so shared MObj/TObj descriptors never broadcast.
    pub fn target(&self, identity: SourceBindingIdentity) -> Option<SourceTarget> {
        match identity.kind {
            SourceObjectKind::Joint => self
                .joints
                .iter()
                .find(|source| source.identity == identity)
                .map(|source| SourceTarget::Joint(source.source_id.clone())),
            SourceObjectKind::Material => self
                .materials
                .iter()
                .find(|source| source.identity == identity)
                .map(|source| SourceTarget::Material(source.source_id.clone())),
            SourceObjectKind::Texture => self
                .textures
                .iter()
                .find(|source| source.identity == identity)
                .map(|source| SourceTarget::Texture(source.source_id.clone())),
        }
    }

    /// Copy the bound hierarchy's validated initial state into the generic
    /// renderer-independent instance description.
    pub fn instance_descriptor(&self) -> InstanceDescriptor {
        InstanceDescriptor {
            joints: self
                .joints
                .iter()
                .map(|source| JointDescriptor {
                    source_id: source.source_id.clone(),
                    parent: source.parent.clone(),
                    local: source.local.initial_runtime_local(),
                    classical_scale: source.classical_scale,
                    visible: source.visible,
                    branch_recurses: source.branch_recurses,
                })
                .collect(),
            materials: self
                .materials
                .iter()
                .map(|source| MaterialDescriptor {
                    source_id: source.source_id.clone(),
                    diffuse: source.diffuse,
                    alpha: source.alpha,
                })
                .collect(),
            textures: self
                .textures
                .iter()
                .map(|source| TextureDescriptor {
                    source_id: source.source_id.clone(),
                    image_slots: source
                        .image_slots
                        .iter()
                        .map(|slot| slot.source_id.clone())
                        .collect(),
                    current_image: source.current_image.clone(),
                    translation: source.translation,
                    scale: source.scale,
                    blend: source.blend,
                    konst: source.konst,
                    tev0: source.tev0,
                })
                .collect(),
        }
    }

    /// Construct one independently mutable runtime instance at initial state.
    pub fn instantiate(&self, id: InstanceId) -> Result<SceneInstance, InstanceError> {
        SceneInstance::new(id, self.instance_descriptor())
    }

    /// Resolve a canonical descriptor offset without discarding occurrences.
    ///
    /// More than one result is valid when a source descriptor is instantiated
    /// at multiple DObj locations. Visual adapters should use the owner/DObj/
    /// texture occurrence fields to disambiguate instead of broadcasting.
    pub fn targets_at(
        &self,
        kind: SourceObjectKind,
        descriptor_offset: DataOffset,
    ) -> Vec<(SourceBindingIdentity, SourceTarget)> {
        match kind {
            SourceObjectKind::Joint => self
                .joints
                .iter()
                .filter(|source| source.identity.descriptor_offset == descriptor_offset)
                .map(|source| {
                    (
                        source.identity,
                        SourceTarget::Joint(source.source_id.clone()),
                    )
                })
                .collect(),
            SourceObjectKind::Material => self
                .materials
                .iter()
                .filter(|source| source.identity.descriptor_offset == descriptor_offset)
                .map(|source| {
                    (
                        source.identity,
                        SourceTarget::Material(source.source_id.clone()),
                    )
                })
                .collect(),
            SourceObjectKind::Texture => self
                .textures
                .iter()
                .filter(|source| source.identity.descriptor_offset == descriptor_offset)
                .map(|source| {
                    (
                        source.identity,
                        SourceTarget::Texture(source.source_id.clone()),
                    )
                })
                .collect(),
        }
    }

    fn contains_joint(&self, offset: u32) -> bool {
        self.parents.contains_key(&offset)
    }

    fn scope_contains(&self, scope: ClipScope, owner: u32) -> bool {
        let (root, recursive) = match scope {
            ClipScope::Node { joint } => (joint, false),
            ClipScope::Subtree { joint } => (joint, true),
        };
        if !recursive {
            return owner == root;
        }
        let mut current = Some(owner);
        while let Some(offset) = current {
            if offset == root {
                return true;
            }
            current = self.parents.get(&offset).copied().flatten();
        }
        false
    }
}

/// One stable animation cue resolved to decoded source bindings.
#[derive(Clone, Debug)]
pub struct BoundClip {
    id: AnimationId,
    hierarchy: String,
    scope: ClipScope,
    bindings: Vec<BoundAnimation>,
}

impl BoundClip {
    pub fn id(&self) -> &AnimationId {
        &self.id
    }

    pub fn hierarchy(&self) -> &str {
        &self.hierarchy
    }

    pub const fn scope(&self) -> ClipScope {
        self.scope
    }

    pub fn bindings(&self) -> &[BoundAnimation] {
        &self.bindings
    }

    /// Evaluate every AObj selected by this source request scope.
    ///
    /// Manifest v1 deliberately accepts integral HSD ticks only. This keeps the
    /// byte-color domain validated by the pinned resource audit; a future
    /// continuous mode must specify how spline overshoot maps to byte channels.
    pub fn sample(&self, requested_frame: f32) -> Result<SampleBatch, SampleError> {
        if !requested_frame.is_finite() {
            return Err(SampleError::NonFiniteFrame);
        }
        if requested_frame.fract() != 0.0 {
            return Err(SampleError::FractionalFrame {
                frame_bits: requested_frame.to_bits(),
            });
        }
        let mut values = Vec::with_capacity(self.bindings.len());
        for binding in &self.bindings {
            let sampled = binding
                .animation
                .sample_requested_frame(requested_frame)
                .map_err(|source| SampleError::Evaluation {
                    aobj_offset: binding.aobj_offset.get(),
                    source,
                })?;
            if !sampled.is_empty() {
                values.push(BoundChannelValues {
                    identity: binding.identity,
                    target: binding.target.clone(),
                    values: sampled,
                });
            }
        }
        Ok(SampleBatch { values })
    }
}

/// Bound native resource ready for renderer-independent sampling.
#[derive(Clone, Debug)]
pub struct BoundPresentation {
    resource: ResourceSpec,
    visual_offsets: VisualOffsetSpaces,
    hierarchies: HashMap<String, BoundHierarchy>,
    clips: HashMap<AnimationId, BoundClip>,
    diagnostics: Vec<BindingDiagnostic>,
}

impl BoundPresentation {
    pub fn resource(&self) -> &ResourceSpec {
        &self.resource
    }

    pub fn hierarchy(&self, id: &str) -> Option<&BoundHierarchy> {
        self.hierarchies.get(id)
    }

    pub fn clip(&self, id: &AnimationId) -> Option<&BoundClip> {
        self.clips.get(id)
    }

    pub fn diagnostics(&self) -> &[BindingDiagnostic] {
        &self.diagnostics
    }

    /// Coordinate spaces the manifest expects a corresponding visual export to
    /// use for its joint, material, and texture identities.
    pub const fn visual_offsets(&self) -> VisualOffsetSpaces {
        self.visual_offsets
    }

    /// Convert a canonical data-section offset back into the corresponding
    /// visual export's declared coordinate for `kind`.
    pub fn visual_offset(&self, kind: SourceObjectKind, offset: DataOffset) -> u32 {
        let space = match kind {
            SourceObjectKind::Joint => self.visual_offsets.joints,
            SourceObjectKind::Material => self.visual_offsets.materials,
            SourceObjectKind::Texture => self.visual_offsets.textures,
        };
        match space {
            OffsetSpace::DataSection => offset.get(),
            OffsetSpace::File => offset.get() + self.resource.data_section_file_offset,
        }
    }

    /// Convert one exported ID into the canonical data-section coordinate.
    pub fn normalize_visual_offset(
        &self,
        kind: SourceObjectKind,
        offset: u32,
    ) -> Result<DataOffset, BindError> {
        let space = match kind {
            SourceObjectKind::Joint => self.visual_offsets.joints,
            SourceObjectKind::Material => self.visual_offsets.materials,
            SourceObjectKind::Texture => self.visual_offsets.textures,
        };
        let normalized = match space {
            OffsetSpace::DataSection => offset,
            OffsetSpace::File => offset
                .checked_sub(self.resource.data_section_file_offset)
                .ok_or(BindError::InvalidVisualOffset { kind, offset })?,
        };
        if normalized == 0 || normalized >= self.resource.data_section_size {
            return Err(BindError::InvalidVisualOffset { kind, offset });
        }
        Ok(DataOffset::new(normalized))
    }
}

/// A deterministic batch of sampled source callback values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SampleBatch {
    values: Vec<BoundChannelValues>,
}

impl SampleBatch {
    pub fn values(&self) -> &[BoundChannelValues] {
        &self.values
    }

    /// Apply the complete batch atomically and report renderer-facing state deltas.
    ///
    /// Byte-valued color channels are not silently clamped or dropped. Values
    /// outside the modeled normalized domain return [`BatchApplyError::Apply`]
    /// and leave `instance` unchanged. The pinned menu resource's integer-frame
    /// samples are covered by the optional full-resource audit test; another
    /// adapter that permits overshoot must first define its target conversion
    /// semantics explicitly.
    pub fn apply(
        &self,
        instance: &mut SceneInstance,
    ) -> Result<Vec<PresentationUpdate>, BatchApplyError> {
        let before = InstanceSnapshot::capture(instance);
        let staging_id = InstanceId::new(instance.id().get() ^ 1);
        let mut staging = instance.clone_as(staging_id)?;
        for sampled in &self.values {
            staging
                .apply_channels(&sampled.target, &sampled.values)
                .map_err(|source| BatchApplyError::Apply {
                    target: sampled.target.clone(),
                    source,
                })?;
        }
        *instance = staging.clone_as(instance.id())?;
        Ok(before.diff(instance))
    }
}

/// Renderer-independent mutations produced by applying one sampled batch.
#[derive(Clone, Debug, PartialEq)]
pub enum PresentationUpdate {
    JointVisibility {
        instance_id: InstanceId,
        source_id: SourceJointId,
        visible: bool,
    },
    JointLocal {
        instance_id: InstanceId,
        source_id: SourceJointId,
        local: crate::presentation::instance::JointLocal,
    },
    /// Composed HSD world matrix (row-major 3x4) after a joint or one of its
    /// ancestors changed. Renderers position joint-local draws from this.
    JointWorld {
        instance_id: InstanceId,
        source_id: SourceJointId,
        world: [[f32; 4]; 3],
    },
    Material {
        instance_id: InstanceId,
        source_id: SourceMaterialId,
        diffuse: [u8; 3],
        alpha: f32,
    },
    Texture {
        instance_id: InstanceId,
        source_id: SourceTextureId,
        current_image: Option<SourceImageId>,
        translation: [f32; 2],
        scale: [f32; 2],
        blend: f32,
        konst: Option<[u8; 4]>,
        tev0: Option<[u8; 4]>,
    },
}

#[derive(Clone, Debug)]
struct InstanceSnapshot {
    joints: Vec<(
        SourceJointId,
        bool,
        crate::presentation::instance::JointLocal,
        [[f32; 4]; 3],
    )>,
    materials: Vec<(SourceMaterialId, [u8; 3], f32)>,
    textures: Vec<TextureSnapshot>,
}

#[derive(Clone, Debug)]
struct TextureSnapshot {
    source_id: SourceTextureId,
    current_image: Option<SourceImageId>,
    translation: [f32; 2],
    scale: [f32; 2],
    blend: f32,
    konst: Option<[u8; 4]>,
    tev0: Option<[u8; 4]>,
}

impl InstanceSnapshot {
    fn capture(instance: &SceneInstance) -> Self {
        Self {
            joints: instance
                .joints()
                .iter()
                .map(|joint| {
                    (
                        joint.source_id().clone(),
                        joint.visible(),
                        joint.local(),
                        joint.world(),
                    )
                })
                .collect(),
            materials: instance
                .materials()
                .iter()
                .map(|material| {
                    (
                        material.source_id().clone(),
                        material.diffuse(),
                        material.alpha(),
                    )
                })
                .collect(),
            textures: instance
                .textures()
                .iter()
                .map(|texture| TextureSnapshot {
                    source_id: texture.source_id().clone(),
                    current_image: texture.current_image().cloned(),
                    translation: texture.translation(),
                    scale: texture.scale(),
                    blend: texture.blend(),
                    konst: texture.konst(),
                    tev0: texture.tev0(),
                })
                .collect(),
        }
    }

    fn diff(self, instance: &SceneInstance) -> Vec<PresentationUpdate> {
        let instance_id = instance.id();
        let mut updates = Vec::new();
        for ((source_id, visible, local, world), after) in
            self.joints.into_iter().zip(instance.joints())
        {
            if visible != after.visible() {
                updates.push(PresentationUpdate::JointVisibility {
                    instance_id,
                    source_id: source_id.clone(),
                    visible: after.visible(),
                });
            }
            if local != after.local() {
                updates.push(PresentationUpdate::JointLocal {
                    instance_id,
                    source_id: source_id.clone(),
                    local: after.local(),
                });
            }
            if world != after.world() {
                updates.push(PresentationUpdate::JointWorld {
                    instance_id,
                    source_id,
                    world: after.world(),
                });
            }
        }
        for ((source_id, diffuse, alpha), after) in
            self.materials.into_iter().zip(instance.materials())
        {
            if diffuse != after.diffuse() || alpha != after.alpha() {
                updates.push(PresentationUpdate::Material {
                    instance_id,
                    source_id,
                    diffuse: after.diffuse(),
                    alpha: after.alpha(),
                });
            }
        }
        for (before, after) in self.textures.into_iter().zip(instance.textures()) {
            if before.current_image.as_ref() != after.current_image()
                || before.translation != after.translation()
                || before.scale != after.scale()
                || before.blend != after.blend()
                || before.konst != after.konst()
                || before.tev0 != after.tev0()
            {
                updates.push(PresentationUpdate::Texture {
                    instance_id,
                    source_id: before.source_id,
                    current_image: after.current_image().cloned(),
                    translation: after.translation(),
                    scale: after.scale(),
                    blend: after.blend(),
                    konst: after.konst(),
                    tev0: after.tev0(),
                });
            }
        }
        updates
    }
}

fn joint_source_id(resource_namespace: &str, identity: SourceBindingIdentity) -> SourceJointId {
    debug_assert_eq!(identity.kind, SourceObjectKind::Joint);
    SourceJointId::new(format!(
        "hsd/{resource_namespace}/jobj/{:08x}",
        identity.descriptor_offset.get()
    ))
}

fn material_source_id(
    resource_namespace: &str,
    identity: SourceBindingIdentity,
) -> SourceMaterialId {
    debug_assert_eq!(identity.kind, SourceObjectKind::Material);
    SourceMaterialId::new(format!(
        "hsd/{resource_namespace}/jobj/{:08x}/dobj/{}/mobj/{:08x}",
        identity.owner_joint_offset.get(),
        identity.dobj_index.unwrap_or_default(),
        identity.descriptor_offset.get()
    ))
}

fn texture_source_id(resource_namespace: &str, identity: SourceBindingIdentity) -> SourceTextureId {
    debug_assert_eq!(identity.kind, SourceObjectKind::Texture);
    SourceTextureId::new(format!(
        "hsd/{resource_namespace}/jobj/{:08x}/dobj/{}/tobj/{}/{:08x}",
        identity.owner_joint_offset.get(),
        identity.dobj_index.unwrap_or_default(),
        identity.texture_index.unwrap_or_default(),
        identity.descriptor_offset.get()
    ))
}

fn image_source_id(resource_namespace: &str, descriptor_offset: DataOffset) -> SourceImageId {
    SourceImageId::new(format!(
        "hsd/{resource_namespace}/image/{:08x}",
        descriptor_offset.get()
    ))
}
