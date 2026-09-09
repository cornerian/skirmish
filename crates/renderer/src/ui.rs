//! Small native menu presentation, drawn by the shared wgpu surface.
//!
//! Built-in bitmap glyphs make this preview independent of extracted game artwork
//! or system fonts. Both menu geometry and glyphs use the same WESL pipeline.

use crate::menu::MenuView;
use bytemuck::{Pod, Zeroable};
use font8x8::UnicodeFonts;
use wgpu::util::DeviceExt;

pub const UI_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/ui.wgsl"));

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct UiVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

/// Triangles in normalized device coordinates. Kept independent of a GPU so
/// layout can be inspected and verified on a headless machine.
#[derive(Clone, Debug, Default)]
pub struct UiFrame {
    pub vertices: Vec<UiVertex>,
}

impl UiFrame {
    pub fn menu(view: &MenuView, width: u32, height: u32) -> Self {
        if width == 0 || height == 0 {
            return Self::default();
        }
        let mut canvas = Canvas::new(width, height);
        canvas.screen_rect([0.0, 0.0, width as f32, height as f32], BACKGROUND);
        canvas.rect([0.0, 0.0, 960.0, 720.0], BACKGROUND);
        canvas.rect([740.0, 0.0, 220.0, 720.0], [0.019, 0.034, 0.064, 1.0]);
        canvas.rect([48.0, 48.0, 6.0, 51.0], ACCENT);
        canvas.text("SKIRMISH", [72.0, 49.0], 6.0, TEXT);
        canvas.text(view.breadcrumb, [48.0, 121.0], 1.5, MUTED);
        canvas.text(view.title, [48.0, 157.0], 3.0, TEXT);
        canvas.rect([48.0, 204.0, 864.0, 1.0], BORDER);

        if view.pending.is_some() {
            canvas.rect([48.0, 230.0, 864.0, 328.0], PANEL);
            canvas.rect([48.0, 230.0, 6.0, 328.0], ACCENT);
            canvas.text("SCREEN UNAVAILABLE", [80.0, 264.0], 2.0, ACCENT);
            canvas.text(view.selected_label, [80.0, 316.0], 3.0, TEXT);
            canvas.text(
                "This screen is not available yet.",
                [80.0, 386.0],
                2.0,
                TEXT,
            );
            canvas.text(
                "Press Back to return to the menu.",
                [80.0, 426.0],
                2.0,
                MUTED,
            );
        } else {
            let row_height = if view.rows.len() > 6 { 35.0 } else { 61.0 };
            let text_scale = if view.rows.len() > 6 { 2.0 } else { 3.0 };
            for (position, row) in view.rows.iter().enumerate() {
                let y = 222.0 + position as f32 * row_height;
                if row.selected {
                    canvas.rect([48.0, y, 864.0, row_height - 5.0], SELECTED);
                    canvas.rect([48.0, y, 6.0, row_height - 5.0], ACCENT);
                    canvas.text(
                        ">",
                        [76.0, y + (row_height - text_scale * 8.0) / 2.0 - 2.0],
                        text_scale,
                        ACCENT,
                    );
                } else {
                    canvas.rect([48.0, y, 864.0, row_height - 5.0], PANEL);
                }
                let text_y = y + (row_height - text_scale * 8.0) / 2.0 - 2.0;
                canvas.text(
                    row.label,
                    [112.0, text_y],
                    text_scale,
                    if row.selected { TEXT } else { MUTED },
                );
                if row.selected {
                    canvas.text("<", [862.0, text_y], text_scale, ACCENT);
                }
            }
        }

        canvas.rect([48.0, 602.0, 864.0, 1.0], BORDER);
        canvas.text(
            "ARROWS / D-PAD  Move    ENTER / A  Select",
            [48.0, 624.0],
            2.0,
            TEXT,
        );
        canvas.text(
            "ESC / B  Back    F1  Scene preview",
            [48.0, 654.0],
            1.5,
            MUTED,
        );
        canvas.frame
    }
}

const BACKGROUND: [f32; 4] = [0.009, 0.016, 0.030, 1.0];
const PANEL: [f32; 4] = [0.019, 0.029, 0.048, 1.0];
const SELECTED: [f32; 4] = [0.043, 0.100, 0.139, 1.0];
const BORDER: [f32; 4] = [0.078, 0.120, 0.174, 1.0];
const TEXT: [f32; 4] = [0.88, 0.94, 1.0, 1.0];
const MUTED: [f32; 4] = [0.40, 0.52, 0.64, 1.0];
const ACCENT: [f32; 4] = [0.96, 0.58, 0.08, 1.0];

struct Canvas {
    frame: UiFrame,
    size: [f32; 2],
    scale: f32,
    origin: [f32; 2],
}

impl Canvas {
    fn new(width: u32, height: u32) -> Self {
        let size = [width as f32, height as f32];
        let scale = (size[0] / 960.0).min(size[1] / 720.0);
        Self {
            frame: UiFrame::default(),
            size,
            scale,
            origin: [
                (size[0] - 960.0 * scale) / 2.0,
                (size[1] - 720.0 * scale) / 2.0,
            ],
        }
    }

    fn rect(&mut self, [x, y, width, height]: [f32; 4], color: [f32; 4]) {
        self.screen_rect(
            [
                self.origin[0] + x * self.scale,
                self.origin[1] + y * self.scale,
                width * self.scale,
                height * self.scale,
            ],
            color,
        );
    }

    fn screen_rect(&mut self, [x, y, width, height]: [f32; 4], color: [f32; 4]) {
        let left = x / self.size[0] * 2.0 - 1.0;
        let right = (x + width) / self.size[0] * 2.0 - 1.0;
        let top = 1.0 - y / self.size[1] * 2.0;
        let bottom = 1.0 - (y + height) / self.size[1] * 2.0;
        for position in [
            [left, top],
            [left, bottom],
            [right, top],
            [right, top],
            [left, bottom],
            [right, bottom],
        ] {
            self.frame.vertices.push(UiVertex { position, color });
        }
    }

    fn text(&mut self, text: &str, [x, y]: [f32; 2], scale: f32, color: [f32; 4]) {
        for (column, character) in text.chars().enumerate() {
            let Some(glyph) = font8x8::BASIC_FONTS.get(character) else {
                continue;
            };
            for (row, bits) in glyph.into_iter().enumerate() {
                // Coalesce adjacent pixels into one rectangle to reduce uploads.
                let mut bit = 0;
                while bit < 8 {
                    if bits & (1 << bit) == 0 {
                        bit += 1;
                        continue;
                    }
                    let start = bit;
                    while bit < 8 && bits & (1 << bit) != 0 {
                        bit += 1;
                    }
                    self.rect(
                        [
                            x + (column * 8 + start) as f32 * scale,
                            y + row as f32 * scale,
                            (bit - start) as f32 * scale,
                            scale,
                        ],
                        color,
                    );
                }
            }
        }
    }
}

pub struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    vertices: Option<wgpu::Buffer>,
    vertex_count: u32,
    capacity: u64,
}

impl UiRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("native menu WESL"),
            source: wgpu::ShaderSource::Wgsl(UI_SHADER.into()),
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("native menu"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            vertices: None,
            vertex_count: 0,
            capacity: 0,
        }
    }

    /// Rebuild on view or drawable-size changes; unchanged frames reuse the buffer.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &MenuView,
        width: u32,
        height: u32,
    ) {
        let frame = UiFrame::menu(view, width, height);
        self.vertex_count = frame.vertices.len() as u32;
        let bytes = bytemuck::cast_slice(&frame.vertices);
        if bytes.is_empty() {
            return;
        }
        if bytes.len() as u64 > self.capacity {
            self.vertices = Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("native menu vertices"),
                    contents: bytes,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                }),
            );
            self.capacity = bytes.len() as u64;
        } else if let Some(buffer) = &self.vertices {
            queue.write_buffer(buffer, 0, bytes);
        }
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let Some(vertices) = &self.vertices {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.draw(0..self.vertex_count, 0..1);
        }
    }
}
