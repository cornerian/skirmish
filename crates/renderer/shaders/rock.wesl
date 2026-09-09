import package::particle_math::{angle, rotate, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = rotate(p, age * 2.2);
    let a = angle(q);
    let sector = floor(a / 1.0471976 + 0.5);
    let facet = cos(a - sector * 1.0471976);
    let d = length(q) * facet - (0.59 + sin(sector * 3.0) * 0.06);
    let light = clamp(0.55 + q.y * 0.25 - q.x * 0.2 + sin(sector * 2.0) * 0.15, 0.15, 0.8);
    return vec4f(vec3f(light * 0.8, light * 0.85, light), coverage(d, footprint));
}
