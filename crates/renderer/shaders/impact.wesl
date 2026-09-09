import package::particle_math::{angle, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let r = length(p);
    let a = angle(p) + f32(seed % 100u) * 0.07;
    let teeth = pow(max(0.0, cos(a * 11.0)), 7.0);
    let radius = (0.27 + 0.61 * teeth) * (0.65 + age * 0.3);
    let solid = coverage(r - radius, footprint);
    let core = exp2(-26.0 * r * r);
    return vec4f(mix(vec3f(1.0, 0.6, 0.13), vec3f(1.0), core), max(solid, core));
}
