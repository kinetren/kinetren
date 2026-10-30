// zoom: centre (p0.yz)
fn trans_zoom(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let o = p0.yz;
    let za = 1.0 + p0.x * k;
    let zb = 1.0 + p0.x * (1.0 - k);
    let sa = samp(0u, (uv - o) / za + o);
    let sb = samp(1u, (uv - o) / zb + o);
    return mix(sa, sb, k);
}
