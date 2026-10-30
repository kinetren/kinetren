// door: the leaf and its mirror, with the incoming one behind them
fn trans_door(uv: vec2<f32>, k: f32) -> vec4<f32> {
    let p0 = P.f0;
    let vert = p0.x > 0.5;
    var sx = uv.x; var sy = uv.y;
    if (vert) { sx = uv.y; sy = uv.x; }
    let sh = ext_at(8u);
    let stl = hpanel(0u, sx, sy);
    if (stl.z > 0.5 && stl.x >= 0.0 && stl.x <= 0.5 && stl.y >= 0.0 && stl.y <= 1.0) {
        var mu = stl.x; var mv = stl.y;
        if (vert) { mu = stl.y; mv = stl.x; }
        let c = samp(0u, vec2<f32>(mu, mv));
        return vec4<f32>(c.rgb * sh, c.a);
    }
    let str = hpanel(0u, 1.0 - sx, sy);
    if (str.z > 0.5 && str.x >= 0.0 && str.x <= 0.5 && str.y >= 0.0 && str.y <= 1.0) {
        let ms = 1.0 - str.x;
        var mu = ms; var mv = str.y;
        if (vert) { mu = str.y; mv = ms; }
        let c = samp(0u, vec2<f32>(mu, mv));
        return vec4<f32>(c.rgb * sh, c.a);
    }
    return samp(1u, uv);
}
