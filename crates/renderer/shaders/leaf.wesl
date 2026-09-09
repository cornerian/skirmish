import package::particle_math::{rotate, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    var q = rotate(p, age * 3.0 + f32(seed % 5u));
    q.x += sin(q.y * 3.0 + age * 5.0) * 0.08;
    let d = abs(q.x) - 0.38 * (1.0 - q.y * q.y / 0.64);
    let blade = coverage(max(d, abs(q.y) - 0.8), footprint);
    let vein = 1.0 - smoothstep(0.01, max(0.02, footprint), abs(q.x));
    let branches = 1.0 - smoothstep(0.01, max(0.025, footprint), abs(fract(q.y * 3.0 - abs(q.x) * 2.0) - 0.5));
    let color = vec3f(0.07, 0.4, 0.015) + (vein + branches * 0.3) * vec3f(0.14, 0.18, 0.025);
    return vec4f(color, blade);
}
