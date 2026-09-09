import package::particle_math::{angle, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let a = angle(p);
    let r = length(p);
    let radius = 0.18 + age * 0.64 + sin(a * 13.0) * 0.035;
    let stroke = 0.06 * (1.0 - age) + 0.01;
    return vec4f(vec3f(0.85, 0.94, 1.0), coverage(abs(r - radius) - stroke, footprint));
}
