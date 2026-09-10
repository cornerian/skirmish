//! Shared GPU path for window presentation and offscreen captures.
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::BufWriter,
    path::Path,
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use glam::Vec3;
use sdl3::video::Window;
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::presentation::instance::InstanceId;

use super::platform::SdlSurface;
use super::scene::{
    Camera, CullMode, GeometrySpace, MaterialSourceId, Mesh, PeAlphaTest, PeBlendFactor,
    PeBlendMode, PeBlendState, PeCompare, PixelEngineState, RenderMode, RenderModeClass, Scene,
    Texture, Vertex, VisualDObjOccurrence, VisualJointOccurrence, VisualMaterialOccurrence,
    VisualTextureOccurrence,
};
use super::viewport::{PresentationTransform, fitted_viewport};
use crate::presentation::texture_matrix::{TextureTransform, WrapMode, texture_matrix};

pub const MESH_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/mesh.wgsl"));
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Runtime identity used by the one-scene compatibility path.
///
/// New callers that clone resident draws should allocate their own non-default
/// identity and use [`RuntimeDrawUpdate`] for every mutation.
pub const DEFAULT_INSTANCE_ID: InstanceId = InstanceId::new(0);

/// Selects draw parts attached to one exported joint for visibility updates.
///
/// Exact selectors include resource provenance and never collide across visual
/// resources. Bare visual offsets are available only for legacy scenes that do
/// not carry exact occurrences. Runtime scene-instance identity is separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportDrawSelector<'a> {
    Exact(&'a VisualJointOccurrence),
    /// Legacy source identity. `instance_id` is the exporter's occurrence
    /// discriminator from [`Mesh::instance_id`], not a runtime [`InstanceId`].
    Legacy {
        joint: u32,
        instance_id: Option<&'a str>,
    },
}

/// Selects every draw using one exact MObj occurrence or legacy visual offset.
///
/// The legacy form intentionally excludes draws that carry exact occurrence
/// metadata, even if their bare MObj offsets happen to match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportMaterialSelector<'a> {
    Exact(&'a VisualMaterialOccurrence),
    Legacy { source_id: MaterialSourceId },
}

/// Selects the first-stage texture of every draw using one exact TObj occurrence.
///
/// Only stage 0 is sampled by the preview shader, so a selector naming a later
/// stage ordinal matches nothing; the presentation driver reports that case
/// explicitly before it reaches the renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportTextureSelector<'a> {
    Exact(&'a VisualTextureOccurrence),
}

/// One type-safe mutable update over immutable scene resources.
///
/// Joint visibility may fan out over all attached materials. Material color is
/// instead selected by exact MObj identity, preventing a joint with several
/// materials from receiving an ambiguous broadcast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrawUpdate<'a> {
    Visibility {
        target: ExportDrawSelector<'a>,
        visible: bool,
    },
    MaterialColor {
        target: ExportMaterialSelector<'a>,
        color: [f32; 4],
    },
    /// Column-major world matrix of one owning joint, applied to every
    /// joint-local draw attached to it within the instance. Matching a
    /// world-baked draw is a batch validation error, never a silent double
    /// transform.
    JointTransform {
        target: ExportDrawSelector<'a>,
        world: [[f32; 4]; 4],
    },
    /// First-stage image plus the animated TObj translation and scale; the
    /// authored rotation, repeat counts, and wrap modes stay with the draw.
    Texture {
        target: ExportTextureSelector<'a>,
        image: Option<usize>,
        translation: [f32; 2],
        scale: [f32; 2],
    },
}

/// One source-targeted draw update scoped to exactly one runtime instance.
///
/// Keeping runtime identity outside the source selector prevents source and
/// exporter occurrence IDs from becoming a second runtime identity system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeDrawUpdate<'a> {
    pub instance_id: InstanceId,
    pub update: DrawUpdate<'a>,
}

/// A rejected member of an otherwise atomic runtime draw batch.
#[derive(Debug, Error)]
#[error("runtime draw update {index}: {source}")]
pub struct RuntimeDrawBatchError {
    index: usize,
    #[source]
    source: anyhow::Error,
}

impl RuntimeDrawBatchError {
    pub const fn index(&self) -> usize {
        self.index
    }

    pub(super) fn new(index: usize, source: anyhow::Error) -> Self {
        Self { index, source }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExportDrawIdentity {
    joint: Option<u32>,
    export_instance_id: Option<String>,
    source_occurrence: Option<VisualDObjOccurrence>,
    material_source_id: Option<MaterialSourceId>,
    material_source_occurrence: Option<VisualMaterialOccurrence>,
    /// Exact occurrence of the sampled first texture stage.
    texture_source_occurrence: Option<VisualTextureOccurrence>,
}

impl ExportDrawIdentity {
    fn matches_joint(&self, selector: ExportDrawSelector<'_>) -> bool {
        match selector {
            ExportDrawSelector::Exact(target) => self
                .source_occurrence
                .as_ref()
                .is_some_and(|source| &source.owner_joint == target),
            ExportDrawSelector::Legacy { joint, instance_id } => {
                self.source_occurrence.is_none()
                    && self.joint == Some(joint)
                    && self.export_instance_id.as_deref() == instance_id
            }
        }
    }

    fn matches_material(&self, selector: ExportMaterialSelector<'_>) -> bool {
        match selector {
            ExportMaterialSelector::Exact(target) => {
                self.material_source_occurrence.as_ref() == Some(target)
            }
            ExportMaterialSelector::Legacy { source_id } => {
                self.source_occurrence.is_none() && self.material_source_id == Some(source_id)
            }
        }
    }

    fn matches_texture(&self, selector: ExportTextureSelector<'_>) -> bool {
        match selector {
            ExportTextureSelector::Exact(target) => {
                self.texture_source_occurrence.as_ref() == Some(target)
            }
        }
    }
}

impl From<&Mesh> for ExportDrawIdentity {
    fn from(mesh: &Mesh) -> Self {
        Self {
            joint: mesh.joint,
            export_instance_id: mesh.instance_id.clone(),
            source_occurrence: mesh.source_occurrence.clone(),
            material_source_id: mesh.material.source_id,
            material_source_occurrence: mesh.material.source_occurrence.clone(),
            texture_source_occurrence: mesh
                .material
                .texture_sources
                .first()
                .and_then(|stage| stage.occurrence.clone()),
        }
    }
}

/// Mutable first-stage texture state of one runtime draw.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DrawTextureState {
    /// Scene texture index; `None` samples the white fallback.
    image: Option<usize>,
    wrap: [WrapMode; 2],
    transform: TextureTransform,
}

impl DrawTextureState {
    fn from_mesh(mesh: &Mesh) -> Self {
        let stage = mesh.material.texture_sources.first();
        Self {
            image: mesh.material.texture,
            wrap: stage.map_or([WrapMode::Repeat; 2], |stage| stage.wrap),
            transform: stage.map_or(
                TextureTransform {
                    rotation: [0.0; 3],
                    scale: [1.0; 3],
                    translation: [0.0; 3],
                    repeat: [1, 1],
                    wrap_t: WrapMode::Repeat,
                },
                |stage| stage.transform,
            ),
        }
    }

    /// Column-major GPU form of the HSD texture matrix over `(s, t, 0, 1)`.
    fn uv_transform(&self) -> Result<[[f32; 4]; 4]> {
        let matrix = texture_matrix(&self.transform)?;
        Ok([
            [matrix[0][0], matrix[1][0], 0.0, 0.0],
            [matrix[0][1], matrix[1][1], 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [
                matrix[0][2] + matrix[0][3],
                matrix[1][2] + matrix[1][3],
                0.0,
                1.0,
            ],
        ])
    }
}

const IDENTITY_TRANSFORM: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// Reinterpret an exported column-major 16-float matrix as four columns.
fn transform_columns(matrix: [f32; 16]) -> [[f32; 4]; 4] {
    std::array::from_fn(|column| std::array::from_fn(|row| matrix[column * 4 + row]))
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DrawState {
    visible: bool,
    material_color: [f32; 4],
    /// Column-major model matrix; identity for world-baked geometry.
    joint_transform: [[f32; 4]; 4],
    texture: DrawTextureState,
}

impl DrawState {
    fn new(mesh: &Mesh, joint_world: Option<[f32; 16]>) -> Result<Self> {
        let joint_transform = match mesh.geometry_space {
            GeometrySpace::World => IDENTITY_TRANSFORM,
            GeometrySpace::JointLocal => transform_columns(joint_world.with_context(|| {
                format!(
                    "{} is joint-local but its owning joint has no serialized world matrix",
                    mesh.name
                )
            })?),
        };
        let texture = DrawTextureState::from_mesh(mesh);
        texture
            .uv_transform()
            .with_context(|| format!("{} first texture stage", mesh.name))?;
        Ok(Self {
            visible: !mesh.hidden,
            material_color: mesh.material.color,
            joint_transform,
            texture,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DrawPresentation {
    identity: ExportDrawIdentity,
    state: DrawState,
    material_render_mode: Option<RenderMode>,
    render_class: DrawRenderClass,
    geometry_space: GeometrySpace,
}

impl DrawPresentation {
    fn from_mesh(
        mesh: &Mesh,
        textures: &[Texture],
        joint_world: Option<[f32; 16]>,
    ) -> Result<Self> {
        Ok(Self {
            identity: mesh.into(),
            state: DrawState::new(mesh, joint_world)?,
            material_render_mode: mesh.material.render_mode,
            render_class: DrawRenderClass::from_mesh(mesh, textures),
            geometry_space: mesh.geometry_space,
        })
    }

    /// True when a joint transform would reach this draw but its geometry was
    /// baked into world space by the exporter.
    fn rejects_joint_transform(&self, target: ExportDrawSelector<'_>) -> bool {
        self.geometry_space == GeometrySpace::World && self.identity.matches_joint(target)
    }

    fn update(&mut self, update: DrawUpdate<'_>) -> bool {
        match update {
            DrawUpdate::Visibility { target, visible } if self.identity.matches_joint(target) => {
                self.state.visible = visible;
                true
            }
            DrawUpdate::JointTransform { target, world }
                if self.geometry_space == GeometrySpace::JointLocal
                    && self.identity.matches_joint(target) =>
            {
                self.state.joint_transform = world;
                true
            }
            DrawUpdate::Texture {
                target,
                image,
                translation,
                scale,
            } if self.identity.matches_texture(target) => {
                self.state.texture.image = image;
                self.state.texture.transform.translation[..2].copy_from_slice(&translation);
                self.state.texture.transform.scale[..2].copy_from_slice(&scale);
                true
            }
            DrawUpdate::MaterialColor { target, color }
                if self.identity.matches_material(target) =>
            {
                let mut color = color;
                if self
                    .material_render_mode
                    .is_some_and(RenderMode::uses_vertex_color)
                {
                    color[..3].fill(1.0);
                }
                if self
                    .material_render_mode
                    .is_some_and(RenderMode::uses_vertex_alpha)
                {
                    color[3] = 1.0;
                }
                self.state.material_color = color;
                true
            }
            _ => false,
        }
    }

    fn material_alpha_uses_hsd_byte_storage(&self) -> bool {
        self.material_render_mode
            .is_some_and(|mode| !mode.uses_vertex_alpha())
    }
}

fn update_runtime_presentation(
    instance_id: InstanceId,
    presentation: &mut DrawPresentation,
    update: RuntimeDrawUpdate<'_>,
) -> bool {
    instance_id == update.instance_id && presentation.update(update.update)
}

fn ensure_new_runtime_instance(
    instance_ids: &HashSet<InstanceId>,
    instance_id: InstanceId,
) -> Result<()> {
    ensure!(
        !instance_ids.contains(&instance_id),
        "runtime draw instance {} already exists",
        instance_id.get()
    );
    Ok(())
}

fn ensure_runtime_instance(
    instance_ids: &HashSet<InstanceId>,
    instance_id: InstanceId,
) -> Result<()> {
    ensure!(
        instance_ids.contains(&instance_id),
        "runtime draw instance {} does not exist",
        instance_id.get()
    );
    Ok(())
}

fn validate_runtime_draw_update(
    instance_ids: &HashSet<InstanceId>,
    texture_count: usize,
    update: RuntimeDrawUpdate<'_>,
) -> Result<()> {
    ensure_runtime_instance(instance_ids, update.instance_id)?;
    match update.update {
        DrawUpdate::MaterialColor { color, .. } => ensure!(
            color.iter().all(|component| component.is_finite()),
            "material color must contain finite components"
        ),
        DrawUpdate::JointTransform { world, .. } => ensure!(
            world
                .iter()
                .flatten()
                .all(|component| component.is_finite()),
            "joint transform must contain finite components"
        ),
        DrawUpdate::Texture {
            image,
            translation,
            scale,
            ..
        } => {
            ensure!(
                translation
                    .iter()
                    .chain(&scale)
                    .all(|component| component.is_finite()),
                "texture translation and scale must contain finite components"
            );
            ensure!(
                image.is_none_or(|image| image < texture_count),
                "texture image {} is outside the {texture_count} resident scene textures",
                image.map_or(-1, |image| image as i64)
            );
        }
        DrawUpdate::Visibility { .. } => {}
    }
    Ok(())
}

/// Validate a batch against the registered instances and the resident draws.
///
/// `draws` yields every runtime draw with its instance so a joint transform
/// aimed at world-baked geometry is rejected before any member is applied.
fn validate_runtime_draw_batch<'d>(
    instance_ids: &HashSet<InstanceId>,
    texture_count: usize,
    draws: impl Iterator<Item = (InstanceId, &'d DrawPresentation)> + Clone,
    updates: &[RuntimeDrawUpdate<'_>],
) -> std::result::Result<(), RuntimeDrawBatchError> {
    for (index, update) in updates.iter().copied().enumerate() {
        validate_runtime_draw_update(instance_ids, texture_count, update)
            .map_err(|source| RuntimeDrawBatchError::new(index, source))?;
        if let DrawUpdate::JointTransform { target, .. } = update.update
            && let Some((_, draw)) = draws.clone().find(|(instance, draw)| {
                *instance == update.instance_id && draw.rejects_joint_transform(target)
            })
        {
            return Err(RuntimeDrawBatchError::new(
                index,
                anyhow::anyhow!(
                    "joint transform targets world-baked geometry under joint {:?} in runtime instance {}",
                    draw.identity.joint,
                    update.instance_id.get()
                ),
            ));
        }
    }
    Ok(())
}

/// Immutable HSD draw-pass classification, with a legacy inference fallback.
///
/// Material animation changes color but never this pass, its depth-write
/// policy, or its sort bucket.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DrawRenderClass {
    Opaque,
    TextureEdge,
    Translucent,
}

impl DrawRenderClass {
    fn from_mesh(mesh: &Mesh, textures: &[Texture]) -> Self {
        match mesh.material.render_mode.map(|mode| mode.class()) {
            Some(RenderModeClass::Opaque) => Self::Opaque,
            Some(RenderModeClass::TextureEdge) => Self::TextureEdge,
            Some(RenderModeClass::Translucent) => Self::Translucent,
            None => Self::infer(mesh, textures),
        }
    }

    fn infer(mesh: &Mesh, textures: &[Texture]) -> Self {
        let transparent = mesh
            .vertices
            .iter()
            .any(|vertex| vertex.color[3] * mesh.material.color[3] < 1.0)
            || mesh.material.texture.is_some_and(|index| {
                textures[index]
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel[3] < 255)
            });
        if transparent {
            Self::Translucent
        } else {
            Self::Opaque
        }
    }

    fn fallback_pixel_engine(self) -> PixelEngineState {
        let bits = match self {
            Self::Opaque => 0,
            Self::TextureEdge => 0x4000_0000,
            Self::Translucent => 0x6000_0000,
        };
        PixelEngineState::from_render_mode(
            RenderMode::from_bits(bits).expect("fixed fallback render mode is valid"),
        )
    }
}

fn compare_draw_order(a_class: DrawRenderClass, b_class: DrawRenderClass) -> std::cmp::Ordering {
    a_class.cmp(&b_class)
}

fn retain_draw(mesh: &Mesh) -> bool {
    !mesh.indices.is_empty() && mesh.material.cull_mode != CullMode::All
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PipelineKey {
    cull_mode: Option<wgpu::Face>,
    blend: Option<wgpu::BlendState>,
    depth_write: bool,
    depth_compare: wgpu::CompareFunction,
    write_mask: wgpu::ColorWrites,
}

impl PipelineKey {
    fn new(cull_mode: CullMode, state: PixelEngineState) -> Result<Self> {
        state.validate_supported()?;
        let write_mask = match (state.color_write, state.alpha_write) {
            (false, false) => wgpu::ColorWrites::empty(),
            (true, false) => wgpu::ColorWrites::COLOR,
            (false, true) => wgpu::ColorWrites::ALPHA,
            (true, true) => wgpu::ColorWrites::ALL,
        };
        let mut blend = gpu_blend_state(state.blend)?;
        if let Some(blend) = &mut blend {
            if !state.color_write {
                blend.color = wgpu::BlendComponent::REPLACE;
            }
            if !state.alpha_write {
                blend.alpha = wgpu::BlendComponent::REPLACE;
            }
        }
        if write_mask.is_empty() {
            blend = None;
        }
        Ok(Self {
            cull_mode: match cull_mode {
                CullMode::None | CullMode::All => None,
                CullMode::Front => Some(wgpu::Face::Front),
                CullMode::Back => Some(wgpu::Face::Back),
            },
            blend,
            depth_write: state.depth.write_enabled,
            depth_compare: if state.depth.test_enabled {
                gpu_compare(state.depth.comparison)
            } else {
                wgpu::CompareFunction::Always
            },
            write_mask,
        })
    }
}

fn gpu_compare(comparison: PeCompare) -> wgpu::CompareFunction {
    match comparison {
        PeCompare::Never => wgpu::CompareFunction::Never,
        PeCompare::Less => wgpu::CompareFunction::Less,
        PeCompare::Equal => wgpu::CompareFunction::Equal,
        PeCompare::LessEqual => wgpu::CompareFunction::LessEqual,
        PeCompare::Greater => wgpu::CompareFunction::Greater,
        PeCompare::NotEqual => wgpu::CompareFunction::NotEqual,
        PeCompare::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
        PeCompare::Always => wgpu::CompareFunction::Always,
    }
}

fn gpu_blend_factor(
    factor: PeBlendFactor,
    source_slot: bool,
    alpha_component: bool,
) -> wgpu::BlendFactor {
    match factor {
        PeBlendFactor::Zero => wgpu::BlendFactor::Zero,
        PeBlendFactor::One => wgpu::BlendFactor::One,
        PeBlendFactor::SourceColor if source_slot && alpha_component => wgpu::BlendFactor::DstAlpha,
        PeBlendFactor::SourceColor if source_slot => wgpu::BlendFactor::Dst,
        PeBlendFactor::InverseSourceColor if source_slot && alpha_component => {
            wgpu::BlendFactor::OneMinusDstAlpha
        }
        PeBlendFactor::InverseSourceColor if source_slot => wgpu::BlendFactor::OneMinusDst,
        PeBlendFactor::SourceColor if alpha_component => wgpu::BlendFactor::SrcAlpha,
        PeBlendFactor::SourceColor => wgpu::BlendFactor::Src,
        PeBlendFactor::InverseSourceColor if alpha_component => wgpu::BlendFactor::OneMinusSrcAlpha,
        PeBlendFactor::InverseSourceColor => wgpu::BlendFactor::OneMinusSrc,
        PeBlendFactor::SourceAlpha => wgpu::BlendFactor::SrcAlpha,
        PeBlendFactor::InverseSourceAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
        PeBlendFactor::DestinationAlpha => wgpu::BlendFactor::DstAlpha,
        PeBlendFactor::InverseDestinationAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
    }
}

fn gpu_blend_state(state: PeBlendState) -> Result<Option<wgpu::BlendState>> {
    let component = |alpha_component| wgpu::BlendComponent {
        src_factor: gpu_blend_factor(state.source_factor, true, alpha_component),
        dst_factor: gpu_blend_factor(state.destination_factor, false, alpha_component),
        operation: wgpu::BlendOperation::Add,
    };
    Ok(match state.mode {
        PeBlendMode::None => None,
        PeBlendMode::Blend => Some(wgpu::BlendState {
            color: component(false),
            alpha: component(true),
        }),
        PeBlendMode::Subtract => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::ReverseSubtract,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::ReverseSubtract,
            },
        }),
        PeBlendMode::Logic => bail!("GX logic blending is not supported"),
    })
}

fn create_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    key: PipelineKey,
) -> wgpu::RenderPipeline {
    let attributes =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("inspection mesh"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            })],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: key.cull_mode,
            front_face: wgpu::FrontFace::Ccw,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(key.depth_write),
            depth_compare: Some(key.depth_compare),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: key.blend,
                write_mask: key.write_mask,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// Per-runtime-draw GPU state: the joint transform plus material color and
/// GX alpha test, matching the shader's `Material` layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawUniform {
    model: [[f32; 4]; 4],
    uv_transform: [[f32; 4]; 4],
    color: [f32; 4],
    alpha_test: [u32; 4],
}

impl DrawUniform {
    fn new(
        state: &DrawState,
        alpha_test: PeAlphaTest,
        material_alpha_uses_hsd_byte_storage: bool,
    ) -> Self {
        let uv_transform = state
            .texture
            .uv_transform()
            .expect("draw texture transforms are validated at load and update time");
        let mut color = state.material_color;
        if material_alpha_uses_hsd_byte_storage {
            // HSD_SetMaterialColor stores the authored float alpha in an unsigned
            // byte before channel/TEV evaluation. Preserve that truncation boundary
            // separately from the preview shader's final TEV-output quantization.
            color[3] = f32::from((color[3] * 255.0) as u8) / 255.0;
        }
        Self {
            model: state.joint_transform,
            uv_transform,
            color,
            alpha_test: [
                u32::from(alpha_test.comparison0.code()),
                u32::from(alpha_test.reference0),
                u32::from(alpha_test.comparison1.code()),
                u32::from(alpha_test.reference1) | (u32::from(alpha_test.operation.code()) << 8),
            ],
        }
    }
}

/// Immutable GPU allocation and authored presentation defaults for one mesh.
///
/// Every runtime draw instance refers here by index, so cloning presentation
/// state never duplicates vertex/index buffers, textures, or pipelines.
struct DrawResource {
    initial_presentation: DrawPresentation,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    alpha_test: PeAlphaTest,
    pipeline: usize,
}

/// Independently mutable state for one runtime occurrence of a draw resource.
///
/// The material buffer and bind group are intentionally per-instance: unlike
/// geometry and texture bindings, their contents change during animation.
struct Draw {
    instance_id: InstanceId,
    resource: usize,
    presentation: DrawPresentation,
    material: wgpu::Buffer,
    material_binding: wgpu::BindGroup,
    /// Index into [`GpuScene::texture_bindings`] for the current image and wrap modes.
    texture_binding: usize,
}

struct GpuScene {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,
    pipelines: Vec<wgpu::RenderPipeline>,
    camera: wgpu::Buffer,
    camera_binding: wgpu::BindGroup,
    /// Scene textures followed by the white fallback.
    texture_views: Vec<wgpu::TextureView>,
    texture_layout: wgpu::BindGroupLayout,
    samplers: HashMap<[WrapMode; 2], wgpu::Sampler>,
    /// Texture/sampler bind groups created on demand per (image, wrap) pair.
    texture_bindings: Vec<wgpu::BindGroup>,
    texture_binding_indices: HashMap<(usize, [WrapMode; 2]), usize>,
    material_layout: wgpu::BindGroupLayout,
    draw_resources: Vec<DrawResource>,
    draws: Vec<Draw>,
    instance_ids: HashSet<InstanceId>,
    center: Vec3,
    radius: f32,
    source_camera: Option<Camera>,
    clear_color: wgpu::Color,
}

impl GpuScene {
    async fn new(
        adapter: &wgpu::Adapter,
        scene: &Scene,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Skirmish renderer"),
                ..Default::default()
            })
            .await
            .context("requesting graphics device")?;
        let limits = device.limits();
        for texture in &scene.textures {
            ensure!(
                texture.width > 0 && texture.height > 0,
                "empty texture {}",
                texture.name
            );
            ensure!(
                texture.width <= limits.max_texture_dimension_2d
                    && texture.height <= limits.max_texture_dimension_2d,
                "texture {} exceeds device dimensions",
                texture.name
            );
            ensure!(
                texture.rgba.len() as u64
                    == u64::from(texture.width) * u64::from(texture.height) * 4,
                "invalid RGBA length for {}",
                texture.name
            );
        }
        for mesh in &scene.meshes {
            ensure!(
                mesh.indices.len().is_multiple_of(3),
                "{} has incomplete triangles",
                mesh.name
            );
            ensure!(
                mesh.indices
                    .iter()
                    .all(|&i| (i as usize) < mesh.vertices.len()),
                "{} has out-of-range indices",
                mesh.name
            );
            ensure!(
                mesh.material
                    .texture
                    .is_none_or(|i| i < scene.textures.len()),
                "{} has invalid texture reference",
                mesh.name
            );
            ensure!(
                mesh.indices.len() <= u32::MAX as usize,
                "{} has too many indices",
                mesh.name
            );
            ensure!(
                std::mem::size_of_val(mesh.vertices.as_slice()) as u64 <= limits.max_buffer_size
                    && std::mem::size_of_val(mesh.indices.as_slice()) as u64
                        <= limits.max_buffer_size,
                "{} exceeds device buffer size",
                mesh.name
            );
        }
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(64),
                },
                count: None,
            }],
        });
        let camera_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("diffuse texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<DrawUniform>() as u64
                    ),
                },
                count: None,
            }],
        });
        let white = Texture {
            name: "white".into(),
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        };
        let texture_views = scene
            .textures
            .iter()
            .chain(std::iter::once(&white))
            .map(|source| {
                device
                    .create_texture_with_data(
                        &queue,
                        &wgpu::TextureDescriptor {
                            label: Some(&source.name),
                            size: extent(source.width, source.height),
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            usage: wgpu::TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        },
                        wgpu::util::TextureDataOrder::LayerMajor,
                        &source.rgba,
                    )
                    .create_view(&Default::default())
            })
            .collect();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WESL mesh + lighting"),
            source: wgpu::ShaderSource::Wgsl(MESH_SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh layout"),
            bind_group_layouts: &[
                Some(&camera_layout),
                Some(&texture_layout),
                Some(&material_layout),
            ],
            immediate_size: 0,
        });
        let mut pipelines = Vec::new();
        let mut pipeline_indices = HashMap::new();
        let mut draw_resources = Vec::new();
        for mesh in &scene.meshes {
            if !retain_draw(mesh) {
                continue;
            }
            let joint_world = mesh.joint.and_then(|joint| scene.joint_world(joint));
            let presentation = DrawPresentation::from_mesh(mesh, &scene.textures, joint_world)?;
            let pixel_engine = mesh.material.pixel_engine.unwrap_or_else(|| {
                mesh.material.render_mode.map_or_else(
                    || presentation.render_class.fallback_pixel_engine(),
                    PixelEngineState::from_render_mode,
                )
            });
            let pipeline_key = PipelineKey::new(mesh.material.cull_mode, pixel_engine)
                .with_context(|| format!("{} has unsupported pixel-engine state", mesh.name))?;
            let pipeline = if let Some(&index) = pipeline_indices.get(&pipeline_key) {
                index
            } else {
                let index = pipelines.len();
                pipelines.push(create_pipeline(
                    &device,
                    &layout,
                    &shader,
                    format,
                    pipeline_key,
                ));
                pipeline_indices.insert(pipeline_key, index);
                index
            };
            draw_resources.push(DrawResource {
                initial_presentation: presentation,
                vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&mesh.name),
                    contents: bytemuck::cast_slice(&mesh.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&mesh.name),
                    contents: bytemuck::cast_slice(&mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
                count: mesh.indices.len() as u32,
                alpha_test: pixel_engine.alpha_test,
                pipeline,
            });
        }
        let (low, high) = scene.bounds().unwrap_or(([-1.0; 3], [1.0; 3]));
        let low = Vec3::from(low);
        let high = Vec3::from(high);
        let center = low * 0.5 + high * 0.5;
        let radius = (high - center).length().max(0.1);
        ensure!(
            center.is_finite() && radius.is_finite(),
            "scene bounds exceed finite camera range"
        );
        if let Some(camera) = scene.camera {
            validate_camera(camera)?;
        }
        ensure!(
            scene
                .clear_color
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value)),
            "scene clear color must contain finite normalized components"
        );
        let mut gpu = Self {
            device,
            queue,
            adapter_name: adapter.get_info().name,
            pipelines,
            camera,
            camera_binding,
            texture_views,
            texture_layout,
            samplers: HashMap::new(),
            texture_bindings: Vec::new(),
            texture_binding_indices: HashMap::new(),
            material_layout,
            draw_resources,
            draws: Vec::new(),
            instance_ids: HashSet::new(),
            center,
            radius,
            source_camera: scene.camera,
            clear_color: wgpu::Color {
                r: f64::from(scene.clear_color[0]),
                g: f64::from(scene.clear_color[1]),
                b: f64::from(scene.clear_color[2]),
                a: f64::from(scene.clear_color[3]),
            },
        };
        gpu.instantiate_draws(DEFAULT_INSTANCE_ID)?;
        if let Some(error) = scope.pop().await {
            bail!("creating graphics resources: {error}");
        }
        Ok(gpu)
    }

    fn instantiate_draws(&mut self, instance_id: InstanceId) -> Result<usize> {
        self.instantiate_draw_resources(instance_id, 0..self.draw_resources.len())
    }

    /// Bind group for one scene texture (or the white fallback) under GX wrap modes.
    fn texture_binding(&mut self, image: Option<usize>, wrap: [WrapMode; 2]) -> usize {
        let white = self.texture_views.len() - 1;
        let image = image.unwrap_or(white);
        if let Some(&index) = self.texture_binding_indices.get(&(image, wrap)) {
            return index;
        }
        let address = |mode: WrapMode| match mode {
            WrapMode::Clamp => wgpu::AddressMode::ClampToEdge,
            WrapMode::Repeat => wgpu::AddressMode::Repeat,
            WrapMode::Mirror => wgpu::AddressMode::MirrorRepeat,
        };
        let sampler = self.samplers.entry(wrap).or_insert_with(|| {
            self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("preview linear"),
                address_mode_u: address(wrap[0]),
                address_mode_v: address(wrap[1]),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            })
        });
        let binding = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw texture"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.texture_views[image]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        let index = self.texture_bindings.len();
        self.texture_bindings.push(binding);
        self.texture_binding_indices.insert((image, wrap), index);
        index
    }

    /// Clone selected authored draws into one independently mutable runtime set.
    ///
    /// Keeping selection in this internal path lets a later presentation bridge
    /// instantiate one bound hierarchy without re-uploading immutable geometry.
    fn instantiate_draw_resources(
        &mut self,
        instance_id: InstanceId,
        resource_indices: impl IntoIterator<Item = usize>,
    ) -> Result<usize> {
        ensure_new_runtime_instance(&self.instance_ids, instance_id)?;
        let resource_indices = resource_indices.into_iter().collect::<Vec<_>>();
        let mut unique = HashSet::with_capacity(resource_indices.len());
        for &resource in &resource_indices {
            ensure!(
                resource < self.draw_resources.len(),
                "draw resource index {resource} is out of range"
            );
            ensure!(
                unique.insert(resource),
                "draw resource index {resource} occurs more than once in one runtime instance"
            );
        }

        let mut draws = Vec::with_capacity(resource_indices.len());
        for resource in resource_indices {
            let texture_binding = {
                let texture = self.draw_resources[resource]
                    .initial_presentation
                    .state
                    .texture;
                self.texture_binding(texture.image, texture.wrap)
            };
            let draw = {
                let source = &self.draw_resources[resource];
                let presentation = source.initial_presentation.clone();
                let uniform = DrawUniform::new(
                    &presentation.state,
                    source.alpha_test,
                    presentation.material_alpha_uses_hsd_byte_storage(),
                );
                let material = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("draw instance material"),
                        contents: bytemuck::bytes_of(&uniform),
                        usage: wgpu::BufferUsages::UNIFORM
                            | wgpu::BufferUsages::COPY_DST
                            | wgpu::BufferUsages::COPY_SRC,
                    });
                let material_binding = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("draw instance material"),
                    layout: &self.material_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: material.as_entire_binding(),
                    }],
                });
                Draw {
                    instance_id,
                    resource,
                    presentation,
                    material,
                    material_binding,
                    texture_binding,
                }
            };
            draws.push(draw);
        }
        let count = draws.len();
        self.draws.extend(draws);
        self.instance_ids.insert(instance_id);
        Ok(count)
    }

    fn update_draws(&mut self, update: DrawUpdate<'_>) -> Result<usize> {
        self.update_instance_draws(RuntimeDrawUpdate {
            instance_id: DEFAULT_INSTANCE_ID,
            update,
        })
    }

    fn update_instance_draws(&mut self, update: RuntimeDrawUpdate<'_>) -> Result<usize> {
        let counts = self.update_instance_draws_batch(std::slice::from_ref(&update))?;
        Ok(counts[0])
    }

    fn update_instance_draws_batch(
        &mut self,
        updates: &[RuntimeDrawUpdate<'_>],
    ) -> std::result::Result<Vec<usize>, RuntimeDrawBatchError> {
        validate_runtime_draw_batch(
            &self.instance_ids,
            self.texture_views.len() - 1,
            self.draws
                .iter()
                .map(|draw| (draw.instance_id, &draw.presentation)),
            updates,
        )?;
        Ok(updates
            .iter()
            .copied()
            .map(|update| self.apply_instance_draw_update(update))
            .collect())
    }

    fn apply_instance_draw_update(&mut self, update: RuntimeDrawUpdate<'_>) -> usize {
        let mut matched = Vec::new();
        for (index, draw) in self.draws.iter_mut().enumerate() {
            if update_runtime_presentation(draw.instance_id, &mut draw.presentation, update) {
                matched.push(index);
            }
        }
        if matches!(update.update, DrawUpdate::Visibility { .. }) {
            return matched.len();
        }
        for &index in &matched {
            if matches!(update.update, DrawUpdate::Texture { .. }) {
                let texture = self.draws[index].presentation.state.texture;
                self.draws[index].texture_binding =
                    self.texture_binding(texture.image, texture.wrap);
            }
            let draw = &self.draws[index];
            let source = &self.draw_resources[draw.resource];
            self.queue.write_buffer(
                &draw.material,
                0,
                bytemuck::bytes_of(&DrawUniform::new(
                    &draw.presentation.state,
                    source.alpha_test,
                    draw.presentation.material_alpha_uses_hsd_byte_storage(),
                )),
            );
        }
        matched.len()
    }

    fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        dimensions: [u32; 2],
        orbit: [f32; 3],
    ) {
        let (view, projection) = if let Some(camera) = self.source_camera {
            (
                glam::camera::rh::view::look_at_mat4(
                    Vec3::from(camera.eye),
                    Vec3::from(camera.interest),
                    Vec3::from(camera.up),
                ),
                glam::camera::rh::proj::directx::perspective(
                    camera.vertical_fov_radians,
                    camera.aspect,
                    camera.near,
                    camera.far,
                ),
            )
        } else {
            let [yaw, pitch, zoom] = orbit;
            let yaw = 0.55 + yaw;
            let pitch = (0.25 + pitch).clamp(-1.45, 1.45);
            let aspect = dimensions[0] as f32 / dimensions[1] as f32;
            let half_fov = 22.5_f32.to_radians();
            let limiting_fov = half_fov.min((half_fov.tan() * aspect).atan());
            let distance = self.radius / limiting_fov.sin() * 1.15 * zoom.clamp(0.2, 5.0);
            let direction = Vec3::new(
                yaw.sin() * pitch.cos(),
                pitch.sin(),
                yaw.cos() * pitch.cos(),
            );
            let eye = self.center + direction * distance;
            (
                glam::camera::rh::view::look_at_mat4(eye, self.center, Vec3::Y),
                glam::camera::rh::proj::directx::perspective(
                    half_fov * 2.0,
                    aspect,
                    self.radius * 0.001,
                    distance + self.radius * 3.0,
                ),
            )
        };
        self.queue.write_buffer(
            &self.camera,
            0,
            bytemuck::cast_slice(&(projection * view).to_cols_array()),
        );
        let mut order: Vec<_> = self
            .draws
            .iter()
            .filter(|draw| draw.presentation.state.visible)
            .collect();
        order.sort_by(|a, b| {
            compare_draw_order(
                self.draw_resources[a.resource]
                    .initial_presentation
                    .render_class,
                self.draw_resources[b.resource]
                    .initial_presentation
                    .render_class,
            )
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        if let Some(camera) = self.source_camera {
            let [x, y, width, height] = fitted_viewport(dimensions, camera.aspect)
                .expect("validated camera aspect and nonzero render target");
            pass.set_viewport(x, y, width, height, 0.0, 1.0);
        }
        pass.set_bind_group(0, &self.camera_binding, &[]);
        for draw in order {
            let resource = &self.draw_resources[draw.resource];
            pass.set_pipeline(&self.pipelines[resource.pipeline]);
            pass.set_bind_group(1, &self.texture_bindings[draw.texture_binding], &[]);
            pass.set_bind_group(2, &draw.material_binding, &[]);
            pass.set_vertex_buffer(0, resource.vertices.slice(..));
            pass.set_index_buffer(resource.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..resource.count, 0, 0..1);
        }
    }
}

fn validate_camera(camera: Camera) -> Result<()> {
    let projection = [
        camera.vertical_fov_radians,
        camera.aspect,
        camera.near,
        camera.far,
    ];
    let scalars = [
        camera.eye.as_slice(),
        camera.interest.as_slice(),
        camera.up.as_slice(),
        projection.as_slice(),
    ];
    ensure!(
        scalars.into_iter().flatten().all(|value| value.is_finite()),
        "source camera contains nonfinite values"
    );
    ensure!(
        camera.vertical_fov_radians > 0.0 && camera.vertical_fov_radians < std::f32::consts::PI,
        "source camera field of view must be between zero and pi"
    );
    ensure!(
        camera.aspect > 0.0 && camera.near > 0.0 && camera.far > camera.near,
        "source camera has an invalid aspect or clip range"
    );
    let direction = Vec3::from(camera.interest) - Vec3::from(camera.eye);
    let up = Vec3::from(camera.up);
    ensure!(
        direction.length_squared() > f32::EPSILON
            && up.length_squared() > f32::EPSILON
            && direction.cross(up).length_squared() > f32::EPSILON,
        "source camera view direction and up vector must define a basis"
    );
    Ok(())
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

fn depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: extent(width, height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

async fn request_adapter(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter> {
    instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface,
            force_fallback_adapter: false,
            ..Default::default()
        })
        .await
        .context("no compatible graphics adapter (a Vulkan/Metal/DX12/GLES device is required)")
}

/// Owns an SDL window and surface, confined to their creating thread.
pub struct WindowRenderer {
    presentation: SdlSurface,
    config: wgpu::SurfaceConfiguration,
    depth: wgpu::TextureView,
    gpu: GpuScene,
    suspended: bool,
}

impl WindowRenderer {
    pub async fn new(window: Window, scene: &Scene) -> Result<Self> {
        let presentation = SdlSurface::new(window)?;
        let surface = presentation.surface();
        let adapter = request_adapter(presentation.instance(), Some(surface)).await?;
        let (width, height) = presentation.window().size_in_pixels();
        let mut config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .context("surface has no supported configuration")?;
        config.format = surface
            .get_capabilities(&adapter)
            .formats
            .into_iter()
            .find(wgpu::TextureFormat::is_srgb)
            .context("surface has no sRGB format")?;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        let gpu = GpuScene::new(&adapter, scene, config.format).await?;
        config.width = config
            .width
            .min(gpu.device.limits().max_texture_dimension_2d);
        config.height = config
            .height
            .min(gpu.device.limits().max_texture_dimension_2d);
        surface.configure(&gpu.device, &config);
        let depth = depth_view(&gpu.device, config.width, config.height);
        Ok(Self {
            presentation,
            config,
            depth,
            gpu,
            suspended: width == 0 || height == 0,
        })
    }

    pub fn adapter_name(&self) -> &str {
        &self.gpu.adapter_name
    }

    pub fn window_id(&self) -> u32 {
        self.presentation.window().id()
    }

    pub fn pixel_size(&self) -> (u32, u32) {
        self.presentation.window().size_in_pixels()
    }

    /// Snapshot the mapping from SDL window coordinates into an authored canvas.
    ///
    /// Pointer adapters should request this after resize and display-change events
    /// so they use the exact surface extent and containment viewport used to draw.
    pub fn presentation_transform(
        &self,
        authored_extent: [f32; 2],
    ) -> Option<PresentationTransform> {
        let (window_width, window_height) = self.presentation.window().size();
        PresentationTransform::new(
            [window_width, window_height],
            [self.config.width, self.config.height],
            authored_extent,
        )
    }

    /// Clone every retained authored draw under a new runtime identity.
    ///
    /// Immutable geometry, textures, and pipelines remain shared. Each clone
    /// receives independent visibility, material state, and a mutable uniform.
    /// This bounded API does not yet add an instance transform, so simultaneous
    /// visible whole-scene clones occupy the same authored location.
    pub fn instantiate_draws(&mut self, instance_id: InstanceId) -> Result<usize> {
        self.gpu.instantiate_draws(instance_id)
    }

    /// Applies one source-targeted update to exactly one runtime draw instance.
    ///
    /// Visibility uses an exported joint/part selector and may fan out to every
    /// attached material within that instance. Color requires an exact source
    /// MObj identity. The returned count exposes missing or intentionally
    /// grouped draw parts; immutable geometry and textures remain resident.
    pub fn update_instance_draws(&mut self, update: RuntimeDrawUpdate<'_>) -> Result<usize> {
        self.gpu.update_instance_draws(update)
    }

    /// Applies a complete renderer-facing presentation batch after validating
    /// every runtime instance and material color.
    ///
    /// No draw state changes if validation fails. The returned vector retains
    /// one exact match count per requested update, including zero matches.
    pub fn update_instance_draws_batch(
        &mut self,
        updates: &[RuntimeDrawUpdate<'_>],
    ) -> std::result::Result<Vec<usize>, RuntimeDrawBatchError> {
        self.gpu.update_instance_draws_batch(updates)
    }

    /// Compatibility update for the original one-scene renderer.
    ///
    /// This always targets [`DEFAULT_INSTANCE_ID`]. New multi-instance callers
    /// should use [`Self::update_instance_draws`] so runtime scope is explicit.
    pub fn update_draws(&mut self, update: DrawUpdate<'_>) -> Result<usize> {
        self.gpu.update_draws(update)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.suspended = width == 0 || height == 0;
        if self.suspended {
            return;
        }
        let max = self.gpu.device.limits().max_texture_dimension_2d;
        self.config.width = width.min(max);
        self.config.height = height.min(max);
        self.presentation
            .surface()
            .configure(&self.gpu.device, &self.config);
        self.depth = depth_view(&self.gpu.device, self.config.width, self.config.height);
    }

    /// Returns false when presentation must be retried after a transient surface event.
    pub fn render(&mut self, yaw: f32, pitch: f32, zoom: f32) -> Result<bool> {
        ensure!(
            [yaw, pitch, zoom].iter().all(|n| n.is_finite()) && zoom > 0.0,
            "invalid camera orbit"
        );
        if self.suspended {
            return Ok(false);
        }
        let (frame, reconfigure) = match self.presentation.surface().get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.presentation
                    .surface()
                    .configure(&self.gpu.device, &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.presentation.recreate()?;
                self.presentation
                    .surface()
                    .configure(&self.gpu.device, &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => bail!("graphics surface validation failed"),
        };
        let target = frame.texture.create_view(&Default::default());
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        self.gpu.draw(
            &mut encoder,
            &target,
            &self.depth,
            [self.config.width, self.config.height],
            [yaw, pitch, zoom],
        );
        self.gpu.queue.submit([encoder.finish()]);
        self.gpu.queue.present(frame);
        if reconfigure {
            self.presentation
                .surface()
                .configure(&self.gpu.device, &self.config);
        }
        // A replacement swapchain needs its own frame after a suboptimal first
        // surface texture.
        Ok(!reconfigure)
    }
}

/// Render with the same shaders and draw path, without opening a window or audio device.
pub fn render_headless(scene: &Scene, width: u32, height: u32, output: &Path) -> Result<()> {
    ensure!(
        (1..=8192).contains(&width) && (1..=8192).contains(&height),
        "capture dimensions must be 1..=8192"
    );
    let rgba = pollster::block_on(render_rgba(scene, width, height))?;
    let file = File::create(output).with_context(|| format!("creating {}", output.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&rgba)?;
    writer.finish()?;
    Ok(())
}

async fn render_rgba(scene: &Scene, width: u32, height: u32) -> Result<Vec<u8>> {
    render_rgba_with_updates(scene, width, height, &[]).await
}

async fn render_rgba_with_updates(
    scene: &Scene,
    width: u32,
    height: u32,
    updates: &[DrawUpdate<'_>],
) -> Result<Vec<u8>> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = request_adapter(&instance, None).await?;
    let mut gpu = GpuScene::new(&adapter, scene, wgpu::TextureFormat::Rgba8UnormSrgb).await?;
    for &update in updates {
        gpu.update_draws(update)?;
    }
    capture_gpu_rgba(&gpu, width, height).await
}

/// Render and read back one frame from an already-created GPU scene.
///
/// Keeping capture separate from construction lets tests exercise the same
/// queue and resident resources across presentation updates.
async fn capture_gpu_rgba(gpu: &GpuScene, width: u32, height: u32) -> Result<Vec<u8>> {
    ensure!(
        width <= gpu.device.limits().max_texture_dimension_2d
            && height <= gpu.device.limits().max_texture_dimension_2d,
        "capture exceeds device dimensions"
    );
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen color"),
        size: extent(width, height),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let row_bytes = width * 4;
    let padded_row_bytes =
        row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture readback"),
        size: u64::from(padded_row_bytes) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    gpu.draw(
        &mut encoder,
        &texture.create_view(&Default::default()),
        &depth_view(&gpu.device, width, height),
        [width, height],
        [0.0, 0.0, 1.0],
    );
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(height),
            },
        },
        extent(width, height),
    );
    let submission = gpu.queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    gpu.device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: Some(Duration::from_secs(30)),
    })?;
    receiver
        .recv_timeout(Duration::from_secs(30))
        .context("waiting for GPU capture")??;
    if let Some(error) = scope.pop().await {
        bail!("offscreen rendering failed: {error}");
    }
    let mapped = buffer.slice(..).get_mapped_range()?;
    let mut rgba = Vec::with_capacity((row_bytes * height) as usize);
    for row in mapped.chunks_exact(padded_row_bytes as usize) {
        rgba.extend_from_slice(&row[..row_bytes as usize]);
    }
    drop(mapped);
    buffer.unmap();
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::super::scene::{PeAlphaOp, RenderMode, TextureSourceId, VisualResourceId};
    use super::*;

    // Some host Vulkan loaders are not safe to initialize twice in parallel.
    // The ignored adapter tests are opt-in, but must still be reliable when a
    // caller selects both with the default multi-threaded Rust test harness.
    static GPU_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    async fn read_material_uniform(gpu: &GpuScene, draw_index: usize) -> Result<DrawUniform> {
        let size = std::mem::size_of::<DrawUniform>() as u64;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("material test readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&gpu.draws[draw_index].material, 0, &buffer, 0, size);
        let submission = gpu.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        gpu.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })?;
        receiver
            .recv_timeout(Duration::from_secs(30))
            .context("waiting for material test readback")??;
        let mapped = buffer.slice(..).get_mapped_range()?;
        let uniform = *bytemuck::from_bytes::<DrawUniform>(&mapped);
        drop(mapped);
        buffer.unmap();
        Ok(uniform)
    }

    fn exported_mesh(
        joint: u32,
        instance_id: Option<&str>,
        hidden: bool,
        material_color: [f32; 4],
    ) -> Mesh {
        let mut mesh = Scene::demo().meshes.remove(0);
        mesh.joint = Some(joint);
        mesh.instance_id = instance_id.map(str::to_owned);
        mesh.hidden = hidden;
        mesh.material.color = material_color;
        mesh
    }

    fn exported_presentation(mesh: &Mesh) -> DrawPresentation {
        DrawPresentation::from_mesh(mesh, &Scene::demo().textures, None).unwrap()
    }

    /// A translated column-major matrix in the exporter's 16-float layout.
    fn translation_matrix(x: f32, y: f32, z: f32) -> [f32; 16] {
        [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, z, 1.0,
        ]
    }

    fn posed_joint(offset: u32, world: [f32; 16]) -> super::super::scene::Joint {
        super::super::scene::Joint {
            name: format!("joint_{offset}"),
            offset,
            parent: None,
            pose: Some(super::super::scene::JointPose {
                flags: 0,
                local: world,
                world,
                inverse_bind: translation_matrix(0.0, 0.0, 0.0),
            }),
        }
    }

    fn exact_exported_mesh(
        resource_id: &str,
        joint: u32,
        dobj_index: u16,
        material_offset: u32,
    ) -> Mesh {
        let mut mesh = exported_mesh(joint, None, true, [1.0; 4]);
        let owner_joint = VisualJointOccurrence {
            resource_id: VisualResourceId::for_test(resource_id),
            visual_offset: joint,
        };
        let owner_dobj = VisualDObjOccurrence {
            owner_joint,
            dobj_index,
        };
        let visual_offset = MaterialSourceId::new(material_offset);
        mesh.source_occurrence = Some(owner_dobj.clone());
        mesh.material.source_id = Some(visual_offset);
        mesh.material.source_occurrence = Some(VisualMaterialOccurrence {
            owner_dobj,
            visual_offset,
        });
        mesh
    }

    fn solid_quad_scene(color: [f32; 4]) -> Scene {
        let vertices = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ]
        .into_iter()
        .map(|position| Vertex {
            position,
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
            color: [1.0; 4],
        })
        .collect();
        Scene {
            source: None,
            resources: Vec::new(),
            joints: Vec::new(),
            meshes: vec![Mesh {
                geometry_space: GeometrySpace::World,
                name: "solid quad".into(),
                joint: Some(7),
                instance_id: Some("quad".into()),
                source_occurrence: None,
                vertices,
                indices: vec![0, 1, 2, 0, 2, 3],
                material: super::super::scene::Material {
                    source_id: Some(MaterialSourceId::new(100)),
                    source_occurrence: None,
                    texture_sources: Vec::new(),
                    render_mode: None,
                    pixel_engine: None,
                    color,
                    texture: None,
                    cull_mode: CullMode::None,
                },
                hidden: false,
            }],
            textures: Vec::new(),
            image_textures: HashMap::new(),
            warnings: Vec::new(),
            camera: Some(Camera {
                eye: [0.0, 0.0, 5.0],
                interest: [0.0, 0.0, 0.0],
                up: [0.0, 1.0, 0.0],
                vertical_fov_radians: 45.0_f32.to_radians(),
                aspect: 1.0,
                near: 0.1,
                far: 10.0,
            }),
            clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn linked_wesl_is_valid_for_the_rendering_backend() {
        assert!(
            MESH_SHADER.contains("var<uniform> material"),
            "compiled mesh shader must consume the dynamic material uniform"
        );
        assert!(
            MESH_SHADER.contains("material.model"),
            "compiled mesh shader must apply the per-draw joint transform"
        );
        let module = wgpu::naga::front::wgsl::parse_str(MESH_SHADER).expect("valid generated WGSL");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("shader validates without optional GPU capabilities");
        assert_eq!(std::mem::size_of::<DrawUniform>(), 160);
        assert!(
            MESH_SHADER.contains("material.uv_transform"),
            "compiled mesh shader must apply the per-draw texture matrix"
        );
    }

    #[test]
    fn gx_blend_factors_map_by_operand_slot_and_component() {
        let cases = [
            (PeBlendFactor::Zero, [wgpu::BlendFactor::Zero; 4]),
            (PeBlendFactor::One, [wgpu::BlendFactor::One; 4]),
            (
                PeBlendFactor::SourceColor,
                [
                    wgpu::BlendFactor::Dst,
                    wgpu::BlendFactor::DstAlpha,
                    wgpu::BlendFactor::Src,
                    wgpu::BlendFactor::SrcAlpha,
                ],
            ),
            (
                PeBlendFactor::InverseSourceColor,
                [
                    wgpu::BlendFactor::OneMinusDst,
                    wgpu::BlendFactor::OneMinusDstAlpha,
                    wgpu::BlendFactor::OneMinusSrc,
                    wgpu::BlendFactor::OneMinusSrcAlpha,
                ],
            ),
            (PeBlendFactor::SourceAlpha, [wgpu::BlendFactor::SrcAlpha; 4]),
            (
                PeBlendFactor::InverseSourceAlpha,
                [wgpu::BlendFactor::OneMinusSrcAlpha; 4],
            ),
            (
                PeBlendFactor::DestinationAlpha,
                [wgpu::BlendFactor::DstAlpha; 4],
            ),
            (
                PeBlendFactor::InverseDestinationAlpha,
                [wgpu::BlendFactor::OneMinusDstAlpha; 4],
            ),
        ];
        for (
            factor,
            [
                source_color,
                source_alpha,
                destination_color,
                destination_alpha,
            ],
        ) in cases
        {
            assert_eq!(gpu_blend_factor(factor, true, false), source_color);
            assert_eq!(gpu_blend_factor(factor, true, true), source_alpha);
            assert_eq!(gpu_blend_factor(factor, false, false), destination_color);
            assert_eq!(gpu_blend_factor(factor, false, true), destination_alpha);
        }
    }

    #[test]
    fn pipeline_keys_deduplicate_only_immutable_gpu_state() {
        let mode = RenderMode::from_bits(0x6000_0000).unwrap();
        let standard = PixelEngineState::from_render_mode(mode);
        let mut metadata_only = standard;
        metadata_only.explicit_descriptor = true;
        metadata_only.depth.compare_before_texture = false;
        metadata_only.alpha_test = PeAlphaTest {
            comparison0: PeCompare::GreaterEqual,
            reference0: 102,
            operation: PeAlphaOp::And,
            comparison1: PeCompare::LessEqual,
            reference1: 255,
        };
        let standard_key = PipelineKey::new(CullMode::Back, standard).unwrap();
        assert_eq!(
            standard_key,
            PipelineKey::new(CullMode::Back, metadata_only).unwrap(),
            "alpha-test controls and equivalent early/late depth metadata stay per draw"
        );
        assert_eq!(standard_key.write_mask, wgpu::ColorWrites::COLOR);
        let standard_blend = standard_key.blend.unwrap();
        assert_eq!(standard_blend.color.src_factor, wgpu::BlendFactor::SrcAlpha);
        assert_eq!(
            standard_blend.color.dst_factor,
            wgpu::BlendFactor::OneMinusSrcAlpha
        );
        assert_eq!(standard_blend.alpha, wgpu::BlendComponent::REPLACE);

        let mut additive = standard;
        additive.blend.destination_factor = PeBlendFactor::One;
        let mut keys = std::collections::HashSet::new();
        keys.insert(standard_key);
        keys.insert(PipelineKey::new(CullMode::Back, standard).unwrap());
        keys.insert(PipelineKey::new(CullMode::Back, additive).unwrap());
        keys.insert(PipelineKey::new(CullMode::None, additive).unwrap());
        assert_eq!(keys.len(), 3, "the menu PE census has three GPU keys");

        let mut subtract = standard;
        subtract.blend.mode = PeBlendMode::Subtract;
        subtract.blend.source_factor = PeBlendFactor::Zero;
        subtract.blend.destination_factor = PeBlendFactor::InverseDestinationAlpha;
        let subtract = PipelineKey::new(CullMode::Back, subtract)
            .unwrap()
            .blend
            .unwrap();
        assert_eq!(
            subtract.color,
            wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::ReverseSubtract,
            }
        );

        let mut depth_disabled_a = standard;
        depth_disabled_a.depth.test_enabled = false;
        depth_disabled_a.depth.comparison = PeCompare::Never;
        let mut depth_disabled_b = depth_disabled_a;
        depth_disabled_b.depth.comparison = PeCompare::Greater;
        assert_eq!(
            PipelineKey::new(CullMode::Back, depth_disabled_a).unwrap(),
            PipelineKey::new(CullMode::Back, depth_disabled_b).unwrap()
        );

        let opaque_mode = RenderMode::from_bits(0).unwrap();
        let opaque_a = PixelEngineState::from_render_mode(opaque_mode);
        let mut opaque_b = opaque_a;
        opaque_b.blend.source_factor = PeBlendFactor::Zero;
        opaque_b.blend.destination_factor = PeBlendFactor::One;
        assert_eq!(
            PipelineKey::new(CullMode::Back, opaque_a).unwrap(),
            PipelineKey::new(CullMode::Back, opaque_b).unwrap(),
            "disabled blend factors must not produce extra pipelines"
        );
    }

    #[test]
    fn material_uniform_packs_gx_alpha_test_and_truncates_material_alpha() {
        let alpha_test = PeAlphaTest {
            comparison0: PeCompare::GreaterEqual,
            reference0: 102,
            operation: PeAlphaOp::Xnor,
            comparison1: PeCompare::LessEqual,
            reference1: 255,
        };
        let state = |material_color| DrawState {
            visible: true,
            material_color,
            joint_transform: transform_columns(translation_matrix(1.0, 2.0, 3.0)),
            texture: DrawTextureState {
                image: None,
                wrap: [WrapMode::Clamp; 2],
                transform: TextureTransform {
                    rotation: [0.0; 3],
                    scale: [1.0; 3],
                    translation: [0.5, -0.25, 0.0],
                    repeat: [2, 1],
                    wrap_t: WrapMode::Clamp,
                },
            },
        };
        let uniform = DrawUniform::new(&state([0.25, 0.5, 0.75, 1.0]), alpha_test, true);
        assert_eq!(uniform.color, [0.25, 0.5, 0.75, 1.0]);
        assert_eq!(uniform.alpha_test, [6, 102, 3, 255 | (3 << 8)]);
        assert_eq!(uniform.model[3], [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(uniform.model[0], [1.0, 0.0, 0.0, 0.0]);
        // HSD: s' = repeat_s / scale * (s - translation) = 2s - 1; t' = t + 0.25.
        assert_eq!(uniform.uv_transform[0], [2.0, 0.0, 0.0, 0.0]);
        assert_eq!(uniform.uv_transform[1], [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(uniform.uv_transform[3], [-1.0, 0.25, 0.0, 1.0]);

        let half_alpha = DrawUniform::new(&state([1.0, 1.0, 1.0, 0.5]), alpha_test, true);
        assert_eq!(half_alpha.color[3], 127.0 / 255.0);

        let legacy_half_alpha = DrawUniform::new(&state([1.0, 1.0, 1.0, 0.5]), alpha_test, false);
        assert_eq!(legacy_half_alpha.color[3], 0.5);
    }

    #[test]
    fn draw_defaults_preserve_export_visibility_and_material_color() {
        let color = [0.25, 0.5, 0.75, 0.125];
        let mesh = exported_mesh(0x1234, Some("cursor-2"), true, color);
        let presentation = exported_presentation(&mesh);

        assert!(
            retain_draw(&mesh),
            "hidden geometry must remain GPU-resident"
        );
        assert_eq!(
            presentation.identity,
            ExportDrawIdentity {
                joint: Some(0x1234),
                export_instance_id: Some("cursor-2".into()),
                source_occurrence: None,
                material_source_id: None,
                material_source_occurrence: None,
                texture_source_occurrence: None,
            }
        );
        assert_eq!(
            presentation.state,
            DrawState {
                visible: false,
                material_color: color,
                joint_transform: IDENTITY_TRANSFORM,
                texture: DrawTextureState {
                    image: Some(0),
                    wrap: [WrapMode::Repeat; 2],
                    transform: TextureTransform {
                        rotation: [0.0; 3],
                        scale: [1.0; 3],
                        translation: [0.0; 3],
                        repeat: [1, 1],
                        wrap_t: WrapMode::Repeat,
                    },
                },
            }
        );
    }

    #[test]
    fn texture_updates_reach_only_the_exact_first_stage_occurrence() {
        let textures = Scene::demo().textures;
        let mut mesh = exact_exported_mesh("resource", 7, 0, 100);
        let material = mesh.material.source_occurrence.clone().unwrap();
        let stage_occurrence = |tobj_index| VisualTextureOccurrence {
            owner_material: material.clone(),
            tobj_index,
            visual_offset: TextureSourceId::new(0x480 + u32::from(tobj_index) * 0x60),
        };
        let stage = |tobj_index| super::super::scene::VisualTextureStageSource {
            visual_offset: Some(TextureSourceId::new(0x480)),
            occurrence: Some(stage_occurrence(tobj_index)),
            image_descriptor: Some(0x500),
            texture: Some(0),
            wrap: [WrapMode::Clamp, WrapMode::Mirror],
            transform: TextureTransform {
                rotation: [0.0; 3],
                scale: [1.0, 1.0, 1.0],
                translation: [0.0; 3],
                repeat: [1, 1],
                wrap_t: WrapMode::Mirror,
            },
        };
        mesh.material.texture_sources = vec![stage(0), stage(1)];
        let mut draw = DrawPresentation::from_mesh(&mesh, &textures, None).unwrap();
        assert_eq!(
            draw.identity.texture_source_occurrence,
            Some(stage_occurrence(0))
        );
        assert_eq!(draw.state.texture.wrap, [WrapMode::Clamp, WrapMode::Mirror]);

        let first = stage_occurrence(0);
        let second = stage_occurrence(1);
        assert!(!draw.update(DrawUpdate::Texture {
            target: ExportTextureSelector::Exact(&second),
            image: Some(3),
            translation: [0.0; 2],
            scale: [1.0; 2],
        }));
        assert!(draw.update(DrawUpdate::Texture {
            target: ExportTextureSelector::Exact(&first),
            image: Some(3),
            translation: [0.25, 0.5],
            scale: [2.0, 4.0],
        }));
        assert_eq!(draw.state.texture.image, Some(3));
        assert_eq!(draw.state.texture.transform.translation, [0.25, 0.5, 0.0]);
        assert_eq!(draw.state.texture.transform.scale, [2.0, 4.0, 1.0]);
        assert_eq!(draw.state.texture.transform.wrap_t, WrapMode::Mirror);

        let instance = InstanceId::new(9);
        let instance_ids = HashSet::from([instance]);
        let out_of_range = RuntimeDrawUpdate {
            instance_id: instance,
            update: DrawUpdate::Texture {
                target: ExportTextureSelector::Exact(&first),
                image: Some(2),
                translation: [0.0; 2],
                scale: [1.0; 2],
            },
        };
        let error =
            validate_runtime_draw_batch(&instance_ids, 2, std::iter::empty(), &[out_of_range])
                .unwrap_err();
        assert!(
            error.to_string().contains("outside the 2 resident"),
            "{error}"
        );
        let non_finite = RuntimeDrawUpdate {
            instance_id: instance,
            update: DrawUpdate::Texture {
                target: ExportTextureSelector::Exact(&first),
                image: None,
                translation: [f32::INFINITY, 0.0],
                scale: [1.0; 2],
            },
        };
        assert!(
            validate_runtime_draw_batch(&instance_ids, 2, std::iter::empty(), &[non_finite])
                .unwrap_err()
                .to_string()
                .contains("finite components")
        );
    }

    #[test]
    fn joint_local_draws_start_at_their_serialized_joint_and_only_they_accept_transforms() {
        let world = translation_matrix(4.0, -2.0, 0.5);
        let textures = Scene::demo().textures;
        let mut local = exported_mesh(7, None, false, [1.0; 4]);
        local.geometry_space = GeometrySpace::JointLocal;
        assert!(
            DrawPresentation::from_mesh(&local, &textures, None)
                .unwrap_err()
                .to_string()
                .contains("no serialized world matrix")
        );
        let mut local = DrawPresentation::from_mesh(&local, &textures, Some(world)).unwrap();
        assert_eq!(local.state.joint_transform, transform_columns(world));
        assert_eq!(local.state.joint_transform[3], [4.0, -2.0, 0.5, 1.0]);

        let baked = exported_mesh(7, None, false, [1.0; 4]);
        let mut baked = DrawPresentation::from_mesh(&baked, &textures, Some(world)).unwrap();
        assert_eq!(baked.state.joint_transform, IDENTITY_TRANSFORM);

        let selector = ExportDrawSelector::Legacy {
            joint: 7,
            instance_id: None,
        };
        let moved = transform_columns(translation_matrix(0.0, 9.0, 0.0));
        let update = DrawUpdate::JointTransform {
            target: selector,
            world: moved,
        };
        assert!(local.update(update));
        assert_eq!(local.state.joint_transform, moved);
        assert!(!baked.update(update));
        assert_eq!(baked.state.joint_transform, IDENTITY_TRANSFORM);
        assert!(baked.rejects_joint_transform(selector));
        assert!(!local.rejects_joint_transform(selector));
        assert!(!baked.rejects_joint_transform(ExportDrawSelector::Legacy {
            joint: 8,
            instance_id: None,
        }));

        let instance = InstanceId::new(3);
        let instance_ids = HashSet::from([instance]);
        let runtime = RuntimeDrawUpdate {
            instance_id: instance,
            update,
        };
        let draws = [(instance, &local), (instance, &baked)];
        let error = validate_runtime_draw_batch(&instance_ids, 0, draws.into_iter(), &[runtime])
            .unwrap_err();
        assert_eq!(error.index(), 0);
        assert!(
            error
                .to_string()
                .contains("world-baked geometry under joint Some(7)"),
            "{error}"
        );
        let other_instance = [(InstanceId::new(4), &baked), (instance, &local)];
        assert!(
            validate_runtime_draw_batch(&instance_ids, 0, other_instance.into_iter(), &[runtime])
                .is_ok(),
            "world-baked draws in other instances must not block the batch"
        );
        let non_finite = RuntimeDrawUpdate {
            instance_id: instance,
            update: DrawUpdate::JointTransform {
                target: selector,
                world: transform_columns(translation_matrix(f32::NAN, 0.0, 0.0)),
            },
        };
        assert!(
            validate_runtime_draw_batch(&instance_ids, 0, std::iter::empty(), &[non_finite])
                .unwrap_err()
                .to_string()
                .contains("finite components")
        );
    }

    #[test]
    fn legacy_export_selector_matches_joint_and_optional_instance() {
        let meshes = [
            exported_mesh(7, Some("clone-a"), true, [1.; 4]),
            exported_mesh(7, Some("clone-a"), true, [1.; 4]),
            exported_mesh(7, Some("clone-b"), true, [1.; 4]),
            exported_mesh(7, None, true, [1.; 4]),
            exported_mesh(8, Some("clone-a"), true, [1.; 4]),
        ];
        let mut presentations = meshes.iter().map(exported_presentation).collect::<Vec<_>>();

        let matched = presentations
            .iter_mut()
            .map(|draw| {
                usize::from(draw.update(DrawUpdate::Visibility {
                    target: ExportDrawSelector::Legacy {
                        joint: 7,
                        instance_id: Some("clone-a"),
                    },
                    visible: true,
                }))
            })
            .sum::<usize>();

        assert_eq!(matched, 2, "one source joint may own several draw parts");
        assert_eq!(
            presentations
                .iter()
                .map(|draw| draw.state.visible)
                .collect::<Vec<_>>(),
            [true, true, false, false, false]
        );
        assert!(
            presentations[3]
                .identity
                .matches_joint(ExportDrawSelector::Legacy {
                    joint: 7,
                    instance_id: None,
                })
        );
    }

    #[test]
    fn partial_update_can_reveal_a_hidden_draw_without_losing_its_material() {
        let color = [0.2, 0.4, 0.6, 0.8];
        let mesh = exported_mesh(42, None, true, color);
        let mut presentation = exported_presentation(&mesh);

        assert!(presentation.update(DrawUpdate::Visibility {
            target: ExportDrawSelector::Legacy {
                joint: 42,
                instance_id: None,
            },
            visible: true,
        }));

        assert!(presentation.state.visible);
        assert_eq!(presentation.state.material_color, color);
    }

    #[test]
    fn legacy_material_selector_requires_source_identity() {
        let material_a = MaterialSourceId::new(100);
        let material_b = MaterialSourceId::new(200);
        let mut meshes = [
            exported_mesh(7, Some("part"), false, [1.0; 4]),
            exported_mesh(7, Some("part"), false, [1.0; 4]),
            exported_mesh(7, Some("part"), false, [1.0; 4]),
            exported_mesh(7, Some("part"), false, [1.0; 4]),
        ];
        meshes[0].material.source_id = Some(material_a);
        meshes[1].material.source_id = Some(material_a);
        meshes[2].material.source_id = Some(material_b);
        let mut presentations = meshes.iter().map(exported_presentation).collect::<Vec<_>>();

        let matched = presentations
            .iter_mut()
            .map(|draw| {
                usize::from(draw.update(DrawUpdate::MaterialColor {
                    target: ExportMaterialSelector::Legacy {
                        source_id: material_a,
                    },
                    color: [0.25, 0.5, 0.75, 1.0],
                }))
            })
            .sum::<usize>();

        assert_eq!(matched, 2, "one MObj may feed several draw parts");
        assert_eq!(
            presentations
                .iter()
                .map(|draw| draw.state.material_color)
                .collect::<Vec<_>>(),
            [
                [0.25, 0.5, 0.75, 1.0],
                [0.25, 0.5, 0.75, 1.0],
                [1.0; 4],
                [1.0; 4],
            ]
        );
        assert!(!presentations[3].update(DrawUpdate::MaterialColor {
            target: ExportMaterialSelector::Legacy {
                source_id: material_a,
            },
            color: [0.0; 4],
        }));
        assert_eq!(presentations[3].state.material_color, [1.0; 4]);
        assert_eq!(presentations[2].state.material_color, [1.0; 4]);
    }

    #[test]
    fn exact_visibility_does_not_broadcast_across_resource_colliding_joints() {
        let mut legacy = exported_mesh(7, None, true, [1.0; 4]);
        legacy.material.source_id = Some(MaterialSourceId::new(100));
        let meshes = [
            exact_exported_mesh("resource-a", 7, 0, 100),
            exact_exported_mesh("resource-a", 7, 0, 100),
            exact_exported_mesh("resource-a", 7, 1, 100),
            exact_exported_mesh("resource-b", 7, 0, 100),
            legacy,
        ];
        let joint_a = meshes[0]
            .source_occurrence
            .as_ref()
            .unwrap()
            .owner_joint
            .clone();
        let mut presentations = meshes.iter().map(exported_presentation).collect::<Vec<_>>();

        let exact_matches = presentations
            .iter_mut()
            .map(|draw| {
                usize::from(draw.update(DrawUpdate::Visibility {
                    target: ExportDrawSelector::Exact(&joint_a),
                    visible: true,
                }))
            })
            .sum::<usize>();
        let legacy_matches = presentations
            .iter_mut()
            .map(|draw| {
                usize::from(draw.update(DrawUpdate::Visibility {
                    target: ExportDrawSelector::Legacy {
                        joint: 7,
                        instance_id: None,
                    },
                    visible: true,
                }))
            })
            .sum::<usize>();

        assert_eq!(exact_matches, 3, "the exact joint owns three draw parts");
        assert_eq!(legacy_matches, 1, "legacy offsets exclude exact draws");
        assert_eq!(
            presentations
                .iter()
                .map(|draw| draw.state.visible)
                .collect::<Vec<_>>(),
            [true, true, true, false, true]
        );
    }

    #[test]
    fn exact_material_does_not_broadcast_across_reused_offsets_or_dobjs() {
        let mut legacy = exported_mesh(7, None, false, [1.0; 4]);
        legacy.material.source_id = Some(MaterialSourceId::new(100));
        let meshes = [
            exact_exported_mesh("resource-a", 7, 0, 100),
            exact_exported_mesh("resource-a", 7, 0, 100),
            exact_exported_mesh("resource-a", 7, 1, 100),
            exact_exported_mesh("resource-b", 7, 0, 100),
            legacy,
        ];
        let material_a0 = meshes[0].material.source_occurrence.clone().unwrap();
        let mut presentations = meshes.iter().map(exported_presentation).collect::<Vec<_>>();

        let exact_matches = presentations
            .iter_mut()
            .map(|draw| {
                usize::from(draw.update(DrawUpdate::MaterialColor {
                    target: ExportMaterialSelector::Exact(&material_a0),
                    color: [0.25, 0.5, 0.75, 1.0],
                }))
            })
            .sum::<usize>();
        let legacy_matches = presentations
            .iter_mut()
            .map(|draw| {
                usize::from(draw.update(DrawUpdate::MaterialColor {
                    target: ExportMaterialSelector::Legacy {
                        source_id: MaterialSourceId::new(100),
                    },
                    color: [0.0, 1.0, 0.0, 1.0],
                }))
            })
            .sum::<usize>();

        assert_eq!(exact_matches, 2, "one exact MObj occurrence is reused");
        assert_eq!(legacy_matches, 1, "legacy offsets exclude exact draws");
        assert_eq!(
            presentations
                .iter()
                .map(|draw| draw.state.material_color)
                .collect::<Vec<_>>(),
            [
                [0.25, 0.5, 0.75, 1.0],
                [0.25, 0.5, 0.75, 1.0],
                [1.0; 4],
                [1.0; 4],
                [0.0, 1.0, 0.0, 1.0],
            ]
        );
    }

    #[test]
    fn exact_updates_are_isolated_by_typed_runtime_instance() {
        let mut mesh = exact_exported_mesh("shared-resource", 7, 0, 100);
        mesh.instance_id = Some("exporter-occurrence-is-not-runtime-identity".into());
        let joint = mesh.source_occurrence.as_ref().unwrap().owner_joint.clone();
        let material = mesh.material.source_occurrence.clone().unwrap();
        let initial = exported_presentation(&mesh);
        let first = InstanceId::new(41);
        let second = InstanceId::new(42);
        let mut presentations = [initial.clone(), initial];

        let reveal_second = RuntimeDrawUpdate {
            instance_id: second,
            update: DrawUpdate::Visibility {
                target: ExportDrawSelector::Exact(&joint),
                visible: true,
            },
        };
        assert!(!update_runtime_presentation(
            first,
            &mut presentations[0],
            reveal_second
        ));
        assert!(update_runtime_presentation(
            second,
            &mut presentations[1],
            reveal_second
        ));
        assert_eq!(
            presentations
                .iter()
                .map(|draw| draw.state.visible)
                .collect::<Vec<_>>(),
            [false, true]
        );

        let recolor_first = RuntimeDrawUpdate {
            instance_id: first,
            update: DrawUpdate::MaterialColor {
                target: ExportMaterialSelector::Exact(&material),
                color: [0.25, 0.5, 0.75, 1.0],
            },
        };
        assert!(update_runtime_presentation(
            first,
            &mut presentations[0],
            recolor_first
        ));
        assert!(!update_runtime_presentation(
            second,
            &mut presentations[1],
            recolor_first
        ));
        assert_eq!(
            presentations
                .iter()
                .map(|draw| draw.state.material_color)
                .collect::<Vec<_>>(),
            [[0.25, 0.5, 0.75, 1.0], [1.0; 4]]
        );
    }

    #[test]
    fn runtime_instance_registry_rejects_duplicate_and_missing_ids() {
        let existing = InstanceId::new(41);
        let missing = InstanceId::new(42);
        let instance_ids = HashSet::from([existing]);

        assert_eq!(
            ensure_new_runtime_instance(&instance_ids, existing)
                .unwrap_err()
                .to_string(),
            "runtime draw instance 41 already exists"
        );
        assert_eq!(
            ensure_runtime_instance(&instance_ids, missing)
                .unwrap_err()
                .to_string(),
            "runtime draw instance 42 does not exist"
        );
        ensure_new_runtime_instance(&instance_ids, missing).unwrap();
        ensure_runtime_instance(&instance_ids, existing).unwrap();
    }

    #[test]
    fn runtime_batch_validation_reports_the_rejected_member() {
        let instance = InstanceId::new(41);
        let instance_ids = HashSet::from([instance]);
        let mesh = exact_exported_mesh("resource", 7, 0, 100);
        let joint = mesh.source_occurrence.as_ref().unwrap().owner_joint.clone();
        let material = mesh.material.source_occurrence.as_ref().unwrap().clone();
        let updates = [
            RuntimeDrawUpdate {
                instance_id: instance,
                update: DrawUpdate::Visibility {
                    target: ExportDrawSelector::Exact(&joint),
                    visible: true,
                },
            },
            RuntimeDrawUpdate {
                instance_id: instance,
                update: DrawUpdate::MaterialColor {
                    target: ExportMaterialSelector::Exact(&material),
                    color: [1.0, f32::NAN, 1.0, 1.0],
                },
            },
        ];

        let error = validate_runtime_draw_batch(&instance_ids, 0, std::iter::empty(), &updates)
            .unwrap_err();

        assert_eq!(error.index(), 1);
        assert_eq!(
            error.to_string(),
            "runtime draw update 1: material color must contain finite components"
        );
        assert!(validate_runtime_draw_batch(&instance_ids, 0, std::iter::empty(), &[]).is_ok());
    }

    #[test]
    fn material_update_preserves_identity_for_vertex_owned_channels() {
        let material = MaterialSourceId::new(100);
        let update = [0.125, 0.25, 0.5, 0.75];
        let cases = [
            ("legacy", None, update),
            (
                "source material color and alpha",
                RenderMode::from_bits(0),
                update,
            ),
            (
                "vertex color, material alpha",
                RenderMode::from_bits(0x0000_2002),
                [1.0, 1.0, 1.0, update[3]],
            ),
            (
                "material color, vertex alpha",
                RenderMode::from_bits(0x0000_4001),
                [update[0], update[1], update[2], 1.0],
            ),
            (
                "vertex alpha inherits vertex diffuse mode",
                RenderMode::from_bits(0x0000_0002),
                [1.0; 4],
            ),
        ];

        for (label, render_mode, expected) in cases {
            let mut mesh = exported_mesh(7, None, false, [0.9; 4]);
            mesh.material.source_id = Some(material);
            mesh.material.render_mode = render_mode;
            let mut presentation = exported_presentation(&mesh);

            assert!(presentation.update(DrawUpdate::MaterialColor {
                target: ExportMaterialSelector::Legacy {
                    source_id: material,
                },
                color: update,
            }));
            assert_eq!(presentation.state.material_color, expected, "{label}");
        }
    }

    #[test]
    fn source_render_mode_selects_three_fixed_gpu_passes() {
        let cases = [
            (0x0000_0011, DrawRenderClass::Opaque, false, true),
            (0x4000_0011, DrawRenderClass::TextureEdge, true, true),
            (0x6000_0011, DrawRenderClass::Translucent, true, false),
        ];
        for (bits, expected, blending, depth_write) in cases {
            let mut mesh = exported_mesh(7, None, false, [1.0, 1.0, 1.0, 0.25]);
            mesh.material.render_mode = RenderMode::from_bits(bits);
            let class = exported_presentation(&mesh).render_class;
            let key = PipelineKey::new(
                mesh.material.cull_mode,
                PixelEngineState::from_render_mode(mesh.material.render_mode.unwrap()),
            )
            .unwrap();

            assert_eq!(class, expected);
            assert_eq!(key.blend.is_some(), blending);
            assert_eq!(key.depth_write, depth_write);
        }
    }

    #[test]
    fn legacy_render_class_is_inferred_once_from_serialized_alpha() {
        let material = MaterialSourceId::new(100);
        let mut opaque = exported_mesh(7, None, false, [1.0; 4]);
        opaque.material.source_id = Some(material);
        let translucent = exported_mesh(7, None, false, [1.0, 1.0, 1.0, 0.25]);
        let mut opaque_presentation = exported_presentation(&opaque);

        assert_eq!(opaque_presentation.render_class, DrawRenderClass::Opaque);
        assert_eq!(
            exported_presentation(&translucent).render_class,
            DrawRenderClass::Translucent
        );
        assert!(opaque_presentation.update(DrawUpdate::MaterialColor {
            target: ExportMaterialSelector::Legacy {
                source_id: material,
            },
            color: [1.0, 1.0, 1.0, 0.25],
        }));
        assert_eq!(
            opaque_presentation.render_class,
            DrawRenderClass::Opaque,
            "animated alpha cannot move a legacy draw between passes"
        );
    }

    #[test]
    fn draw_order_keeps_fixed_passes_and_authored_order_within_each_pass() {
        let mut draws = [
            (DrawRenderClass::Translucent, "xlu-a"),
            (DrawRenderClass::Opaque, "opa-a"),
            (DrawRenderClass::TextureEdge, "tex-a"),
            (DrawRenderClass::Opaque, "opa-b"),
            (DrawRenderClass::Translucent, "xlu-b"),
            (DrawRenderClass::TextureEdge, "tex-b"),
        ];

        draws.sort_by(|(a, _), (b, _)| compare_draw_order(*a, *b));

        assert_eq!(
            draws.map(|(_, name)| name),
            ["opa-a", "opa-b", "tex-a", "tex-b", "xlu-a", "xlu-b"]
        );
    }

    #[test]
    fn material_alpha_does_not_change_the_fixed_render_class() {
        let mut mesh = exported_mesh(42, None, false, [1.0; 4]);
        let material = MaterialSourceId::new(100);
        mesh.material.source_id = Some(material);
        mesh.material.render_mode = RenderMode::from_bits(0);
        let mut presentation = exported_presentation(&mesh);
        assert_eq!(presentation.render_class, DrawRenderClass::Opaque);

        assert!(presentation.update(DrawUpdate::MaterialColor {
            target: ExportMaterialSelector::Legacy {
                source_id: material,
            },
            color: [1.0, 1.0, 1.0, 0.25],
        }));

        assert_eq!(presentation.state.material_color[3], 0.25);
        assert_eq!(presentation.render_class, DrawRenderClass::Opaque);
    }

    #[test]
    #[ignore = "requires a Vulkan/Metal/DX12/GLES graphics adapter"]
    fn gpu_capture_draws_geometry_and_unpads_rows() {
        let _guard = GPU_TEST_LOCK.lock().unwrap();
        let image = pollster::block_on(render_rgba(&Scene::demo(), 257, 193)).unwrap();
        assert_eq!(image.len(), 257 * 193 * 4);
        let background = &image[..4];
        let foreground = image
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| *p != background)
            .count();
        assert!(foreground > 500, "only {foreground} non-background pixels");
        assert!(image.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    }

    #[test]
    #[ignore = "requires a Vulkan/Metal/DX12/GLES graphics adapter"]
    fn gpu_alpha_test_uses_the_gx_u8_boundary() {
        let _guard = GPU_TEST_LOCK.lock().unwrap();
        let scene = |alpha, reference| {
            let mut scene = solid_quad_scene([1.0, 0.0, 0.0, alpha]);
            let mode = RenderMode::from_bits(0x6000_0000).unwrap();
            let mut state = PixelEngineState::from_render_mode(mode);
            state.explicit_descriptor = true;
            state.depth.compare_before_texture = false;
            state.alpha_test = PeAlphaTest {
                comparison0: PeCompare::GreaterEqual,
                reference0: reference,
                operation: PeAlphaOp::And,
                comparison1: PeCompare::LessEqual,
                reference1: 255,
            };
            scene.meshes[0].material.render_mode = Some(mode);
            scene.meshes[0].material.pixel_engine = Some(state);
            scene
        };
        let (below, boundary, truncated_half) = pollster::block_on(async {
            Ok::<_, anyhow::Error>((
                render_rgba(&scene(101.0 / 255.0, 102), 257, 193).await?,
                render_rgba(&scene(102.0 / 255.0, 102), 257, 193).await?,
                render_rgba(&scene(0.5, 128), 257, 193).await?,
            ))
        })
        .unwrap();

        let below_red = below
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[0] > 0)
            .count();
        let boundary_red = boundary
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[0] > 0)
            .count();
        let truncated_half_red = truncated_half
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[0] > 0)
            .count();
        assert_eq!(below_red, 0, "alpha byte 101 must fail GEQUAL 102");
        assert!(
            boundary_red > 500,
            "only {boundary_red} pixels passed at alpha byte 102"
        );
        assert_eq!(
            truncated_half_red, 0,
            "material alpha 0.5 truncates to byte 127 and must fail GEQUAL 128"
        );
    }

    #[test]
    #[ignore = "requires a Vulkan/Metal/DX12/GLES graphics adapter"]
    fn gpu_clone_shares_geometry_and_reveals_only_the_targeted_instance() {
        let _guard = GPU_TEST_LOCK.lock().unwrap();
        let mut scene = Scene::demo();
        scene.meshes[0].joint = Some(7);
        scene.meshes[0].instance_id = Some("cube".into());
        scene.meshes[0].material.source_id = Some(MaterialSourceId::new(100));
        scene.meshes[0].hidden = true;
        let clone = InstanceId::new(41);
        let (hidden, visible) = pollster::block_on(async {
            let instance = wgpu::Instance::new(
                wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            );
            let adapter = request_adapter(&instance, None).await?;
            let mut gpu =
                GpuScene::new(&adapter, &scene, wgpu::TextureFormat::Rgba8UnormSrgb).await?;
            let hidden = capture_gpu_rgba(&gpu, 257, 193).await?;
            let resource_count = gpu.draw_resources.len();
            assert_eq!(gpu.draws.len(), resource_count);
            assert_eq!(gpu.instantiate_draws(clone)?, resource_count);
            assert_eq!(
                gpu.draw_resources.len(),
                resource_count,
                "cloning must not upload another immutable draw resource"
            );
            assert_eq!(gpu.draws.len(), resource_count * 2);
            assert!(gpu.instantiate_draws(clone).is_err());
            assert_eq!(
                gpu.update_instance_draws(RuntimeDrawUpdate {
                    instance_id: clone,
                    update: DrawUpdate::Visibility {
                        target: ExportDrawSelector::Legacy {
                            joint: 7,
                            instance_id: Some("cube"),
                        },
                        visible: true,
                    },
                })?,
                1
            );
            assert_eq!(
                gpu.draws
                    .iter()
                    .filter(|draw| {
                        draw.instance_id == DEFAULT_INSTANCE_ID
                            && draw.presentation.identity.matches_joint(
                                ExportDrawSelector::Legacy {
                                    joint: 7,
                                    instance_id: Some("cube"),
                                },
                            )
                    })
                    .map(|draw| draw.presentation.state.visible)
                    .collect::<Vec<_>>(),
                [false],
                "the compatibility draw must remain hidden"
            );
            assert_eq!(
                gpu.draws
                    .iter()
                    .filter(|draw| {
                        draw.instance_id == clone
                            && draw.presentation.identity.matches_joint(
                                ExportDrawSelector::Legacy {
                                    joint: 7,
                                    instance_id: Some("cube"),
                                },
                            )
                    })
                    .filter(|draw| draw.presentation.state.visible)
                    .count(),
                1
            );
            assert_eq!(
                gpu.update_instance_draws(RuntimeDrawUpdate {
                    instance_id: clone,
                    update: DrawUpdate::MaterialColor {
                        target: ExportMaterialSelector::Legacy {
                            source_id: MaterialSourceId::new(100),
                        },
                        color: [0.0, 0.0, 1.0, 1.0],
                    },
                })?,
                1
            );
            let cube_draw = |instance_id| {
                gpu.draws.iter().position(|draw| {
                    draw.instance_id == instance_id
                        && draw
                            .presentation
                            .identity
                            .matches_joint(ExportDrawSelector::Legacy {
                                joint: 7,
                                instance_id: Some("cube"),
                            })
                })
            };
            let default_cube = cube_draw(DEFAULT_INSTANCE_ID).unwrap();
            let cloned_cube = cube_draw(clone).unwrap();
            assert_eq!(
                read_material_uniform(&gpu, default_cube).await?.color,
                [1.0; 4]
            );
            assert_eq!(
                read_material_uniform(&gpu, cloned_cube).await?.color,
                [0.0, 0.0, 1.0, 1.0]
            );
            let visible = capture_gpu_rgba(&gpu, 257, 193).await?;
            Ok::<_, anyhow::Error>((hidden, visible))
        })
        .unwrap();

        let changed = hidden
            .as_chunks::<4>()
            .0
            .iter()
            .zip(visible.as_chunks::<4>().0)
            .filter(|(before, after)| before != after)
            .count();
        assert!(changed > 500, "only {changed} pixels changed after reveal");
    }

    #[test]
    #[ignore = "requires a Vulkan/Metal/DX12/GLES graphics adapter"]
    fn gpu_joint_local_draws_render_where_the_baked_export_would_and_can_be_moved() {
        let _guard = GPU_TEST_LOCK.lock().unwrap();
        let offset = [0.8, -0.4, 0.0];
        let mut baked = solid_quad_scene([1.0, 0.0, 0.0, 1.0]);
        for vertex in &mut baked.meshes[0].vertices {
            for (position, offset) in vertex.position.iter_mut().zip(offset) {
                *position += offset;
            }
        }
        let mut local = solid_quad_scene([1.0, 0.0, 0.0, 1.0]);
        local.meshes[0].geometry_space = GeometrySpace::JointLocal;
        local.joints = vec![posed_joint(
            7,
            translation_matrix(offset[0], offset[1], offset[2]),
        )];
        assert_eq!(local.bounds(), baked.bounds());

        let (baked_image, local_image, moved_image, rejected) = pollster::block_on(async {
            let instance = wgpu::Instance::new(
                wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            );
            let adapter = request_adapter(&instance, None).await?;
            let baked_gpu =
                GpuScene::new(&adapter, &baked, wgpu::TextureFormat::Rgba8UnormSrgb).await?;
            let baked_image = capture_gpu_rgba(&baked_gpu, 257, 193).await?;
            let mut local_gpu =
                GpuScene::new(&adapter, &local, wgpu::TextureFormat::Rgba8UnormSrgb).await?;
            let local_image = capture_gpu_rgba(&local_gpu, 257, 193).await?;
            let selector = ExportDrawSelector::Legacy {
                joint: 7,
                instance_id: Some("quad"),
            };
            assert_eq!(
                local_gpu.update_draws(DrawUpdate::JointTransform {
                    target: selector,
                    world: transform_columns(translation_matrix(-1.5, 1.0, 0.0)),
                })?,
                1
            );
            assert_eq!(
                read_material_uniform(&local_gpu, 0).await?.model[3],
                [-1.5, 1.0, 0.0, 1.0]
            );
            let moved_image = capture_gpu_rgba(&local_gpu, 257, 193).await?;
            let mut baked_gpu = baked_gpu;
            let rejected = baked_gpu
                .update_draws(DrawUpdate::JointTransform {
                    target: selector,
                    world: IDENTITY_TRANSFORM,
                })
                .unwrap_err()
                .to_string();
            Ok::<_, anyhow::Error>((baked_image, local_image, moved_image, rejected))
        })
        .unwrap();

        assert_eq!(
            baked_image, local_image,
            "joint-local geometry must land exactly where the exporter would have baked it"
        );
        let red = |image: &[u8]| {
            image
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| pixel[0] > 0)
                .count()
        };
        assert!(red(&local_image) > 500);
        assert!(red(&moved_image) > 500);
        let moved_pixels = local_image
            .as_chunks::<4>()
            .0
            .iter()
            .zip(moved_image.as_chunks::<4>().0)
            .filter(|(before, after)| before != after)
            .count();
        assert!(moved_pixels > 500, "only {moved_pixels} pixels moved");
        assert!(rejected.contains("world-baked geometry"), "{rejected}");
    }

    /// A textured quad with an exact first-stage occurrence over a 2x1 image
    /// (left red, right blue) and a solid green alternate.
    fn textured_quad_scene(wrap: [WrapMode; 2]) -> (Scene, VisualTextureOccurrence) {
        let mut scene = solid_quad_scene([1.0, 1.0, 1.0, 1.0]);
        scene.textures = vec![
            Texture {
                name: "red-blue".into(),
                width: 2,
                height: 1,
                rgba: vec![255, 0, 0, 255, 0, 0, 255, 255],
            },
            Texture {
                name: "green".into(),
                width: 1,
                height: 1,
                rgba: vec![0, 255, 0, 255],
            },
        ];
        let mesh = exact_exported_mesh("resource", 7, 0, 100);
        let material = mesh.material.source_occurrence.clone().unwrap();
        let occurrence = VisualTextureOccurrence {
            owner_material: material,
            tobj_index: 0,
            visual_offset: TextureSourceId::new(0x480),
        };
        let quad = &mut scene.meshes[0];
        quad.hidden = false;
        quad.joint = mesh.joint;
        quad.source_occurrence = mesh.source_occurrence.clone();
        quad.material.source_occurrence = mesh.material.source_occurrence.clone();
        quad.material.texture = Some(0);
        quad.material.texture_sources = vec![super::super::scene::VisualTextureStageSource {
            visual_offset: Some(TextureSourceId::new(0x480)),
            occurrence: Some(occurrence.clone()),
            image_descriptor: Some(0x500),
            texture: Some(0),
            wrap,
            transform: TextureTransform {
                rotation: [0.0; 3],
                scale: [1.0; 3],
                translation: [0.0; 3],
                repeat: [1, 1],
                wrap_t: wrap[1],
            },
        }];
        // Sample only the right (blue) half of the image.
        for (vertex, u) in quad.vertices.iter_mut().zip([0.5, 1.0, 1.0, 0.5]) {
            vertex.uv = [u, 0.5];
        }
        (scene, occurrence)
    }

    fn dominant_channel_counts(image: &[u8]) -> [usize; 3] {
        let mut counts = [0; 3];
        for pixel in image.as_chunks::<4>().0 {
            let brightest = (0..3).max_by_key(|&channel| pixel[channel]).unwrap();
            if pixel[brightest] > 64 {
                counts[brightest] += 1;
            }
        }
        counts
    }

    #[test]
    #[ignore = "requires a Vulkan/Metal/DX12/GLES graphics adapter"]
    fn gpu_texture_updates_switch_images_and_translate_uvs_under_gx_wrapping() {
        let _guard = GPU_TEST_LOCK.lock().unwrap();
        let (clamped, occurrence) = textured_quad_scene([WrapMode::Clamp; 2]);
        let (repeated, _) = textured_quad_scene([WrapMode::Repeat; 2]);
        let (initial, switched, shifted, repeated_initial, wrapped) = pollster::block_on(async {
            let instance = wgpu::Instance::new(
                wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            );
            let adapter = request_adapter(&instance, None).await?;
            let mut gpu =
                GpuScene::new(&adapter, &clamped, wgpu::TextureFormat::Rgba8UnormSrgb).await?;
            let initial = capture_gpu_rgba(&gpu, 129, 129).await?;
            assert_eq!(
                gpu.update_draws(DrawUpdate::Texture {
                    target: ExportTextureSelector::Exact(&occurrence),
                    image: Some(1),
                    translation: [0.0; 2],
                    scale: [1.0; 2],
                })?,
                1
            );
            let switched = capture_gpu_rgba(&gpu, 129, 129).await?;
            // HSD subtracts the translation: s' = s - 0.5 moves the sampled
            // window onto the red half.
            assert_eq!(
                gpu.update_draws(DrawUpdate::Texture {
                    target: ExportTextureSelector::Exact(&occurrence),
                    image: Some(0),
                    translation: [0.5, 0.0],
                    scale: [1.0; 2],
                })?,
                1
            );
            let shifted = capture_gpu_rgba(&gpu, 129, 129).await?;
            let mut repeated_gpu =
                GpuScene::new(&adapter, &repeated, wgpu::TextureFormat::Rgba8UnormSrgb).await?;
            let repeated_initial = capture_gpu_rgba(&repeated_gpu, 129, 129).await?;
            // With repeat wrapping, s' = s - 1.0 samples the same blue half.
            assert_eq!(
                repeated_gpu.update_draws(DrawUpdate::Texture {
                    target: ExportTextureSelector::Exact(&occurrence),
                    image: Some(0),
                    translation: [1.0, 0.0],
                    scale: [1.0; 2],
                })?,
                1
            );
            let wrapped = capture_gpu_rgba(&repeated_gpu, 129, 129).await?;
            Ok::<_, anyhow::Error>((initial, switched, shifted, repeated_initial, wrapped))
        })
        .unwrap();

        let [red, green, blue] = dominant_channel_counts(&initial);
        assert!(
            blue > 500 && red == 0 && green == 0,
            "initial {red}/{green}/{blue}"
        );
        let [red, green, blue] = dominant_channel_counts(&switched);
        assert!(
            green > 500 && red == 0 && blue == 0,
            "switched {red}/{green}/{blue}"
        );
        let [red, green, blue] = dominant_channel_counts(&shifted);
        assert!(
            red > 500 && green == 0 && blue == 0,
            "shifted {red}/{green}/{blue}"
        );
        assert_ne!(
            initial, repeated_initial,
            "clamp and repeat wrapping differ at the texel boundary"
        );
        assert_eq!(
            repeated_initial, wrapped,
            "a whole-period repeat translation is invisible"
        );
    }

    #[test]
    #[ignore = "requires a Vulkan/Metal/DX12/GLES graphics adapter"]
    fn gpu_material_update_reaches_a_resident_uniform_between_frames() {
        let _guard = GPU_TEST_LOCK.lock().unwrap();
        let scene = solid_quad_scene([1.0, 0.0, 0.0, 1.0]);

        let (red, green) = pollster::block_on(async {
            let instance = wgpu::Instance::new(
                wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            );
            let adapter = request_adapter(&instance, None).await?;
            let mut gpu =
                GpuScene::new(&adapter, &scene, wgpu::TextureFormat::Rgba8UnormSrgb).await?;
            let red = capture_gpu_rgba(&gpu, 257, 193).await?;
            assert_eq!(
                gpu.update_draws(DrawUpdate::MaterialColor {
                    target: ExportMaterialSelector::Legacy {
                        source_id: MaterialSourceId::new(100),
                    },
                    color: [0.0, 0.0, 1.0, 1.0],
                })?,
                1
            );
            assert_eq!(
                gpu.update_draws(DrawUpdate::MaterialColor {
                    target: ExportMaterialSelector::Legacy {
                        source_id: MaterialSourceId::new(100),
                    },
                    color: [0.0, 1.0, 0.0, 1.0],
                })?,
                1
            );
            let quad = gpu
                .draws
                .iter()
                .position(|draw| {
                    draw.presentation
                        .identity
                        .matches_joint(ExportDrawSelector::Legacy {
                            joint: 7,
                            instance_id: Some("quad"),
                        })
                })
                .expect("uploaded quad draw");
            assert_eq!(
                gpu.draws[quad].presentation.state.material_color,
                [0.0, 1.0, 0.0, 1.0]
            );
            let uniform = read_material_uniform(&gpu, quad).await?;
            assert_eq!(uniform.color, [0.0, 1.0, 0.0, 1.0]);
            assert_eq!(uniform.alpha_test, [7, 0, 7, 0]);
            let green = capture_gpu_rgba(&gpu, 257, 193).await?;
            Ok::<_, anyhow::Error>((red, green))
        })
        .unwrap();

        let changed = red
            .as_chunks::<4>()
            .0
            .iter()
            .zip(green.as_chunks::<4>().0)
            .filter(|(before, after)| before != after)
            .count();
        let red_dominant = red
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[0] > pixel[1] && pixel[0] > pixel[2])
            .count();
        let green_dominant = green
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[1] > pixel[0] && pixel[1] > pixel[2])
            .count();
        let transitioned = red
            .as_chunks::<4>()
            .0
            .iter()
            .zip(green.as_chunks::<4>().0)
            .filter(|(before, after)| {
                before[0] > before[1]
                    && before[0] > before[2]
                    && after[1] > after[0]
                    && after[1] > after[2]
            })
            .count();
        assert!(
            transitioned > 500,
            "only {transitioned} pixels changed from red-dominant to green-dominant; \
             {changed} changed, {red_dominant} started red-dominant, \
             {green_dominant} ended green-dominant"
        );
    }
}
