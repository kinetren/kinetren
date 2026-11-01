// vhs_tape, the frame (`sim::fin`): the time-base shift per grid row, the bilinear read, the comb on
// chroma, BT.601 back to RGB, opaque. T0 = luma (stage 7), T1 = U|V (stage 8), T2 = the line table.
// u0 = (frame w, frame h, W, H), u1 = (M, A, comb rows).

fn lum(x: i32, y: i32) -> f32 { return ld0f(x, y); }
fn uvc(x: i32, y: i32) -> vec2<f32> {
    let t = ld1(x, y);
    let a = vec2<f32>(lo16(t, 2.0, 16384.0), hi16(t, 2.0, 16384.0));
    let cd = i32(P.u1.z);
    if (cd > 0 && y >= cd) {
        let t2 = ld1(x, y - cd);
        let b = vec2<f32>(lo16(t2, 2.0, 16384.0), hi16(t2, 2.0, 16384.0));
        return vec2<f32>(0.5 * (a.x + b.x), 0.5 * (a.y + b.y));
    }
    return a;
}

fn grid_row(xo: f32, ow: f32, yr: i32) -> vec3<f32> {
    let w = f32(P.u0.z);
    let gx = clamp(f32(P.u1.x) + (xo + 0.5) * (f32(P.u1.y) / ow) - 0.5 + ld2f(7, yr), 0.0, w - 1.0);
    let x0 = i32(floor(gx));
    let x1 = x0 + 1;
    let wx = gx - f32(x0);
    let l0 = lum(x0, yr);
    let l1 = lum(x1, yr);
    let c0 = uvc(x0, yr);
    let c1 = uvc(x1, yr);
    return vec3<f32>(l0 + (l1 - l0) * wx, c0.x + (c1.x - c0.x) * wx, c0.y + (c1.y - c0.y) * wx);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let xo = floor(pos.x);
    let yo = floor(pos.y);
    let ow = f32(P.u0.x);
    let oh = f32(P.u0.y);
    let h = i32(P.u0.w);
    let gy = clamp((yo + 0.5) * (f32(P.u0.w) / oh) - 0.5, 0.0, f32(h - 1));
    let y0 = i32(floor(gy));
    let y1 = min(y0 + 1, h - 1);
    let wy = gy - f32(y0);
    let a = grid_row(xo, ow, y0);
    let b = grid_row(xo, ow, y1);
    let yy = a.x + (b.x - a.x) * wy;
    let uu = a.y + (b.y - a.y) * wy;
    let vv = a.z + (b.z - a.z) * wy;
    let r = yy + vv * (1.0 / 0.877);
    let bl = yy + uu * (1.0 / 0.492);
    let g = (yy - 0.299 * r - 0.114 * bl) * (1.0 / 0.587);
    let q = clamp(floor(vec3<f32>(r, g, bl) * 255.0 + 0.5), vec3<f32>(0.0), vec3<f32>(255.0));
    return vec4<u32>(u32(q.x), u32(q.y), u32(q.z), 255u);
}
