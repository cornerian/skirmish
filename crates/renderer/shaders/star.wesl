import package::particle_math::{star_distance, coverage, rotate};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = rotate(p, age * 0.9);
    let d = star_distance(q);
    let rim = coverage(d - 0.03, footprint);
    let fill = coverage(d + 0.015, footprint);
    return vec4f(mix(vec3f(0.78, 0.57, 0.08), vec3f(1.0, 0.92, 0.55), fill), rim);
}
