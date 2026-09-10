//! Shared GPU path for window presentation and offscreen captures.
use std::{collections::HashMap, fs::File, io::BufWriter, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use glam::Vec3;
use sdl3::video::Window;
use wgpu::util::DeviceExt;

use super::platform::SdlSurface;
use super::scene::{
    Camera, CullMode, MaterialSourceId, Mesh, PeAlphaTest, PeBlendFactor, PeBlendMode,
    PeBlendState, PeCompare, PixelEngineState, RenderMode, RenderModeClass, Scene, Texture, Vertex,
    VisualDObjOccurrence, VisualJointOccurrence, VisualMaterialOccurrence,
};
use super::viewport::{PresentationTransform, fitted_viewport};

pub const MESH_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/mesh.wgsl"));
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Selects draw parts attached to one exported joint for visibility updates.
///
/// Exact selectors include resource provenance and never collide across visual
/// resources. Bare visual offsets are available only for legacy scenes that do
/// not carry exact occurrences. Runtime scene-instance identity is separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportDrawSelector<'a> {
    Exact(&'a VisualJointOccurrence),
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExportDrawIdentity {
    joint: Option<u32>,
    instance_id: Option<String>,
    source_occurrence: Option<VisualDObjOccurrence>,
    material_source_id: Option<MaterialSourceId>,
    material_source_occurrence: Option<VisualMaterialOccurrence>,
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
                    && self.instance_id.as_deref() == instance_id
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
}

impl From<&Mesh> for ExportDrawIdentity {
    fn from(mesh: &Mesh) -> Self {
        Self {
            joint: mesh.joint,
            instance_id: mesh.instance_id.clone(),
            source_occurrence: mesh.source_occurrence.clone(),
            material_source_id: mesh.material.source_id,
            material_source_occurrence: mesh.material.source_occurrence.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DrawState {
    visible: bool,
    material_color: [f32; 4],
}

impl From<&Mesh> for DrawState {
    fn from(mesh: &Mesh) -> Self {
        Self {
            visible: !mesh.hidden,
            material_color: mesh.material.color,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DrawPresentation {
    identity: ExportDrawIdentity,
    state: DrawState,
    material_render_mode: Option<RenderMode>,
    render_class: DrawRenderClass,
}

impl DrawPresentation {
    fn from_mesh(mesh: &Mesh, textures: &[Texture]) -> Self {
        Self {
            identity: mesh.into(),
            state: mesh.into(),
            material_render_mode: mesh.material.render_mode,
            render_class: DrawRenderClass::from_mesh(mesh, textures),
        }
    }

    fn update(&mut self, update: DrawUpdate<'_>) -> bool {
        match update {
            DrawUpdate::Visibility { target, visible } if self.identity.matches_joint(target) => {
                self.state.visible = visible;
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

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    color: [f32; 4],
    alpha_test: [u32; 4],
}

impl MaterialUniform {
    fn new(
        mut color: [f32; 4],
        alpha_test: PeAlphaTest,
        material_alpha_uses_hsd_byte_storage: bool,
    ) -> Self {
        if material_alpha_uses_hsd_byte_storage {
            // HSD_SetMaterialColor stores the authored float alpha in an unsigned
            // byte before channel/TEV evaluation. Preserve that truncation boundary
            // separately from the preview shader's final TEV-output quantization.
            color[3] = f32::from((color[3] * 255.0) as u8) / 255.0;
        }
        Self {
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

struct Draw {
    presentation: DrawPresentation,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    material: wgpu::Buffer,
    material_binding: wgpu::BindGroup,
    count: u32,
    texture: usize,
    alpha_test: PeAlphaTest,
    pipeline: usize,
}

struct GpuScene {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,
    pipelines: Vec<wgpu::RenderPipeline>,
    camera: wgpu::Buffer,
    camera_binding: wgpu::BindGroup,
    textures: Vec<wgpu::BindGroup>,
    draws: Vec<Draw>,
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
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<MaterialUniform>() as u64
                    ),
                },
                count: None,
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("preview linear repeat"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let white = Texture {
            name: "white".into(),
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        };
        let textures = scene
            .textures
            .iter()
            .chain(std::iter::once(&white))
            .map(|source| {
                let texture = device.create_texture_with_data(
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
                );
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&source.name),
                    layout: &texture_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &texture.create_view(&Default::default()),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                })
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
        let mut draws = Vec::new();
        for mesh in &scene.meshes {
            if !retain_draw(mesh) {
                continue;
            }
            let presentation = DrawPresentation::from_mesh(mesh, &scene.textures);
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
            let uniform = MaterialUniform::new(
                mesh.material.color,
                pixel_engine.alpha_test,
                presentation.material_alpha_uses_hsd_byte_storage(),
            );
            let material = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} material", mesh.name)),
                contents: bytemuck::bytes_of(&uniform),
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            });
            let material_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("{} material", mesh.name)),
                layout: &material_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: material.as_entire_binding(),
                }],
            });
            draws.push(Draw {
                presentation,
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
                material,
                material_binding,
                count: mesh.indices.len() as u32,
                texture: mesh.material.texture.unwrap_or(scene.textures.len()),
                alpha_test: pixel_engine.alpha_test,
                pipeline,
            });
        }
        if let Some(error) = scope.pop().await {
            bail!("creating graphics resources: {error}");
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
        Ok(Self {
            device,
            queue,
            adapter_name: adapter.get_info().name,
            pipelines,
            camera,
            camera_binding,
            textures,
            draws,
            center,
            radius,
            source_camera: scene.camera,
            clear_color: wgpu::Color {
                r: f64::from(scene.clear_color[0]),
                g: f64::from(scene.clear_color[1]),
                b: f64::from(scene.clear_color[2]),
                a: f64::from(scene.clear_color[3]),
            },
        })
    }

    fn update_draws(&mut self, update: DrawUpdate<'_>) -> Result<usize> {
        if let DrawUpdate::MaterialColor { color, .. } = update {
            ensure!(
                color.iter().all(|component| component.is_finite()),
                "material color must contain finite components"
            );
        }
        let mut matched = 0;
        for draw in &mut self.draws {
            if !draw.presentation.update(update) {
                continue;
            }
            if matches!(update, DrawUpdate::MaterialColor { .. }) {
                self.queue.write_buffer(
                    &draw.material,
                    0,
                    bytemuck::bytes_of(&MaterialUniform::new(
                        draw.presentation.state.material_color,
                        draw.alpha_test,
                        draw.presentation.material_alpha_uses_hsd_byte_storage(),
                    )),
                );
            }
            matched += 1;
        }
        Ok(matched)
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
            compare_draw_order(a.presentation.render_class, b.presentation.render_class)
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
            pass.set_pipeline(&self.pipelines[draw.pipeline]);
            pass.set_bind_group(1, &self.textures[draw.texture], &[]);
            pass.set_bind_group(2, &draw.material_binding, &[]);
            pass.set_vertex_buffer(0, draw.vertices.slice(..));
            pass.set_index_buffer(draw.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..draw.count, 0, 0..1);
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

    /// Applies one source-targeted visibility or material update to resident draws.
    ///
    /// Visibility uses an exported joint/part selector and may fan out to every
    /// attached material. Color requires an exact source MObj identity.
    /// The returned count exposes missing or intentionally grouped draw parts;
    /// immutable geometry and textures remain resident.
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
    use super::super::scene::{PeAlphaOp, RenderMode, VisualResourceId};
    use super::*;

    // Some host Vulkan loaders are not safe to initialize twice in parallel.
    // The ignored adapter tests are opt-in, but must still be reliable when a
    // caller selects both with the default multi-threaded Rust test harness.
    static GPU_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    async fn read_material_uniform(gpu: &GpuScene, draw_index: usize) -> Result<MaterialUniform> {
        let size = std::mem::size_of::<MaterialUniform>() as u64;
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
        let uniform = *bytemuck::from_bytes::<MaterialUniform>(&mapped);
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
        DrawPresentation::from_mesh(mesh, &Scene::demo().textures)
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
        let module = wgpu::naga::front::wgsl::parse_str(MESH_SHADER).expect("valid generated WGSL");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("shader validates without optional GPU capabilities");
        assert_eq!(std::mem::size_of::<MaterialUniform>(), 32);
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
        let uniform = MaterialUniform::new([0.25, 0.5, 0.75, 1.0], alpha_test, true);
        assert_eq!(uniform.color, [0.25, 0.5, 0.75, 1.0]);
        assert_eq!(uniform.alpha_test, [6, 102, 3, 255 | (3 << 8)]);

        let half_alpha = MaterialUniform::new([1.0, 1.0, 1.0, 0.5], alpha_test, true);
        assert_eq!(half_alpha.color[3], 127.0 / 255.0);

        let legacy_half_alpha = MaterialUniform::new([1.0, 1.0, 1.0, 0.5], alpha_test, false);
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
                instance_id: Some("cursor-2".into()),
                source_occurrence: None,
                material_source_id: None,
                material_source_occurrence: None,
            }
        );
        assert_eq!(
            presentation.state,
            DrawState {
                visible: false,
                material_color: color,
            }
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
    fn gpu_update_reveals_preuploaded_hidden_geometry() {
        let _guard = GPU_TEST_LOCK.lock().unwrap();
        let mut scene = Scene::demo();
        scene.meshes[0].joint = Some(7);
        scene.meshes[0].instance_id = Some("cube".into());
        scene.meshes[0].hidden = true;
        let (hidden, visible) = pollster::block_on(async {
            let instance = wgpu::Instance::new(
                wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            );
            let adapter = request_adapter(&instance, None).await?;
            let mut gpu =
                GpuScene::new(&adapter, &scene, wgpu::TextureFormat::Rgba8UnormSrgb).await?;
            let hidden = capture_gpu_rgba(&gpu, 257, 193).await?;
            assert_eq!(
                gpu.update_draws(DrawUpdate::Visibility {
                    target: ExportDrawSelector::Legacy {
                        joint: 7,
                        instance_id: Some("cube"),
                    },
                    visible: true,
                })?,
                1
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
