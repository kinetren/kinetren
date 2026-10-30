// luma
fn trans_luma(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let c = samp(0u, uv);
    var l = (0.299 * c.r + 0.587 * c.g + 0.114 * c.b) / 255.0;
    if (p0.x > 0.5) { l = 1.0 - l; }
    return mix(c, samp(1u, uv), front_mask(l, k, p0.y));
}
