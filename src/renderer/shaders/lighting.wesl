// Inspection light only; game lighting and GX TEV are not emulated.
fn inspection_light(normal: vec3f) -> f32 {
    let length_squared = dot(normal, normal);
    let n = normal * inverseSqrt(max(length_squared, 0.000001));
    return 0.4 + 0.6 * max(dot(n, normalize(vec3f(0.4, 0.8, 0.6))), 0.0);
}
