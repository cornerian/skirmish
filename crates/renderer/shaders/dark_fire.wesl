import package::fire;

// Violet EfGnData flame frames have their own palette, not an orange-fire tint.
fn shade(p: vec2f, age: f32, seed: u32, footprint: f32) -> vec4f {
    let flame = fire::shade(p, age, seed, footprint);
    let heat = smoothstep(0.02, 0.8, flame.g);
    let color = mix(vec3f(0.035, 0.002, 0.06), vec3f(0.8, 0.025, 0.95), heat);
    return vec4f(color, flame.a);
}
