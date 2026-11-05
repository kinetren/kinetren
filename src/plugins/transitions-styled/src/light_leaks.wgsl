// A glare spot.
fn leak_blob(uv: vec2<f32>, bx: f32, by: f32, aspect: f32) -> f32 {
    let dx = (uv.x - bx) * aspect;
    let dy = uv.y - by;
    let d = sqrt(dx * dx + dy * dy) / 0.45;
    let w = max(1.0 - d, 0.0);
    return w * w;
}

// light_leaks: two spots along the diagonals
fn trans_light_leaks(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let od = out_dims();
    let aspect = od.x / od.y;
    let pi = 3.14159265;
    let base = mix(samp(0u, uv), samp(1u, uv), k);
    let wave = sin(pi * k);
    let w = (leak_blob(uv, k * 1.2 - 0.1, 0.3 + 0.4 * k, aspect)
        + leak_blob(uv, 1.1 - 1.2 * k, 0.7 - 0.4 * k, aspect)) * p0.x * wave;
    let gate = clamp(w, 0.0, 1.0) * base.a / 255.0;
    let tint = vec3<f32>(p0.y, p0.z, p0.w);
    return vec4<f32>(mix(base.rgb, tint, gate), base.a);
}
