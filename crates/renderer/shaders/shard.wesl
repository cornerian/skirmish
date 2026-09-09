import package::particle_math::{rotate, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = rotate(p, age * 1.7 + f32(seed % 17u));
    let d = max(abs(q.x) - (0.7 - q.y) * 0.25, max(q.y - 0.7, -q.y - 0.55));
    let light = select(0.45, 0.92, q.x > 0.0);
    return vec4f(vec3f(light, light * 0.95, light * 0.84), coverage(d, footprint));
}
