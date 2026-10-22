// Radial blur (R.47): a mirror of kt_fx::video::radial_blur:
// the average of EXACTLY 16 taps of a zoom ray or an arc (a constant of both backends).
// u0 = (w, h, spin); f0 = (amount, cx_norm, cy_norm, 0).
fn bilin(sx: f32, sy: f32) -> vec4<f32> {
    let sw = i32(P.u0.x);
    let sh = i32(P.u0.y);
    let xf = floor(sx);
    let yf = floor(sy);
    let fx = sx - xf;
    let fy = sy - yf;
    let x = i32(xf);
    let y = i32(yf);
    var acc = vec4<f32>(0.0);
    for (var dy = 0; dy < 2; dy = dy + 1) {
        let wy = select(fy, 1.0 - fy, dy == 0);
        let yi = clamp(y + dy, 0, sh - 1);
        for (var dx = 0; dx < 2; dx = dx + 1) {
            let wx = select(fx, 1.0 - fx, dx == 0);
            let xi = clamp(x + dx, 0, sw - 1);
            acc = acc + vec4<f32>(textureLoad(T0, vec2<i32>(xi, yi), 0)) * (wy * wx);
        }
    }
    return acc;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = floor(pos.xy);
    let amount = P.f0.x;
    let cx = P.f0.y * f32(P.u0.x);
    let cy = P.f0.z * f32(P.u0.y);
    let px = p.x + 0.5 - cx;
    let py = p.y + 0.5 - cy;
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < 16; i = i + 1) {
        let t = f32(i) / 15.0;
        var sx = 0.0;
        var sy = 0.0;
        if (P.u0.z != 0u) {
            let a = 0.6 * amount * (t - 0.5);
            let sa = sin(a);
            let ca = cos(a);
            sx = px * ca - py * sa;
            sy = px * sa + py * ca;
        } else {
            let k = 1.0 - 0.5 * amount * t;
            sx = px * k;
            sy = py * k;
        }
        acc = acc + bilin(cx + sx - 0.5, cy + sy - 0.5);
    }
    return vec4<u32>(clamp(round(acc / 16.0), vec4<f32>(0.0), vec4<f32>(255.0)));
}
