// slide
fn trans_slide(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let b = uv + p0.xy * (1.0 - k);
    if (b.x >= 0.0 && b.x < 1.0 && b.y >= 0.0 && b.y < 1.0) { return samp(1u, b); }
    return samp(0u, uv);
}
