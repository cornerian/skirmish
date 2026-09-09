import package::particle_math::{angle, rotate, segment, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let a = angle(p) + age * 0.4;
    let q = rotate(p, age * 0.4 - floor(a / 1.0471976 + 0.5) * 1.0471976)
          - vec2f(smoothstep(0.45, 1.0, age) * 0.12, 0.0);
    var d = segment(q, vec2f(0.08, 0.0), vec2f(0.83, 0.0));
    for (var i = 0u; i < 3u; i++) {
        let x = 0.25 + f32(i) * 0.18;
        d = min(d, segment(vec2f(q.x, abs(q.y)), vec2f(x, 0.0), vec2f(x + 0.14, 0.13)));
    }
    return vec4f(vec3f(0.6, 0.87, 1.0), coverage(d - 0.015, footprint));
}
