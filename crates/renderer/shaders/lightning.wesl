import package::particle_math::{hash, segment};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    var distance = 2.0;
    var previous = vec2f(0.0, -0.87);
    for (var i = 1u; i <= 8u; i++) {
        let x = (hash(vec2u(i, 0u), seed) - 0.5) * 0.8;
        let next = vec2f(x + sin(age * 18.0 + f32(i)) * 0.025, -0.87 + f32(i) * 0.2175);
        distance = min(distance, segment(p, previous, next));
        if (i == 3u || i == 6u) {
            distance = min(distance, segment(p, next, next + vec2f(select(-0.3, 0.3, i == 3u), 0.22)));
        }
        previous = next;
    }
    let width = max(0.009, footprint);
    let core = exp2(-2.0 * distance * distance / (width * width));
    let glow = exp2(-40.0 * distance) * 0.4;
    return vec4f(mix(vec3f(0.28, 0.43, 1.0), vec3f(0.95, 0.98, 1.0), core), core + glow);
}
