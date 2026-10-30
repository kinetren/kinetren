// page_roll: the visible top turn of the roll
fn trans_page_roll(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let pi = 3.14159265;
    let r = max(p0.z, 0.01);
    let shadow = p0.w;
    let c = axis_c(uv, p0.xy);
    let f = 1.0 - k * (1.0 + 2.0 * r);
    let back = 1.0 - shadow * 0.35;
    let dcr = f - c;
    if (abs(dcr) <= r) {
        let ad = abs(dcr);
        let th1 = atan2(ad, sqrt(max(r * r - ad * ad, 0.0)));
        var th = pi + th1;
        if (dcr >= 0.0) { th = pi - th1; }
        let mw = f + r * th;
        if (mw <= 1.0) {
            let sh = back * (1.0 - 0.2 * (1.0 - abs(sin(th))));
            let s0 = axis_sample(0u, uv, p0.xy, mw);
            return vec4<f32>(s0.rgb * sh, s0.a);
        }
        let m1 = f + r * th1;
        if (dcr >= 0.0 && m1 <= 1.0) {
            let s0 = axis_sample(0u, uv, p0.xy, m1);
            return vec4<f32>(s0.rgb * back, s0.a);
        }
    }
    if (c <= f) { return samp(0u, uv); }
    let sh = 1.0 - shadow * 0.5 * max(1.0 - min((c - f) / max(r, 1e-4), 1.0), 0.0);
    let s1 = samp(1u, uv);
    return vec4<f32>(s1.rgb * sh, s1.a);
}
