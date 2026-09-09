import package::particle_math::{turbulence, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = p / (0.48 + age * 0.48);
    let n = turbulence(q + vec2f(age * 0.3, -age * 0.5), footprint, seed);
    let r = length(q);
    let density = coverage(r + n * 0.5 - 0.8, max(footprint, 0.04));
    let heat = clamp(1.1 - r + n * 1.4 - age * 0.7, 0.0, 1.0);
    let hot = mix(vec3f(0.95, 0.055, 0.002), vec3f(1.0, 0.95, 0.48), heat);
    let color = mix(vec3f(0.11, 0.085, 0.075), hot, smoothstep(0.0, 0.4, heat));
    return vec4f(color, density);
}
