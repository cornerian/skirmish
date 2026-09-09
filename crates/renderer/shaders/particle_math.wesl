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

fn coverage(distance: f32, footprint: f32) -> f32 {
    let width = max(0.004, footprint);
    return 1.0 - smoothstep(-width, width, distance);
}

fn segment(p: vec2f, a: vec2f, b: vec2f) -> f32 {
    let v = b - a;
    let t = clamp(dot(p - a, v) / max(dot(v, v), 0.000001), 0.0, 1.0);
    return length(p - a - t * v);
}

fn rotate(p: vec2f, angle: f32) -> vec2f {
    let s = sin(angle);
    let c = cos(angle);
    return vec2f(c * p.x - s * p.y, s * p.x + c * p.y);
}

fn angle(p: vec2f) -> f32 {
    // atan2(0, 0) is indeterminate in WGSL.
    return atan2(p.y, p.x + 0.000001);
}

fn star_distance(p: vec2f) -> f32 {
    let a = angle(p) + 1.5707963;
    let sector = floor(a / 1.2566371 + 0.5);
    // Signed polar boundary of a five-point star; avoids loops over edges.
    let t = abs(a - sector * 1.2566371);
    let radius = 0.7 * 0.4 / max(0.001, 0.4 * cos(t) + 0.623 * sin(t));
    return length(p) - radius;
}
