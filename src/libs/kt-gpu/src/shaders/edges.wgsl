// Собель по яркости — зеркало edges (kt-fx/src/video.rs): целочисленная
// люма (77R+150G+29B)>>8, sqrt в f32 (у порога законные выбросы, t181),
// кайма кадра — чёрная с альфой 255. u0 = (w, h, threshold, 0).

fn luma(p: vec2<i32>) -> i32 {
    let c = textureLoad(T0, p, 0);
    return (77 * i32(c.r) + 150 * i32(c.g) + 29 * i32(c.b)) >> 8u;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let w = i32(P.u0.x);
    let h = i32(P.u0.y);
    if (p.x < 1 || p.y < 1 || p.x >= w - 1 || p.y >= h - 1) {
        return vec4<u32>(0u, 0u, 0u, 255u);
    }
    let a = luma(p + vec2<i32>(-1, -1));
    let b = luma(p + vec2<i32>(-1, 0));
    let c = luma(p + vec2<i32>(-1, 1));
    let d = luma(p + vec2<i32>(0, -1));
    let e = luma(p + vec2<i32>(0, 1));
    let f = luma(p + vec2<i32>(1, -1));
    let g = luma(p + vec2<i32>(1, 0));
    let i = luma(p + vec2<i32>(1, 1));
    let gx = -a - 2 * b - c + f + 2 * g + i;
    let gy = -a - 2 * d - f + c + 2 * e + i;
    let mag = min(i32(sqrt(f32(gx * gx + gy * gy))), 255);
    var v = 0u;
    if (mag >= i32(P.u0.z)) { v = u32(mag); }
    return vec4<u32>(v, v, v, 255u);
}
