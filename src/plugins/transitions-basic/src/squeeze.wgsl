// The axial “back to uv” of squeeze: (R.51).
fn sq_back(uv: vec2<f32>, c2: f32, d: vec2<f32>) -> vec2<f32> {
    if (d.x > 0.5) { return vec2<f32>(c2, uv.y); }
    if (d.x < -0.5) { return vec2<f32>(1.0 - c2, uv.y); }
    if (d.y > 0.5) { return vec2<f32>(uv.x, c2); }
    return vec2<f32>(uv.x, 1.0 - c2);
}

// squeeze: A is squeezed, B unfolds along dir
fn trans_squeeze(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    var c = 1.0 - uv.y;
    if (p0.x > 0.5) { c = uv.x; }
    else if (p0.x < -0.5) { c = 1.0 - uv.x; }
    else if (p0.y > 0.5) { c = uv.y; }
    if (c < k) {
        let c2 = c / max(k, 1e-4);
        return samp(1u, sq_back(uv, c2, p0.xy));
    }
    let c2 = (c - k) / max(1.0 - k, 1e-4);
    return samp(0u, sq_back(uv, c2, p0.xy));
}
