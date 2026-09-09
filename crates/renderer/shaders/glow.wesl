import package::particle_math::{angle, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let r = length(p);
    let core = exp2(-90.0 * r * r);
    let halo = exp2(-9.0 * r * r) * 0.28;
    let rays = pow(max(0.0, cos(angle(p) * 12.0)), 10.0);
    let filtered = 1.0 - smoothstep(0.1, 0.4, footprint * 12.0 / max(r, 0.05));
    let alpha = (core + halo * (1.0 + rays * filtered)) * coverage(r - 0.95, footprint);
    return vec4f(vec3f(0.85, 0.93, 1.0), alpha);
}
