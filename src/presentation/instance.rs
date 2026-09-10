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
source_id!(SourceImageId);

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
    /// Whether a recursive branch update continues into this joint's children.
    /// Instance-boundary nodes set this to false.
    pub branch_recurses: bool,
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
    /// Immutable ordered source-image table. Null HSD slots remain `None`.
    pub image_slots: Vec<Option<SourceImageId>>,
    /// Image currently resolved on the TObj. It need not occur in the animation
    /// table because an authored initial image may be replaced only later.
    pub current_image: Option<SourceImageId>,
    pub translation: [f32; 2],
    pub scale: [f32; 2],
    pub blend: f32,
    pub konst: Option<[u8; 4]>,
    pub tev0: Option<[u8; 4]>,
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
    branch_recurses: bool,
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

    /// Whether this JObj's own draw objects are visible.
    ///
    /// HSD traverses ordinary children even when their parent has
    /// `JOBJ_HIDDEN`. Branch animation performs recursion by writing the flag
    /// to every descendant, rather than by inheriting an ancestor's state.
    pub const fn visible(&self) -> bool {
        self.visible
    }

    pub const fn branch_recurses(&self) -> bool {
        self.branch_recurses
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
    image_slots: Vec<Option<SourceImageId>>,
    current_image: Option<SourceImageId>,
    translation: [f32; 2],
    scale: [f32; 2],
    blend: f32,
    konst: Option<[u8; 4]>,
    tev0: Option<[u8; 4]>,
}

impl TextureState {
    pub fn source_id(&self) -> &SourceTextureId {
        &self.source_id
    }

    pub fn image_slots(&self) -> &[Option<SourceImageId>] {
        &self.image_slots
    }

    pub fn current_image(&self) -> Option<&SourceImageId> {
        self.current_image.as_ref()
    }

    pub const fn translation(&self) -> [f32; 2] {
        self.translation
    }

    pub const fn scale(&self) -> [f32; 2] {
        self.scale
    }

    pub const fn blend(&self) -> f32 {
        self.blend
    }

    pub const fn konst(&self) -> Option<[u8; 4]> {
        self.konst
    }

    pub const fn tev0(&self) -> Option<[u8; 4]> {
        self.tev0
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
        let mut visited = 0;
        while let Some(index) = roots.pop_front() {
            visited += 1;
            roots.extend(children[index].iter().copied());
        }
        if visited != descriptor.joints.len() {
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
            for image in texture
                .image_slots
                .iter()
                .filter_map(Option::as_ref)
                .chain(texture.current_image.iter())
            {
                require_id(SourceKind::Image, image.as_str())?;
            }
            if !texture
                .translation
                .into_iter()
                .chain(texture.scale)
                .chain([texture.blend])
                .all(f32::is_finite)
            {
                return Err(InstanceError::NonFiniteInitialValue {
                    kind: SourceKind::Texture,
                    source_id: texture.source_id.to_string(),
                    field: "translation, scale, or blend",
                });
            }
            if texture.konst.is_some() != texture.tev0.is_some() {
                return Err(InstanceError::IncompleteTextureTev(
                    texture.source_id.clone(),
                ));
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
                branch_recurses: joint.branch_recurses,
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
                image_slots: texture.image_slots,
                current_image: texture.current_image,
                translation: texture.translation,
                scale: texture.scale,
                blend: texture.blend,
                konst: texture.konst,
                tev0: texture.tev0,
            })
            .collect();
        let instance = Self {
            id,
            joints,
            materials,
            textures,
            joint_indices,
            material_indices,
            texture_indices,
            children,
        };
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
                            let index = image_index(value.value, texture.image_slots.len())
                                .expect("image sample validated before mutation");
                            // HSD intentionally retains the previously resolved
                            // image when the selected table entry is null.
                            if let Some(image) = &texture.image_slots[index] {
                                texture.current_image = Some(image.clone());
                            }
                        }
                        Channel::TextureTranslationU => texture.translation[0] = value.value,
                        Channel::TextureTranslationV => texture.translation[1] = value.value,
                        Channel::TextureScaleU => texture.scale[0] = value.value,
                        Channel::TextureScaleV => texture.scale[1] = value.value,
                        Channel::TextureBlend => texture.blend = value.value,
                        Channel::TextureKonstR => {
                            texture.konst.as_mut().expect("TEV state validated")[0] =
                                quantize(value.value);
                        }
                        Channel::TextureKonstG => {
                            texture.konst.as_mut().expect("TEV state validated")[1] =
                                quantize(value.value);
                        }
                        Channel::TextureKonstB => {
                            texture.konst.as_mut().expect("TEV state validated")[2] =
                                quantize(value.value);
                        }
                        Channel::TextureKonstAlpha => {
                            texture.konst.as_mut().expect("TEV state validated")[3] =
                                quantize(value.value);
                        }
                        Channel::TextureTev0R => {
                            texture.tev0.as_mut().expect("TEV state validated")[0] =
                                quantize(value.value);
                        }
                        Channel::TextureTev0G => {
                            texture.tev0.as_mut().expect("TEV state validated")[1] =
                                quantize(value.value);
                        }
                        Channel::TextureTev0B => {
                            texture.tev0.as_mut().expect("TEV state validated")[2] =
                                quantize(value.value);
                        }
                        Channel::TextureTev0Alpha => {
                            texture.tev0.as_mut().expect("TEV state validated")[3] =
                                quantize(value.value);
                        }
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
                            image_index(value.value, self.textures[index].image_slots.len())
                                .map_err(|reason| ApplyError::InvalidTextureImage {
                                    texture: id.clone(),
                                    value_bits: value.value.to_bits(),
                                    image_count: self.textures[index].image_slots.len(),
                                    reason,
                                })?;
                        }
                        Channel::TextureKonstR
                        | Channel::TextureKonstG
                        | Channel::TextureKonstB
                        | Channel::TextureKonstAlpha
                        | Channel::TextureTev0R
                        | Channel::TextureTev0G
                        | Channel::TextureTev0B
                        | Channel::TextureTev0Alpha => {
                            validate_normalized(*value)?;
                            let present = if matches!(
                                value.channel,
                                Channel::TextureKonstR
                                    | Channel::TextureKonstG
                                    | Channel::TextureKonstB
                                    | Channel::TextureKonstAlpha
                            ) {
                                self.textures[index].konst.is_some()
                            } else {
                                self.textures[index].tev0.is_some()
                            };
                            if !present {
                                return Err(ApplyError::TextureColorWithoutTev {
                                    texture: id.clone(),
                                    channel: value.channel,
                                });
                            }
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
            if self.joints[index].branch_recurses {
                stack.extend(self.children[index].iter().copied());
            }
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
    // HSD spells this as `255.0 * val->fv`: the unsuffixed constant promotes
    // the sampled f32 to double before the truncating byte conversion.
    (255.0_f64 * f64::from(value)) as u8
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
        | Channel::TextureScaleU
        | Channel::TextureScaleV
        | Channel::TextureBlend
        | Channel::TextureKonstR
        | Channel::TextureKonstG
        | Channel::TextureKonstB
        | Channel::TextureKonstAlpha
        | Channel::TextureTev0R
        | Channel::TextureTev0G
        | Channel::TextureTev0B
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
    Image,
}

impl fmt::Display for SourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Joint => "joint",
            Self::Material => "material",
            Self::Texture => "texture",
            Self::Image => "image",
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
    #[error("source texture {0} must provide both konst and TEV0 colors or neither")]
    IncompleteTextureTev(SourceTextureId),
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
    #[error("texture {texture} has no TEV descriptor for {channel:?}")]
    TextureColorWithoutTev {
        texture: SourceTextureId,
        channel: Channel,
    },
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
            branch_recurses: true,
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
            image_slots: vec![
                Some("image-0".into()),
                None,
                Some("image-2".into()),
                Some("image-3".into()),
            ],
            current_image: Some("initial-image".into()),
            translation: [0.0, 0.0],
            scale: [1.0, 1.0],
            blend: 1.0,
            konst: Some([255; 4]),
            tev0: Some([255; 4]),
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
                branch_recurses: true,
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

        for (translation, scale, blend) in [
            ([f32::NAN, 0.0], [1.0, 1.0], 1.0),
            ([0.0, 0.0], [f32::INFINITY, 1.0], 1.0),
            ([0.0, 0.0], [1.0, 1.0], f32::INFINITY),
        ] {
            let mut bad_texture = texture("texture");
            bad_texture.translation = translation;
            bad_texture.scale = scale;
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
    fn accepts_empty_and_nullable_texture_image_tables_but_rejects_empty_image_ids() {
        let mut empty = texture("empty");
        empty.image_slots.clear();
        let descriptor = InstanceDescriptor {
            textures: vec![empty],
            ..InstanceDescriptor::default()
        };
        assert!(SceneInstance::new(InstanceId::new(1), descriptor).is_ok());

        let mut empty_slot_id = texture("bad-slot");
        empty_slot_id.image_slots[0] = Some("".into());
        let descriptor = InstanceDescriptor {
            textures: vec![empty_slot_id],
            ..InstanceDescriptor::default()
        };
        assert_eq!(
            SceneInstance::new(InstanceId::new(1), descriptor).unwrap_err(),
            InstanceError::EmptySourceId(SourceKind::Image)
        );

        for (konst, tev0) in [(Some([1; 4]), None), (None, Some([2; 4]))] {
            let mut incomplete_tev = texture("incomplete-tev");
            incomplete_tev.konst = konst;
            incomplete_tev.tev0 = tev0;
            let descriptor = InstanceDescriptor {
                textures: vec![incomplete_tev],
                ..InstanceDescriptor::default()
            };
            assert_eq!(
                SceneInstance::new(InstanceId::new(1), descriptor).unwrap_err(),
                InstanceError::IncompleteTextureTev("incomplete-tev".into())
            );
        }
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
    fn later_child_branch_update_overrides_an_earlier_parent_branch_update() {
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

        assert!(!instance.joint(&"leaf".into()).unwrap().visible());
        instance
            .apply_channel(
                &SourceTarget::Joint("branch".into()),
                sample(Channel::JointBranchVisibility, 0.5),
            )
            .unwrap();
        assert!(!instance.joint(&"branch".into()).unwrap().visible());
        assert!(!instance.joint(&"leaf".into()).unwrap().visible());
        assert!(instance.joint(&"unrelated".into()).unwrap().visible());

        instance
            .apply_channel(
                &SourceTarget::Joint("leaf".into()),
                sample(Channel::JointBranchVisibility, 1.0),
            )
            .unwrap();
        assert!(instance.joint(&"leaf".into()).unwrap().visible());
        assert!(!instance.joint(&"branch".into()).unwrap().visible());

        instance
            .apply_channel(
                &SourceTarget::Joint("branch".into()),
                sample(Channel::JointBranchVisibility, 0.500_1),
            )
            .unwrap();
        assert!(instance.joint(&"branch".into()).unwrap().visible());
        assert!(instance.joint(&"leaf".into()).unwrap().visible());
    }

    #[test]
    fn branch_updates_stop_after_setting_an_instance_boundary() {
        let descriptor = InstanceDescriptor {
            joints: vec![
                joint("root", None, true),
                JointDescriptor {
                    branch_recurses: false,
                    ..joint("instance", Some("root"), true)
                },
                joint("instance-child", Some("instance"), true),
                joint("ordinary", Some("root"), true),
                joint("ordinary-child", Some("ordinary"), true),
            ],
            ..InstanceDescriptor::default()
        };
        let mut instance = SceneInstance::new(InstanceId::new(1), descriptor).unwrap();

        instance
            .apply_channel(
                &SourceTarget::Joint("root".into()),
                sample(Channel::JointBranchVisibility, 0.0),
            )
            .unwrap();

        assert!(!instance.joint(&"root".into()).unwrap().visible());
        assert!(!instance.joint(&"instance".into()).unwrap().visible());
        assert!(instance.joint(&"instance-child".into()).unwrap().visible());
        assert!(!instance.joint(&"ordinary".into()).unwrap().visible());
        assert!(!instance.joint(&"ordinary-child".into()).unwrap().visible());
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
    fn applies_every_texture_channel_and_resolves_image_identity() {
        let mut instance = instance();
        instance
            .apply_channels(
                &SourceTarget::Texture("texture".into()),
                &[
                    sample(Channel::TextureImage, 2.9),
                    sample(Channel::TextureTranslationU, -0.5),
                    sample(Channel::TextureTranslationV, 1.5),
                    sample(Channel::TextureScaleU, 2.0),
                    sample(Channel::TextureScaleV, 0.5),
                    sample(Channel::TextureBlend, 0.25),
                    sample(Channel::TextureKonstR, 0.0),
                    sample(Channel::TextureKonstG, 0.5),
                    sample(Channel::TextureKonstB, 0.75),
                    sample(Channel::TextureKonstAlpha, 0.5),
                    sample(Channel::TextureTev0R, 1.0),
                    sample(Channel::TextureTev0G, 0.75),
                    sample(Channel::TextureTev0B, 0.5),
                    sample(Channel::TextureTev0Alpha, 0.75),
                ],
            )
            .unwrap();

        let state = instance.texture(&"texture".into()).unwrap();
        assert_eq!(state.current_image(), Some(&SourceImageId::from("image-2")));
        assert_eq!(state.translation(), [-0.5, 1.5]);
        assert_eq!(state.scale(), [2.0, 0.5]);
        assert_eq!(state.blend(), 0.25);
        assert_eq!(state.konst(), Some([0, 127, 191, 127]));
        assert_eq!(state.tev0(), Some([255, 191, 127, 191]));
    }

    #[test]
    fn null_texture_image_slot_is_a_noop_and_empty_table_rejects_only_timg() {
        let mut instance = instance();
        let slots_before = instance
            .texture(&"texture".into())
            .unwrap()
            .image_slots()
            .to_vec();
        instance
            .apply_channel(
                &SourceTarget::Texture("texture".into()),
                sample(Channel::TextureImage, 1.9),
            )
            .unwrap();
        let state = instance.texture(&"texture".into()).unwrap();
        assert_eq!(
            state.current_image(),
            Some(&SourceImageId::from("initial-image"))
        );
        assert_eq!(state.image_slots(), slots_before);

        let mut descriptor = descriptor();
        descriptor.textures[0].image_slots.clear();
        let mut empty = SceneInstance::new(InstanceId::new(9), descriptor).unwrap();
        empty
            .apply_channel(
                &SourceTarget::Texture("texture".into()),
                sample(Channel::TextureBlend, 0.5),
            )
            .unwrap();
        assert_eq!(empty.texture(&"texture".into()).unwrap().blend(), 0.5);
        assert!(matches!(
            empty.apply_channel(
                &SourceTarget::Texture("texture".into()),
                sample(Channel::TextureImage, 0.0),
            ),
            Err(ApplyError::InvalidTextureImage {
                reason: ImageIndexError::OutOfRange,
                image_count: 0,
                ..
            })
        ));
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

        let mut no_tev_descriptor = descriptor();
        no_tev_descriptor.textures[0].konst = None;
        no_tev_descriptor.textures[0].tev0 = None;
        let mut no_tev = SceneInstance::new(InstanceId::new(10), no_tev_descriptor).unwrap();
        let no_tev_before = no_tev.texture(&"texture".into()).unwrap().clone();
        assert!(matches!(
            no_tev.apply_channels(
                &SourceTarget::Texture("texture".into()),
                &[
                    sample(Channel::TextureScaleU, 3.0),
                    sample(Channel::TextureTev0R, 0.5),
                ],
            ),
            Err(ApplyError::TextureColorWithoutTev {
                channel: Channel::TextureTev0R,
                ..
            })
        ));
        assert_eq!(no_tev.texture(&"texture".into()).unwrap(), &no_tev_before);

        assert!(matches!(
            instance.apply_channels(
                &SourceTarget::Texture("texture".into()),
                &[
                    sample(Channel::TextureScaleU, 3.0),
                    sample(Channel::TextureTev0G, 1.01),
                ],
            ),
            Err(ApplyError::ColorSampleOutOfRange {
                channel: Channel::TextureTev0G,
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
                branch_recurses: true,
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
        assert!(!instance.joint(&"matrix".into()).unwrap().visible());
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
