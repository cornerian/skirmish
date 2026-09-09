import package::particle_math::coverage;

fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    return vec4f(vec3f(1.0), coverage(length(p) - 0.5, footprint));
}
