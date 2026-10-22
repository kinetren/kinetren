// Sobel on luma: a mirror of edges (kt-fx/src/video.rs): integer
// luma (77R+150G+29B)>>8, sqrt in f32 (at the threshold the legitimate outliers, t181),
// the frame border is black (alpha: see below).
// u0 = (w, h, threshold, thickness, color_r, color_g, color_b, 0).
//
// thickness is the Sobel sampling step (R.33 S5): 1 gives the former 3×3 kernel;
// with a larger step the line is thicker and the outline wider by that same step. The colour of the lines
// multiplies the grey value; 255,255,255 is the former white.
//
// ALPHA (R.53, item 418): a_out = max(a_in, line strength): the outline RAISES
// the opacity rather than filling it. On an opaque input (a_in = 255)
// the formula is identical to the former fill with 255, so the bytes did not move.
// Edited in the SAME COMMIT as `edges` in kt-fx/src/video.rs.

fn luma(p: vec2<i32>) -> i32 {
    let c = textureLoad(T0, p, 0);
    return (77 * i32(c.r) + 150 * i32(c.g) + 29 * i32(c.b)) >> 8u;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {
    let p = vec2<i32>(floor(pos.xy));
    let w = i32(P.u0.x);
    let h = i32(P.u0.y);
    let d = max(i32(P.u0.w), 1);
    let a_in = textureLoad(T0, p, 0).w;
    if (p.x < d || p.y < d || p.x >= w - d || p.y >= h - d) {
        return vec4<u32>(0u, 0u, 0u, a_in);
    }
    let a = luma(p + vec2<i32>(-d, -d));
    let b = luma(p + vec2<i32>(-d, 0));
    let c = luma(p + vec2<i32>(-d, d));
    let dd = luma(p + vec2<i32>(0, -d));
    let e = luma(p + vec2<i32>(0, d));
    let f = luma(p + vec2<i32>(d, -d));
    let g = luma(p + vec2<i32>(d, 0));
    let i = luma(p + vec2<i32>(d, d));
    let gx = -a - 2 * b - c + f + 2 * g + i;
    let gy = -a - 2 * dd - f + c + 2 * e + i;
    let mag = min(i32(sqrt(f32(gx * gx + gy * gy))), 255);
    var v = 0u;
    if (mag >= i32(P.u0.z)) { v = u32(mag); }
    // line colour (integer, as CPU: v*color/255)
    let col = vec3<u32>(P.u1.x, P.u1.y, P.u1.z);
    return vec4<u32>((vec3<u32>(v) * col) / 255u, max(a_in, v));
}
