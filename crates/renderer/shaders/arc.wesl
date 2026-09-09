import package::particle_math::{angle, coverage, rotate};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = rotate(p, age * 2.0);
    let a = angle(q);
    let progress = clamp((a + 2.0) / 4.0, 0.0, 1.0);
    let width = 0.015 + progress * 0.08;
    let ring = coverage(abs(length(q) - 0.68) - width, footprint);
    let ends = smoothstep(-2.0, -1.8, a) * (1.0 - smoothstep(1.8, 2.0, a));
    return vec4f(vec3f(0.8, 0.9, 1.0), ring * ends * progress);
}
