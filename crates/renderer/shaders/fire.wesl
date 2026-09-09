import package::particle_math::turbulence;

// Detail rises independently of the simulation-owned world position.
fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let h = p.y * 0.5 + 0.5;
    let n = turbulence(vec2f(p.x, p.y - age * 2.8), footprint, seed);
    let width = mix(0.56, 0.02, h * h);
    let sway = sin(h * 8.0 - age * 6.0) * h * 0.11;
    let d = abs(p.x - sway + n * h * 0.4) - width + n * 0.22;
    let edge = 1.0 - smoothstep(-0.07, max(0.07, footprint), d);
    let base = smoothstep(0.02, 0.22, h);
    let tip = 1.0 - smoothstep(0.85, 0.99, h + n * 0.15);
    let alpha = edge * base * tip * smoothstep(0.0, 0.04, age) * (1.0 - smoothstep(0.7, 1.0, age));
    let heat = clamp(1.0 - h - abs(p.x) + n * 0.4, 0.0, 1.0);
    let color = mix(vec3f(1.0, 0.055, 0.005), vec3f(1.0, 0.9, 0.32), heat);
    return vec4f(color, alpha);
}
