import package::particle_math::{angle, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let r = length(p);
    let phase = angle(p) * 4.0 + r * 12.0 - age * 12.0;
    let arms = pow(max(0.0, cos(phase)), 5.0);
    let attenuation = 1.0 - smoothstep(0.4, 1.2, footprint * (12.0 + 4.0 / max(r, 0.04)));
    let mask = smoothstep(0.04, 0.2, r) * (1.0 - smoothstep(0.6, 0.93, r));
    return vec4f(vec3f(0.65, 0.82, 1.0), (arms * attenuation + 0.07) * mask);
}
