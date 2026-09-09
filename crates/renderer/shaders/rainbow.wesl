import package::particle_math::{angle, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let a = angle(p) + age * 2.0;
    let r = length(p);
    let hue = vec3f(0.5) + 0.5 * cos(vec3f(a) + vec3f(0.0, 2.0943951, 4.1887902));
    let petals = 0.35 + 0.4 * pow(max(0.0, cos(a * 6.0)), 4.0);
    let alpha = coverage(r - petals, footprint) * (0.2 + 0.8 * exp2(-5.0 * r * r));
    return vec4f(hue, alpha);
}
