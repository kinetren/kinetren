// cube: two faces, the order is fixed (B, then A)
fn trans_cube(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let pi = 3.14159265;
    let vert = abs(p0.y) > 0.5;
    var sx = uv.x; var sy = uv.y;
    if (vert) { sx = uv.y; sy = uv.x; }
    let sh = 1.0 - p0.w * sin(pi * k) * 0.5;
    let stb = hpanel(8u, sx, sy);
    if (stb.z > 0.5 && stb.x >= 0.0 && stb.x <= 1.0 && stb.y >= 0.0 && stb.y <= 1.0) {
        var mu = stb.x; var mv = stb.y;
        if (vert) { mu = stb.y; mv = stb.x; }
        let c = samp(1u, vec2<f32>(mu, mv));
        return vec4<f32>(c.rgb * sh, c.a);
    }
    let sta = hpanel(0u, sx, sy);
    if (sta.z > 0.5 && sta.x >= 0.0 && sta.x <= 1.0 && sta.y >= 0.0 && sta.y <= 1.0) {
        var mu = sta.x; var mv = sta.y;
        if (vert) { mu = sta.y; mv = sta.x; }
        let c = samp(0u, vec2<f32>(mu, mv));
        return vec4<f32>(c.rgb * sh, c.a);
    }
    return vec4<f32>(0.0);
}
