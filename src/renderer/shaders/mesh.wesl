import package::lighting::inspection_light;

struct Camera {
    view_projection: mat4x4f,
}

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var color_texture: texture_2d<f32>;
@group(1) @binding(1) var color_sampler: sampler;

struct Material {
    color: vec4f,
}

@group(2) @binding(0) var<uniform> material: Material;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) normal: vec3f,
    @location(1) uv: vec2f,
    @location(2) color: vec4f,
}

@vertex
fn vs_main(
    @location(0) position: vec3f,
    @location(1) normal: vec3f,
    @location(2) uv: vec2f,
    @location(3) color: vec4f,
) -> VertexOutput {
    var out: VertexOutput;
    out.position = camera.view_projection * vec4f(position, 1.0);
    out.normal = normal;
    out.uv = uv;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let vertex_color = in.color * material.color;
    let color = textureSample(color_texture, color_sampler, in.uv) * vertex_color;
    if color.a < 0.01 { discard; }
    return vec4f(color.rgb * inspection_light(in.normal), color.a);
}
