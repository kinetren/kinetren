// page_peel: a cylindrical curl (item 401)
fn trans_page_peel(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let pi = 3.14159265;
    let r = max(p0.z, 0.01);
    let shadow = p0.w;
    let c = axis_c(uv, p0.xy);
    let f = 1.0 - k * (1.0 + pi * r);
    let back = 1.0 - shadow * 0.35;
    let m_tail = 2.0 * f + pi * r - c;
    if (c <= f && m_tail >= f + pi * r && m_tail <= 1.0) {
        let s0 = axis_sample(0u, uv, p0.xy, m_tail);
        return vec4<f32>(s0.rgb * back, s0.a);
    }
    let dc = c - f;
    if (dc >= 0.0 && dc <= r) {
        let th1 = atan2(dc, sqrt(max(r * r - dc * dc, 0.0)));
        let th2 = pi - th1;
        let m2 = f + r * th2;
        if (m2 <= 1.0) {
            let s0 = axis_sample(0u, uv, p0.xy, m2);
            return vec4<f32>(s0.rgb * back, s0.a);
        }
        let m1 = f + r * th1;
        if (m1 <= 1.0) {
            let sh = 1.0 - shadow * 0.3 * sin(th1);
            let s0 = axis_sample(0u, uv, p0.xy, m1);
            return vec4<f32>(s0.rgb * sh, s0.a);
        }
    }
    if (c <= f) { return samp(0u, uv); }
    let sh = 1.0 - shadow * 0.5 * max(1.0 - min(dc / max(r, 1e-4), 1.0), 0.0);
    let s1 = samp(1u, uv);
    return vec4<f32>(s1.rgb * sh, s1.a);
}
