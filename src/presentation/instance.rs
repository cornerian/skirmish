//! Renderer-independent mutable instances of immutable presentation resources.
//!
//! Source identities are opaque names supplied by an asset adapter. Runtime
//! identities are supplied by the caller, so cloning never aliases mutable
//! state and never invents an identity from an exporter or renderer detail.

use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::Arc,
};

use thiserror::Error;

use crate::animation::{Channel, ChannelValue};

macro_rules! source_id {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Arc<str>);

        impl $name {
            pub fn new(value: impl AsRef<str>) -> Self {
                Self(Arc::from(value.as_ref()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(Arc::from(value))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

source_id!(SourceJointId);
source_id!(SourceMaterialId);
source_id!(SourceTextureId);

/// Caller-owned identity of one independently mutable runtime instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct InstanceId(u64);

impl InstanceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Local scale/rotation/translation values in the source's authored units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalSrt {
    pub scale: [f32; 3],
    pub rotation: [f32; 3],
    pub translation: [f32; 3],
}

/// A local joint pose whose interpretation remains an asset-adapter concern.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JointLocal {
    Srt(LocalSrt),
    /// Row-major matrix supplied by an adapter for a matrix-authored joint.
    Matrix([[f32; 4]; 4]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct JointDescriptor {
    pub source_id: SourceJointId,
    pub parent: Option<SourceJointId>,
    pub local: JointLocal,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialDescriptor {
    pub source_id: SourceMaterialId,
    pub diffuse: [u8; 3],
    pub alpha: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextureDescriptor {
    pub source_id: SourceTextureId,
    /// Number of ordered source images available to texture animation.
    pub image_count: usize,
    pub image_index: usize,
    pub translation: [f32; 2],
    pub blend: f32,
    pub konst_alpha: u8,
    pub tev0_alpha: u8,
}

/// Immutable source description used to construct one mutable instance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstanceDescriptor {
    pub joints: Vec<JointDescriptor>,
    pub materials: Vec<MaterialDescriptor>,
    pub textures: Vec<TextureDescriptor>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JointState {
    source_id: SourceJointId,
    parent: Option<SourceJointId>,
    local: JointLocal,
    visible: bool,
    effective_visible: bool,
}

impl JointState {
    pub fn source_id(&self) -> &SourceJointId {
        &self.source_id
    }

    pub fn parent(&self) -> Option<&SourceJointId> {
        self.parent.as_ref()
    }

    pub const fn local(&self) -> JointLocal {
        self.local
    }

    /// Source-local hidden flag, before ancestor visibility is applied.
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// Visibility after every ancestor's local hidden flag is applied.
    pub const fn effective_visible(&self) -> bool {
        self.effective_visible
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialState {
    source_id: SourceMaterialId,
    diffuse: [u8; 3],
    alpha: f32,
}

impl MaterialState {
    pub fn source_id(&self) -> &SourceMaterialId {
        &self.source_id
    }

    pub const fn diffuse(&self) -> [u8; 3] {
        self.diffuse
    }

    pub const fn alpha(&self) -> f32 {
        self.alpha
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextureState {
    source_id: SourceTextureId,
    image_count: usize,
    image_index: usize,
    translation: [f32; 2],
    blend: f32,
    konst_alpha: u8,
    tev0_alpha: u8,
}

impl TextureState {
    pub fn source_id(&self) -> &SourceTextureId {
        &self.source_id
    }

    pub const fn image_count(&self) -> usize {
        self.image_count
    }

    pub const fn image_index(&self) -> usize {
        self.image_index
    }

    pub const fn translation(&self) -> [f32; 2] {
        self.translation
    }

    pub const fn blend(&self) -> f32 {
        self.blend
    }

    pub const fn konst_alpha(&self) -> u8 {
        self.konst_alpha
    }

    pub const fn tev0_alpha(&self) -> u8 {
        self.tev0_alpha
    }
}

/// Typed target for values sampled from one source animation object.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SourceTarget {
    Joint(SourceJointId),
    Material(SourceMaterialId),
    Texture(SourceTextureId),
}

impl SourceTarget {
    const fn kind(&self) -> SourceKind {
        match self {
            Self::Joint(_) => SourceKind::Joint,
            Self::Material(_) => SourceKind::Material,
            Self::Texture(_) => SourceKind::Texture,
        }
    }
}

/// One independently mutable copy of a presentation resource hierarchy.
#[derive(Debug)]
pub struct SceneInstance {
    id: InstanceId,
    joints: Vec<JointState>,
    materials: Vec<MaterialState>,
    textures: Vec<TextureState>,
    joint_indices: HashMap<SourceJointId, usize>,
    material_indices: HashMap<SourceMaterialId, usize>,
    texture_indices: HashMap<SourceTextureId, usize>,
    children: Vec<Vec<usize>>,
    topological: Vec<usize>,
}

impl SceneInstance {
    pub fn new(id: InstanceId, descriptor: InstanceDescriptor) -> Result<Self, InstanceError> {
        let mut joint_indices = HashMap::with_capacity(descriptor.joints.len());
        for (index, joint) in descriptor.joints.iter().enumerate() {
            require_id(SourceKind::Joint, joint.source_id.as_str())?;
            validate_joint_local(&joint.source_id, joint.local)?;
            if joint_indices
                .insert(joint.source_id.clone(), index)
                .is_some()
            {
                return Err(InstanceError::DuplicateJoint(joint.source_id.clone()));
            }
        }

        let mut children = vec![Vec::new(); descriptor.joints.len()];
        let mut roots = VecDeque::new();
        for (index, joint) in descriptor.joints.iter().enumerate() {
            if let Some(parent) = &joint.parent {
                let Some(&parent_index) = joint_indices.get(parent) else {
                    return Err(InstanceError::MissingParent {
                        joint: joint.source_id.clone(),
                        parent: parent.clone(),
                    });
                };
                children[parent_index].push(index);
            } else {
                roots.push_back(index);
            }
        }
        let mut topological = Vec::with_capacity(descriptor.joints.len());
        while let Some(index) = roots.pop_front() {
            topological.push(index);
            roots.extend(children[index].iter().copied());
        }
        if topological.len() != descriptor.joints.len() {
            return Err(InstanceError::JointHierarchyCycle);
        }

        let mut material_indices = HashMap::with_capacity(descriptor.materials.len());
        for (index, material) in descriptor.materials.iter().enumerate() {
            require_id(SourceKind::Material, material.source_id.as_str())?;
            if !material.alpha.is_finite() {
                return Err(InstanceError::NonFiniteInitialValue {
                    kind: SourceKind::Material,
                    source_id: material.source_id.to_string(),
                    field: "alpha",
                });
            }
            if material_indices
                .insert(material.source_id.clone(), index)
                .is_some()
            {
                return Err(InstanceError::DuplicateMaterial(material.source_id.clone()));
            }
        }

        let mut texture_indices = HashMap::with_capacity(descriptor.textures.len());
        for (index, texture) in descriptor.textures.iter().enumerate() {
            require_id(SourceKind::Texture, texture.source_id.as_str())?;
            if texture.image_count == 0 {
                return Err(InstanceError::EmptyTextureImageTable(
                    texture.source_id.clone(),
                ));
            }
            if texture.image_index >= texture.image_count {
                return Err(InstanceError::InitialTextureImageOutOfRange {
                    texture: texture.source_id.clone(),
                    image_index: texture.image_index,
                    image_count: texture.image_count,
                });
            }
            if !texture
                .translation
                .into_iter()
                .chain([texture.blend])
                .all(f32::is_finite)
            {
                return Err(InstanceError::NonFiniteInitialValue {
                    kind: SourceKind::Texture,
                    source_id: texture.source_id.to_string(),
                    field: "translation or blend",
                });
            }
            if texture_indices
                .insert(texture.source_id.clone(), index)
                .is_some()
            {
                return Err(InstanceError::DuplicateTexture(texture.source_id.clone()));
            }
        }

        let joints = descriptor
            .joints
            .into_iter()
            .map(|joint| JointState {
                source_id: joint.source_id,
                parent: joint.parent,
                local: joint.local,
                visible: joint.visible,
                effective_visible: false,
            })
            .collect();
        let materials = descriptor
            .materials
            .into_iter()
            .map(|material| MaterialState {
                source_id: material.source_id,
                diffuse: material.diffuse,
                alpha: material.alpha,
            })
            .collect();
        let textures = descriptor
            .textures
            .into_iter()
            .map(|texture| TextureState {
                source_id: texture.source_id,
                image_count: texture.image_count,
                image_index: texture.image_index,
                translation: texture.translation,
                blend: texture.blend,
                konst_alpha: texture.konst_alpha,
                tev0_alpha: texture.tev0_alpha,
            })
            .collect();
        let mut instance = Self {
            id,
            joints,
            materials,
            textures,
            joint_indices,
            material_indices,
            texture_indices,
            children,
            topological,
        };
        instance.recompute_effective_visibility();
        Ok(instance)
    }

    pub const fn id(&self) -> InstanceId {
        self.id
    }

    /// Clone all mutable state under a new caller-supplied runtime identity.
    pub fn clone_as(&self, id: InstanceId) -> Result<Self, InstanceError> {
        if id == self.id {
            return Err(InstanceError::ReusedInstanceId(id));
        }
        Ok(Self {
            id,
            joints: self.joints.clone(),
            materials: self.materials.clone(),
            textures: self.textures.clone(),
            joint_indices: self.joint_indices.clone(),
            material_indices: self.material_indices.clone(),
            texture_indices: self.texture_indices.clone(),
            children: self.children.clone(),
            topological: self.topological.clone(),
        })
    }

    pub fn joints(&self) -> &[JointState] {
        &self.joints
    }

    pub fn materials(&self) -> &[MaterialState] {
        &self.materials
    }

    pub fn textures(&self) -> &[TextureState] {
        &self.textures
    }

    pub fn joint(&self, id: &SourceJointId) -> Option<&JointState> {
        self.joint_indices.get(id).map(|&index| &self.joints[index])
    }

    pub fn material(&self, id: &SourceMaterialId) -> Option<&MaterialState> {
        self.material_indices
            .get(id)
            .map(|&index| &self.materials[index])
    }

    pub fn texture(&self, id: &SourceTextureId) -> Option<&TextureState> {
        self.texture_indices
            .get(id)
            .map(|&index| &self.textures[index])
    }

    /// Apply all values emitted by one animation object as one transaction.
    ///
    /// Every fallible condition is checked before mutable state changes. Values
    /// remain in source order, so repeated channels retain callback ordering.
    pub fn apply_channels(
        &mut self,
        target: &SourceTarget,
        values: &[ChannelValue],
    ) -> Result<(), ApplyError> {
        self.validate_channels(target, values)?;
        match target {
            SourceTarget::Joint(id) => {
                let index = self.joint_indices[id];
                for &value in values {
                    match value.channel {
                        Channel::JointRotationX => self.srt_mut(index).rotation[0] = value.value,
                        Channel::JointRotationY => self.srt_mut(index).rotation[1] = value.value,
                        Channel::JointRotationZ => self.srt_mut(index).rotation[2] = value.value,
                        Channel::JointTranslationX => {
                            self.srt_mut(index).translation[0] = value.value;
                        }
                        Channel::JointTranslationY => {
                            self.srt_mut(index).translation[1] = value.value;
                        }
                        Channel::JointTranslationZ => {
                            self.srt_mut(index).translation[2] = value.value;
                        }
                        Channel::JointScaleX => {
                            self.srt_mut(index).scale[0] = source_scale(value.value);
                        }
                        Channel::JointScaleY => {
                            self.srt_mut(index).scale[1] = source_scale(value.value);
                        }
                        Channel::JointScaleZ => {
                            self.srt_mut(index).scale[2] = source_scale(value.value);
                        }
                        Channel::JointBranchVisibility => {
                            self.set_branch_visibility(index, value.value > 0.5);
                        }
                        _ => unreachable!("channel family validated before mutation"),
                    }
                }
                self.recompute_effective_visibility();
            }
            SourceTarget::Material(id) => {
                let material = &mut self.materials[self.material_indices[id]];
                for &value in values {
                    match value.channel {
                        Channel::MaterialDiffuseR => material.diffuse[0] = quantize(value.value),
                        Channel::MaterialDiffuseG => material.diffuse[1] = quantize(value.value),
                        Channel::MaterialDiffuseB => material.diffuse[2] = quantize(value.value),
                        Channel::MaterialAlpha => material.alpha = 1.0 - value.value,
                        _ => unreachable!("channel family validated before mutation"),
                    }
                }
            }
            SourceTarget::Texture(id) => {
                let texture = &mut self.textures[self.texture_indices[id]];
                for &value in values {
                    match value.channel {
                        Channel::TextureImage => {
                            texture.image_index = image_index(value.value, texture.image_count)
                                .expect("image sample validated before mutation");
                        }
                        Channel::TextureTranslationU => texture.translation[0] = value.value,
                        Channel::TextureTranslationV => texture.translation[1] = value.value,
                        Channel::TextureBlend => texture.blend = value.value,
                        Channel::TextureKonstAlpha => texture.konst_alpha = quantize(value.value),
                        Channel::TextureTev0Alpha => texture.tev0_alpha = quantize(value.value),
                        _ => unreachable!("channel family validated before mutation"),
                    }
                }
            }
        }
        Ok(())
    }

    pub fn apply_channel(
        &mut self,
        target: &SourceTarget,
        value: ChannelValue,
    ) -> Result<(), ApplyError> {
        self.apply_channels(target, &[value])
    }

    fn validate_channels(
        &self,
        target: &SourceTarget,
        values: &[ChannelValue],
    ) -> Result<(), ApplyError> {
        for value in values {
            let expected = channel_kind(value.channel);
            if expected != target.kind() {
                return Err(ApplyError::TargetKindMismatch {
                    channel: value.channel,
                    expected,
                    actual: target.kind(),
                });
            }
            if !value.value.is_finite() {
                return Err(ApplyError::NonFiniteSample(value.channel));
            }
        }

        match target {
            SourceTarget::Joint(id) => {
                let Some(&index) = self.joint_indices.get(id) else {
                    return Err(ApplyError::MissingJoint(id.clone()));
                };
                if matches!(self.joints[index].local, JointLocal::Matrix(_))
                    && values.iter().any(|value| is_srt_channel(value.channel))
                {
                    return Err(ApplyError::MatrixJointCannotApplySrt(id.clone()));
                }
            }
            SourceTarget::Material(id) => {
                if !self.material_indices.contains_key(id) {
                    return Err(ApplyError::MissingMaterial(id.clone()));
                }
                for value in values {
                    if matches!(
                        value.channel,
                        Channel::MaterialDiffuseR
                            | Channel::MaterialDiffuseG
                            | Channel::MaterialDiffuseB
                    ) {
                        validate_normalized(*value)?;
                    }
                }
            }
            SourceTarget::Texture(id) => {
                let Some(&index) = self.texture_indices.get(id) else {
                    return Err(ApplyError::MissingTexture(id.clone()));
                };
                for value in values {
                    match value.channel {
                        Channel::TextureImage => {
                            image_index(value.value, self.textures[index].image_count).map_err(
                                |reason| ApplyError::InvalidTextureImage {
                                    texture: id.clone(),
                                    value_bits: value.value.to_bits(),
                                    image_count: self.textures[index].image_count,
                                    reason,
                                },
                            )?;
                        }
                        Channel::TextureKonstAlpha | Channel::TextureTev0Alpha => {
                            validate_normalized(*value)?;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn srt_mut(&mut self, index: usize) -> &mut LocalSrt {
        match &mut self.joints[index].local {
            JointLocal::Srt(srt) => srt,
            JointLocal::Matrix(_) => unreachable!("matrix-backed SRT update was validated"),
        }
    }

    fn set_branch_visibility(&mut self, root: usize, visible: bool) {
        let mut stack = vec![root];
        while let Some(index) = stack.pop() {
            self.joints[index].visible = visible;
            stack.extend(self.children[index].iter().copied());
        }
    }

    fn recompute_effective_visibility(&mut self) {
        for &index in &self.topological {
            let ancestor_visible = self.joints[index]
                .parent
                .as_ref()
                .map(|parent| self.joints[self.joint_indices[parent]].effective_visible)
                .unwrap_or(true);
            self.joints[index].effective_visible = ancestor_visible && self.joints[index].visible;
        }
    }
}

fn require_id(kind: SourceKind, value: &str) -> Result<(), InstanceError> {
    if value.is_empty() {
        Err(InstanceError::EmptySourceId(kind))
    } else {
        Ok(())
    }
}

fn validate_joint_local(id: &SourceJointId, local: JointLocal) -> Result<(), InstanceError> {
    let finite = match local {
        JointLocal::Srt(srt) => srt
            .scale
            .into_iter()
            .chain(srt.rotation)
            .chain(srt.translation)
            .all(f32::is_finite),
        JointLocal::Matrix(matrix) => matrix.into_iter().flatten().all(f32::is_finite),
    };
    if finite {
        Ok(())
    } else {
        Err(InstanceError::NonFiniteInitialValue {
            kind: SourceKind::Joint,
            source_id: id.to_string(),
            field: "local pose",
        })
    }
}

fn source_scale(value: f32) -> f32 {
    if value.abs() < 1.0e-3 { 1.0e-3 } else { value }
}

fn quantize(value: f32) -> u8 {
    (255.0 * value) as u8
}

fn validate_normalized(value: ChannelValue) -> Result<(), ApplyError> {
    if (0.0..=1.0).contains(&value.value) {
        Ok(())
    } else {
        Err(ApplyError::ColorSampleOutOfRange {
            channel: value.channel,
            value_bits: value.value.to_bits(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageIndexError {
    Negative,
    ExceedsSourceInteger,
    OutOfRange,
}

fn image_index(value: f32, image_count: usize) -> Result<usize, ImageIndexError> {
    let truncated = value.trunc();
    if truncated < 0.0 {
        return Err(ImageIndexError::Negative);
    }
    // HSD casts to a signed 32-bit `int` before indexing its pointer table.
    if truncated >= 2_147_483_648.0 {
        return Err(ImageIndexError::ExceedsSourceInteger);
    }
    let index = truncated as usize;
    if index < image_count {
        Ok(index)
    } else {
        Err(ImageIndexError::OutOfRange)
    }
}

fn channel_kind(channel: Channel) -> SourceKind {
    match channel {
        Channel::JointRotationX
        | Channel::JointRotationY
        | Channel::JointRotationZ
        | Channel::JointTranslationX
        | Channel::JointTranslationY
        | Channel::JointTranslationZ
        | Channel::JointScaleX
        | Channel::JointScaleY
        | Channel::JointScaleZ
        | Channel::JointBranchVisibility => SourceKind::Joint,
        Channel::MaterialDiffuseR
        | Channel::MaterialDiffuseG
        | Channel::MaterialDiffuseB
        | Channel::MaterialAlpha => SourceKind::Material,
        Channel::TextureImage
        | Channel::TextureTranslationU
        | Channel::TextureTranslationV
        | Channel::TextureBlend
        | Channel::TextureKonstAlpha
        | Channel::TextureTev0Alpha => SourceKind::Texture,
    }
}

fn is_srt_channel(channel: Channel) -> bool {
    matches!(
        channel,
        Channel::JointRotationX
            | Channel::JointRotationY
            | Channel::JointRotationZ
            | Channel::JointTranslationX
            | Channel::JointTranslationY
            | Channel::JointTranslationZ
            | Channel::JointScaleX
            | Channel::JointScaleY
            | Channel::JointScaleZ
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Joint,
    Material,
    Texture,
}

impl fmt::Display for SourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Joint => "joint",
            Self::Material => "material",
            Self::Texture => "texture",
        })
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum InstanceError {
    #[error("{0} source identity must not be empty")]
    EmptySourceId(SourceKind),
    #[error("duplicate source joint {0}")]
    DuplicateJoint(SourceJointId),
    #[error("duplicate source material {0}")]
    DuplicateMaterial(SourceMaterialId),
    #[error("duplicate source texture {0}")]
    DuplicateTexture(SourceTextureId),
    #[error("joint {joint} references missing parent {parent}")]
    MissingParent {
        joint: SourceJointId,
        parent: SourceJointId,
    },
    #[error("source joint hierarchy contains a cycle")]
    JointHierarchyCycle,
    #[error("{kind} {source_id} has a non-finite initial {field}")]
    NonFiniteInitialValue {
        kind: SourceKind,
        source_id: String,
        field: &'static str,
    },
    #[error("texture {0} has an empty image table")]
    EmptyTextureImageTable(SourceTextureId),
    #[error(
        "texture {texture} initial image {image_index} is outside its {image_count}-image table"
    )]
    InitialTextureImageOutOfRange {
        texture: SourceTextureId,
        image_index: usize,
        image_count: usize,
    },
    #[error("runtime instance identity {} is already in use by this instance", .0.get())]
    ReusedInstanceId(InstanceId),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ApplyError {
    #[error("{channel:?} targets {expected}, not {actual}")]
    TargetKindMismatch {
        channel: Channel,
        expected: SourceKind,
        actual: SourceKind,
    },
    #[error("{0:?} produced a non-finite sample")]
    NonFiniteSample(Channel),
    #[error("missing source joint {0}")]
    MissingJoint(SourceJointId),
    #[error("missing source material {0}")]
    MissingMaterial(SourceMaterialId),
    #[error("missing source texture {0}")]
    MissingTexture(SourceTextureId),
    #[error("matrix-backed joint {0} cannot consume SRT animation channels")]
    MatrixJointCannotApplySrt(SourceJointId),
    #[error("{channel:?} sample bits 0x{value_bits:08x} are outside 0..=1")]
    ColorSampleOutOfRange { channel: Channel, value_bits: u32 },
    #[error(
        "texture {texture} image sample bits 0x{value_bits:08x} are invalid for its \
         {image_count}-image table: {reason:?}"
    )]
    InvalidTextureImage {
        texture: SourceTextureId,
        value_bits: u32,
        image_count: usize,
        reason: ImageIndexError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srt() -> LocalSrt {
        LocalSrt {
            scale: [1.0; 3],
            rotation: [0.0; 3],
            translation: [0.0; 3],
        }
    }

    fn joint(id: &str, parent: Option<&str>, visible: bool) -> JointDescriptor {
        JointDescriptor {
            source_id: id.into(),
            parent: parent.map(SourceJointId::from),
            local: JointLocal::Srt(srt()),
            visible,
        }
    }

    fn material(id: &str) -> MaterialDescriptor {
        MaterialDescriptor {
            source_id: id.into(),
            diffuse: [10, 20, 30],
            alpha: 0.75,
        }
    }

    fn texture(id: &str) -> TextureDescriptor {
        TextureDescriptor {
            source_id: id.into(),
            image_count: 4,
            image_index: 1,
            translation: [0.0, 0.0],
            blend: 1.0,
            konst_alpha: 255,
            tev0_alpha: 255,
        }
    }

    fn descriptor() -> InstanceDescriptor {
        InstanceDescriptor {
            joints: vec![
                joint("root", None, true),
                joint("child", Some("root"), true),
            ],
            materials: vec![material("material")],
            textures: vec![texture("texture")],
        }
    }

    fn instance() -> SceneInstance {
        SceneInstance::new(InstanceId::new(7), descriptor()).unwrap()
    }

    const fn sample(channel: Channel, value: f32) -> ChannelValue {
        ChannelValue { channel, value }
    }

    #[test]
    fn rejects_duplicate_identity_in_each_source_namespace() {
        let duplicate_joint = InstanceDescriptor {
            joints: vec![joint("same", None, true), joint("same", None, true)],
            ..InstanceDescriptor::default()
        };
        assert_eq!(
            SceneInstance::new(InstanceId::new(1), duplicate_joint).unwrap_err(),
            InstanceError::DuplicateJoint("same".into())
        );

        let duplicate_material = InstanceDescriptor {
            materials: vec![material("same"), material("same")],
            ..InstanceDescriptor::default()
        };
        assert_eq!(
            SceneInstance::new(InstanceId::new(1), duplicate_material).unwrap_err(),
            InstanceError::DuplicateMaterial("same".into())
        );

        let duplicate_texture = InstanceDescriptor {
            textures: vec![texture("same"), texture("same")],
            ..InstanceDescriptor::default()
        };
        assert_eq!(
            SceneInstance::new(InstanceId::new(1), duplicate_texture).unwrap_err(),
            InstanceError::DuplicateTexture("same".into())
        );
    }

    #[test]
    fn rejects_empty_source_identities() {
        let empty_joint = InstanceDescriptor {
            joints: vec![joint("", None, true)],
            ..InstanceDescriptor::default()
        };
        assert_eq!(
            SceneInstance::new(InstanceId::new(1), empty_joint).unwrap_err(),
            InstanceError::EmptySourceId(SourceKind::Joint)
        );

        let empty_material = InstanceDescriptor {
            materials: vec![material("")],
            ..InstanceDescriptor::default()
        };
        assert_eq!(
            SceneInstance::new(InstanceId::new(1), empty_material).unwrap_err(),
            InstanceError::EmptySourceId(SourceKind::Material)
        );

        let empty_texture = InstanceDescriptor {
            textures: vec![texture("")],
            ..InstanceDescriptor::default()
        };
        assert_eq!(
            SceneInstance::new(InstanceId::new(1), empty_texture).unwrap_err(),
            InstanceError::EmptySourceId(SourceKind::Texture)
        );
    }

    #[test]
    fn rejects_missing_parents_and_joint_cycles() {
        let missing_parent = InstanceDescriptor {
            joints: vec![joint("child", Some("absent"), true)],
            ..InstanceDescriptor::default()
        };
        assert_eq!(
            SceneInstance::new(InstanceId::new(1), missing_parent).unwrap_err(),
            InstanceError::MissingParent {
                joint: "child".into(),
                parent: "absent".into(),
            }
        );

        let cycle = InstanceDescriptor {
            joints: vec![
                joint("first", Some("second"), true),
                joint("second", Some("first"), true),
            ],
            ..InstanceDescriptor::default()
        };
        assert_eq!(
            SceneInstance::new(InstanceId::new(1), cycle).unwrap_err(),
            InstanceError::JointHierarchyCycle
        );
    }

    #[test]
    fn rejects_non_finite_initial_state() {
        let mut bad_srt = joint("joint", None, true);
        bad_srt.local = JointLocal::Srt(LocalSrt {
            rotation: [f32::NAN, 0.0, 0.0],
            ..srt()
        });
        let bad_joint = InstanceDescriptor {
            joints: vec![bad_srt],
            ..InstanceDescriptor::default()
        };
        assert!(matches!(
            SceneInstance::new(InstanceId::new(1), bad_joint),
            Err(InstanceError::NonFiniteInitialValue {
                kind: SourceKind::Joint,
                ..
            })
        ));

        let mut matrix = [[0.0; 4]; 4];
        matrix[2][1] = f32::INFINITY;
        let bad_matrix = InstanceDescriptor {
            joints: vec![JointDescriptor {
                source_id: "matrix".into(),
                parent: None,
                local: JointLocal::Matrix(matrix),
                visible: true,
            }],
            ..InstanceDescriptor::default()
        };
        assert!(matches!(
            SceneInstance::new(InstanceId::new(1), bad_matrix),
            Err(InstanceError::NonFiniteInitialValue {
                kind: SourceKind::Joint,
                ..
            })
        ));

        let mut bad_material = material("material");
        bad_material.alpha = f32::NEG_INFINITY;
        let bad_material = InstanceDescriptor {
            materials: vec![bad_material],
            ..InstanceDescriptor::default()
        };
        assert!(matches!(
            SceneInstance::new(InstanceId::new(1), bad_material),
            Err(InstanceError::NonFiniteInitialValue {
                kind: SourceKind::Material,
                ..
            })
        ));

        for (translation, blend) in [([f32::NAN, 0.0], 1.0), ([0.0, 0.0], f32::INFINITY)] {
            let mut bad_texture = texture("texture");
            bad_texture.translation = translation;
            bad_texture.blend = blend;
            let descriptor = InstanceDescriptor {
                textures: vec![bad_texture],
                ..InstanceDescriptor::default()
            };
            assert!(matches!(
                SceneInstance::new(InstanceId::new(1), descriptor),
                Err(InstanceError::NonFiniteInitialValue {
                    kind: SourceKind::Texture,
                    ..
                })
            ));
        }
    }

    #[test]
    fn validates_texture_image_tables() {
        let mut empty = texture("empty");
        empty.image_count = 0;
        empty.image_index = 0;
        let descriptor = InstanceDescriptor {
            textures: vec![empty],
            ..InstanceDescriptor::default()
        };
        assert_eq!(
            SceneInstance::new(InstanceId::new(1), descriptor).unwrap_err(),
            InstanceError::EmptyTextureImageTable("empty".into())
        );

        let mut out_of_range = texture("short");
        out_of_range.image_count = 2;
        out_of_range.image_index = 2;
        let descriptor = InstanceDescriptor {
            textures: vec![out_of_range],
            ..InstanceDescriptor::default()
        };
        assert_eq!(
            SceneInstance::new(InstanceId::new(1), descriptor).unwrap_err(),
            InstanceError::InitialTextureImageOutOfRange {
                texture: "short".into(),
                image_index: 2,
                image_count: 2,
            }
        );
    }

    #[test]
    fn applies_every_joint_srt_channel_and_source_scale_floor() {
        let mut instance = instance();
        instance
            .apply_channels(
                &SourceTarget::Joint("root".into()),
                &[
                    sample(Channel::JointRotationX, 1.0),
                    sample(Channel::JointRotationY, 2.0),
                    sample(Channel::JointRotationZ, 3.0),
                    sample(Channel::JointTranslationX, 4.0),
                    sample(Channel::JointTranslationY, 5.0),
                    sample(Channel::JointTranslationZ, 6.0),
                    sample(Channel::JointScaleX, -0.000_9),
                    sample(Channel::JointScaleY, 0.000_9),
                    sample(Channel::JointScaleZ, -0.001),
                ],
            )
            .unwrap();

        assert_eq!(
            instance.joint(&"root".into()).unwrap().local(),
            JointLocal::Srt(LocalSrt {
                scale: [0.001, 0.001, -0.001],
                rotation: [1.0, 2.0, 3.0],
                translation: [4.0, 5.0, 6.0],
            })
        );
    }

    #[test]
    fn branch_visibility_updates_descendants_and_effective_visibility() {
        let descriptor = InstanceDescriptor {
            joints: vec![
                joint("root", None, true),
                joint("branch", Some("root"), true),
                joint("leaf", Some("branch"), false),
                joint("unrelated", Some("root"), true),
            ],
            ..InstanceDescriptor::default()
        };
        let mut instance = SceneInstance::new(InstanceId::new(1), descriptor).unwrap();

        assert!(!instance.joint(&"leaf".into()).unwrap().effective_visible());
        instance
            .apply_channel(
                &SourceTarget::Joint("branch".into()),
                sample(Channel::JointBranchVisibility, 0.5),
            )
            .unwrap();
        assert!(!instance.joint(&"branch".into()).unwrap().visible());
        assert!(!instance.joint(&"leaf".into()).unwrap().visible());
        assert!(
            instance
                .joint(&"unrelated".into())
                .unwrap()
                .effective_visible()
        );

        instance
            .apply_channel(
                &SourceTarget::Joint("leaf".into()),
                sample(Channel::JointBranchVisibility, 1.0),
            )
            .unwrap();
        assert!(instance.joint(&"leaf".into()).unwrap().visible());
        assert!(!instance.joint(&"leaf".into()).unwrap().effective_visible());

        instance
            .apply_channel(
                &SourceTarget::Joint("branch".into()),
                sample(Channel::JointBranchVisibility, 0.500_1),
            )
            .unwrap();
        assert!(
            instance
                .joint(&"branch".into())
                .unwrap()
                .effective_visible()
        );
        assert!(instance.joint(&"leaf".into()).unwrap().effective_visible());
    }

    #[test]
    fn applies_material_quantization_and_alpha_inversion() {
        let mut instance = instance();
        instance
            .apply_channels(
                &SourceTarget::Material("material".into()),
                &[
                    sample(Channel::MaterialDiffuseR, 0.5),
                    sample(Channel::MaterialDiffuseG, 0.75),
                    sample(Channel::MaterialDiffuseB, 1.0),
                    sample(Channel::MaterialAlpha, 0.25),
                ],
            )
            .unwrap();

        let state = instance.material(&"material".into()).unwrap();
        assert_eq!(state.diffuse(), [127, 191, 255]);
        assert_eq!(state.alpha(), 0.75);
    }

    #[test]
    fn applies_every_texture_channel_and_truncates_image_index() {
        let mut instance = instance();
        instance
            .apply_channels(
                &SourceTarget::Texture("texture".into()),
                &[
                    sample(Channel::TextureImage, 2.9),
                    sample(Channel::TextureTranslationU, -0.5),
                    sample(Channel::TextureTranslationV, 1.5),
                    sample(Channel::TextureBlend, 0.25),
                    sample(Channel::TextureKonstAlpha, 0.5),
                    sample(Channel::TextureTev0Alpha, 0.75),
                ],
            )
            .unwrap();

        let state = instance.texture(&"texture".into()).unwrap();
        assert_eq!(state.image_index(), 2);
        assert_eq!(state.translation(), [-0.5, 1.5]);
        assert_eq!(state.blend(), 0.25);
        assert_eq!(state.konst_alpha(), 127);
        assert_eq!(state.tev0_alpha(), 191);
    }

    #[test]
    fn reports_target_mismatches_and_each_missing_identity() {
        let mut instance = instance();
        assert_eq!(
            instance
                .apply_channel(
                    &SourceTarget::Material("material".into()),
                    sample(Channel::JointRotationX, 1.0),
                )
                .unwrap_err(),
            ApplyError::TargetKindMismatch {
                channel: Channel::JointRotationX,
                expected: SourceKind::Joint,
                actual: SourceKind::Material,
            }
        );
        assert_eq!(
            instance
                .apply_channel(
                    &SourceTarget::Joint("missing".into()),
                    sample(Channel::JointRotationX, 1.0),
                )
                .unwrap_err(),
            ApplyError::MissingJoint("missing".into())
        );
        assert_eq!(
            instance
                .apply_channel(
                    &SourceTarget::Material("missing".into()),
                    sample(Channel::MaterialAlpha, 1.0),
                )
                .unwrap_err(),
            ApplyError::MissingMaterial("missing".into())
        );
        assert_eq!(
            instance
                .apply_channel(
                    &SourceTarget::Texture("missing".into()),
                    sample(Channel::TextureBlend, 1.0),
                )
                .unwrap_err(),
            ApplyError::MissingTexture("missing".into())
        );
    }

    #[test]
    fn rejected_batches_leave_all_mutable_state_unchanged() {
        let mut instance = instance();

        let before = instance.joint(&"root".into()).unwrap().clone();
        assert!(matches!(
            instance.apply_channels(
                &SourceTarget::Joint("root".into()),
                &[
                    sample(Channel::JointTranslationX, 99.0),
                    sample(Channel::JointTranslationY, f32::NAN),
                ],
            ),
            Err(ApplyError::NonFiniteSample(Channel::JointTranslationY))
        ));
        assert_eq!(instance.joint(&"root".into()).unwrap(), &before);

        let before = instance.material(&"material".into()).unwrap().clone();
        assert!(matches!(
            instance.apply_channels(
                &SourceTarget::Material("material".into()),
                &[
                    sample(Channel::MaterialAlpha, 0.5),
                    sample(Channel::MaterialDiffuseR, 1.01),
                ],
            ),
            Err(ApplyError::ColorSampleOutOfRange {
                channel: Channel::MaterialDiffuseR,
                ..
            })
        ));
        assert_eq!(instance.material(&"material".into()).unwrap(), &before);

        let before = instance.texture(&"texture".into()).unwrap().clone();
        assert!(matches!(
            instance.apply_channels(
                &SourceTarget::Texture("texture".into()),
                &[
                    sample(Channel::TextureTranslationU, 9.0),
                    sample(Channel::TextureImage, 4.0),
                ],
            ),
            Err(ApplyError::InvalidTextureImage {
                reason: ImageIndexError::OutOfRange,
                ..
            })
        ));
        assert_eq!(instance.texture(&"texture".into()).unwrap(), &before);
    }

    #[test]
    fn matrix_joint_rejects_srt_transactionally_but_accepts_visibility() {
        let matrix = [
            [1.0, 0.0, 0.0, 4.0],
            [0.0, 1.0, 0.0, 5.0],
            [0.0, 0.0, 1.0, 6.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let descriptor = InstanceDescriptor {
            joints: vec![JointDescriptor {
                source_id: "matrix".into(),
                parent: None,
                local: JointLocal::Matrix(matrix),
                visible: true,
            }],
            ..InstanceDescriptor::default()
        };
        let mut instance = SceneInstance::new(InstanceId::new(1), descriptor).unwrap();

        assert_eq!(
            instance
                .apply_channels(
                    &SourceTarget::Joint("matrix".into()),
                    &[
                        sample(Channel::JointBranchVisibility, 0.0),
                        sample(Channel::JointRotationX, 2.0),
                    ],
                )
                .unwrap_err(),
            ApplyError::MatrixJointCannotApplySrt("matrix".into())
        );
        let state = instance.joint(&"matrix".into()).unwrap();
        assert_eq!(state.local(), JointLocal::Matrix(matrix));
        assert!(state.visible());

        instance
            .apply_channel(
                &SourceTarget::Joint("matrix".into()),
                sample(Channel::JointBranchVisibility, 0.0),
            )
            .unwrap();
        assert!(
            !instance
                .joint(&"matrix".into())
                .unwrap()
                .effective_visible()
        );
    }

    #[test]
    fn clone_has_a_distinct_runtime_identity_and_independent_mutable_state() {
        let original = instance();
        assert_eq!(
            original.clone_as(original.id()).unwrap_err(),
            InstanceError::ReusedInstanceId(InstanceId::new(7))
        );

        let mut cloned = original.clone_as(InstanceId::new(8)).unwrap();
        assert_eq!(original.id().get(), 7);
        assert_eq!(cloned.id().get(), 8);
        assert!(Arc::ptr_eq(
            &original.joints[0].source_id.0,
            &cloned.joints[0].source_id.0
        ));

        cloned
            .apply_channel(
                &SourceTarget::Joint("root".into()),
                sample(Channel::JointTranslationX, 42.0),
            )
            .unwrap();
        assert_ne!(
            original.joint(&"root".into()).unwrap().local(),
            cloned.joint(&"root".into()).unwrap().local()
        );
    }
}
