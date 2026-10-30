// push: B stands right up BEHIND A, on screen further along +dir (item 396)
fn trans_push(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let a = uv - p0.xy * k;
    if (a.x >= 0.0 && a.x < 1.0 && a.y >= 0.0 && a.y < 1.0) { return samp(0u, a); }
    return samp(1u, a + p0.xy);
}
