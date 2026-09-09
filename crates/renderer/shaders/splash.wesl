import package::particle_math::{hash, segment, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    var d = 2.0;
    for (var i = 0u; i < 9u; i++) {
        let a = 0.22 + f32(i) * 0.338;
        let speed = 0.5 + hash(vec2u(i, 0u), seed) * 0.4;
        let endpoint = vec2f(cos(a) * speed, sin(a) * speed) * (0.3 + age * 0.8) + vec2f(0.0, -0.35 - age * age * 0.35);
        let begin = mix(vec2f(0.0, -0.4), endpoint, age);
        d = min(d, segment(p, begin, endpoint) - 0.008 * (1.0 - age));
        let droplet = (length((p - endpoint) / vec2f(0.024, 0.042)) - 1.0) * 0.024;
        d = min(d, droplet);
    }
    let base = (length((p - vec2f(0.0, -0.4)) / vec2f(0.22, 0.07)) - 1.0) * 0.07;
    let alpha = max(coverage(d, footprint), coverage(base, footprint) * (1.0 - age));
    return vec4f(vec3f(0.65, 0.86, 1.0), alpha);
}
