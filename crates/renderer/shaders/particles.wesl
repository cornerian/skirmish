import package::feather;
import package::leaf;
import package::splash;
import package::music_note;
import package::rainbow;
import package::coin;
import package::rock;
import package::mote;
import package::crystal;
import package::bubble;
import package::streak;
import package::snowflake;
import package::explosion;
import package::star;
import package::arc;
import package::glint;
import package::energy_burst;
import package::shockwave;
import package::shard;
import package::lightning;
import package::vortex;
import package::impact;
import package::sparkles;
import package::embers;
import package::glow;
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
        case 26u: { field = feather::shade(in.uv, in.age, in.seed, footprint); }
        case 25u: { field = leaf::shade(in.uv, in.age, in.seed, footprint); }
        case 24u: { field = splash::shade(in.uv, in.age, in.seed, footprint); }
        case 23u: { field = music_note::shade(in.uv, in.age, in.seed, footprint); }
        case 22u: { field = rainbow::shade(in.uv, in.age, in.seed, footprint); }
        case 21u: { field = coin::shade(in.uv, in.age, in.seed, footprint); }
        case 20u: { field = rock::shade(in.uv, in.age, in.seed, footprint); }
        case 19u: { field = mote::shade(in.uv, in.age, in.seed, footprint); }
        case 18u: { field = crystal::shade(in.uv, in.age, in.seed, footprint); }
        case 17u: { field = bubble::shade(in.uv, in.age, in.seed, footprint); }
        case 16u: { field = streak::shade(in.uv, in.age, in.seed, footprint); }
        case 15u: { field = snowflake::shade(in.uv, in.age, in.seed, footprint); }
        case 14u: { field = explosion::shade(in.uv, in.age, in.seed, footprint); }
        case 13u: { field = star::shade(in.uv, in.age, in.seed, footprint); }
        case 12u: { field = arc::shade(in.uv, in.age, in.seed, footprint); }
        case 11u: { field = glint::shade(in.uv, in.age, in.seed, footprint); }
        case 10u: { field = energy_burst::shade(in.uv, in.age, in.seed, footprint); }
        case 9u: { field = shockwave::shade(in.uv, in.age, in.seed, footprint); }
        case 8u: { field = shard::shade(in.uv, in.age, in.seed, footprint); }
        case 7u: { field = lightning::shade(in.uv, in.age, in.seed, footprint); }
        case 6u: { field = vortex::shade(in.uv, in.age, in.seed, footprint); }
        case 5u: { field = impact::shade(in.uv, in.age, in.seed, footprint); }
        case 4u: { field = sparkles::shade(in.uv, in.age, in.seed, footprint); }
        case 3u: { field = embers::shade(in.uv, in.age, in.seed, footprint); }
        case 2u: { field = glow::shade(in.uv, in.age, in.seed, footprint); }
        case 1u: { field = fire::shade(in.uv, in.age, in.seed, footprint); }
        default: { field = smoke::shade(in.uv, in.age, in.seed, footprint); }
    }
    let lifetime = select(1.0, smoothstep(0.0, 0.04, in.age) * (1.0 - in.age), in.effect >= 2u);
    let alpha = clamp(field.a * in.color.a * lifetime, 0.0, 1.0);
    // Emissive particles use source alpha zero with premultiplied blending:
    // source RGB adds while destination color and alpha remain intact.
    let emissive = in.effect == 22u || in.effect == 16u || in.effect == 11u || in.effect == 10u || in.effect == 7u || in.effect == 5u || in.effect == 4u || in.effect == 3u || in.effect == 2u;
    return vec4f(field.rgb * in.color.rgb * alpha, select(alpha, 0.0, emissive));
}
