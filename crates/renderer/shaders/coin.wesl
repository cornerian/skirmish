import package::particle_math::{star_distance, coverage};

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let spin = cos(age * 6.2831853);
    let aspect = max(0.13, abs(spin));
    let q = p / vec2f(aspect, 1.0);
    let r = length(q);
    let border = coverage(r - 0.74, footprint / aspect);
    let face = coverage(r - 0.65, footprint / aspect);
    let embossed = coverage(star_distance(q / 0.68), footprint / aspect);
    let bevel = clamp(0.7 - q.x * 0.4 + q.y * 0.25, 0.25, 1.0);
    let color = mix(vec3f(0.4, 0.2, 0.025), vec3f(1.0, 0.71, 0.13) * bevel, face);
    return vec4f(color + embossed * vec3f(0.12, 0.09, 0.025), border);
}
