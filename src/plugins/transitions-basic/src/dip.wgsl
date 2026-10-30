// dip
fn trans_dip(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    var src = samp(1u, uv);
    var t = 2.0 - k * 2.0;
    if (k < 0.5) { src = samp(0u, uv); t = k * 2.0; }
    return vec4<f32>(mix(src.rgb, p0.rgb, t), src.a);
}
