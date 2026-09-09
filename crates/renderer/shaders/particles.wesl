import package::smoke;
import package::fire;

struct Camera {
    view_projection: mat4x4f,
    right: vec4f,
    up: vec4f,
}
@group(0) @binding(0) var<uniform> camera: Camera;

struct Output {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec4f,
    @location(2) @interpolate(flat) age: f32,
    @location(3) @interpolate(flat) seed: u32,
    @location(4) @interpolate(flat) effect: u32,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32,
           @location(0) center_age: vec4f,
           @location(1) size_rotation: vec4f,
           @location(2) color: vec4f,
           @location(3) seed: u32,
           @location(4) effect: u32) -> Output {
    let corners = array<vec2f, 6>(vec2f(-1, -1), vec2f(1, -1), vec2f(-1, 1),
                                 vec2f(-1, 1), vec2f(1, -1), vec2f(1, 1));
    let uv = corners[vertex];
    let local = uv * size_rotation.xy;
    let rotated = vec2f(local.x * size_rotation.z - local.y * size_rotation.w,
                       local.x * size_rotation.w + local.y * size_rotation.z);
    let world = center_age.xyz + camera.right.xyz * rotated.x + camera.up.xyz * rotated.y;
    var out: Output;
    out.position = camera.view_projection * vec4f(world, 1.0);
    out.uv = uv;
    out.color = color;
    out.age = center_age.w;
    out.seed = seed;
    out.effect = effect;
    return out;
}

@fragment
fn fs_main(in: Output) -> @location(0) vec4f {
    let footprint = max(length(dpdx(in.uv)), length(dpdy(in.uv)));
    var field: vec4f;
    switch in.effect {
        case 1u: { field = fire::shade(in.uv, in.age, in.seed, footprint); }
        default: { field = smoke::shade(in.uv, in.age, in.seed, footprint); }
    }
    let alpha = clamp(field.a * in.color.a, 0.0, 1.0);
    return vec4f(field.rgb * in.color.rgb * alpha, alpha);
}
