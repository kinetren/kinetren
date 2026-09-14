// Полутоновый растр (Р.47): зеркало kt_fx::video::halftone — чёрные
// точки на белом по люме центра ячейки; узор в ЛОГИЧЕСКИХ координатах
// l = p/K. u0 = (w, h); f0 = (size, cos, sin, K).
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
    let s = P.f0.x;
    let ca = P.f0.y;
    let sa = P.f0.z;
    let k = P.f0.w;
    let lx = (f32(p.x) + 0.5) / k;
    let ly = (f32(p.y) + 0.5) / k;
    let ux = lx * ca + ly * sa;
    let uy = -lx * sa + ly * ca;
    let ccx = floor(ux / s) * s + 0.5 * s;
    let ccy = floor(uy / s) * s + 0.5 * s;
    let gx = ccx * ca - ccy * sa;
    let gy = ccx * sa + ccy * ca;
    let smp = bilin(gx * k - 0.5, gy * k - 0.5);
    let l = (77.0 * smp.x + 150.0 * smp.y + 29.0 * smp.z) / 256.0;
    let rd = 0.5 * s * 1.25 * sqrt(1.0 - clamp(l / 255.0, 0.0, 1.0));
    let dx = ux - ccx;
    let dy = uy - ccy;
    var v = 255u;
    if (sqrt(dx * dx + dy * dy) <= rd) {
        v = 0u;
    }
    return vec4<u32>(v, v, v, c.a);
}
