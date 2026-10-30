// stretch: B stretches out of the central line
fn trans_stretch(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    var c = uv.x - 0.5;
    if (p0.x > 0.5) { c = uv.y - 0.5; }
    let h = k * 0.5;
    if (abs(c) < h) {
        let c2 = c / max(k, 1e-4) + 0.5;
        if (p0.x > 0.5) { return samp(1u, vec2<f32>(uv.x, c2)); }
        return samp(1u, vec2<f32>(c2, uv.y));
    }
    return samp(0u, uv);
}
