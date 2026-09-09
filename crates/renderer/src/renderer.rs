//! Shared GPU path for window presentation and offscreen captures.
use std::{fs::File, io::BufWriter, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use glam::Vec3;
use sdl3::video::Window;
use wgpu::util::DeviceExt;

use crate::particles::{Particle, ParticleRenderer};
use crate::scene::{CullMode, Scene, Texture, Vertex};
use crate::{menu::MenuView, platform::SdlSurface, ui::UiRenderer};

pub const MESH_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/mesh.wgsl"));
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.018,
    g: 0.025,
    b: 0.045,
    a: 1.0,
};

struct Draw {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    texture: usize,
    pipeline: usize,
    center: Vec3,
    transparent: bool,
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
    particles: ParticleRenderer,
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
            bind_group_layouts: &[Some(&camera_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
        let mut pipelines = Vec::new();
        for transparent in [false, true] {
            for cull_mode in [None, Some(wgpu::Face::Front), Some(wgpu::Face::Back)] {
                pipelines.push(
                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("inspection mesh"),
                        layout: Some(&layout),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: Some("vs_main"),
                            compilation_options: Default::default(),
                            buffers: &[Some(wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<Vertex>() as u64,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &attributes,
                            })],
                        },
                        primitive: wgpu::PrimitiveState {
                            cull_mode,
                            front_face: wgpu::FrontFace::Ccw,
                            ..Default::default()
                        },
                        depth_stencil: Some(wgpu::DepthStencilState {
                            format: DEPTH_FORMAT,
                            depth_write_enabled: Some(!transparent),
                            depth_compare: Some(wgpu::CompareFunction::Less),
                            stencil: Default::default(),
                            bias: Default::default(),
                        }),
                        multisample: Default::default(),
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: Some("fs_main"),
                            compilation_options: Default::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format,
                                blend: transparent.then_some(wgpu::BlendState::ALPHA_BLENDING),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        multiview_mask: None,
                        cache: None,
                    }),
                );
            }
        }
        let mut draws = Vec::new();
        for mesh in &scene.meshes {
            if mesh.hidden
                || matches!(mesh.material.cull_mode, CullMode::All)
                || mesh.indices.is_empty()
            {
                continue;
            }
            let mut vertices = mesh.vertices.clone();
            for vertex in &mut vertices {
                for (component, factor) in vertex.color.iter_mut().zip(mesh.material.color) {
                    *component *= factor;
                }
            }
            let transparent = vertices.iter().any(|v| v.color[3] < 1.0)
                || mesh.material.texture.is_some_and(|i| {
                    scene.textures[i]
                        .rgba
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|p| p[3] < 255)
                });
            let cull = match mesh.material.cull_mode {
                CullMode::None | CullMode::All => 0,
                CullMode::Front => 1,
                CullMode::Back => 2,
            };
            let mut low = Vec3::splat(f32::INFINITY);
            let mut high = Vec3::splat(f32::NEG_INFINITY);
            for vertex in &vertices {
                low = low.min(Vec3::from(vertex.position));
                high = high.max(Vec3::from(vertex.position));
            }
            draws.push(Draw {
                vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&mesh.name),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&mesh.name),
                    contents: bytemuck::cast_slice(&mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
                count: mesh.indices.len() as u32,
                texture: mesh.material.texture.unwrap_or(scene.textures.len()),
                pipeline: cull + if transparent { 3 } else { 0 },
                center: low * 0.5 + high * 0.5,
                transparent,
            });
        }
        let particles = ParticleRenderer::new(&device, format, DEPTH_FORMAT);
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
            particles,
        })
    }

    fn draw(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        dimensions: [u32; 2],
        orbit: [f32; 3],
    ) {
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
        let view = glam::camera::rh::view::look_at_mat4(eye, self.center, Vec3::Y);
        let projection = glam::camera::rh::proj::directx::perspective(
            half_fov * 2.0,
            aspect,
            self.radius * 0.001,
            distance + self.radius * 3.0,
        );
        self.queue.write_buffer(
            &self.camera,
            0,
            bytemuck::cast_slice(&(projection * view).to_cols_array()),
        );
        self.particles.prepare(&self.queue, view, projection);
        let mut order: Vec<_> = self.draws.iter().collect();
        order.sort_by(|a, b| {
            a.transparent.cmp(&b.transparent).then_with(|| {
                if a.transparent {
                    view.transform_point3(a.center)
                        .z
                        .total_cmp(&view.transform_point3(b.center).z)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR),
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
        pass.set_bind_group(0, &self.camera_binding, &[]);
        for draw in order {
            pass.set_pipeline(&self.pipelines[draw.pipeline]);
            pass.set_bind_group(1, &self.textures[draw.texture], &[]);
            pass.set_vertex_buffer(0, draw.vertices.slice(..));
            pass.set_index_buffer(draw.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..draw.count, 0, 0..1);
        }
        self.particles.draw(&mut pass);
    }
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
    ui: UiRenderer,
    menu: Option<MenuView>,
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
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let ui = UiRenderer::new(&gpu.device, config.format);
        if let Some(error) = scope.pop().await {
            bail!("creating menu graphics resources: {error}");
        }
        Ok(Self {
            presentation,
            config,
            depth,
            gpu,
            ui,
            menu: None,
            suspended: width == 0 || height == 0,
        })
    }

    pub fn adapter_name(&self) -> &str {
        &self.gpu.adapter_name
    }

    pub fn set_particles(&mut self, particles: &[Particle]) -> Result<()> {
        self.gpu.particles.set_particles(particles)
    }

    pub fn window_id(&self) -> u32 {
        self.presentation.window().id()
    }

    pub fn pixel_size(&self) -> (u32, u32) {
        self.presentation.window().size_in_pixels()
    }

    pub fn set_menu(&mut self, view: Option<MenuView>) {
        if let Some(view) = &view {
            self.ui.prepare(
                &self.gpu.device,
                &self.gpu.queue,
                view,
                self.config.width,
                self.config.height,
            );
        }
        self.menu = view;
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
        if let Some(view) = &self.menu {
            self.ui.prepare(
                &self.gpu.device,
                &self.gpu.queue,
                view,
                self.config.width,
                self.config.height,
            );
        }
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
        if self.menu.is_some() {
            draw_menu(&self.ui, &mut encoder, &target);
        }
        self.gpu.queue.submit([encoder.finish()]);
        self.gpu.queue.present(frame);
        if reconfigure {
            self.presentation
                .surface()
                .configure(&self.gpu.device, &self.config);
        }
        // A replacement swapchain needs its own frame, including when an
        // otherwise idle menu received a suboptimal first surface texture.
        Ok(!reconfigure)
    }
}

/// Render with the same shaders and draw path, without opening a window or audio device.
pub fn render_headless(scene: &Scene, width: u32, height: u32, output: &Path) -> Result<()> {
    render_headless_view(scene, None, &[], width, height, output)
}

/// Capture procedural particles through the native scene draw path.
pub fn render_particles_headless(
    scene: &Scene,
    particles: &[Particle],
    width: u32,
    height: u32,
    output: &Path,
) -> Result<()> {
    render_headless_view(scene, None, particles, width, height, output)
}

/// Captures a menu through the same GPU overlay used by the SDL window.
pub fn render_menu_headless(
    scene: &Scene,
    menu: &MenuView,
    width: u32,
    height: u32,
    output: &Path,
) -> Result<()> {
    render_headless_view(scene, Some(menu), &[], width, height, output)
}

fn render_headless_view(
    scene: &Scene,
    menu: Option<&MenuView>,
    particles: &[Particle],
    width: u32,
    height: u32,
    output: &Path,
) -> Result<()> {
    ensure!(
        (1..=8192).contains(&width) && (1..=8192).contains(&height),
        "capture dimensions must be 1..=8192"
    );
    crate::particles::validate_batch(particles)?;
    let rgba = pollster::block_on(render_rgba(scene, menu, particles, width, height))?;
    let file = File::create(output).with_context(|| format!("creating {}", output.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&rgba)?;
    writer.finish()?;
    Ok(())
}

async fn render_rgba(
    scene: &Scene,
    menu: Option<&MenuView>,
    particles: &[Particle],
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = request_adapter(&instance, None).await?;
    let mut gpu = GpuScene::new(&adapter, scene, wgpu::TextureFormat::Rgba8UnormSrgb).await?;
    gpu.particles.set_particles(particles)?;
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
    if let Some(menu) = menu {
        let mut ui = UiRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8UnormSrgb);
        ui.prepare(&gpu.device, &gpu.queue, menu, width, height);
        draw_menu(&ui, &mut encoder, &texture.create_view(&Default::default()));
    }
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

fn draw_menu(ui: &UiRenderer, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("menu overlay"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    ui.draw(&mut pass);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_wesl_is_valid_for_the_rendering_backend() {
        let module = wgpu::naga::front::wgsl::parse_str(MESH_SHADER).expect("valid generated WGSL");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("shader validates without optional GPU capabilities");
    }

    #[test]
    #[ignore = "requires a Vulkan/Metal/DX12/GLES graphics adapter"]
    fn gpu_particles_have_seeded_detail_fade_and_depth() {
        use crate::particles::ParticleEffect;
        let empty = Scene {
            meshes: vec![],
            textures: vec![],
            warnings: vec![],
        };
        let capture = |scene: &Scene, particles: &[Particle]| {
            pollster::block_on(render_rgba(scene, None, particles, 257, 193)).unwrap()
        };
        let background = capture(&empty, &[]);
        let mut particle = Particle::preview(ParticleEffect::Smoke, 0.35);
        let first = capture(&empty, &[particle]);
        assert_ne!(first, background, "live effect must draw");
        assert_eq!(
            first,
            capture(&empty, &[particle]),
            "fixed inputs must reproduce"
        );
        particle.seed += 1;
        assert_ne!(
            first,
            capture(&empty, &[particle]),
            "seed must change detail"
        );
        particle.age = 0.8;
        assert_ne!(
            first,
            capture(&empty, &[particle]),
            "age must change appearance"
        );
        for age in [-1.0, 0.0, 1.0, 2.0] {
            particle.age = age;
            assert_eq!(background, capture(&empty, &[particle]));
        }
        let scene = Scene::demo();
        let baseline = capture(&scene, &[]);
        particle.age = 0.35;
        particle.position = [0.0, 1.0, 0.0];
        particle.half_size = [0.25; 2];
        assert_eq!(
            baseline,
            capture(&scene, &[particle]),
            "opaque cube must occlude an interior particle"
        );
        // Far behind the camera and outside the view volume.
        particle.position = [1e8; 3];
        assert_eq!(baseline, capture(&scene, &[particle]));
        assert!(first.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    }

    #[test]
    #[ignore = "requires a Vulkan/Metal/DX12/GLES graphics adapter"]
    fn gpu_capture_draws_geometry_and_unpads_rows() {
        let image = pollster::block_on(render_rgba(&Scene::demo(), None, &[], 257, 193)).unwrap();
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
}
