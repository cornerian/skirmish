import package::particle_math::{turbulence, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = vec2f(p.x * 1.5, p.y);
    let n = turbulence(q + vec2f(0.0, -age * 2.0), footprint, seed);
    let d = length(q) + n * 0.5 - 0.65;
    let edge = coverage(d, max(0.015, footprint));
    let rim = smoothstep(-0.22, 0.0, d);
    return vec4f(mix(vec3f(0.015, 0.004, 0.03), vec3f(0.3, 0.025, 0.5), rim), edge);
}
