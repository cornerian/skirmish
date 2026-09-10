import package::lighting::inspection_light;

struct Camera {
    view_projection: mat4x4f,
}

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var color_texture: texture_2d<f32>;
@group(1) @binding(1) var color_sampler: sampler;

struct Material {
    // Column-major model matrix for this runtime draw: the owning joint's
    // world transform for joint-local geometry, identity for world-baked parts.
    model: mat4x4f,
    // Column-major HSD texture matrix (rows 0 and 1 of MakeTextureMtx) over
    // (s, t, 0, 1); the original z column is folded into the translation
    // because a regular GX texgen feeds 1 there.
    uv_transform: mat4x4f,
    color: vec4f,
    alpha_test: vec4u,
}

@group(2) @binding(0) var<uniform> material: Material;

fn gx_compare(value: u32, reference: u32, comparison: u32) -> bool {
    if comparison == 0u { return false; }
    if comparison == 1u { return value < reference; }
    if comparison == 2u { return value == reference; }
    if comparison == 3u { return value <= reference; }
    if comparison == 4u { return value > reference; }
    if comparison == 5u { return value != reference; }
    if comparison == 6u { return value >= reference; }
    return true;
}

fn gx_alpha_passes(alpha: f32, control: vec4u) -> bool {
    // The export stores GX byte references. Quantize this preview's floating-point
    // texture/vertex alpha at the comparison boundary; upstream TEV math remains
    // an approximation rather than bit-exact GX fixed-point evaluation.
    let value = u32(clamp(alpha * 255.0 + 0.5, 0.0, 255.0));
    let left = gx_compare(value, control.y, control.x);
    let right = gx_compare(value, control.w & 255u, control.z);
    let operation = control.w >> 8u;
    if operation == 0u { return left && right; }
    if operation == 1u { return left || right; }
    if operation == 2u { return left != right; }
    return left == right;
}

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
    out.position = camera.view_projection * material.model * vec4f(position, 1.0);
    // The inspection light renormalizes, so non-uniform joint scale only
    // changes direction here, never magnitude.
    out.normal = (material.model * vec4f(normal, 0.0)).xyz;
    out.uv = (material.uv_transform * vec4f(uv, 0.0, 1.0)).xy;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let vertex_color = in.color * material.color;
    let color = textureSample(color_texture, color_sampler, in.uv) * vertex_color;
    if !gx_alpha_passes(color.a, material.alpha_test) { discard; }
    return vec4f(color.rgb * inspection_light(in.normal), color.a);
}
