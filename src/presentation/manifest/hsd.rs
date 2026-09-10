//! Native HSD descriptor traversal behind the generic presentation manifest.

use std::collections::{HashMap, HashSet};

use crate::{
    animation::{Channel, ChannelTarget, DataOffset, DecodeError, HsdDataSection},
    presentation::instance::{LocalSrt, SourceTarget},
};

use super::{
    AuthoredJointLocal, BindError, BindingDiagnostic, BindingDiagnosticKind, BoundAnimation,
    BoundJointSource, BoundMaterialSource, BoundTextureSource, HierarchySpec,
    SourceBindingIdentity, SourceObjectKind, TextureImageSlot, image_source_id, joint_source_id,
    material_source_id, texture_source_id,
};

const JOINT_SIZE: usize = 0x40;
const ANIM_JOINT_SIZE: usize = 0x14;
const MAT_ANIM_JOINT_SIZE: usize = 0x0c;
const SHAPE_ANIM_JOINT_SIZE: usize = 0x0c;
const DOBJ_DESC_SIZE: usize = 0x10;
const MOBJ_DESC_SIZE: usize = 0x18;
const MATERIAL_SIZE: usize = 0x14;
const MAT_ANIM_SIZE: usize = 0x10;
const TEX_ANIM_SIZE: usize = 0x18;
const TOBJ_DESC_SIZE: usize = 0x5c;
const IMAGE_DESC_SIZE: usize = 0x18;
const TOBJ_TEV_DESC_SIZE: usize = 0x20;

const JOBJ_HIDDEN: u32 = 1 << 4;
const JOBJ_PTCL: u32 = 1 << 5;
const JOBJ_INSTANCE: u32 = 1 << 12;
const JOBJ_SPLINE: u32 = 1 << 14;
const JOBJ_USER_DEF_MTX: u32 = 1 << 23;

#[derive(Clone, Debug)]
pub(super) struct DerivedHierarchy {
    pub joints: Vec<BoundJointSource>,
    pub materials: Vec<BoundMaterialSource>,
    pub textures: Vec<BoundTextureSource>,
    pub bindings: Vec<BoundAnimation>,
    pub parents: HashMap<u32, Option<u32>>,
    pub diagnostics: Vec<BindingDiagnostic>,
}

#[derive(Clone, Copy)]
pub(super) struct HsdView<'a> {
    bytes: &'a [u8],
}

impl<'a> HsdView<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub(super) const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    fn descriptor(
        self,
        offset: u32,
        length: usize,
        context: &'static str,
    ) -> Result<&'a [u8], BindError> {
        if offset == 0 {
            return Err(BindError::NullOffset { context });
        }
        if !offset.is_multiple_of(4) {
            return Err(BindError::MisalignedDescriptor { context, offset });
        }
        let start = offset as usize;
        let end = start
            .checked_add(length)
            .filter(|&end| end <= self.bytes.len())
            .ok_or(BindError::DescriptorOutOfBounds {
                context,
                offset,
                length,
                data_length: self.bytes.len(),
            })?;
        Ok(&self.bytes[start..end])
    }

    fn u32(self, offset: u32, field: usize, context: &'static str) -> Result<u32, BindError> {
        let bytes = self.descriptor(offset, field + 4, context)?;
        Ok(u32::from_be_bytes(
            bytes[field..field + 4].try_into().unwrap(),
        ))
    }

    fn u16(self, offset: u32, field: usize, context: &'static str) -> Result<u16, BindError> {
        let bytes = self.descriptor(offset, field + 2, context)?;
        Ok(u16::from_be_bytes(
            bytes[field..field + 2].try_into().unwrap(),
        ))
    }

    fn f32(self, offset: u32, field: usize, name: &'static str) -> Result<f32, BindError> {
        let bits = self.u32(offset, field, name)?;
        let value = f32::from_bits(bits);
        if !value.is_finite() {
            return Err(BindError::NonFiniteDescriptor {
                field: name,
                offset,
            });
        }
        Ok(value)
    }

    fn bytes_at(
        self,
        offset: u32,
        length: usize,
        context: &'static str,
    ) -> Result<&'a [u8], BindError> {
        self.descriptor(offset, length, context)
    }
}

#[derive(Clone, Copy, Debug)]
struct PendingJoint {
    model: u32,
    animation: u32,
    material_animation: u32,
    parent: Option<u32>,
}

pub(super) fn derive_hierarchy(
    resource_id: &str,
    spec: &HierarchySpec,
    view: HsdView<'_>,
) -> Result<DerivedHierarchy, BindError> {
    view.descriptor(spec.model_root, JOINT_SIZE, "model root")?;
    if let Some(offset) = spec.joint_animation_root {
        view.descriptor(offset, ANIM_JOINT_SIZE, "joint animation root")?;
    }
    if let Some(offset) = spec.material_animation_root {
        view.descriptor(offset, MAT_ANIM_JOINT_SIZE, "material animation root")?;
    }

    let decoder =
        HsdDataSection::new(view.bytes()).map_err(|source| BindError::AnimationDecode {
            hierarchy: spec.id.clone(),
            aobj_offset: 0,
            source,
        })?;
    let mut output = DerivedHierarchy {
        joints: Vec::new(),
        materials: Vec::new(),
        textures: Vec::new(),
        bindings: Vec::new(),
        parents: HashMap::new(),
        diagnostics: Vec::new(),
    };
    if let Some(offset) = spec.shape_animation_root {
        view.descriptor(offset, SHAPE_ANIM_JOINT_SIZE, "shape animation root")?;
        output.diagnostics.push(BindingDiagnostic {
            hierarchy: spec.id.clone(),
            owner_joint_offset: None,
            descriptor_offset: DataOffset::new(offset),
            kind: BindingDiagnosticKind::ShapeAnimationNotModeled,
        });
    }

    let mut seen_models = HashSet::new();
    let mut seen_materials = HashSet::new();
    let mut seen_textures = HashSet::new();
    let mut stack = vec![PendingJoint {
        model: spec.model_root,
        animation: spec.joint_animation_root.unwrap_or(0),
        material_animation: spec.material_animation_root.unwrap_or(0),
        parent: None,
    }];

    while let Some(pending) = stack.pop() {
        if !seen_models.insert(pending.model) {
            return Err(BindError::DescriptorCycle {
                kind: "model joint",
                offset: pending.model,
            });
        }
        let flags = view.u32(pending.model, 4, "model joint")?;
        let model_child = view.u32(pending.model, 8, "model joint")?;
        let instance_target = if flags & JOBJ_INSTANCE != 0 {
            view.descriptor(model_child, JOINT_SIZE, "instance target")?;
            Some(DataOffset::new(model_child))
        } else {
            None
        };
        let identity = joint_identity(pending.model);
        let source_id = joint_source_id(resource_id, identity);
        let parent = pending
            .parent
            .map(|offset| joint_source_id(resource_id, joint_identity(offset)));
        let local = joint_local(view, pending.model, flags)?;
        output.joints.push(BoundJointSource {
            identity,
            source_id,
            parent,
            local,
            visible: flags & JOBJ_HIDDEN == 0,
            branch_recurses: flags & JOBJ_INSTANCE == 0,
            instance_target,
        });
        output.parents.insert(pending.model, pending.parent);

        bind_joint_animation(resource_id, spec, view, &decoder, pending, &mut output)?;
        if flags & (JOBJ_PTCL | JOBJ_SPLINE) == 0 {
            bind_material_animations(
                resource_id,
                spec,
                view,
                &decoder,
                pending,
                &mut seen_materials,
                &mut seen_textures,
                &mut output,
            )?;
        }

        if flags & JOBJ_INSTANCE == 0 {
            let animation_child = optional_field(
                view,
                pending.animation,
                0,
                ANIM_JOINT_SIZE,
                "joint animation",
            )?;
            let material_child = optional_field(
                view,
                pending.material_animation,
                0,
                MAT_ANIM_JOINT_SIZE,
                "material animation joint",
            )?;
            let mut children = aligned_children(
                view,
                model_child,
                animation_child,
                material_child,
                pending.model,
            )?;
            children.reverse();
            stack.extend(children);
        }
    }

    Ok(output)
}

fn joint_identity(offset: u32) -> SourceBindingIdentity {
    SourceBindingIdentity {
        kind: SourceObjectKind::Joint,
        descriptor_offset: DataOffset::new(offset),
        owner_joint_offset: DataOffset::new(offset),
        dobj_index: None,
        texture_index: None,
    }
}

fn joint_local(
    view: HsdView<'_>,
    offset: u32,
    flags: u32,
) -> Result<AuthoredJointLocal, BindError> {
    let vector = |field: usize, name: &'static str| -> Result<[f32; 3], BindError> {
        Ok([
            view.f32(offset, field, name)?,
            view.f32(offset, field + 4, name)?,
            view.f32(offset, field + 8, name)?,
        ])
    };
    let authored_srt = LocalSrt {
        rotation: vector(0x14, "joint rotation")?,
        scale: vector(0x20, "joint scale")?,
        translation: vector(0x2c, "joint translation")?,
    };
    if flags & JOBJ_USER_DEF_MTX == 0 {
        return Ok(AuthoredJointLocal::Srt(authored_srt));
    }

    let matrix = view.u32(offset, 0x38, "model joint")?;
    let envelope_matrix = if matrix == 0 {
        None
    } else {
        let mut values = [0.0; 12];
        for (index, value) in values.iter_mut().enumerate() {
            *value = view.f32(matrix, 4 * index, "joint envelope matrix")?;
        }
        Some(values)
    };
    Ok(AuthoredJointLocal::UserDefined {
        authored_srt,
        envelope_matrix,
    })
}

fn optional_field(
    view: HsdView<'_>,
    descriptor: u32,
    field: usize,
    size: usize,
    context: &'static str,
) -> Result<u32, BindError> {
    if descriptor == 0 {
        Ok(0)
    } else {
        view.descriptor(descriptor, size, context)?;
        view.u32(descriptor, field, context)
    }
}

fn aligned_children(
    view: HsdView<'_>,
    mut model: u32,
    mut animation: u32,
    mut material: u32,
    parent: u32,
) -> Result<Vec<PendingJoint>, BindError> {
    let mut output = Vec::new();
    let mut model_siblings = HashSet::new();
    let mut animation_siblings = HashSet::new();
    let mut material_siblings = HashSet::new();
    while model != 0 {
        if !model_siblings.insert(model) {
            return Err(BindError::DescriptorCycle {
                kind: "model sibling",
                offset: model,
            });
        }
        if animation != 0 && !animation_siblings.insert(animation) {
            return Err(BindError::DescriptorCycle {
                kind: "joint-animation sibling",
                offset: animation,
            });
        }
        if material != 0 && !material_siblings.insert(material) {
            return Err(BindError::DescriptorCycle {
                kind: "material-animation sibling",
                offset: material,
            });
        }
        output.push(PendingJoint {
            model,
            animation,
            material_animation: material,
            parent: Some(parent),
        });
        model = view.u32(model, 0x0c, "model joint")?;
        animation = optional_field(view, animation, 4, ANIM_JOINT_SIZE, "joint animation")?;
        material = optional_field(
            view,
            material,
            4,
            MAT_ANIM_JOINT_SIZE,
            "material animation joint",
        )?;
    }
    Ok(output)
}

fn bind_joint_animation(
    resource_id: &str,
    spec: &HierarchySpec,
    view: HsdView<'_>,
    decoder: &HsdDataSection<'_>,
    pending: PendingJoint,
    output: &mut DerivedHierarchy,
) -> Result<(), BindError> {
    let aobj = optional_field(
        view,
        pending.animation,
        8,
        ANIM_JOINT_SIZE,
        "joint animation",
    )?;
    if aobj == 0 {
        return Ok(());
    }
    let identity = joint_identity(pending.model);
    push_binding(
        spec,
        decoder,
        pending.model,
        identity,
        SourceTarget::Joint(joint_source_id(resource_id, identity)),
        aobj,
        ChannelTarget::Joint,
        output,
    )
    .map(|_| ())
}

#[allow(clippy::too_many_arguments)]
fn bind_material_animations(
    resource_id: &str,
    spec: &HierarchySpec,
    view: HsdView<'_>,
    decoder: &HsdDataSection<'_>,
    pending: PendingJoint,
    seen_materials: &mut HashSet<SourceBindingIdentity>,
    seen_textures: &mut HashSet<SourceBindingIdentity>,
    output: &mut DerivedHierarchy,
) -> Result<(), BindError> {
    let mut dobj = view.u32(pending.model, 0x10, "model joint")?;
    let mut matanim = optional_field(
        view,
        pending.material_animation,
        8,
        MAT_ANIM_JOINT_SIZE,
        "material animation joint",
    )?;
    let mut seen_dobjs = HashSet::new();
    let mut seen_matanims = HashSet::new();
    let mut dobj_ordinal = 0_usize;

    while dobj != 0 {
        if !seen_dobjs.insert(dobj) {
            return Err(BindError::DescriptorCycle {
                kind: "DObj",
                offset: dobj,
            });
        }
        view.descriptor(dobj, DOBJ_DESC_SIZE, "DObj")?;
        if matanim != 0 {
            if !seen_matanims.insert(matanim) {
                return Err(BindError::DescriptorCycle {
                    kind: "material animation",
                    offset: matanim,
                });
            }
            view.descriptor(matanim, MAT_ANIM_SIZE, "material animation")?;
            let mobj = view.u32(dobj, 8, "DObj")?;
            if mobj != 0 {
                view.descriptor(mobj, MOBJ_DESC_SIZE, "MObj")?;
                let dobj_index =
                    u16::try_from(dobj_ordinal).map_err(|_| BindError::TooManyOccurrences {
                        hierarchy: spec.id.clone(),
                        kind: "DObj",
                    })?;
                let material_identity = SourceBindingIdentity {
                    kind: SourceObjectKind::Material,
                    descriptor_offset: DataOffset::new(mobj),
                    owner_joint_offset: DataOffset::new(pending.model),
                    dobj_index: Some(dobj_index),
                    texture_index: None,
                };
                let material_aobj = view.u32(matanim, 4, "material animation")?;
                if material_aobj != 0 {
                    if seen_materials.insert(material_identity) {
                        output.materials.push(material_source(
                            resource_id,
                            view,
                            material_identity,
                        )?);
                    }
                    let _ = push_binding(
                        spec,
                        decoder,
                        pending.model,
                        material_identity,
                        SourceTarget::Material(material_source_id(resource_id, material_identity)),
                        material_aobj,
                        ChannelTarget::Material,
                        output,
                    )?;
                }

                let texanim_head = view.u32(matanim, 8, "material animation")?;
                bind_texture_animations(
                    resource_id,
                    spec,
                    view,
                    decoder,
                    pending.model,
                    dobj_index,
                    mobj,
                    texanim_head,
                    seen_textures,
                    output,
                )?;
            }
        }

        dobj = view.u32(dobj, 4, "DObj")?;
        matanim = if matanim == 0 {
            0
        } else {
            view.u32(matanim, 0, "material animation")?
        };
        dobj_ordinal += 1;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn bind_texture_animations(
    resource_id: &str,
    spec: &HierarchySpec,
    view: HsdView<'_>,
    decoder: &HsdDataSection<'_>,
    owner_joint: u32,
    dobj_index: u16,
    mobj: u32,
    texanim_head: u32,
    seen_textures: &mut HashSet<SourceBindingIdentity>,
    output: &mut DerivedHierarchy,
) -> Result<(), BindError> {
    let mut tobj = view.u32(mobj, 8, "MObj")?;
    let mut seen_tobjs = HashSet::new();
    let mut texture_ordinal = 0_usize;
    while tobj != 0 {
        if !seen_tobjs.insert(tobj) {
            return Err(BindError::DescriptorCycle {
                kind: "TObj",
                offset: tobj,
            });
        }
        view.descriptor(tobj, TOBJ_DESC_SIZE, "TObj")?;
        let texture_index =
            u16::try_from(texture_ordinal).map_err(|_| BindError::TooManyOccurrences {
                hierarchy: spec.id.clone(),
                kind: "TObj",
            })?;
        let map_id = view.u32(tobj, 8, "TObj")?;
        if let Some(texanim) = lookup_texanim(view, texanim_head, map_id)? {
            let aobj = view.u32(texanim, 8, "texture animation")?;
            if aobj != 0 {
                let identity = SourceBindingIdentity {
                    kind: SourceObjectKind::Texture,
                    descriptor_offset: DataOffset::new(tobj),
                    owner_joint_offset: DataOffset::new(owner_joint),
                    dobj_index: Some(dobj_index),
                    texture_index: Some(texture_index),
                };
                let texture = texture_source(resource_id, view, identity, mobj, texanim)?;
                let has_image_table = !texture.image_slots.is_empty();
                let has_tev = texture.konst.is_some();
                if seen_textures.insert(identity) {
                    output.textures.push(texture);
                }
                let tracks = push_binding(
                    spec,
                    decoder,
                    owner_joint,
                    identity,
                    SourceTarget::Texture(texture_source_id(resource_id, identity)),
                    aobj,
                    ChannelTarget::Texture,
                    output,
                )?;
                if tracks.texture_image && !has_image_table {
                    return Err(BindError::TextureImageTrackWithoutTable {
                        hierarchy: spec.id.clone(),
                        tobj_offset: tobj,
                        aobj_offset: aobj,
                    });
                }
                if tracks.texture_color && !has_tev {
                    return Err(BindError::TextureColorTrackWithoutTev {
                        hierarchy: spec.id.clone(),
                        tobj_offset: tobj,
                        aobj_offset: aobj,
                    });
                }
            }
        }
        tobj = view.u32(tobj, 4, "TObj")?;
        texture_ordinal += 1;
    }
    Ok(())
}

fn lookup_texanim(
    view: HsdView<'_>,
    mut texanim: u32,
    map_id: u32,
) -> Result<Option<u32>, BindError> {
    let mut seen = HashSet::new();
    while texanim != 0 {
        if !seen.insert(texanim) {
            return Err(BindError::DescriptorCycle {
                kind: "texture animation",
                offset: texanim,
            });
        }
        view.descriptor(texanim, TEX_ANIM_SIZE, "texture animation")?;
        if view.u32(texanim, 4, "texture animation")? == map_id {
            return Ok(Some(texanim));
        }
        texanim = view.u32(texanim, 0, "texture animation")?;
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn push_binding(
    spec: &HierarchySpec,
    decoder: &HsdDataSection<'_>,
    owner_joint: u32,
    identity: SourceBindingIdentity,
    target: SourceTarget,
    aobj: u32,
    channel_target: ChannelTarget,
    output: &mut DerivedHierarchy,
) -> Result<BoundTrackSet, BindError> {
    let report = decoder
        .decode_aobj_supported(DataOffset::new(aobj), channel_target)
        .map_err(|source| BindError::AnimationDecode {
            hierarchy: spec.id.clone(),
            aobj_offset: aobj,
            source,
        })?;
    for skipped in report.skipped_tracks {
        let kind = match skipped.error {
            DecodeError::UnsupportedChannel {
                target,
                channel,
                descriptor_offset,
            } => {
                debug_assert_eq!(descriptor_offset, skipped.descriptor_offset.get());
                BindingDiagnosticKind::UnsupportedChannel {
                    target,
                    channel,
                    fobj_offset: skipped.descriptor_offset,
                }
            }
            DecodeError::UnsupportedFractionEncoding {
                encoding,
                descriptor_offset,
                field,
            } => {
                debug_assert_eq!(descriptor_offset, skipped.descriptor_offset.get());
                BindingDiagnosticKind::UnsupportedFractionEncoding {
                    encoding,
                    fobj_offset: skipped.descriptor_offset,
                    field,
                }
            }
            DecodeError::UnsupportedOpcode {
                opcode,
                stream_offset,
            } => BindingDiagnosticKind::UnsupportedOpcode {
                opcode,
                fobj_offset: skipped.descriptor_offset,
                stream_offset: DataOffset::new(stream_offset),
            },
            source => {
                return Err(BindError::AnimationDecode {
                    hierarchy: spec.id.clone(),
                    aobj_offset: aobj,
                    source,
                });
            }
        };
        output.diagnostics.push(BindingDiagnostic {
            hierarchy: spec.id.clone(),
            owner_joint_offset: Some(DataOffset::new(owner_joint)),
            descriptor_offset: DataOffset::new(aobj),
            kind,
        });
    }
    let track_set = BoundTrackSet {
        texture_image: report
            .animation
            .tracks
            .iter()
            .any(|track| track.channel == Channel::TextureImage),
        texture_color: report.animation.tracks.iter().any(|track| {
            matches!(
                track.channel,
                Channel::TextureKonstR
                    | Channel::TextureKonstG
                    | Channel::TextureKonstB
                    | Channel::TextureKonstAlpha
                    | Channel::TextureTev0R
                    | Channel::TextureTev0G
                    | Channel::TextureTev0B
                    | Channel::TextureTev0Alpha
            )
        }),
    };
    if !report.animation.tracks.is_empty() {
        output.bindings.push(BoundAnimation {
            identity,
            target,
            owner_joint_offset: DataOffset::new(owner_joint),
            aobj_offset: DataOffset::new(aobj),
            animation: report.animation,
        });
    }
    Ok(track_set)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BoundTrackSet {
    texture_image: bool,
    texture_color: bool,
}

fn material_source(
    resource_id: &str,
    view: HsdView<'_>,
    identity: SourceBindingIdentity,
) -> Result<BoundMaterialSource, BindError> {
    let mobj = identity.descriptor_offset.get();
    let material = view.u32(mobj, 0x0c, "MObj")?;
    let bytes = view.bytes_at(material, MATERIAL_SIZE, "material")?;
    let alpha = view.f32(material, 0x0c, "material alpha")?;
    Ok(BoundMaterialSource {
        identity,
        source_id: material_source_id(resource_id, identity),
        diffuse: [bytes[4], bytes[5], bytes[6]],
        alpha,
    })
}

fn texture_source(
    resource_id: &str,
    view: HsdView<'_>,
    identity: SourceBindingIdentity,
    owner_material: u32,
    texanim: u32,
) -> Result<BoundTextureSource, BindError> {
    let tobj = identity.descriptor_offset.get();
    let initial_image = view.u32(tobj, 0x4c, "TObj")?;
    if initial_image != 0 {
        view.descriptor(initial_image, IMAGE_DESC_SIZE, "image descriptor")?;
    }
    let image_table = view.u32(texanim, 0x0c, "texture animation")?;
    let image_count = usize::from(view.u16(texanim, 0x14, "texture animation")?);
    let mut image_slots = Vec::with_capacity(image_count);
    if image_count != 0 {
        let table = view.bytes_at(
            image_table,
            image_count
                .checked_mul(4)
                .ok_or(BindError::DescriptorOutOfBounds {
                    context: "texture image table",
                    offset: image_table,
                    length: usize::MAX,
                    data_length: view.bytes().len(),
                })?,
            "texture image table",
        )?;
        for slot in table.as_chunks::<4>().0 {
            let image = u32::from_be_bytes(*slot);
            if image != 0 {
                view.descriptor(image, IMAGE_DESC_SIZE, "image descriptor")?;
            }
            image_slots.push(TextureImageSlot {
                image_descriptor: (image != 0).then(|| DataOffset::new(image)),
                source_id: (image != 0)
                    .then(|| image_source_id(resource_id, DataOffset::new(image))),
            });
        }
    }
    let tev = view.u32(tobj, 0x58, "TObj")?;
    let (konst, tev0) = if tev == 0 {
        (None, None)
    } else {
        let bytes = view.bytes_at(tev, TOBJ_TEV_DESC_SIZE, "TObj TEV descriptor")?;
        (
            Some(bytes[16..20].try_into().expect("validated TEV descriptor")),
            Some(bytes[20..24].try_into().expect("validated TEV descriptor")),
        )
    };
    Ok(BoundTextureSource {
        identity,
        source_id: texture_source_id(resource_id, identity),
        owner_material_offset: DataOffset::new(owner_material),
        initial_image_descriptor: (initial_image != 0).then(|| DataOffset::new(initial_image)),
        current_image: (initial_image != 0)
            .then(|| image_source_id(resource_id, DataOffset::new(initial_image))),
        image_slots,
        translation: [
            view.f32(tobj, 0x28, "texture translation")?,
            view.f32(tobj, 0x2c, "texture translation")?,
        ],
        scale: [
            view.f32(tobj, 0x1c, "texture scale")?,
            view.f32(tobj, 0x20, "texture scale")?,
        ],
        blend: view.f32(tobj, 0x44, "texture blend")?,
        konst,
        tev0,
    })
}
