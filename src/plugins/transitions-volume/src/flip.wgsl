// flip: a panel by the homography from ext (item 401)
fn trans_flip(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let vert = p0.x > 0.5;
    var sx = uv.x; var sy = uv.y;
    if (vert) { sx = uv.y; sy = uv.x; }
    let st = hpanel(0u, sx, sy);
    if (st.z > 0.5 && st.x >= 0.0 && st.x <= 1.0 && st.y >= 0.0 && st.y <= 1.0) {
        let front = ext_at(8u) >= 0.0;
        var ms = st.x;
        if (!front) { ms = 1.0 - st.x; }
        var mu = ms; var mv = st.y;
        if (vert) { mu = st.y; mv = ms; }
        var which = 1u;
        if (front) { which = 0u; }
        let c = samp(which, vec2<f32>(mu, mv));
        return vec4<f32>(c.rgb * ext_at(9u), c.a);
    }
    return vec4<f32>(0.0);
}
