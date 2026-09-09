import package::particle_math::coverage;

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = abs(p);
    let width = 0.06 * (1.0 - q.y / 0.92);
    let d = max(q.x - width, q.y - 0.92);
    let alpha = coverage(d, footprint) * min(1.0, 0.06 / max(0.001, footprint));
    return vec4f(vec3f(0.9, 0.96, 1.0), alpha);
}
