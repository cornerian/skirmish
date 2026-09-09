import package::particle_math::{hash, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    var field = 0.0;
    for (var i = 0u; i < 5u; i++) {
        let n = hash(vec2u(i, 0u), seed);
        let a = f32(i) * 1.2566371 + n;
        let center = vec2f(cos(a), sin(a)) * (0.18 + age * 0.5);
        let q = (p - center) / vec2f(0.1 + n * 0.08, 0.18);
        field += exp2(-3.0 * dot(q, q));
    }
    return vec4f(mix(vec3f(1.0, 0.13, 0.01), vec3f(1.0, 0.85, 0.3), min(field, 1.0)),
                 min(field, 1.0) * coverage(length(p) - 0.95, footprint));
}
