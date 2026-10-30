// barn
fn trans_barn(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    var s = abs(uv.y - 0.5) * 2.0;
    if (p0.x > 0.5) { s = abs(uv.x - 0.5) * 2.0; }
    return mix(samp(0u, uv), samp(1u, uv), front_mask(s, k, p0.y));
}
