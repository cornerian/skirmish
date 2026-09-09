import package::particle_math::{coverage, turbulence};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = p / 0.77;
    let r = length(q);
    let body = coverage(r - 1.0, footprint);
    let z = sqrt(max(0.0, 1.0 - dot(q, q)));
    let rim = pow(1.0 - z, 3.0);
    let shine = exp2(-90.0 * dot(q - vec2f(-0.34, 0.42), q - vec2f(-0.34, 0.42)));
    let n = turbulence(q * 0.6 + vec2f(age * 0.2), footprint, seed);
    let color = vec3f(0.45, 0.67, 0.9) * (0.5 + rim) + vec3f(shine * 0.8 + n * 0.2);
    return vec4f(color, body * clamp(0.1 + rim * 0.75 + shine, 0.0, 1.0));
}
