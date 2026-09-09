import package::particle_math::{rotate, coverage, segment};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = rotate(p, age * 2.0 - 0.7);
    let d = abs(q.x) - 0.24 * (1.0 - q.y * q.y / 0.56);
    let barbs = abs(fract(q.y * 8.0 - abs(q.x) * 6.0) - 0.5);
    let attenuation = 1.0 - smoothstep(0.03, 0.09, footprint);
    let gap = mix(0.84, smoothstep(0.04, 0.12, barbs), attenuation);
    let blade = coverage(max(d, abs(q.y) - 0.75), footprint) * gap;
    let stem = coverage(segment(q, vec2f(0.0, -0.88), vec2f(0.0, 0.73)) - 0.012, footprint);
    return vec4f(vec3f(0.92, 0.92, 0.8), max(blade, stem));
}
