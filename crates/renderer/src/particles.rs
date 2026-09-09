//! Procedural presentation of particles; callers own emission and simulation.
use anyhow::{Result, ensure};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

pub const PARTICLE_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/particles.wgsl"));
pub const MAX_PARTICLES: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
#[repr(u32)]
pub enum ParticleEffect {
    Smoke,
    Fire,
}

/// A current presentation sample. No gameplay clocks or RNG are advanced here.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub effect: ParticleEffect,
    pub position: [f32; 3],
    pub half_size: [f32; 2],
    pub rotation: f32,
    /// Linear, unpremultiplied RGBA, in 0..=1.
    pub color: [f32; 4],
    /// Normalized lifetime: only 0 < age < 1 is rendered.
    pub age: f32,
    pub seed: u32,
}

impl Particle {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.position
                .iter()
                .chain(self.half_size.iter())
                .chain(self.color.iter())
                .chain([&self.rotation, &self.age])
                .all(|x| x.is_finite()),
            "nonfinite particle"
        );
        ensure!(
            self.half_size.iter().all(|x| *x > 0.0 && *x <= 1e6),
            "invalid particle size"
        );
        ensure!(
            self.position.iter().all(|x| x.abs() <= 1e12),
            "particle position out of range"
        );
        ensure!(
            self.color.iter().all(|x| (0.0..=1.0).contains(x)),
            "invalid particle color"
        );
        Ok(())
    }

    /// Synthetic review sample; does not assert original spawn parameters.
    pub fn preview(effect: ParticleEffect, age: f32) -> Self {
        Self {
            effect,
            position: [0.0, 0.0, 0.0],
            half_size: [1.25; 2],
            rotation: 0.0,
            color: [1.0; 4],
            age,
            seed: 1777722793,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    center_age: [f32; 4],
    size_rotation: [f32; 4],
    color: [f32; 4],
    seed: u32,
    effect: u32,
}

pub struct ParticleRenderer {
    pipeline: wgpu::RenderPipeline,
    camera: wgpu::Buffer,
    binding: wgpu::BindGroup,
    buffer: wgpu::Buffer,
    particles: Vec<Particle>,
    order: Vec<usize>,
    instances: Vec<Instance>,
}

impl ParticleRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        depth: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WESL particles"),
            source: wgpu::ShaderSource::Wgsl(PARTICLE_SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(96),
                },
                count: None,
            }],
        });
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle camera"),
            size: 96,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particle camera"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particle layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Uint32, 4 => Uint32];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("instanced procedural particles"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attributes,
                })],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
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
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bounded particle instances"),
            size: (MAX_PARTICLES * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            camera,
            binding,
            buffer,
            particles: Vec::with_capacity(MAX_PARTICLES),
            order: Vec::with_capacity(MAX_PARTICLES),
            instances: Vec::with_capacity(MAX_PARTICLES),
        }
    }

    /// Reject invalid/oversized batches atomically. Empty input clears the frame.
    pub fn set_particles(&mut self, particles: &[Particle]) -> Result<()> {
        validate_batch(particles)?;
        self.particles.clear();
        self.particles.extend_from_slice(particles);
        Ok(())
    }

    pub fn prepare(&mut self, queue: &wgpu::Queue, view: Mat4, projection: Mat4) {
        self.order.clear();
        self.order.extend(
            self.particles
                .iter()
                .enumerate()
                .filter_map(|(i, p)| (p.age > 0.0 && p.age < 1.0 && p.color[3] > 0.0).then_some(i)),
        );
        // Unstable sort needs no scratch allocation; input index breaks depth ties.
        self.order.sort_unstable_by(|&a, &b| {
            let depth = |i: usize| {
                view.transform_point3(Vec3::from(self.particles[i].position))
                    .z
            };
            depth(a).total_cmp(&depth(b)).then(a.cmp(&b))
        });
        self.instances.clear();
        for &i in &self.order {
            let p = self.particles[i];
            let (sin, cos) = p.rotation.sin_cos();
            self.instances.push(Instance {
                center_age: [p.position[0], p.position[1], p.position[2], p.age],
                size_rotation: [p.half_size[0], p.half_size[1], cos, sin],
                color: p.color,
                seed: p.seed,
                effect: p.effect as u32,
            });
        }
        if self.instances.is_empty() {
            return;
        }
        let inverse = view.transpose();
        let mut camera = [0.0_f32; 24];
        camera[..16].copy_from_slice(&(projection * view).to_cols_array());
        camera[16..19].copy_from_slice(&inverse.x_axis.truncate().to_array());
        camera[20..23].copy_from_slice(&inverse.y_axis.truncate().to_array());
        queue.write_buffer(&self.camera, 0, bytemuck::cast_slice(&camera));
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.instances));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.instances.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.binding, &[]);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.draw(0..6, 0..self.instances.len() as u32);
    }
}

pub fn validate_batch(particles: &[Particle]) -> Result<()> {
    ensure!(
        particles.len() <= MAX_PARTICLES,
        "particle capacity exceeded ({MAX_PARTICLES})"
    );
    for particle in particles {
        particle.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shaders_validate_without_optional_capabilities() {
        let module = wgpu::naga::front::wgsl::parse_str(PARTICLE_SHADER).unwrap();
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
    }

    #[test]
    fn invalid_samples_and_capacity_are_rejected() {
        let mut p = Particle::preview(ParticleEffect::Smoke, 0.5);
        assert!(validate_batch(&[p]).is_ok());
        assert!(validate_batch(&vec![p; MAX_PARTICLES + 1]).is_err());
        p.age = f32::NAN;
        assert!(validate_batch(&[p]).is_err());
        p.age = 0.5;
        p.half_size[0] = 0.0;
        assert!(validate_batch(&[p]).is_err());
    }
}
