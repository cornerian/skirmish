import package::particle_math::coverage;

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = abs(p);
    let d = min(q.x * 0.12 + q.y, q.x + q.y * 0.12) - 0.065;
    let core = exp2(-150.0 * dot(p, p));
    let rays = coverage(d, footprint) * (1.0 - smoothstep(0.15, 0.9, max(q.x, q.y)));
    return vec4f(vec3f(0.9, 0.96, 1.0), max(core, rays));
}
