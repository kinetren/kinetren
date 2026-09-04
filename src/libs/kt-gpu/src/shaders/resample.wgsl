// Билинейная выборка рамки [x0,y0,cw,ch] источника в цель ow×oh —
// зеркало bilinear_into (kt-fx/src/video.rs): те же веса и кламп краёв,
// f32 вместо f64 (допуск ±1-2 LSB, конформанс t181).
// u0 = (ow, oh, sw, sh); f0 = (x0, y0, cw, ch).

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let o = floor(pos.xy);
    let ow = f32(P.u0.x);
    let oh = f32(P.u0.y);
    let sw = i32(P.u0.z);
    let sh = i32(P.u0.w);
    let sx = P.f0.x + (o.x + 0.5) / ow * P.f0.z - 0.5;
    let sy = P.f0.y + (o.y + 0.5) / oh * P.f0.w - 0.5;
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
    return vec4<u32>(clamp(round(acc), vec4<f32>(0.0), vec4<f32>(255.0)));
}
