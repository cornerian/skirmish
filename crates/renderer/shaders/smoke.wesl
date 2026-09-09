import package::particle_math::turbulence;

fn merge(a: f32, b: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / 0.3, 0.0, 1.0);
    return mix(b, a, h) - 0.3 * h * (1.0 - h);
}

// Redesign of the catalogued EfCoData smoke-like puff, not its bytecode.
// Three filtered noise bands replace six CPU octaves and baked frames.
fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = p / mix(0.62, 1.0, age);
    let n = turbulence(q * 0.6 + vec2f(0.0, -age * 0.6), footprint, seed);
    let lobe = merge(length(q + vec2f(0.26, -0.05)),
                   merge(length(q + vec2f(-0.22, -0.18)),
                       length(q + vec2f(0.0, 0.3))));
    let density = 1.0 - smoothstep(0.22, 0.74, lobe + n * 0.6);
    let edge = 1.0 - smoothstep(0.8, 0.99, max(abs(p.x), abs(p.y)));
    let alpha = density * edge * (1.0 - age) * smoothstep(0.0, 0.04, age);
    let light = clamp(0.7 + n + q.y * 0.12, 0.35, 1.0);
    return vec4f(vec3f(light), alpha);
}
