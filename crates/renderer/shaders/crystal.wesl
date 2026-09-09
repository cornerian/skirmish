import package::particle_math::{coverage, rotate};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let q = abs(rotate(p, age * 0.5));
    let d = max(q.x * 0.8660254 + q.y * 0.5, q.y) - 0.62;
    let inner = coverage(d + 0.065, footprint);
    let facets = 0.4 + 0.4 * step(q.x, q.y * 0.6);
    let light = mix(1.0, facets, inner);
    return vec4f(vec3f(0.6, 0.85, 1.0) * light, coverage(d, footprint));
}
