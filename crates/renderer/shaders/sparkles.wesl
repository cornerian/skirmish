import package::particle_math::{hash, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    var alpha = 0.0;
    for (var i = 0u; i < 12u; i++) {
        let a = hash(vec2u(i, 1u), seed) * 6.2831853;
        let radius = sqrt(hash(vec2u(i, 2u), seed)) * (0.3 + 0.55 * age);
        let q = abs(p - vec2f(cos(a), sin(a)) * radius);
        let size = 0.008 + hash(vec2u(i, 3u), seed) * 0.022;
        let d = min(q.x * 0.2 + q.y, q.x + q.y * 0.2) - size;
        alpha = max(alpha, coverage(d, footprint) * min(1.0, size / max(footprint, 0.001)));
    }
    return vec4f(vec3f(1.0, 0.94, 0.75), alpha);
}
