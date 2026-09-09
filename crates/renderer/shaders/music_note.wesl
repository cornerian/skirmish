import package::particle_math::{coverage, segment, rotate};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = rotate(p, sin(age * 4.0) * 0.13);
    let head = (length(rotate(q - vec2f(-0.12, -0.45), -0.35) / vec2f(0.3, 0.21)) - 1.0) * 0.21;
    let stem = segment(q, vec2f(0.15, -0.43), vec2f(0.15, 0.72)) - 0.035;
    let flag = segment(q, vec2f(0.15, 0.69), vec2f(0.48, 0.52)) - 0.07;
    let d = min(head, min(stem, flag));
    let gloss = exp2(-25.0 * dot(q - vec2f(-0.2, -0.38), q - vec2f(-0.2, -0.38)));
    return vec4f(vec3f(0.04 + gloss * 0.6), coverage(d, footprint));
}
