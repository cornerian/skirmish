// Compact integer noise: stable seeds, no textures or trigonometric hash.
fn hash(p: vec2u, seed: u32) -> f32 {
    var h = (p.x * 1597334677u) ^ (p.y * 3812015801u) ^ seed;
    h = (h ^ (h >> 16u)) * 2246822519u;
    h = (h ^ (h >> 13u)) * 3266489917u;
    return f32(h ^ (h >> 16u)) / 4294967295.0;
}

fn noise(p: vec2f, seed: u32) -> f32 {
    let cell = vec2i(floor(p));
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(bitcast<vec2u>(cell), seed),
                   hash(bitcast<vec2u>(cell + vec2i(1, 0)), seed), w.x),
               mix(hash(bitcast<vec2u>(cell + vec2i(0, 1)), seed),
                   hash(bitcast<vec2u>(cell + vec2i(1, 1)), seed), w.x), w.y);
}

// Remove frequencies above the pixel footprint to avoid distant shimmer.
fn turbulence(p: vec2f, footprint: f32, seed: u32) -> f32 {
    var sum = 0.0;
    var frequency = 4.0;
    var amplitude = 0.55;
    for (var octave = 0u; octave < 3u; octave++) {
        let weight = 1.0 - smoothstep(0.25, 0.7, footprint * frequency);
        sum += (noise(p * frequency, seed + octave) - 0.5) * amplitude * weight;
        frequency *= 2.0;
        amplitude *= 0.5;
    }
    return sum;
}
