import package::particle_math::{angle, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let r = length(p);
    let a = angle(p) + age * 0.4;
    let rays = pow(max(0.0, cos(a * 12.0)), 7.0);
    let core = exp2(-45.0 * r * r);
    let corona = coverage(r - (0.27 + 0.55 * rays), footprint) * 0.7;
    let halo = exp2(-5.0 * r * r) * 0.2;
    return vec4f(mix(vec3f(0.35, 0.015, 1.0), vec3f(1.0, 0.65, 0.98), core + corona * 0.4),
                 (core + corona + halo) * coverage(r - 0.95, footprint));
}
