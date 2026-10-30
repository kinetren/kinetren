// split: the halves of A slide apart, B is in the gap
fn trans_split(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    var c = uv.x - 0.5;
    if (p0.x > 0.5) { c = uv.y - 0.5; }
    let h = k * 0.5;
    if (abs(c) >= h) {
        let off = h * sign(c);
        if (p0.x > 0.5) { return samp(0u, vec2<f32>(uv.x, uv.y - off)); }
        return samp(0u, vec2<f32>(uv.x - off, uv.y));
    }
    return samp(1u, uv);
}
