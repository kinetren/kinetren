// Chromatic aberration (R.47): a mirror of kt_fx::video::chroma_ab:
// R from the scale (1−a) to the centre, B from (1+a), G and alpha in place;
// bilinear by hand with edge clamping (as resample). u0 = (w, h); f0.x = amount.
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
    let p = vec2<i32>(floor(pos.xy));
    let c = textureLoad(T0, p, 0);
    let a = P.f0.x;
    let cw = f32(P.u0.x) / 2.0;
    let ch = f32(P.u0.y) / 2.0;
    let px = f32(p.x) + 0.5 - cw;
    let py = f32(p.y) + 0.5 - ch;
    let r = bilin(cw + px * (1.0 - a) - 0.5, ch + py * (1.0 - a) - 0.5).x;
    let b = bilin(cw + px * (1.0 + a) - 0.5, ch + py * (1.0 + a) - 0.5).z;
    return vec4<u32>(
        u32(clamp(round(r), 0.0, 255.0)),
        c.g,
        u32(clamp(round(b), 0.0, 255.0)),
        c.a,
    );
}
